// Pas de fenêtre console en release (l'app vit dans le tray) ; conservée en
// debug pour les logs.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod anim;
mod auth;
mod autostart;
mod calendar;
mod client_config;
mod google;
mod notify;
mod overlay_win;
mod pkce;
mod scheduler;
mod settings;
mod sprite;
mod token_store;
mod tray;
mod update;

use chrono::{Duration, Local, Utc};
use std::cell::RefCell;
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use windows::core::w;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::*;

const WM_APP_PASSAGE: u32 = WM_APP + 1;
const WM_APP_MENU: u32 = WM_APP + 2;
const WM_APP_NOTIFY: u32 = WM_APP + 3;

struct AppState {
    settings: settings::Settings,
    connected: bool,
    access_token: Option<String>,
    token_expires_at: Option<chrono::DateTime<Local>>,
    events: Vec<calendar::Event>,
    last_fetch: Option<std::time::Instant>,
    fired: HashSet<String>,
    notified_revoked: bool,
    banner_slot: Option<String>, // texte du prochain passage, lu par WM_APP_PASSAGE
    alert: bool, // réunion dans ≤ 5 min (icône badge) — recalculé à chaque tick
    // Liens visio des lignes du menu, figés à la construction : le clic sur
    // meet_N ouvre la réunion telle qu'affichée, même si le cache a été
    // rafraîchi entre l'ouverture du menu et le clic.
    menu_links: Vec<Option<String>>,
}

#[derive(Clone, Copy)]
struct SendHwnd(HWND);
// PostMessageW est thread-safe ; le HWND ne sert qu'à poster depuis le scheduler.
unsafe impl Send for SendHwnd {}
unsafe impl Sync for SendHwnd {}

static CONNECT_IN_FLIGHT: AtomicBool = AtomicBool::new(false);

type Shared = Arc<Mutex<AppState>>;

// État partagé, lu par le proc de la fenêtre message-only et par rebuild_tray_menu()
// (tous deux exécutés sur le thread principal via la boucle de messages). Un seul
// OnceLock suffit : plus simple que de « fuiter » l'Arc dans GWLP_USERDATA (cf. Task 18 §2).
static STATE: OnceLock<Shared> = OnceLock::new();

// tray_icon::TrayIcon n'est pas `Send` : impossible dans un `static` (qui exige `Sync`).
// Il n'est touché que sur le thread principal (build_tray + rebuild_tray_menu appelés
// depuis la boucle de messages), donc un thread_local est le conteneur correct.
thread_local! {
    static TRAY: RefCell<Option<tray_icon::TrayIcon>> = const { RefCell::new(None) };
    // Dernier état d'alerte appliqué à l'icône (évite les réinstallations inutiles).
    static LAST_ICON_ALERT: std::cell::Cell<Option<bool>> = const { std::cell::Cell::new(None) };
}

fn main() {
    env_logger::init();
    unsafe {
        use windows::Win32::UI::HiDpi::*;
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }

    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("--fly") {
        let text = args.get(2).cloned().unwrap_or_else(|| "Prochaine réunion".into());
        overlay_win::fly(&text);
        run_message_loop_until_no_window();
        return;
    }

    let Some(cfg) = client_config::ClientConfig::load() else {
        fatal("client_config.json introuvable — voir docs/GOOGLE_OAUTH_SETUP.md");
        return;
    };
    let cfg = Arc::new(cfg);

    let s = settings::Settings::load();
    autostart::apply(s.autostart);
    let state: Shared = Arc::new(Mutex::new(AppState {
        connected: token_store::load().is_some(), // « connecté » ⇔ refresh token présent (spec 4.8)
        settings: s,
        access_token: None,
        token_expires_at: None,
        events: Vec::new(),
        last_fetch: None,
        fired: HashSet::new(),
        notified_revoked: false,
        banner_slot: None,
        alert: false,
        menu_links: Vec::new(),
    }));
    let _ = STATE.set(state.clone());

    let msg_hwnd = create_message_window();

    // Tray (doit vivre sur le thread principal, gardé vivant via le thread_local TRAY)
    build_tray(&state);
    wire_menu_events(state.clone(), cfg.clone(), SendHwnd(msg_hwnd));
    wire_tray_click(state.clone(), SendHwnd(msg_hwnd));

    // Thread scheduler : 1er tick immédiat, puis toutes les 60 s (spec 4.6)
    {
        let state = state.clone();
        let cfg = cfg.clone();
        let post = SendHwnd(msg_hwnd);
        std::thread::spawn(move || loop {
            tick(&state, &cfg, post);
            std::thread::sleep(std::time::Duration::from_secs(60));
        });
    }

    unsafe {
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

pub fn open_browser(url: &str) {
    use windows::core::HSTRING;
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    unsafe {
        ShellExecuteW(None, &HSTRING::from("open"), &HSTRING::from(url), None, None, SW_SHOWNORMAL);
    }
}

fn message_box(text: &str, style: MESSAGEBOX_STYLE) {
    use windows::core::HSTRING;
    unsafe {
        MessageBoxW(None, &HSTRING::from(text), w!("Avion Messager"), style);
    }
}

fn fatal(text: &str) {
    message_box(text, MB_ICONERROR);
}

pub fn info_box(text: &str) {
    message_box(text, MB_ICONINFORMATION);
}

fn run_message_loop_until_no_window() {
    unsafe {
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
            if FindWindowW(w!("AvionOverlay"), None).is_err() {
                break;
            }
        }
    }
}

/// Un tick : refresh du token si besoin, fetch <= toutes les 5 min, décision de tir.
fn tick(state: &Shared, cfg: &client_config::ClientConfig, post: SendHwnd) {
    let now = Local::now();

    // 0. Pas de décision de tir si déconnecté (spec 4.7) : le cache d'événements et les
    // menus restent affichés (vol manuel toujours possible), mais aucun tir automatique
    // ni aucune tentative de refresh/fetch tant que le compte n'est pas reconnecté.
    let connected = state.lock().unwrap().connected;
    if !connected {
        return;
    }

    // 1. Access token (spec 4.7)
    let needs = {
        let st = state.lock().unwrap();
        auth::needs_refresh(st.token_expires_at, now)
    };
    if needs {
        if let Some(rt) = token_store::load() {
            match auth::refresh(cfg, &rt) {
                Ok(t) => {
                    let new_rt = t.refresh_token;
                    let mut st = state.lock().unwrap();
                    st.access_token = Some(t.access_token);
                    st.token_expires_at = Some(now + Duration::seconds(t.expires_in));
                    drop(st);
                    if let Some(new_rt) = new_rt {
                        let _ = token_store::save(&new_rt);
                    }
                }
                Err(auth::RefreshError::Revoked) => {
                    // définitif (spec 4.7) : trousseau effacé, déconnecté, menu, toast unique
                    token_store::delete();
                    let mut st = state.lock().unwrap();
                    st.connected = false;
                    st.access_token = None;
                    st.token_expires_at = None;
                    let first = !st.notified_revoked;
                    st.notified_revoked = true;
                    drop(st);
                    post_msg(post, WM_APP_MENU);
                    if first {
                        post_msg(post, WM_APP_NOTIFY);
                    }
                    return;
                }
                Err(auth::RefreshError::Transient(e)) => {
                    log::warn!("refresh token : échec transitoire : {e}");
                    return; // réessai au tick suivant
                }
            }
        }
    }

    // 2. Fetch (au plus toutes les 5 min en cas de succès, spec 4.6)
    let (token, stale) = {
        let st = state.lock().unwrap();
        let stale = st.last_fetch.map_or(true, |t| t.elapsed().as_secs() >= 300);
        (st.access_token.clone(), stale)
    };
    if let (Some(token), true) = (token, stale) {
        match google::fetch_events(&token, Utc::now()) {
            Ok(events) => {
                let mut st = state.lock().unwrap();
                st.events = events;
                st.last_fetch = Some(std::time::Instant::now());
                drop(st);
                post_msg(post, WM_APP_MENU);
            }
            Err(e) => log::warn!("agenda : échec du rafraîchissement (cache conservé) : {e}"),
        }
    }

    // 3. Décision de tir (spec 4.5) + bascule de l'icône d'alerte.
    // Tout est calculé sous un seul verrou (aucun appel bloquant ici),
    // les messages sont postés après l'unique drop.
    let mut st = state.lock().unwrap();
    scheduler::prune_fired(&mut st.fired, now);
    let imminent = scheduler::imminent(&st.events, now);
    let alert_changed = imminent != st.alert;
    st.alert = imminent;
    let lead = st.settings.lead_minutes;
    let blocked = scheduler::gates_blocked(
        st.settings.paused,
        st.settings.suppress_during_meeting,
        calendar::meeting_in_progress(&st.events, now),
    );
    let mut fire = false;
    if !blocked {
        if let Some(e) = scheduler::due(&st.events, now, lead, &st.fired) {
            let key = scheduler::event_key(e);
            let text = calendar::banner_text(e);
            st.fired.insert(key);
            st.banner_slot = Some(text);
            fire = true;
        }
    }
    drop(st);
    if alert_changed {
        post_msg(post, WM_APP_MENU); // rafraîchit l'icône (badge)
    }
    if fire {
        post_msg(post, WM_APP_PASSAGE);
    }
}

fn post_msg(h: SendHwnd, msg: u32) {
    unsafe {
        let _ = PostMessageW(h.0, msg, WPARAM(0), LPARAM(0));
    }
}

fn build_menu_state(state: &Shared) -> tray::MenuState {
    let mut st = state.lock().unwrap();
    let now = Local::now();
    let upcoming = calendar::upcoming(&st.events, now, tray::MAX_UPCOMING);
    // Instantané des liens visio : le handler meet_N résout la ligne telle
    // qu'elle est affichée, indépendamment des rafraîchissements ultérieurs.
    st.menu_links = upcoming.iter().map(|e| e.meet_link.clone()).collect();
    tray::MenuState {
        connected: st.connected,
        upcoming: upcoming
            .iter()
            .map(|e| (calendar::banner_text(e), e.meet_link.is_some()))
            .collect(),
        paused: st.settings.paused,
        suppress_during_meeting: st.settings.suppress_during_meeting,
        lead_minutes: st.settings.lead_minutes,
        alert: st.alert,
    }
}

/// Vue complète du tray (menu + info-bulle + état d'alerte de l'icône) —
/// partagée par la construction initiale et les reconstructions.
fn make_tray_view(state: &Shared) -> (muda::Menu, String, bool) {
    let ms = build_menu_state(state);
    let menu = muda::Menu::new();
    fill_menu(&menu, tray::menu_items(&ms));
    (menu, tray_tooltip(&ms), ms.alert)
}

fn fill_menu(menu: &muda::Menu, items: Vec<tray::Item>) {
    for it in items {
        match it {
            tray::Item::Action { id, label, enabled } => {
                let _ = menu.append(&muda::MenuItem::with_id(id, &label, enabled, None));
            }
            tray::Item::Check { id, label, checked } => {
                let _ = menu.append(&muda::CheckMenuItem::with_id(&id, &label, true, checked, None));
            }
            tray::Item::Separator => {
                let _ = menu.append(&muda::PredefinedMenuItem::separator());
            }
            tray::Item::Submenu { label, items } => {
                let sub = muda::Submenu::new(&label, true);
                for it in items {
                    if let tray::Item::Check { id, label, checked } = it {
                        let _ = sub.append(&muda::CheckMenuItem::with_id(&id, &label, true, checked, None));
                    }
                }
                let _ = menu.append(&sub);
            }
        }
    }
}

fn tray_icon_from(alert: bool) -> tray_icon::Icon {
    let bmp = sprite::render_icon(alert);
    // BGRA prémultiplié -> RGBA droit (alpha 0/255 : la conversion est un swap R<->B)
    let rgba: Vec<u8> = bmp.px.chunks_exact(4).flat_map(|p| [p[2], p[1], p[0], p[3]]).collect();
    tray_icon::Icon::from_rgba(rgba, 32, 32).unwrap()
}

/// Info-bulle : la prochaine réunion (heure absolue — jamais périmée), sinon le nom.
fn tray_tooltip(ms: &tray::MenuState) -> String {
    ms.upcoming
        .first()
        .map(|(line, _)| line.clone())
        .unwrap_or_else(|| "Avion Messager".into())
}

fn build_tray(state: &Shared) {
    let (menu, tooltip, alert) = make_tray_view(state);
    let tray = tray_icon::TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_icon(tray_icon_from(alert))
        .with_tooltip(tooltip)
        .with_menu_on_left_click(false) // clic gauche = vol manuel, menu au clic droit
        .build()
        .expect("création du tray");
    LAST_ICON_ALERT.with(|c| c.set(Some(alert)));
    TRAY.with(|t| *t.borrow_mut() = Some(tray));
}

/// Clic gauche sur l'icône tray = « Faire passer l'avion ».
fn wire_tray_click(state: Shared, post: SendHwnd) {
    use tray_icon::{MouseButton, MouseButtonState, TrayIconEvent};
    TrayIconEvent::set_event_handler(Some(move |ev: TrayIconEvent| {
        if let TrayIconEvent::Click {
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
            ..
        } = ev
        {
            manual_fly(&state, post);
        }
    }));
}

fn rebuild_tray_menu() {
    let Some(state) = STATE.get() else { return };
    let (menu, tooltip, alert) = make_tray_view(state);
    TRAY.with(|t| {
        if let Some(tray) = t.borrow().as_ref() {
            let _ = tray.set_menu(Some(Box::new(menu)));
            let _ = tray.set_tooltip(Some(tooltip));
            // HICON réinstallé seulement au changement d'état (évite un
            // clignotement possible à chaque toggle ou fetch).
            if LAST_ICON_ALERT.with(|c| c.replace(Some(alert))) != Some(alert) {
                let _ = tray.set_icon(Some(tray_icon_from(alert)));
            }
        }
    });
}

fn wire_menu_events(state: Shared, cfg: Arc<client_config::ClientConfig>, post: SendHwnd) {
    muda::MenuEvent::set_event_handler(Some(move |ev: muda::MenuEvent| {
        let id = ev.id().0.as_str();
        match id {
            "connect" => spawn_connect(state.clone(), cfg.clone(), post),
            "disconnect" => {
                token_store::delete();
                let mut st = state.lock().unwrap();
                st.connected = false;
                st.access_token = None;
                st.token_expires_at = None;
                drop(st);
                post_msg(post, WM_APP_MENU);
            }
            "pause" => toggle(&state, post, |s| s.paused = !s.paused),
            "suppress_meeting" => {
                toggle(&state, post, |s| s.suppress_during_meeting = !s.suppress_during_meeting)
            }
            "fly" => manual_fly(&state, post),
            "check_updates" => update::check_and_prompt(),
            "quit" => unsafe { PostQuitMessage(0) },
            _ => {
                if let Some(m) = id.strip_prefix("lead_").and_then(|v| v.parse().ok()) {
                    toggle(&state, post, |s| s.lead_minutes = m);
                } else if let Some(i) = id.strip_prefix("meet_").and_then(|v| v.parse::<usize>().ok()) {
                    // ligne de réunion cliquée : lien figé à la construction du
                    // menu (voir build_menu_state) — jamais une autre réunion
                    // que celle affichée.
                    let link = state.lock().unwrap().menu_links.get(i).cloned().flatten();
                    if let Some(link) = link {
                        open_browser(&link);
                    }
                }
            }
        }
    }));
}

/// Passage manuel : ignore les portes (spec 4.5) ; prochain() ou placeholder.
fn manual_fly(state: &Shared, post: SendHwnd) {
    let mut st = state.lock().unwrap();
    let text = calendar::next_up(&st.events, Local::now())
        .map(|e| calendar::banner_text(&e))
        .unwrap_or_else(|| "Aucune réunion à venir".into());
    st.banner_slot = Some(text);
    drop(st);
    post_msg(post, WM_APP_PASSAGE);
}

fn toggle(state: &Shared, post: SendHwnd, f: impl FnOnce(&mut settings::Settings)) {
    let mut st = state.lock().unwrap();
    f(&mut st.settings);
    st.settings.save();
    drop(st);
    post_msg(post, WM_APP_MENU);
}

fn spawn_connect(state: Shared, cfg: Arc<client_config::ClientConfig>, post: SendHwnd) {
    // garde single-flight (spec 4.7) : un second connect est ignoré
    if CONNECT_IN_FLIGHT
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return;
    }
    std::thread::spawn(move || {
        match auth::run_connect_flow(&cfg) {
            Ok(t) => {
                if let Some(rt) = &t.refresh_token {
                    let _ = token_store::save(rt);
                }
                let mut st = state.lock().unwrap();
                st.connected = true;
                st.access_token = Some(t.access_token);
                st.token_expires_at = Some(Local::now() + Duration::seconds(t.expires_in));
                st.notified_revoked = false;
                st.last_fetch = None;
                drop(st);
                post_msg(post, WM_APP_MENU);
                // Fetch immédiat : les réunions apparaissent tout de suite dans
                // le menu, sans attendre le prochain tick (jusqu'à 60 s).
                tick(&state, &cfg, post);
            }
            Err(e) => log::warn!("connexion Google : {e}"),
        }
        CONNECT_IN_FLIGHT.store(false, Ordering::SeqCst);
    });
}

fn create_message_window() -> HWND {
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    unsafe extern "system" fn proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
        match msg {
            WM_APP_PASSAGE => {
                if let Some(state) = STATE.get() {
                    let text = state.lock().unwrap().banner_slot.take();
                    if let Some(text) = text {
                        overlay_win::fly(&text);
                    }
                }
                LRESULT(0)
            }
            WM_APP_MENU => {
                rebuild_tray_menu();
                LRESULT(0)
            }
            WM_APP_NOTIFY => {
                notify::reconnect_toast();
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wp, lp),
        }
    }
    unsafe {
        let class = w!("AvionMessagerMsg");
        let hinstance = GetModuleHandleW(None).unwrap();
        let wc = WNDCLASSW {
            lpfnWndProc: Some(proc),
            hInstance: hinstance.into(),
            lpszClassName: class,
            ..Default::default()
        };
        RegisterClassW(&wc);
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            class,
            w!(""),
            WINDOW_STYLE(0),
            0,
            0,
            0,
            0,
            HWND_MESSAGE,
            None,
            hinstance,
            None,
        )
        .unwrap()
    }
}

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

use windows::core::w;

pub fn open_browser(url: &str) {
    use windows::core::HSTRING;
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    unsafe {
        ShellExecuteW(None, &HSTRING::from("open"), &HSTRING::from(url), None, None, SW_SHOWNORMAL);
    }
}

fn main() {
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
    // (le câblage complet arrive en Task 18)
}

fn run_message_loop_until_no_window() {
    use windows::Win32::UI::WindowsAndMessaging::*;
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

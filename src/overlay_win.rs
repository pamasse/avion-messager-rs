use crate::{anim, sprite};
use std::time::Instant;
use windows::core::w;
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, SIZE, WPARAM};
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GetDC, GetMonitorInfoW,
    MonitorFromPoint, ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB,
    DIB_RGB_COLORS, HBITMAP, MONITORINFO, MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::WindowsAndMessaging::*;

struct Flight {
    started: Instant,
    // Écran du vol : origine (coordonnées bureau virtuel) et dimensions.
    mon_x: i32,
    mon_y: i32,
    screen_w: i32,
    screen_h: i32,
    scale: f32,
}

/// Écran où se trouve le curseur (origine + dimensions) — l'avion vole là où
/// l'utilisateur regarde, pas forcément sur l'écran principal.
unsafe fn cursor_monitor() -> (i32, i32, i32, i32) {
    let mut pt = POINT::default();
    let _ = GetCursorPos(&mut pt);
    let hmon = MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST);
    let mut mi = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    if GetMonitorInfoW(hmon, &mut mi).as_bool() {
        let r = mi.rcMonitor;
        (r.left, r.top, r.right - r.left, r.bottom - r.top)
    } else {
        (0, 0, GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN))
    }
}

const TIMER_ID: usize = 1;

pub fn fly(text: &str) {
    unsafe {
        let class = w!("AvionOverlay");
        let hinstance = windows::Win32::System::LibraryLoader::GetModuleHandleW(None).unwrap();
        let wc = WNDCLASSW {
            lpfnWndProc: Some(wnd_proc),
            hInstance: hinstance.into(),
            lpszClassName: class,
            ..Default::default()
        };
        RegisterClassW(&wc); // échec si déjà enregistrée : sans importance

        // Un seul avion à la fois : si un vol est en cours, on ignore celui-ci.
        if FindWindowW(class, None).is_ok() {
            return;
        }

        let (mon_x, mon_y, screen_w, screen_h) = cursor_monitor();

        // Fenêtre créée à l'origine de l'écran cible : avec le processus
        // per-monitor DPI aware, GetDpiForWindow rend alors le DPI de CET écran.
        let hwnd = CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            class,
            w!(""),
            WS_POPUP,
            mon_x,
            mon_y,
            0,
            0,
            None,
            None,
            hinstance,
            None,
        )
        .unwrap();

        let scale = GetDpiForWindow(hwnd) as f32 / 96.0;
        let bmp = sprite::render_rig(text, scale);
        paint_layered(hwnd, &bmp);

        let flight =
            Box::new(Flight { started: Instant::now(), mon_x, mon_y, screen_w, screen_h, scale });
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(flight) as isize);

        let (x, y) = anim::position(0, screen_w, screen_h, scale);
        let _ = SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            mon_x + x,
            mon_y + y,
            bmp.w as i32,
            bmp.h as i32,
            SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );
        SetTimer(hwnd, TIMER_ID, 16, None);
    }
}

unsafe fn paint_layered(hwnd: HWND, bmp: &sprite::Bitmap) {
    let mut info = BITMAPINFO::default();
    info.bmiHeader = BITMAPINFOHEADER {
        biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
        biWidth: bmp.w as i32,
        biHeight: -(bmp.h as i32), // top-down
        biPlanes: 1,
        biBitCount: 32,
        biCompression: BI_RGB.0,
        ..Default::default()
    };
    let screen_dc = GetDC(None);
    let mem_dc = CreateCompatibleDC(screen_dc);
    let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
    let hbmp: HBITMAP =
        CreateDIBSection(screen_dc, &info, DIB_RGB_COLORS, &mut bits, None, 0).unwrap();
    std::ptr::copy_nonoverlapping(bmp.px.as_ptr(), bits as *mut u8, bmp.px.len());
    let old = SelectObject(mem_dc, hbmp);

    let blend = windows::Win32::Graphics::Gdi::BLENDFUNCTION {
        BlendOp: 0, // AC_SRC_OVER
        SourceConstantAlpha: 255,
        AlphaFormat: 1, // AC_SRC_ALPHA
        ..Default::default()
    };
    let size = SIZE { cx: bmp.w as i32, cy: bmp.h as i32 };
    let src = POINT { x: 0, y: 0 };
    let _ = UpdateLayeredWindow(
        hwnd,
        screen_dc,
        None,
        Some(&size),
        mem_dc,
        Some(&src),
        COLORREF(0),
        Some(&blend),
        ULW_ALPHA,
    );

    SelectObject(mem_dc, old);
    let _ = DeleteObject(hbmp);
    let _ = DeleteDC(mem_dc);
    ReleaseDC(None, screen_dc);
}

unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_TIMER => {
            let flight = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Flight;
            if !flight.is_null() {
                let f = &*flight;
                let t = f.started.elapsed().as_millis() as u32;
                if anim::finished(t) {
                    let _ = KillTimer(hwnd, TIMER_ID);
                    let _ = DestroyWindow(hwnd); // create-on-fire / destroy-after (spec 5.1)
                } else {
                    let (x, y) = anim::position(t, f.screen_w, f.screen_h, f.scale);
                    let _ = SetWindowPos(
                        hwnd,
                        HWND_TOPMOST,
                        f.mon_x + x,
                        f.mon_y + y,
                        0,
                        0,
                        SWP_NOSIZE | SWP_NOACTIVATE | SWP_NOREDRAW,
                    );
                }
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            let flight = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Flight;
            if !flight.is_null() {
                drop(Box::from_raw(flight));
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            }
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

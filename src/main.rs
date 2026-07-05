mod auth;
mod calendar;
mod client_config;
mod google;
mod pkce;
mod scheduler;
mod settings;
mod token_store;

pub fn open_browser(url: &str) {
    use windows::core::HSTRING;
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    unsafe {
        ShellExecuteW(None, &HSTRING::from("open"), &HSTRING::from(url), None, None, SW_SHOWNORMAL);
    }
}

fn main() {}

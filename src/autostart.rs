use winreg::enums::HKEY_CURRENT_USER;
use winreg::RegKey;

const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const VALUE: &str = "Avion Messager";

/// Gaté release (spec 4.9) : un build debug n'enregistre pas d'entrée.
pub fn apply(enabled: bool) {
    if cfg!(debug_assertions) {
        return;
    }
    let Ok((key, _)) = RegKey::predef(HKEY_CURRENT_USER).create_subkey(RUN_KEY) else { return };
    if enabled {
        if let Ok(exe) = std::env::current_exe() {
            let _ = key.set_value(VALUE, &format!("\"{}\"", exe.display()));
        }
    } else {
        let _ = key.delete_value(VALUE);
    }
}

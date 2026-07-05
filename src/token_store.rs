use keyring::Entry;

const SERVICE: &str = "com.pierre.avionmessager";
const USER: &str = "google-refresh-token";

fn entry() -> Result<Entry, keyring::Error> {
    Entry::new(SERVICE, USER)
}

pub fn save(refresh_token: &str) -> Result<(), keyring::Error> {
    entry()?.set_password(refresh_token)
}

pub fn load() -> Option<String> {
    entry().ok()?.get_password().ok()
}

pub fn delete() {
    if let Ok(e) = entry() {
        let _ = e.delete_credential();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore] // touche le trousseau OS — lancer avec `cargo test -- --ignored`
    fn round_trip_trousseau() {
        save("token-de-test").unwrap();
        assert_eq!(load().as_deref(), Some("token-de-test"));
        delete();
        assert!(load().is_none());
    }
}

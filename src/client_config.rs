use serde::Deserialize;

#[derive(Deserialize, Clone)]
pub struct ClientConfig {
    pub client_id: String,
    pub client_secret: String,
}

impl ClientConfig {
    /// Ordre de recherche (spec 4.10) : dossier config app, ./client_config.json,
    /// ../client_config.json.
    pub fn load() -> Option<ClientConfig> {
        let candidates = [
            crate::settings::config_dir().join("client_config.json"),
            std::path::PathBuf::from("client_config.json"),
            std::path::PathBuf::from("../client_config.json"),
        ];
        candidates.iter().find_map(|p| {
            serde_json::from_str(&std::fs::read_to_string(p).ok()?).ok()
        })
    }
}

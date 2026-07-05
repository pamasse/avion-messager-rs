use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const APP_ID: &str = "com.pierre.avionmessager";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub lead_minutes: u32,
    pub paused: bool,
    pub suppress_during_meeting: bool,
    pub autostart: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { lead_minutes: 10, paused: false, suppress_during_meeting: true, autostart: true }
    }
}

pub fn config_dir() -> PathBuf {
    let dir = dirs::config_dir().expect("dossier de config introuvable").join(APP_ID);
    std::fs::create_dir_all(&dir).ok();
    dir
}

impl Settings {
    pub fn load_from(path: &Path) -> Settings {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save_to(&self, path: &Path) {
        if let Ok(json) = serde_json::to_string_pretty(self) {
            std::fs::write(path, json).ok();
        }
    }

    pub fn load() -> Settings {
        Self::load_from(&config_dir().join("settings.json"))
    }

    pub fn save(&self) {
        self.save_to(&config_dir().join("settings.json"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("avion-test-{name}-{}.json", std::process::id()))
    }

    #[test]
    fn fichier_absent_donne_les_defauts() {
        let s = Settings::load_from(&tmp("absent"));
        assert_eq!(s.lead_minutes, 10);
        assert!(!s.paused);
        assert!(s.suppress_during_meeting);
        assert!(s.autostart);
    }

    #[test]
    fn champ_absent_prend_son_defaut_retrocompat() {
        let p = tmp("partiel");
        std::fs::write(&p, r#"{"lead_minutes":5}"#).unwrap();
        let s = Settings::load_from(&p);
        assert_eq!(s.lead_minutes, 5);
        assert!(s.suppress_during_meeting); // défaut
        std::fs::remove_file(&p).ok();
    }

    #[test]
    fn aucune_validation_valeur_hors_choix_acceptee() {
        let p = tmp("horschoix");
        std::fs::write(&p, r#"{"lead_minutes":7}"#).unwrap();
        assert_eq!(Settings::load_from(&p).lead_minutes, 7);
        std::fs::remove_file(&p).ok();
    }

    #[test]
    fn round_trip_save_load() {
        let p = tmp("roundtrip");
        let s = Settings { lead_minutes: 30, paused: true, suppress_during_meeting: false, autostart: false };
        s.save_to(&p);
        assert_eq!(Settings::load_from(&p), s);
        std::fs::remove_file(&p).ok();
    }
}

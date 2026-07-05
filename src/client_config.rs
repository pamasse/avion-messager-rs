// Squelette : la logique de recherche du fichier de config arrive en Task 12.
#![allow(dead_code)]

#[derive(serde::Deserialize, Clone)]
pub struct ClientConfig {
    pub client_id: String,
    pub client_secret: String,
}

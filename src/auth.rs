#![allow(dead_code)] // câblé en Task 10/18

use crate::client_config::ClientConfig;
use chrono::{DateTime, Duration, Local};
use serde::Deserialize;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::time::{Duration as StdDuration, Instant};

pub(crate) fn urlencode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

pub(crate) fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let decoded = (bytes[i] == b'%' && i + 2 < bytes.len()).then(|| {
            let h = (bytes[i + 1] as char).to_digit(16)?;
            let l = (bytes[i + 2] as char).to_digit(16)?;
            Some((h * 16 + l) as u8)
        }).flatten();
        match decoded {
            Some(b) => { out.push(b); i += 3; }
            None => { out.push(bytes[i]); i += 1; }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

const SCOPE: &str = "https://www.googleapis.com/auth/calendar.readonly";

pub fn auth_url(client_id: &str, redirect_uri: &str, challenge: &str) -> String {
    format!(
        "https://accounts.google.com/o/oauth2/v2/auth?response_type=code&client_id={}&redirect_uri={}&scope={}&code_challenge={}&code_challenge_method=S256&access_type=offline&prompt=consent",
        urlencode(client_id),
        urlencode(redirect_uri),
        urlencode(SCOPE),
        urlencode(challenge),
    )
}

pub fn parse_redirect(request_line: &str) -> Result<String, String> {
    let path = request_line
        .strip_prefix("GET ")
        .and_then(|r| r.split(' ').next())
        .ok_or("requête illisible")?;
    let query = path.split_once('?').map(|(_, q)| q).unwrap_or("");
    let mut code = None;
    for pair in query.split('&') {
        match pair.split_once('=') {
            Some(("code", v)) => code = Some(percent_decode(v)),
            Some(("error", v)) => return Err(format!("erreur OAuth : {v}")),
            _ => {}
        }
    }
    code.ok_or_else(|| "pas de code dans la redirection".into())
}

pub fn needs_refresh(expires_at: Option<DateTime<Local>>, now: DateTime<Local>) -> bool {
    match expires_at {
        None => true,
        Some(t) => t - now <= Duration::seconds(60),
    }
}

pub fn is_invalid_grant(body: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|v| v.get("error").and_then(|e| e.as_str()).map(|e| e == "invalid_grant"))
        .unwrap_or(false)
}

#[derive(Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub expires_in: i64,
    #[serde(default)]
    pub refresh_token: Option<String>,
}

pub enum RefreshError {
    Revoked,
    Transient(String),
}

const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";

fn token_request(form: &[(&str, &str)]) -> Result<TokenResponse, RefreshError> {
    match ureq::post(TOKEN_URL).send_form(form) {
        Ok(resp) => {
            let body = resp.into_string().map_err(|e| RefreshError::Transient(e.to_string()))?;
            serde_json::from_str(&body).map_err(|e| RefreshError::Transient(e.to_string()))
        }
        Err(ureq::Error::Status(code, resp)) => {
            let body = resp.into_string().unwrap_or_default();
            if is_invalid_grant(&body) {
                Err(RefreshError::Revoked)
            } else {
                Err(RefreshError::Transient(format!("HTTP {code}")))
            }
        }
        Err(e) => Err(RefreshError::Transient(e.to_string())),
    }
}

pub fn exchange_code(
    cfg: &ClientConfig,
    code: &str,
    verifier: &str,
    redirect_uri: &str,
) -> Result<TokenResponse, RefreshError> {
    token_request(&[
        ("grant_type", "authorization_code"),
        ("code", code),
        ("client_id", &cfg.client_id),
        ("client_secret", &cfg.client_secret),
        ("redirect_uri", redirect_uri),
        ("code_verifier", verifier),
    ])
}

pub fn refresh(cfg: &ClientConfig, refresh_token: &str) -> Result<TokenResponse, RefreshError> {
    token_request(&[
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh_token),
        ("client_id", &cfg.client_id),
        ("client_secret", &cfg.client_secret),
    ])
}

const CONNECT_TIMEOUT: StdDuration = StdDuration::from_secs(300); // 5 min (spec 4.7)

/// Flux complet : bind loopback AVANT construction de l'URI, ouvre le
/// navigateur, attend la redirection (<= 5 min), échange le code.
pub fn run_connect_flow(cfg: &ClientConfig) -> Result<TokenResponse, String> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    let redirect_uri = format!("http://127.0.0.1:{port}");

    let verifier = crate::pkce::new_verifier();
    let url = auth_url(&cfg.client_id, &redirect_uri, &crate::pkce::challenge_s256(&verifier));
    crate::open_browser(&url);

    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    let deadline = Instant::now() + CONNECT_TIMEOUT;
    let mut stream = loop {
        match listener.accept() {
            Ok((s, _)) => break s,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                if Instant::now() >= deadline {
                    return Err("délai de connexion dépassé (5 min)".into());
                }
                std::thread::sleep(StdDuration::from_millis(100));
            }
            Err(e) => return Err(e.to_string()),
        }
    };

    stream.set_nonblocking(false).ok();
    let mut line = String::new();
    BufReader::new(&stream).read_line(&mut line).map_err(|e| e.to_string())?;
    let result = parse_redirect(line.trim());

    let page = "<html><meta charset=utf-8><body style=\"font-family:sans-serif\">\
                Avion Messager est connecté — tu peux fermer cet onglet.</body></html>";
    let _ = write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        page.len(),
        page
    );

    let code = result?;
    exchange_code(cfg, &code, &verifier, &redirect_uri).map_err(|e| match e {
        RefreshError::Revoked => "échange refusé (invalid_grant)".to_string(),
        RefreshError::Transient(m) => m,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn url_autorisation_contient_les_parametres_obligatoires() {
        let url = auth_url("ID.apps.googleusercontent.com", "http://127.0.0.1:4242", "CHALL");
        assert!(url.starts_with("https://accounts.google.com/o/oauth2/v2/auth?"));
        assert!(url.contains("response_type=code"));
        assert!(url.contains("client_id=ID.apps.googleusercontent.com"));
        assert!(url.contains("redirect_uri=http%3A%2F%2F127.0.0.1%3A4242"));
        assert!(url.contains("scope=https%3A%2F%2Fwww.googleapis.com%2Fauth%2Fcalendar.readonly"));
        assert!(url.contains("code_challenge=CHALL"));
        assert!(url.contains("code_challenge_method=S256"));
        assert!(url.contains("access_type=offline"));
        assert!(url.contains("prompt=consent"));
    }

    #[test]
    fn parse_redirect_extrait_et_decode_le_code() {
        let line = "GET /?code=4%2F0AbCdEf&scope=x HTTP/1.1";
        assert_eq!(parse_redirect(line).unwrap(), "4/0AbCdEf");
    }

    #[test]
    fn parse_redirect_erreur_ou_absence_echoue_proprement() {
        assert!(parse_redirect("GET /?error=access_denied HTTP/1.1").is_err());
        assert!(parse_redirect("GET /favicon.ico HTTP/1.1").is_err());
    }

    #[test]
    fn needs_refresh_a_60s_ou_moins() {
        let now = Local.with_ymd_and_hms(2026, 7, 5, 10, 0, 0).unwrap();
        assert!(needs_refresh(None, now));
        assert!(needs_refresh(Some(now + Duration::seconds(60)), now));
        assert!(!needs_refresh(Some(now + Duration::seconds(61)), now));
    }

    #[test]
    fn invalid_grant_par_champ_json_pas_substring() {
        assert!(is_invalid_grant(r#"{"error":"invalid_grant","error_description":"x"}"#));
        assert!(!is_invalid_grant(r#"{"error":"server_error"}"#));
        // un substring hors du champ error ne doit PAS matcher (voie robuste, spec 4.7)
        assert!(!is_invalid_grant(r#"{"error":"other","error_description":"invalid_grant"}"#));
        assert!(!is_invalid_grant("pas du json invalid_grant"));
    }

    #[test]
    fn token_response_se_deserialise() {
        let body = r#"{"access_token":"at","expires_in":3599,"refresh_token":"rt","scope":"s","token_type":"Bearer"}"#;
        let t: TokenResponse = serde_json::from_str(body).unwrap();
        assert_eq!(t.access_token, "at");
        assert_eq!(t.expires_in, 3599);
        assert_eq!(t.refresh_token.as_deref(), Some("rt"));
        // refresh_token absent (cas refresh) → None
        let t2: TokenResponse =
            serde_json::from_str(r#"{"access_token":"at","expires_in":10}"#).unwrap();
        assert!(t2.refresh_token.is_none());
    }
}

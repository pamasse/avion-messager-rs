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

/// Page servie sur le loopback après la redirection OAuth : l'avion pixel art
/// (SVG inline, aucune ressource externe) et le verdict dans une banderole.
const LANDING_TEMPLATE: &str = r##"<!doctype html>
<html lang="fr"><head><meta charset="utf-8"><title>Avion Messager</title>
<style>
  body{margin:0;min-height:100vh;display:flex;flex-direction:column;align-items:center;
       justify-content:center;gap:28px;background:linear-gradient(#aee3ff,#eef9ff);
       font-family:'Segoe UI',sans-serif;color:#2b2f36}
  svg{shape-rendering:crispEdges}
  .banner{background:#e23b3b;border-top:5px solid #f7a9a9;border-bottom:5px solid #a81f1f;
          color:#fff;font-family:Consolas,'Courier New',monospace;font-weight:700;
          font-size:20px;padding:14px 28px}
  p{margin:0;opacity:.75}
</style></head>
<body>
<svg width="220" viewBox="24 6 146 98" xmlns="http://www.w3.org/2000/svg" role="img" aria-label="Avion pixel art">
  <rect x="26" y="8" width="16" height="34" fill="#c9d4e0"/>
  <rect x="26" y="8" width="16" height="6" fill="#e6edf4"/>
  <rect x="40" y="30" width="92" height="10" fill="#e23b3b"/>
  <rect x="30" y="40" width="112" height="10" fill="#e23b3b"/>
  <rect x="30" y="50" width="112" height="10" fill="#c62f2f"/>
  <rect x="40" y="60" width="92" height="10" fill="#c62f2f"/>
  <rect x="96" y="32" width="14" height="8" fill="#bfe3ff"/>
  <rect x="114" y="32" width="14" height="8" fill="#bfe3ff"/>
  <rect x="48" y="70" width="72" height="10" fill="#c9d4e0"/>
  <rect x="48" y="70" width="72" height="4" fill="#e6edf4"/>
  <rect x="64" y="80" width="4" height="12" fill="#3b3f47"/>
  <rect x="104" y="80" width="4" height="12" fill="#3b3f47"/>
  <rect x="56" y="92" width="20" height="8" fill="#2b2f36"/>
  <rect x="96" y="92" width="20" height="8" fill="#2b2f36"/>
  <rect x="142" y="40" width="12" height="20" fill="#2b2f36"/>
  <rect x="150" y="42" width="10" height="10" fill="#ffcf3f"/>
  <rect x="160" y="22" width="6" height="56" fill="#3b3f47"/>
</svg>
<div class="banner">{{message}}</div>
<p>{{detail}}</p>
</body></html>"##;

fn landing_page(ok: bool) -> String {
    let (message, detail) = if ok {
        ("Avion Messager est connecté ✈", "Tu peux fermer cet onglet — l'avion s'occupe du reste.")
    } else {
        ("La connexion a échoué", "Tu peux fermer cet onglet et réessayer depuis l'icône de la barre système.")
    };
    LANDING_TEMPLATE.replace("{{message}}", message).replace("{{detail}}", detail)
}

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

    let page = landing_page(result.is_ok());
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
    fn page_loopback_selon_le_verdict() {
        let ok = landing_page(true);
        assert!(ok.contains("Avion Messager est connecté"));
        let ko = landing_page(false);
        assert!(ko.contains("La connexion a échoué"));
        // gabarit entièrement résolu (aucun placeholder résiduel) et autonome
        // (pas de ressource externe ; le xmlns du SVG est un identifiant, pas un chargement)
        for page in [&ok, &ko] {
            assert!(!page.contains("{{"));
            assert!(!page.contains("<script") && !page.contains("<link") && !page.contains("url("));
        }
    }

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

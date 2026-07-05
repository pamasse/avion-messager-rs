#![allow(dead_code)] // câblé en Task 10/18

use chrono::{DateTime, Duration, Local};

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
}

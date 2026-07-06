//! « Rechercher des mises à jour » : compare la version locale au dernier tag
//! GitHub et ouvre la page de release s'il y a plus récent. Pas d'auto-update
//! (choix assumé) — on informe, l'utilisateur décide.

const RELEASES_API: &str =
    "https://api.github.com/repos/pamasse/avion-messager-rs/releases/latest";
const RELEASES_PAGE: &str = "https://github.com/pamasse/avion-messager-rs/releases/latest";

fn parse_version(s: &str) -> Option<(u32, u32, u32)> {
    let mut parts = s.trim().trim_start_matches('v').split('.');
    Some((
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
    ))
}

pub fn is_newer(tag: &str, current: &str) -> bool {
    match (parse_version(tag), parse_version(current)) {
        (Some(t), Some(c)) => t > c,
        _ => false,
    }
}

/// Vérifie en arrière-plan (le handler de menu ne doit pas bloquer sur du réseau).
pub fn check_and_prompt() {
    std::thread::spawn(|| {
        let latest_tag = ureq::get(RELEASES_API)
            .set("User-Agent", "avion-messager") // exigé par l'API GitHub
            .call()
            .ok()
            .and_then(|r| r.into_json::<serde_json::Value>().ok())
            .and_then(|v| v.get("tag_name").and_then(|t| t.as_str()).map(String::from));
        match latest_tag {
            Some(tag) if is_newer(&tag, env!("CARGO_PKG_VERSION")) => {
                crate::open_browser(RELEASES_PAGE)
            }
            Some(_) => crate::info_box(&format!(
                "Avion Messager est à jour (v{}).",
                env!("CARGO_PKG_VERSION")
            )),
            None => crate::info_box(
                "Impossible de vérifier les mises à jour — réessaie plus tard.",
            ),
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_newer_compare_les_versions() {
        assert!(is_newer("v0.2.0", "0.1.0"));
        assert!(is_newer("1.0.0", "0.9.9"));
        assert!(is_newer("v0.1.10", "0.1.9")); // numérique, pas lexicographique
        assert!(!is_newer("v0.1.0", "0.1.0")); // égalité → pas plus récent
        assert!(!is_newer("v0.0.9", "0.1.0"));
        assert!(!is_newer("n'importe quoi", "0.1.0")); // illisible → false, pas de panique
    }
}

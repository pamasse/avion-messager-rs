use crate::calendar::Event;
use chrono::{DateTime, Duration, Local};
use std::collections::HashSet;

pub fn trigger_time(start: DateTime<Local>, lead_minutes: u32) -> DateTime<Local> {
    start - Duration::minutes(lead_minutes as i64)
}

pub fn event_key(e: &Event) -> String {
    format!("{}|{}", e.start.to_rfc3339(), e.summary)
}

pub fn due<'a>(
    events: &'a [Event],
    now: DateTime<Local>,
    lead_minutes: u32,
    fired: &HashSet<String>,
) -> Option<&'a Event> {
    events
        .iter()
        .filter(|e| trigger_time(e.start, lead_minutes) <= now && now < e.start)
        .filter(|e| !fired.contains(&event_key(e)))
        .min_by_key(|e| e.start)
}

pub fn gates_blocked(paused: bool, suppress_during_meeting: bool, in_meeting: bool) -> bool {
    paused || (suppress_during_meeting && in_meeting)
}

pub fn prune_fired(fired: &mut HashSet<String>, now: DateTime<Local>) {
    fired.retain(|key| {
        match key
            .split_once('|')
            .and_then(|(s, _)| DateTime::parse_from_rfc3339(s).ok())
        {
            Some(start) => start.with_timezone(&Local) >= now, // futur → garde
            None => true, // illisible → conserve
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calendar::Event;
    use chrono::{Local, TimeZone};
    use std::collections::HashSet;

    fn t(h: u32, m: u32) -> chrono::DateTime<Local> {
        Local.with_ymd_and_hms(2026, 7, 5, h, m, 0).unwrap()
    }

    fn ev(summary: &str, start: chrono::DateTime<Local>) -> Event {
        Event { summary: summary.into(), start, end: start }
    }

    #[test]
    fn instant_declenchement_soustrait_le_delai() {
        assert_eq!(trigger_time(t(14, 30), 10), t(14, 20)); // ex. 10.6
    }

    #[test]
    fn du_dans_la_fenetre_seulement() {
        let events = [ev("Réu", t(14, 30))];
        let none = HashSet::new();
        assert!(due(&events, t(14, 25), 10, &none).is_some()); // dû
        assert!(due(&events, t(14, 10), 10, &none).is_none()); // avant l'ouverture
        assert!(due(&events, t(14, 31), 10, &none).is_none()); // après le début
        assert!(due(&events, t(14, 20), 10, &none).is_some()); // borne incluse
        assert!(due(&events, t(14, 30), 10, &none).is_none()); // début exclu
    }

    #[test]
    fn du_ignore_les_cles_deja_declenchees() {
        let events = [ev("Réu", t(14, 30))];
        let mut fired = HashSet::new();
        fired.insert(event_key(&events[0]));
        assert!(due(&events, t(14, 25), 10, &fired).is_none()); // ex. 10.7
    }

    #[test]
    fn du_prend_le_debut_le_plus_proche() {
        let events = [ev("Loin", t(14, 40)), ev("Proche", t(14, 32))];
        let none = HashSet::new();
        let got = due(&events, t(14, 31), 10, &none).unwrap();
        assert_eq!(got.summary, "Proche");
    }

    #[test]
    fn portes_pause_ou_anti_reunion_en_cours() {
        assert!(gates_blocked(true, false, false));  // pause seule bloque
        assert!(gates_blocked(true, true, true));
        assert!(gates_blocked(false, true, true));   // anti-réunion + en cours
        assert!(!gates_blocked(false, true, false)); // anti-réunion sans réunion
        assert!(!gates_blocked(false, false, true)); // réunion sans anti-réunion
        assert!(!gates_blocked(false, false, false));
    }

    #[test]
    fn purge_retire_passe_garde_futur_et_illisible() {
        let past = ev("Vieille", t(9, 0));
        let future = ev("Future", t(15, 0));
        let mut fired: HashSet<String> = [
            event_key(&past),
            event_key(&future),
            "n'importe quoi sans date".to_string(),
        ]
        .into();
        prune_fired(&mut fired, t(10, 0));
        assert!(!fired.contains(&event_key(&past)));
        assert!(fired.contains(&event_key(&future)));
        assert!(fired.contains("n'importe quoi sans date")); // jamais de panique
    }
}

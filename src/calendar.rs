use chrono::{DateTime, Local, Timelike};
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq)]
pub struct Event {
    pub summary: String,
    pub start: DateTime<Local>,
    pub end: DateTime<Local>,
    pub meet_link: Option<String>, // hangoutLink Google (visio), si présent
}

pub fn meeting_in_progress(events: &[Event], now: DateTime<Local>) -> bool {
    events.iter().any(|e| e.start <= now && now < e.end)
}

pub fn upcoming(events: &[Event], now: DateTime<Local>, n: usize) -> Vec<Event> {
    let mut v: Vec<Event> = events.iter().filter(|e| e.start > now).cloned().collect();
    v.sort_by_key(|e| e.start); // tri stable (Vec::sort_by_key est stable)
    v.truncate(n);
    v
}

pub fn next_up(events: &[Event], now: DateTime<Local>) -> Option<Event> {
    upcoming(events, now, 1).into_iter().next()
}

pub fn banner_text(event: &Event) -> String {
    format!("{:02} h {:02} — {}", event.start.hour(), event.start.minute(), event.summary)
}

#[derive(Deserialize)]
struct ApiList {
    #[serde(default)]
    items: Vec<ApiEvent>,
}

#[derive(Deserialize)]
struct ApiEvent {
    summary: Option<String>,
    start: Option<ApiTime>,
    end: Option<ApiTime>,
    #[serde(rename = "hangoutLink")]
    hangout_link: Option<String>,
    #[serde(default)]
    attendees: Vec<ApiAttendee>,
}

#[derive(Deserialize)]
struct ApiTime {
    #[serde(rename = "dateTime")]
    date_time: Option<String>,
}

#[derive(Deserialize)]
struct ApiAttendee {
    #[serde(rename = "self", default)]
    is_self: bool,
    #[serde(rename = "responseStatus")]
    response_status: Option<String>,
}

fn parse_local(s: &str) -> Option<DateTime<Local>> {
    DateTime::parse_from_rfc3339(s).ok().map(|d| d.with_timezone(&Local))
}

pub fn parse_events(body: &str) -> Vec<Event> {
    let list: ApiList = match serde_json::from_str(body) {
        Ok(l) => l,
        Err(_) => return Vec::new(),
    };
    list.items
        .into_iter()
        .filter(|e| {
            !e.attendees.iter().any(|a| {
                a.is_self && a.response_status.as_deref() == Some("declined")
            })
        })
        .filter_map(|e| {
            let start = parse_local(e.start.as_ref()?.date_time.as_deref()?)?;
            let end = e
                .end
                .and_then(|t| t.date_time)
                .and_then(|s| parse_local(&s))
                .unwrap_or(start);
            Some(Event {
                summary: e.summary.unwrap_or_else(|| "(Sans titre)".into()),
                start,
                end,
                meet_link: e.hangout_link,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    pub fn t(h: u32, m: u32) -> DateTime<Local> {
        Local.with_ymd_and_hms(2026, 7, 5, h, m, 0).unwrap()
    }

    pub fn ev(summary: &str, start: DateTime<Local>, end: DateTime<Local>) -> Event {
        Event { summary: summary.into(), start, end, meet_link: None }
    }

    #[test]
    fn reunion_en_cours_debut_inclus_fin_exclue() {
        let events = [ev("Réu", t(14, 0), t(15, 0))];
        assert!(!meeting_in_progress(&events, t(13, 59)));
        assert!(meeting_in_progress(&events, t(14, 0)));   // début inclus
        assert!(meeting_in_progress(&events, t(14, 30)));
        assert!(!meeting_in_progress(&events, t(15, 0)));  // fin exclue
        assert!(!meeting_in_progress(&events, t(15, 1)));
    }

    #[test]
    fn prochains_filtre_trie_tronque() {
        let events = [
            ev("A", t(15, 0), t(16, 0)),
            ev("B", t(9, 0), t(10, 0)),
            ev("C", t(11, 0), t(12, 0)),
        ];
        let got = upcoming(&events, t(10, 0), 5);
        assert_eq!(got, vec![events[2].clone(), events[0].clone()]); // [11:00, 15:00]
        assert_eq!(upcoming(&events, t(10, 0), 1), vec![events[2].clone()]);
    }

    #[test]
    fn prochains_tri_stable_a_debut_egal() {
        let events = [ev("X", t(11, 0), t(12, 0)), ev("Y", t(11, 0), t(12, 0))];
        let got = upcoming(&events, t(10, 0), 5);
        assert_eq!(got[0].summary, "X");
        assert_eq!(got[1].summary, "Y");
    }

    #[test]
    fn prochain_est_le_premier_futur() {
        let events = [ev("A", t(15, 0), t(16, 0)), ev("C", t(11, 0), t(12, 0))];
        assert_eq!(next_up(&events, t(10, 0)), Some(events[1].clone()));
        assert_eq!(next_up(&events, t(16, 0)), None);
    }

    #[test]
    fn banderole_zero_padding_24h() {
        assert_eq!(banner_text(&ev("Point produit", t(9, 5), t(10, 0))), "09 h 05 — Point produit");
        assert_eq!(banner_text(&ev("Point produit", t(14, 30), t(15, 0))), "14 h 30 — Point produit");
    }

    #[test]
    fn parse_garde_seulement_les_horodates() {
        let body = r#"{"items":[
            {"summary":"Journée entière","start":{"date":"2026-07-05"}},
            {"summary":"Réu","start":{"dateTime":"2026-07-05T14:00:00+02:00"},
             "end":{"dateTime":"2026-07-05T15:00:00+02:00"}}
        ]}"#;
        let events = parse_events(body);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].summary, "Réu");
    }

    #[test]
    fn parse_refus_self_declined_ignore() {
        let body = r#"{"items":[{"summary":"Refusée",
            "start":{"dateTime":"2026-07-05T14:00:00+02:00"},
            "end":{"dateTime":"2026-07-05T15:00:00+02:00"},
            "attendees":[{"email":"a@b.c","responseStatus":"accepted"},
                         {"self":true,"responseStatus":"declined"}]}]}"#;
        assert!(parse_events(body).is_empty());
    }

    #[test]
    fn parse_resume_absent_donne_sans_titre() {
        let body = r#"{"items":[{"start":{"dateTime":"2026-07-05T14:00:00+02:00"},
            "end":{"dateTime":"2026-07-05T15:00:00+02:00"}}]}"#;
        assert_eq!(parse_events(body)[0].summary, "(Sans titre)");
    }

    #[test]
    fn parse_fin_absente_donne_fin_egale_debut() {
        let body = r#"{"items":[{"summary":"Instant",
            "start":{"dateTime":"2026-07-05T14:00:00+02:00"}}]}"#;
        let e = &parse_events(body)[0];
        assert_eq!(e.end, e.start);
    }

    #[test]
    fn parse_hangout_link_optionnel() {
        let body = r#"{"items":[
            {"summary":"Visio","start":{"dateTime":"2026-07-05T14:00:00+02:00"},
             "hangoutLink":"https://meet.google.com/abc-defg-hij"},
            {"summary":"Sans visio","start":{"dateTime":"2026-07-05T15:00:00+02:00"}}
        ]}"#;
        let events = parse_events(body);
        assert_eq!(events[0].meet_link.as_deref(), Some("https://meet.google.com/abc-defg-hij"));
        assert_eq!(events[1].meet_link, None);
    }

    #[test]
    fn parse_corps_illisible_donne_vide() {
        assert!(parse_events("pas du json").is_empty());
        assert!(parse_events("{}").is_empty());
    }
}

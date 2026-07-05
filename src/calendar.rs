use chrono::{DateTime, Local};

#[derive(Debug, Clone, PartialEq)]
pub struct Event {
    pub summary: String,
    pub start: DateTime<Local>,
    pub end: DateTime<Local>,
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    pub fn t(h: u32, m: u32) -> DateTime<Local> {
        Local.with_ymd_and_hms(2026, 7, 5, h, m, 0).unwrap()
    }

    pub fn ev(summary: &str, start: DateTime<Local>, end: DateTime<Local>) -> Event {
        Event { summary: summary.into(), start, end }
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
}

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
}

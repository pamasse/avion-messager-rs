use crate::calendar::{parse_events, Event};
use chrono::{DateTime, Duration, SecondsFormat, Utc};

pub fn time_min(now_utc: DateTime<Utc>) -> String {
    (now_utc - Duration::hours(6)).to_rfc3339_opts(SecondsFormat::Secs, false)
}

#[allow(dead_code)]
// câblé en Task 18
pub fn fetch_events(access_token: &str, now_utc: DateTime<Utc>) -> Result<Vec<Event>, ureq::Error> {
    let body = ureq::get("https://www.googleapis.com/calendar/v3/calendars/primary/events")
        .query("singleEvents", "true")
        .query("orderBy", "startTime")
        .query("maxResults", "25")
        .query("timeMin", &time_min(now_utc))
        .set("Authorization", &format!("Bearer {access_token}"))
        .call()?
        .into_string()?;
    Ok(parse_events(&body))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};

    #[test]
    fn time_min_est_moins_6h_avec_offset_plus_00_00() {
        let now = Utc.with_ymd_and_hms(2026, 7, 5, 10, 0, 0).unwrap();
        assert_eq!(time_min(now), "2026-07-05T04:00:00+00:00");
    }
}

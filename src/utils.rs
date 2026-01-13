use chrono::{DateTime, Duration, LocalResult, NaiveDate, NaiveDateTime, TimeZone, Utc};
use chrono_tz::America::Santiago;

pub fn format_hours(decimal_hours: f64) -> String {
    let hours = decimal_hours as i32;
    let minutes = ((decimal_hours - hours as f64) * 60.0) as i32;
    format!("{:02}:{:02}", hours, minutes)
}

pub fn santiago_today_naive() -> NaiveDate {
    Utc::now().with_timezone(&Santiago).date_naive()
}

pub fn santiago_day_bounds_utc(date: NaiveDate) -> (DateTime<Utc>, DateTime<Utc>) {
    let start_local = santiago_start_of_day(date);
    let next_day = date.succ_opt().unwrap_or_else(|| date + Duration::days(1));
    let end_local = santiago_start_of_day(next_day);
    (
        start_local.with_timezone(&Utc),
        end_local.with_timezone(&Utc),
    )
}

fn santiago_start_of_day(date: NaiveDate) -> DateTime<chrono_tz::Tz> {
    let naive = NaiveDateTime::new(
        date,
        chrono::NaiveTime::from_hms_opt(0, 0, 0).expect("valid time"),
    );
    match Santiago.from_local_datetime(&naive) {
        LocalResult::Single(dt) => dt,
        LocalResult::Ambiguous(dt, _) => dt,
        LocalResult::None => {
            let shifted = naive + Duration::hours(1);
            match Santiago.from_local_datetime(&shifted) {
                LocalResult::Single(dt) => dt,
                LocalResult::Ambiguous(dt, _) => dt,
                LocalResult::None => Santiago.from_utc_datetime(&shifted),
            }
        }
    }
}

#[allow(dead_code)]
pub fn get_last_clock_out(
    conn: &rusqlite::Connection,
    worker_id: i64,
) -> rusqlite::Result<Option<String>> {
    let mut stmt = conn.prepare(
        "SELECT clock_out FROM timesheets WHERE worker_id = ? AND clock_out IS NOT NULL ORDER BY id DESC LIMIT 1"
    )?;
    let mut rows = stmt.query([worker_id])?;
    if let Some(row) = rows.next()? {
        let time: String = row.get(0)?;
        let dt = chrono::DateTime::parse_from_rfc3339(&time)
            .expect("Invalid time")
            .with_timezone(&chrono::Utc)
            .with_timezone(&Santiago);
        Ok(Some(dt.format("%H:%M:%S").to_string()))
    } else {
        Ok(None)
    }
}

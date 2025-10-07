use chrono_tz::America::Santiago;

pub fn format_hours(decimal_hours: f64) -> String {
    let hours = decimal_hours as i32;
    let minutes = ((decimal_hours - hours as f64) * 60.0) as i32;
    format!("{:02}:{:02}", hours, minutes)
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

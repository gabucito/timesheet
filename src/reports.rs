use crate::db::{self, TimesheetEntry};
use chrono::{NaiveDate, Utc};
use chrono_tz::America::Santiago;
use rusqlite::Connection;
use std::fmt::{self, Write as _};
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

#[derive(Debug)]
pub enum ReportError {
    Database(rusqlite::Error),
    Io(std::io::Error),
}

impl fmt::Display for ReportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReportError::Database(e) => write!(f, "database error: {}", e),
            ReportError::Io(e) => write!(f, "io error: {}", e),
        }
    }
}

impl std::error::Error for ReportError {}

impl From<rusqlite::Error> for ReportError {
    fn from(value: rusqlite::Error) -> Self {
        ReportError::Database(value)
    }
}

impl From<std::io::Error> for ReportError {
    fn from(value: std::io::Error) -> Self {
        ReportError::Io(value)
    }
}

struct ReportRow {
    date: String,
    clock_in: String,
    clock_out: String,
    duration_minutes: i64,
    duration_label: String,
    is_open: bool,
}

pub fn generate_monthly_reports(
    conn: &Connection,
    month: NaiveDate,
    output_root: &Path,
) -> Result<(), ReportError> {
    let month_key = month.format("%Y-%m").to_string();
    fs::create_dir_all(output_root)?;

    let workers = db::get_workers(conn)?;
    for worker in workers {
        let worker_rows = build_rows(conn, worker.id, &month_key)?;
        let worker_dir = output_root;
        let sanitized_name = sanitize_filename(&worker.name);

        let html_path = worker_dir.join(format!("{}_{}.html", month_key, sanitized_name));
        let csv_path = worker_dir.join(format!("{}_{}.csv", month_key, sanitized_name));

        write_html_report(
            &html_path,
            &worker.name,
            &month_key,
            &worker_rows.rows,
            worker_rows.total_minutes,
            worker_rows.has_open_sessions,
        )?;
        write_csv_report(
            &csv_path,
            &worker.name,
            &month_key,
            &worker_rows.rows,
            worker_rows.total_minutes,
        )?;
    }

    Ok(())
}

struct WorkerRows {
    rows: Vec<ReportRow>,
    total_minutes: i64,
    has_open_sessions: bool,
}

fn build_rows(
    conn: &Connection,
    worker_id: i64,
    month_key: &str,
) -> Result<WorkerRows, ReportError> {
    let entries = db::get_monthly_timesheet_entries(conn, worker_id, month_key)?;
    let mut rows = Vec::new();
    let mut total_minutes = 0;
    let mut has_open_sessions = false;
    for entry in entries {
        let row = to_report_row(&entry);
        if row.duration_minutes >= 0 {
            total_minutes += row.duration_minutes;
        }
        if row.is_open {
            has_open_sessions = true;
        }
        rows.push(row);
    }
    Ok(WorkerRows {
        rows,
        total_minutes,
        has_open_sessions,
    })
}

fn to_report_row(entry: &TimesheetEntry) -> ReportRow {
    let start_utc = entry.clock_in;
    let end_utc = entry.clock_out.unwrap_or_else(Utc::now);
    let mut duration_minutes = (end_utc - start_utc).num_minutes();
    if duration_minutes < 0 {
        duration_minutes = 0;
    }
    let start_local = start_utc.with_timezone(&Santiago);
    let end_local = end_utc.with_timezone(&Santiago);
    let is_open = entry.clock_out.is_none();

    ReportRow {
        date: start_local.date_naive().to_string(),
        clock_in: start_local.format("%H:%M").to_string(),
        clock_out: if is_open {
            format!("{}*", end_local.format("%H:%M"))
        } else {
            end_local.format("%H:%M").to_string()
        },
        duration_minutes,
        duration_label: format_duration(duration_minutes),
        is_open,
    }
}

fn write_html_report(
    path: &Path,
    worker_name: &str,
    month: &str,
    rows: &[ReportRow],
    total_minutes: i64,
    has_open_sessions: bool,
) -> Result<(), ReportError> {
    let mut html = String::new();
    writeln!(
        html,
        "<!DOCTYPE html><html><head><meta charset=\"utf-8\"><title>Timesheet {name} {month}</title>\
<style>body{{font-family:Arial,sans-serif;padding:20px}}h1{{margin-bottom:0}}table{{border-collapse:collapse;width:100%;margin-top:16px}}th,td{{border:1px solid #555;padding:6px;text-align:center}}th{{background-color:#eee}}</style></head><body>",
        name = worker_name,
        month = month
    )
    .expect("write to string");
    writeln!(
        html,
        "<h1>{}</h1><h2>Month: {}</h2>",
        escape_html(worker_name),
        month
    )
    .expect("write to string");
    html.push_str("<table><thead><tr><th>Date</th><th>Clock In</th><th>Clock Out</th><th>Duration (HH:MM)</th></tr></thead><tbody>");

    if rows.is_empty() {
        html.push_str("<tr><td colspan=\"4\">No recorded sessions for this month.</td></tr>");
    } else {
        for row in rows {
            writeln!(
                html,
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                row.date, row.clock_in, row.clock_out, row.duration_label
            )
            .expect("write to string");
        }
    }
    html.push_str("</tbody></table>");

    writeln!(
        html,
        "<p><strong>Total:</strong> {} ({} minutes)</p>",
        format_duration(total_minutes),
        total_minutes
    )
    .expect("write to string");

    if has_open_sessions {
        html.push_str("<p>* Entries marked with an asterisk do not have a recorded clock out; the current time was used to compute the duration.</p>");
    }

    html.push_str("</body></html>");

    let mut file = File::create(path)?;
    file.write_all(html.as_bytes())?;
    Ok(())
}

fn write_csv_report(
    path: &Path,
    worker_name: &str,
    month: &str,
    rows: &[ReportRow],
    total_minutes: i64,
) -> Result<(), ReportError> {
    let mut contents = String::new();
    writeln!(contents, "Worker,{}", worker_name).expect("write to string");
    writeln!(contents, "Month,{}", month).expect("write to string");
    contents.push_str("Date,Clock In,Clock Out,Duration Minutes,Duration HH:MM\n");
    if rows.is_empty() {
        contents.push_str("-, -, -, 0, 00:00\n");
    } else {
        for row in rows {
            writeln!(
                contents,
                "{},{},{},{},{}",
                row.date, row.clock_in, row.clock_out, row.duration_minutes, row.duration_label
            )
            .expect("write to string");
        }
    }
    writeln!(
        contents,
        "Total,,,{},{}",
        total_minutes,
        format_duration(total_minutes)
    )
    .expect("write to string");

    let mut file = File::create(path)?;
    file.write_all(contents.as_bytes())?;
    Ok(())
}

fn sanitize_filename(name: &str) -> String {
    let mut result = String::with_capacity(name.len());
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            result.push(ch);
        } else if ch.is_ascii_whitespace() || ch == '-' {
            result.push('_');
        } else {
            result.push('_');
        }
    }
    if result.is_empty() {
        "worker".to_string()
    } else {
        result
    }
}

fn format_duration(minutes: i64) -> String {
    let hours = minutes / 60;
    let mins = minutes % 60;
    format!("{:02}:{:02}", hours, mins)
}

fn escape_html(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

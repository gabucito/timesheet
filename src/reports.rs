use crate::db::{self, TimesheetEntry};
use chrono::{Datelike, Duration, NaiveDate, Utc, Weekday};
use chrono_tz::America::Santiago;
use rusqlite::Connection;
use std::collections::BTreeMap;
use std::fmt::{self, Write as _};
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

#[derive(Clone)]
struct WorkerReportData {
    worker_name: String,
    day_groups: Vec<DayGroup>,
    total_minutes: i64,
    has_open_sessions: bool,
}

#[derive(Debug)]
pub enum ReportError {
    Database(rusqlite::Error),
    Io(std::io::Error),
    InvalidMonth(String),
}

impl fmt::Display for ReportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReportError::Database(e) => write!(f, "database error: {}", e),
            ReportError::Io(e) => write!(f, "io error: {}", e),
            ReportError::InvalidMonth(m) => write!(f, "invalid month value: {}", m),
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

#[derive(Clone)]
struct ReportRow {
    date: NaiveDate,
    clock_in: String,
    clock_out: String,
    duration_minutes: i64,
    duration_label: String,
    is_open: bool,
}

#[derive(Clone)]
struct DayGroup {
    date: NaiveDate,
    weekday_name: String,
    rows: Vec<ReportRow>,
    is_weekend: bool,
}

pub fn generate_monthly_reports(
    conn: &Connection,
    month: NaiveDate,
    selected_date: NaiveDate,
    output_root: &Path,
) -> Result<(), ReportError> {
    let month_key = month.format("%Y-%m").to_string();
    fs::create_dir_all(output_root)?;

    let workers = db::get_workers(conn)?;
    let mut all_worker_data = Vec::new();

    for worker in workers {
        let worker_rows = build_rows(conn, worker.id, &month_key, selected_date)?;
        let worker_dir = output_root;
        let sanitized_name = sanitize_filename(&worker.name);

        let html_path = worker_dir.join(format!("{}_{}.html", month_key, sanitized_name));
        let csv_path = worker_dir.join(format!("{}_{}.csv", month_key, sanitized_name));

        write_html_report(
            &html_path,
            &worker.name,
            &month_key,
            &worker_rows.day_groups,
            worker_rows.total_minutes,
            worker_rows.has_open_sessions,
        )?;
        write_csv_report(
            &csv_path,
            &worker.name,
            &month_key,
            &worker_rows.day_groups,
            worker_rows.total_minutes,
        )?;

        // Collect data for merged report
        all_worker_data.push(WorkerReportData {
            worker_name: worker.name,
            day_groups: worker_rows.day_groups,
            total_minutes: worker_rows.total_minutes,
            has_open_sessions: worker_rows.has_open_sessions,
        });
    }

    // Generate merged HTML report
    let merged_html_path = output_root.join(format!("{}_all_workers.html", month_key));
    write_merged_html_report(&merged_html_path, &month_key, &all_worker_data)?;

    Ok(())
}

struct WorkerRows {
    day_groups: Vec<DayGroup>,
    total_minutes: i64,
    has_open_sessions: bool,
}

fn build_rows(
    conn: &Connection,
    worker_id: i64,
    month_key: &str,
    selected_date: NaiveDate,
) -> Result<WorkerRows, ReportError> {
    let entries = db::get_monthly_timesheet_entries(conn, worker_id, month_key)?;
    let mut grouped: BTreeMap<NaiveDate, Vec<ReportRow>> = BTreeMap::new();
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
        grouped.entry(row.date).or_default().push(row);
    }

    let month_start = NaiveDate::parse_from_str(&format!("{}-01", month_key), "%Y-%m-%d")
        .map_err(|_| ReportError::InvalidMonth(month_key.to_string()))?;

    let mut day_groups = Vec::new();
    let mut current_day = month_start;
    // Include days up to and including the selected date
    let end_date = selected_date.min(month_start + Duration::days(30)); // Cap at end of month
    while current_day <= end_date && current_day.month() == month_start.month() {
        let mut rows = grouped.remove(&current_day).unwrap_or_default();
        if rows.is_empty() {
            rows.push(ReportRow {
                date: current_day,
                clock_in: "--:--:--".to_string(),
                clock_out: "--:--:--".to_string(),
                duration_minutes: 0,
                duration_label: format_duration(0),
                is_open: false,
            });
        } else {
            rows.sort_by(|a, b| a.clock_in.cmp(&b.clock_in));
        }
        let weekday_name = weekday_name_es(current_day.weekday()).to_string();
        day_groups.push(DayGroup {
            date: current_day,
            weekday_name,
            rows,
            is_weekend: current_day.weekday() == Weekday::Sun,
        });

        current_day += Duration::days(1);
    }

    Ok(WorkerRows {
        day_groups,
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
        date: start_local.date_naive(),
        clock_in: start_local.format("%H:%M:%S").to_string(),
        clock_out: if is_open {
            format!("{}*", end_local.format("%H:%M:%S"))
        } else {
            end_local.format("%H:%M:%S").to_string()
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
    day_groups: &[DayGroup],
    total_minutes: i64,
    has_open_sessions: bool,
) -> Result<(), ReportError> {
    let mut html = String::new();
    writeln!(
        html,
        "<!DOCTYPE html><html><head><meta charset=\"utf-8\"><title>Timesheet {name} {month}</title>\
<style>body{{font-family:Arial,sans-serif;padding:20px}}h1{{margin-bottom:0}}table{{border-collapse:collapse;width:100%;margin-top:16px}}th,td{{border:1px solid #555;padding:6px;text-align:center}}th{{background-color:#eee}}table tbody tr.day-even td{{background-color:#f7f7f7}}table tbody tr.day-odd td{{background-color:#ffffff}}table tbody tr.weekend td{{color:#e33d3d}}table tbody tr td:first-child{{font-weight:600}}</style></head><body>",
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

    if day_groups.is_empty() {
        html.push_str("<tr><td colspan=\"4\">No recorded sessions for this month.</td></tr>");
    } else {
        for (index, group) in day_groups.iter().enumerate() {
            let base_class = if index % 2 == 0 {
                "day-even"
            } else {
                "day-odd"
            };
            let class = if group.is_weekend {
                format!("{} weekend", base_class)
            } else {
                base_class.to_string()
            };
            let rowspan = group.rows.len();
            for (row_idx, row) in group.rows.iter().enumerate() {
                html.push_str(&format!("<tr class=\"{}\">", class));
                if row_idx == 0 {
                    writeln!(
                        html,
                        "<td rowspan=\"{rowspan}\"><strong>{date}</strong><br/><small>{weekday}</small></td>",
                        date = group.date.format("%m/%d"),
                        weekday = group.weekday_name,
                    )
                    .expect("write to string");
                }
                writeln!(
                    html,
                    "<td>{}</td><td>{}</td><td>{}</td></tr>",
                    row.clock_in, row.clock_out, row.duration_label
                )
                .expect("write to string");
            }
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
    day_groups: &[DayGroup],
    total_minutes: i64,
) -> Result<(), ReportError> {
    let mut contents = String::new();
    writeln!(contents, "Worker,{}", worker_name).expect("write to string");
    writeln!(contents, "Month,{}", month).expect("write to string");
    contents.push_str("Date,Day,Clock In,Clock Out,Duration Minutes,Duration HH:MM\n");
    if day_groups.is_empty() {
        contents.push_str("-, -, -, -, 0, 00:00\n");
    } else {
        for group in day_groups {
            for (idx, row) in group.rows.iter().enumerate() {
                let date_text = if idx == 0 {
                    group.date.format("%m/%d").to_string()
                } else {
                    "".to_string()
                };
                let day_text = if idx == 0 {
                    group.weekday_name.clone()
                } else {
                    "".to_string()
                };
                writeln!(
                    contents,
                    "{},{},{},{},{},{}",
                    date_text,
                    day_text,
                    row.clock_in,
                    row.clock_out,
                    row.duration_minutes,
                    row.duration_label
                )
                .expect("write to string");
            }
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

fn write_merged_html_report(
    path: &Path,
    month: &str,
    worker_data: &[WorkerReportData],
) -> Result<(), ReportError> {
    let mut html = String::new();
    writeln!(
        html,
        "<!DOCTYPE html><html><head><meta charset=\"utf-8\"><title>All Workers Timesheet {month}</title>\
<style>@media print {{ .page-break {{ page-break-before: always; }} }} body{{font-family:Arial,sans-serif;padding:20px}}h1{{margin-bottom:0}}h2{{margin-top:40px;margin-bottom:10px;padding-top:20px;border-top:2px solid #333}}table{{border-collapse:collapse;width:100%;margin-top:16px}}th,td{{border:1px solid #555;padding:6px;text-align:center}}th{{background-color:#eee}}table tbody tr.day-even td{{background-color:#f7f7f7}}table tbody tr.day-odd td{{background-color:#ffffff}}table tbody tr.weekend td{{color:#e33d3d}}table tbody tr td:first-child{{font-weight:600}}</style></head><body>",
        month = month
    )
    .expect("write to string");
    writeln!(
        html,
        "<h1>All Workers Timesheet</h1><h2>Month: {}</h2>",
        month
    )
    .expect("write to string");

    for (worker_idx, worker) in worker_data.iter().enumerate() {
        if worker_idx > 0 {
            html.push_str("<div class=\"page-break\"></div>");
        }

        writeln!(html, "<h2>{}</h2>", escape_html(&worker.worker_name)).expect("write to string");

        html.push_str("<table><thead><tr><th>Date</th><th>Clock In</th><th>Clock Out</th><th>Duration (HH:MM)</th></tr></thead><tbody>");

        if worker.day_groups.is_empty() {
            html.push_str("<tr><td colspan=\"4\">No recorded sessions for this month.</td></tr>");
        } else {
            for (index, group) in worker.day_groups.iter().enumerate() {
                let base_class = if index % 2 == 0 {
                    "day-even"
                } else {
                    "day-odd"
                };
                let class = if group.is_weekend {
                    format!("{} weekend", base_class)
                } else {
                    base_class.to_string()
                };
                let rowspan = group.rows.len();
                for (row_idx, row) in group.rows.iter().enumerate() {
                    html.push_str(&format!("<tr class=\"{}\">", class));
                    if row_idx == 0 {
                        writeln!(
                            html,
                            "<td rowspan=\"{rowspan}\"><strong>{date}</strong><br/><small>{weekday}</small></td>",
                            date = group.date.format("%m/%d"),
                            weekday = group.weekday_name,
                        )
                        .expect("write to string");
                    }
                    writeln!(
                        html,
                        "<td>{}</td><td>{}</td><td>{}</td></tr>",
                        row.clock_in, row.clock_out, row.duration_label
                    )
                    .expect("write to string");
                }
            }
        }
        html.push_str("</tbody></table>");

        writeln!(
            html,
            "<p><strong>Total:</strong> {} ({} minutes)</p>",
            format_duration(worker.total_minutes),
            worker.total_minutes
        )
        .expect("write to string");

        if worker.has_open_sessions {
            html.push_str("<p>* Entries marked with an asterisk do not have a recorded clock out; the current time was used to compute the duration.</p>");
        }
    }

    html.push_str("</body></html>");

    let mut file = File::create(path)?;
    file.write_all(html.as_bytes())?;
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

fn weekday_name_es(weekday: Weekday) -> &'static str {
    match weekday {
        Weekday::Mon => "lunes",
        Weekday::Tue => "martes",
        Weekday::Wed => "miércoles",
        Weekday::Thu => "jueves",
        Weekday::Fri => "viernes",
        Weekday::Sat => "sábado",
        Weekday::Sun => "domingo",
    }
}

mod db;

use chrono::{Datelike, Weekday};
use chrono_tz::America::Santiago;
use slint::SharedString;
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;
use std::sync::Mutex;

slint::include_modules!();

static LAST_SCAN_TIME: Mutex<Option<chrono::DateTime<chrono::Utc>>> = Mutex::new(None);

#[derive(Clone)]
struct WorkerDisplay {
    id: i64,
    name: String,
    clock_in_time: String,
    clock_out_time: String,
    duration: String,
    color: slint::Color,
}

fn format_hours(decimal_hours: f64) -> String {
    let hours = decimal_hours as i32;
    let minutes = ((decimal_hours - hours as f64) * 60.0) as i32;
    format!("{:02}:{:02}", hours, minutes)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let conn = db::init_db()?;
    let conn = Rc::new(RefCell::new(conn));

    let ui = MainWindow::new()?;
    ui.set_error_message("".into());
    let now = chrono::Utc::now();
    ui.set_selected_date(now.format("%Y-%m-%d").to_string().into());

    // Initialize current time display
    let santiago_time = now.with_timezone(&Santiago);
    ui.set_current_time_display(santiago_time.format("%H:%M:%S").to_string().into());
    let ui_handle = ui.as_weak();
    let ui_handle_barcode = ui_handle.clone();
    let ui_handle_add = ui_handle.clone();
    let ui_handle_edit = ui_handle.clone();

    // Load workers and update UI
    match db::get_workers(&conn.borrow()) {
        Ok(workers) => {
            let mut worker_displays = Vec::new();
            let today = chrono::Utc::now().format("%Y-%m-%d").to_string();

            let mut shown_workers = HashSet::new();

            for worker in &workers {
                // Get all timesheet entries for today
                match db::get_daily_timesheet_entries(&conn.borrow(), worker.id, &today) {
                    Ok(entries) => {
                        let is_first_entry = !shown_workers.contains(&worker.id);
                        if is_first_entry {
                            shown_workers.insert(worker.id);
                        }

                        for (index, entry) in entries.iter().enumerate() {
                            let display_name = if is_first_entry && index == 0 {
                                worker.name.clone()
                            } else {
                                "".to_string()
                            };

                            let clock_in_time = entry
                                .clock_in
                                .with_timezone(&Santiago)
                                .format("%H:%M:%S")
                                .to_string();

                            let (clock_out_time, color) = if let Some(out_time) = entry.clock_out {
                                (
                                    out_time
                                        .with_timezone(&Santiago)
                                        .format("%H:%M:%S")
                                        .to_string(),
                                    slint::Color::from_rgb_u8(0, 255, 0),
                                ) // Green for completed
                            } else {
                                (
                                    "In Progress".to_string(),
                                    slint::Color::from_rgb_u8(255, 165, 0),
                                ) // Orange for ongoing
                            };

                            let duration = if let Some(out_time) = entry.clock_out {
                                let hours =
                                    (out_time - entry.clock_in).num_seconds() as f64 / 3600.0;
                                format_hours(hours)
                            } else {
                                let hours = (chrono::Utc::now() - entry.clock_in).num_seconds()
                                    as f64
                                    / 3600.0;
                                format!("{} (ongoing)", format_hours(hours))
                            };

                            let display = WorkerDisplay {
                                id: worker.id,
                                name: display_name,
                                clock_in_time,
                                clock_out_time,
                                duration,
                                color,
                            };
                            worker_displays.push(display);
                        }
                    }
                    Err(_) => {
                        if !shown_workers.contains(&worker.id) {
                            shown_workers.insert(worker.id);
                            // No entries for today, show empty row with name
                            let display = WorkerDisplay {
                                id: worker.id,
                                name: worker.name.clone(),
                                clock_in_time: "".to_string(),
                                clock_out_time: "".to_string(),
                                duration: "0:00".to_string(),
                                color: slint::Color::from_rgb_u8(200, 200, 200), // Gray
                            };
                            worker_displays.push(display);
                        }
                    }
                }
            }

            let worker_items: Vec<WorkerItem> = worker_displays
                .into_iter()
                .zip(workers.iter().cycle())
                .map(|(w, worker)| WorkerItem {
                    name: SharedString::from(w.name),
                    checked_in_time: SharedString::from(w.clock_in_time),
                    checked_out_time: SharedString::from(w.clock_out_time),
                    current_total_hours: SharedString::from(w.duration),
                    color: w.color,
                    barcode: SharedString::from(worker.barcode.clone()),
                })
                .collect();

            ui.set_workers(Rc::new(slint::VecModel::from(worker_items)).into());

            let names: Vec<SharedString> = workers
                .iter()
                .map(|w| SharedString::from(w.name.clone()))
                .collect();
            ui.set_worker_names(Rc::new(slint::VecModel::from(names)).into());

            // Compute reports
            let mut report_items = Vec::new();
            let now = chrono::Utc::now();
            let today = now.format("%Y-%m-%d").to_string();
            let selected_date_str = ui.get_selected_date().to_string();
            let selected_naive = chrono::NaiveDate::parse_from_str(&selected_date_str, "%Y-%m-%d")
                .unwrap_or(now.date_naive());
            let today = selected_naive.format("%Y-%m-%d").to_string();
            let month = selected_naive.format("%Y-%m").to_string();
            let month = if month.is_empty() {
                now.format("%Y-%m").to_string()
            } else {
                month
            };
            let month = if month.is_empty() {
                now.format("%Y-%m").to_string()
            } else {
                month
            };
            let week = selected_naive.week(chrono::Weekday::Mon);
            let week_start = week.first_day();
            let week_end = week.last_day();
            let week_start_str = week_start.format("%Y-%m-%d").to_string();
            let week_end_str = week_end.format("%Y-%m-%d").to_string();

            for worker in &workers {
                let daily = db::get_daily_hours(&*conn.borrow(), worker.id, &today).unwrap_or(0.0);
                let weekly = db::get_weekly_hours(
                    &*conn.borrow(),
                    worker.id,
                    &week_start_str,
                    &week_end_str,
                )
                .unwrap_or(0.0);
                let monthly =
                    db::get_monthly_hours(&*conn.borrow(), worker.id, &month).unwrap_or(0.0);
                report_items.push(ReportItem {
                    name: SharedString::from(worker.name.clone()),
                    daily_hours: SharedString::from(format_hours(daily)),
                    weekly_hours: SharedString::from(format_hours(weekly)),
                    monthly_hours: SharedString::from(format_hours(monthly)),
                });
            }
            ui.set_reports(Rc::new(slint::VecModel::from(report_items)).into());
        }
        Err(e) => {
            ui.set_error_message(format!("Failed to load workers: {}", e).into());
        }
    }

    // Handle barcode input
    let conn_clone2 = conn.clone();
    ui.on_barcode_scanned(move |barcode_str| {
        // Check if enough time has passed since last scan (5 seconds cooldown)
        let now = chrono::Utc::now();
        {
            let mut last_scan = LAST_SCAN_TIME.lock().unwrap();
            if let Some(last_time) = *last_scan {
                if now.signed_duration_since(last_time) < chrono::Duration::seconds(5) {
                    return; // Ignore scan, too soon after last one
                }
            }
            *last_scan = Some(now);
        }

        let conn = conn_clone2.borrow();
        let worker_result = db::get_worker_by_barcode(&*conn, &barcode_str);
        match worker_result {
            Ok(Some(worker)) => {
                let status_result = db::get_current_status(&*conn, worker.id);
                match status_result {
                    Ok(Some(_)) => {
                        if let Err(e) = db::clock_out(&*conn, worker.id) {
                            if let Some(ui) = ui_handle_barcode.upgrade() {
                                ui.set_error_message(format!("Failed to clock out: {}", e).into());
                            }
                            return;
                        }
                    }
                    Ok(None) => {
                        if let Err(e) = db::clock_in(&*conn, worker.id) {
                            if let Some(ui) = ui_handle_barcode.upgrade() {
                                ui.set_error_message(format!("Failed to clock in: {}", e).into());
                            }
                            return;
                        }
                    }
                    Err(e) => {
                        if let Some(ui) = ui_handle_barcode.upgrade() {
                            ui.set_error_message(format!("Failed to get status: {}", e).into());
                        }
                        return;
                    }
                }
                if let Some(ui) = ui_handle_barcode.upgrade() {
                    ui.set_error_message("".into());
                }
                refresh_workers(&conn_clone2, &ui_handle_barcode);
            }
            Ok(None) => {
                if let Some(ui) = ui_handle_barcode.upgrade() {
                    ui.set_error_message("Worker not found".into());
                }
            }
            Err(e) => {
                if let Some(ui) = ui_handle_barcode.upgrade() {
                    ui.set_error_message(format!("Error finding worker: {}", e).into());
                }
            }
        }
    });

    // Handle add worker
    let conn_clone3 = conn.clone();
    ui.on_add_worker(move |name, barcode| {
        let name = name.trim();
        let barcode = barcode.trim();
        if !name.is_empty() && !barcode.is_empty() {
            let conn = conn_clone3.borrow();
            match db::add_worker(&*conn, name, barcode) {
                Ok(_) => {
                    if let Some(ui) = ui_handle_add.upgrade() {
                        ui.set_error_message("".into());
                    }
                    refresh_workers(&conn_clone3, &ui_handle_add);
                }
                Err(e) => {
                    if let Some(ui) = ui_handle_add.upgrade() {
                        ui.set_error_message(format!("Failed to add worker: {}", e).into());
                    }
                }
            }
        } else {
            if let Some(ui) = ui_handle_add.upgrade() {
                ui.set_error_message("Name and barcode are required".into());
            }
        }
    });

    // Handle edit worker
    let conn_clone4 = conn.clone();
    ui.on_edit_worker(move |old_name, new_name, new_barcode| {
        let old_name = old_name.trim();
        let new_name = new_name.trim();
        let new_barcode = new_barcode.trim();
        if !old_name.is_empty() && !new_name.is_empty() && !new_barcode.is_empty() {
            let conn = conn_clone4.borrow();
            match db::get_workers(&*conn) {
                Ok(workers) => {
                    if let Some(worker) = workers.into_iter().find(|w| w.name == old_name) {
                        match db::update_worker(&*conn, worker.id, new_name, new_barcode) {
                            Ok(_) => {
                                if let Some(ui) = ui_handle_edit.upgrade() {
                                    ui.set_error_message("".into());
                                }
                                refresh_workers(&conn_clone4, &ui_handle_edit);
                            }
                            Err(e) => {
                                if let Some(ui) = ui_handle_edit.upgrade() {
                                    ui.set_error_message(
                                        format!("Failed to update worker: {}", e).into(),
                                    );
                                }
                            }
                        }
                    } else {
                        if let Some(ui) = ui_handle_edit.upgrade() {
                            ui.set_error_message("Worker not found".into());
                        }
                    }
                }
                Err(e) => {
                    if let Some(ui) = ui_handle_edit.upgrade() {
                        ui.set_error_message(format!("Failed to get workers: {}", e).into());
                    }
                }
            }
        }
    });

    // Handle date changed
    let conn_clone_date = conn.clone();
    let ui_handle_date = ui_handle.clone();
    ui.on_date_changed(move || {
        refresh_workers(&conn_clone_date, &ui_handle_date);
    });

    // Set up timer to refresh ongoing hours every 10 seconds
    let conn_clone_worker_timer = conn.clone();
    let ui_handle_worker_timer = ui_handle.clone();
    let worker_timer = slint::Timer::default();
    worker_timer.start(
        slint::TimerMode::Repeated,
        std::time::Duration::from_secs(10),
        move || {
            refresh_workers(&conn_clone_worker_timer, &ui_handle_worker_timer);
        },
    );

    // Set up timer to update current time every second
    let ui_handle_time_timer = ui_handle.clone();
    let time_timer = slint::Timer::default();
    time_timer.start(
        slint::TimerMode::Repeated,
        std::time::Duration::from_secs(1),
        move || {
            if let Some(ui) = ui_handle_time_timer.upgrade() {
                let now = chrono::Utc::now().with_timezone(&Santiago);
                ui.set_current_time_display(now.format("%H:%M:%S").to_string().into());
            }
        },
    );

    ui.run()?;
    Ok(())
}

fn refresh_workers(conn: &Rc<RefCell<rusqlite::Connection>>, ui_handle: &slint::Weak<MainWindow>) {
    if let Some(ui) = ui_handle.upgrade() {
        let conn_ref = conn.borrow();
        match db::get_workers(&*conn_ref) {
            Ok(workers) => {
                let mut worker_displays = Vec::new();
                let today = chrono::Utc::now().format("%Y-%m-%d").to_string();

                for worker in &workers {
                    // Get all timesheet entries for today
                    match db::get_daily_timesheet_entries(&*conn_ref, worker.id, &today) {
                        Ok(entries) => {
                            for entry in entries {
                                let clock_in_time = entry
                                    .clock_in
                                    .with_timezone(&Santiago)
                                    .format("%H:%M:%S")
                                    .to_string();

                                let (clock_out_time, color) =
                                    if let Some(out_time) = entry.clock_out {
                                        (
                                            out_time
                                                .with_timezone(&Santiago)
                                                .format("%H:%M:%S")
                                                .to_string(),
                                            slint::Color::from_rgb_u8(0, 255, 0),
                                        ) // Green for completed
                                    } else {
                                        (
                                            "In Progress".to_string(),
                                            slint::Color::from_rgb_u8(255, 165, 0),
                                        ) // Orange for ongoing
                                    };

                                let duration = if let Some(out_time) = entry.clock_out {
                                    let hours =
                                        (out_time - entry.clock_in).num_seconds() as f64 / 3600.0;
                                    format_hours(hours)
                                } else {
                                    let hours = (chrono::Utc::now() - entry.clock_in).num_seconds()
                                        as f64
                                        / 3600.0;
                                    format!("{} (ongoing)", format_hours(hours))
                                };

                                let display = WorkerDisplay {
                                    id: worker.id,
                                    name: worker.name.clone(),
                                    clock_in_time,
                                    clock_out_time,
                                    duration,
                                    color,
                                };
                                worker_displays.push(display);
                            }
                        }
                        Err(_) => {
                            // No entries for today, show empty row
                            let display = WorkerDisplay {
                                id: worker.id,
                                name: worker.name.clone(),
                                clock_in_time: "".to_string(),
                                clock_out_time: "".to_string(),
                                duration: "0:00".to_string(),
                                color: slint::Color::from_rgb_u8(200, 200, 200), // Gray
                            };
                            worker_displays.push(display);
                        }
                    }
                }
                let worker_items: Vec<WorkerItem> = worker_displays
                    .into_iter()
                    .zip(workers.iter().cycle())
                    .map(|(w, worker)| WorkerItem {
                        name: SharedString::from(w.name),
                        checked_in_time: SharedString::from(w.clock_in_time),
                        checked_out_time: SharedString::from(w.clock_out_time),
                        current_total_hours: SharedString::from(w.duration),
                        color: w.color,
                        barcode: SharedString::from(worker.barcode.clone()),
                    })
                    .collect();
                ui.set_workers(Rc::new(slint::VecModel::from(worker_items)).into());

                let names: Vec<SharedString> = workers
                    .iter()
                    .map(|w| SharedString::from(w.name.clone()))
                    .collect();
                ui.set_worker_names(Rc::new(slint::VecModel::from(names)).into());

                // Update reports
                let mut report_items = Vec::new();
                let now = chrono::Utc::now();
                let today = now.format("%Y-%m-%d").to_string();
                let selected_date_str = ui.get_selected_date().to_string();
                let selected_naive =
                    chrono::NaiveDate::parse_from_str(&selected_date_str, "%Y-%m-%d")
                        .unwrap_or(now.date_naive());
                let today = selected_naive.format("%Y-%m-%d").to_string();
                let month = selected_naive.format("%Y-%m").to_string();
                // Week start (Monday), end (Sunday)
                let week = selected_naive.week(Weekday::Mon);
                let week_start = week.first_day();
                let week_end = week.last_day();
                let week_start_str = week_start.format("%Y-%m-%d").to_string();
                let week_end_str = week_end.format("%Y-%m-%d").to_string();

                for worker in &workers {
                    let daily = db::get_daily_hours(&*conn_ref, worker.id, &today).unwrap_or(0.0);
                    let weekly =
                        db::get_weekly_hours(&*conn_ref, worker.id, &week_start_str, &week_end_str)
                            .unwrap_or(0.0);
                    let monthly =
                        db::get_monthly_hours(&*conn_ref, worker.id, &month).unwrap_or(0.0);
                    report_items.push(ReportItem {
                        name: SharedString::from(worker.name.clone()),
                        daily_hours: SharedString::from(format_hours(daily)),
                        weekly_hours: SharedString::from(format_hours(weekly)),
                        monthly_hours: SharedString::from(format_hours(monthly)),
                    });
                }
                ui.set_reports(Rc::new(slint::VecModel::from(report_items)).into());
            }
            Err(e) => {
                ui.set_error_message(format!("Failed to refresh workers: {}", e).into());
            }
        }
    }
}

fn get_last_clock_out(
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

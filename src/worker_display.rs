use chrono::Weekday;
use chrono_tz::America::Santiago;
use slint::SharedString;
use std::cell::RefCell;
use std::rc::Rc;

use crate::types::{TimesheetDisplay, DataWorker};
use crate::utils::format_hours;
use crate::ui::{WorkerWithTimes, WorkerInfo, ReportItem};

pub fn refresh_workers(conn: &Rc<RefCell<rusqlite::Connection>>, ui_handle: &slint::Weak<crate::ui::MainWindow>) {
    if let Some(ui) = ui_handle.upgrade() {
        let conn_ref = conn.borrow();
        match crate::db::get_workers(&conn_ref) {
            Ok(workers) => {
                let today = chrono::Utc::now().format("%Y-%m-%d").to_string();

                let workers_data: Vec<DataWorker> = workers
                    .iter()
                    .map(|worker| {
                        match crate::db::get_daily_timesheet_entries(&conn_ref, worker.id, &today) {
                            Ok(entries) if !entries.is_empty() => {
                                let times = entries
                                    .iter()
                                    .enumerate()
                                    .map(|(index, entry)| {
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
                                                    slint::Color::from_argb_u8(0, 0, 0, 0),
                                                ) // Transparent for completed
                                            } else {
                                                (
                                                    "En Progreso".to_string(),
                                                    slint::Color::from_rgb_u8(255, 165, 0),
                                                ) // Orange for ongoing
                                            };

                                        let duration = if let Some(out_time) = entry.clock_out {
                                            let hours = (out_time - entry.clock_in).num_seconds()
                                                as f64
                                                / 3600.0;
                                            format_hours(hours)
                                        } else {
                                            let hours = (chrono::Utc::now() - entry.clock_in)
                                                .num_seconds()
                                                as f64
                                                / 3600.0;
                                            format!("{} (en curso)", format_hours(hours))
                                        };

                                        TimesheetDisplay {
                                            checked_in_time: clock_in_time,
                                            checked_out_time: clock_out_time,
                                            current_total_hours: duration,
                                            color,
                                            show_name: index == 0,
                                        }
                                    })
                                    .collect();
                                DataWorker {
                                    worker: worker.clone(),
                                    times,
                                }
                            }
                            Ok(_) | Err(_) => {
                                // No entries or error: show worker with placeholder data
                                let times = vec![TimesheetDisplay {
                                    checked_in_time: "".to_string(),
                                    checked_out_time: "".to_string(),
                                    current_total_hours: "0:00".to_string(),
                                    color: slint::Color::from_rgb_u8(200, 200, 200), // Gray
                                    show_name: true,
                                }];
                                DataWorker {
                                    worker: worker.clone(),
                                    times,
                                }
                            }
                        }
                    })
                    .collect();

                let worker_items: Vec<WorkerWithTimes> = workers_data
                    .into_iter()
                    .flat_map(|w| {
                        w.times.into_iter().map(move |t| WorkerWithTimes {
                            name: SharedString::from(if t.show_name {
                                w.worker.name.clone()
                            } else {
                                "".to_string()
                            }),
                            checked_in_time: SharedString::from(t.checked_in_time),
                            checked_out_time: SharedString::from(t.checked_out_time),
                            current_total_hours: SharedString::from(t.current_total_hours),
                            color: t.color,
                            barcode: SharedString::from(w.worker.barcode.clone()),
                            show_name: t.show_name,
                        })
                    })
                    .collect();
                ui.set_workers(Rc::new(slint::VecModel::from(worker_items)).into());

                // keep worker_names updated
                let names: Vec<SharedString> = workers
                    .iter()
                    .map(|w| SharedString::from(w.name.clone()))
                    .collect();
                ui.set_worker_names(Rc::new(slint::VecModel::from(names)).into());

                // 🔧 NEW: also refresh management_workers for the Workers tab
                let management_worker_items: Vec<WorkerInfo> = workers
                    .iter()
                    .map(|w| WorkerInfo {
                        name: SharedString::from(w.name.clone()),
                        barcode: SharedString::from(w.barcode.clone()),
                    })
                    .collect();
                ui.set_management_workers(
                    Rc::new(slint::VecModel::from(management_worker_items)).into(),
                );

                // Update reports
                let mut report_items = Vec::new();
                let now = chrono::Utc::now();
                let _today = now.format("%Y-%m-%d").to_string();
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
                    let daily = crate::db::get_daily_hours(&conn_ref, worker.id, &today).unwrap_or(0.0);
                    let weekly =
                        crate::db::get_weekly_hours(&conn_ref, worker.id, &week_start_str, &week_end_str)
                            .unwrap_or(0.0);
                    let monthly =
                        crate::db::get_monthly_hours(&conn_ref, worker.id, &month).unwrap_or(0.0);
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
                ui.set_error_dialog_message(
                    format!("Error al actualizar trabajadores: {}", e).into(),
                );
                ui.set_show_error_dialog(true);
                ui.set_trigger_error_dialog_show(true);
            }
        }
    }
}
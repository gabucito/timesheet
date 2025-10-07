use chrono::Weekday;
use chrono_tz::America::Santiago;
use slint::SharedString;
use std::cell::RefCell;
use std::rc::Rc;

use crate::types::{DataWorker, TimesheetDisplay};
use crate::ui::{ReportItem, WorkerInfo, WorkerWithTimes};
use crate::utils::format_hours;

pub fn refresh_workers(
    conn: &Rc<RefCell<rusqlite::Connection>>,
    ui_handle: &slint::Weak<crate::ui::MainWindow>,
) {
    if let Some(ui) = ui_handle.upgrade() {
        let conn_ref = conn.borrow();
        match crate::db::get_workers(&conn_ref) {
            Ok(workers) => {
                let today = chrono::Utc::now().format("%Y-%m-%d").to_string();

                // Sort workers: in progress first (by last check-in desc), then not in progress (by last check-out desc)
                let mut worker_sort_list: Vec<(
                    crate::db::Worker,
                    bool,
                    Option<chrono::DateTime<chrono::Utc>>,
                )> = workers
                    .into_iter()
                    .map(|w| {
                        let entries =
                            crate::db::get_daily_timesheet_entries(&conn_ref, w.id, &today)
                                .unwrap_or_default();
                        let is_in_progress = entries.iter().any(|e| e.clock_out.is_none());
                        let sort_time = if is_in_progress {
                            entries
                                .iter()
                                .find(|e| e.clock_out.is_none())
                                .map(|e| e.clock_in)
                        } else {
                            entries.iter().filter_map(|e| e.clock_out).max()
                        };
                        (w, is_in_progress, sort_time)
                    })
                    .collect();
                worker_sort_list.sort_by_key(|&(_, in_progress, time)| {
                    (
                        std::cmp::Reverse(in_progress),
                        std::cmp::Reverse(
                            time.unwrap_or_else(|| chrono::DateTime::from_timestamp(0, 0).unwrap()),
                        ),
                    )
                });
                let sorted_workers: Vec<crate::db::Worker> =
                    worker_sort_list.into_iter().map(|(w, _, _)| w).collect();

                // Separate into two groups
                let (in_progress_workers, not_in_progress_workers): (
                    Vec<crate::db::Worker>,
                    Vec<crate::db::Worker>,
                ) = sorted_workers.into_iter().partition(|w| {
                    let entries = crate::db::get_daily_timesheet_entries(&conn_ref, w.id, &today)
                        .unwrap_or_default();
                    entries.iter().any(|e| e.clock_out.is_none())
                });

                // Restore sorted_workers for other uses
                let mut sorted_workers = in_progress_workers.clone();
                sorted_workers.extend(not_in_progress_workers.clone());

                let in_progress_workers_data: Vec<DataWorker> = in_progress_workers
                    .iter()
                    .map(|worker| {
                        match crate::db::get_daily_timesheet_entries(&conn_ref, worker.id, &today) {
                            Ok(entries) => {
                                let entries_to_show = if entries.len() > 2 {
                                    entries[entries.len() - 2..].to_vec()
                                } else {
                                    entries
                                };
                                if !entries_to_show.is_empty() {
                                    let times = entries_to_show
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

                                            // Duration removed from display

                                            TimesheetDisplay {
                                                checked_in_time: clock_in_time,
                                                checked_out_time: clock_out_time,
                                                color,
                                                show_name: index == 0,
                                            }
                                        })
                                        .collect();
                                    DataWorker {
                                        worker: worker.clone(),
                                        times,
                                    }
                                } else {
                                    // No entries: show worker with placeholder data
                                    let times = vec![TimesheetDisplay {
                                        checked_in_time: "".to_string(),
                                        checked_out_time: "".to_string(),
                                        color: slint::Color::from_rgb_u8(200, 200, 200), // Gray
                                        show_name: true,
                                    }];
                                    DataWorker {
                                        worker: worker.clone(),
                                        times,
                                    }
                                }
                            }
                            Err(_) => {
                                // No entries or error: show worker with placeholder data
                                let times = vec![TimesheetDisplay {
                                    checked_in_time: "".to_string(),
                                    checked_out_time: "".to_string(),
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

                let not_in_progress_workers_data: Vec<DataWorker> = not_in_progress_workers
                    .iter()
                    .map(|worker| {
                        match crate::db::get_daily_timesheet_entries(&conn_ref, worker.id, &today) {
                            Ok(entries) => {
                                let entries_to_show = if entries.len() > 2 {
                                    entries[entries.len() - 2..].to_vec()
                                } else {
                                    entries
                                };
                                if !entries_to_show.is_empty() {
                                    let times = entries_to_show
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

                                            // Duration removed from display

                                            TimesheetDisplay {
                                                checked_in_time: clock_in_time,
                                                checked_out_time: clock_out_time,
                                                color,
                                                show_name: index == 0,
                                            }
                                        })
                                        .collect();
                                    DataWorker {
                                        worker: worker.clone(),
                                        times,
                                    }
                                } else {
                                    // No entries: show worker with placeholder data
                                    let times = vec![TimesheetDisplay {
                                        checked_in_time: "".to_string(),
                                        checked_out_time: "".to_string(),
                                        color: slint::Color::from_rgb_u8(200, 200, 200), // Gray
                                        show_name: true,
                                    }];
                                    DataWorker {
                                        worker: worker.clone(),
                                        times,
                                    }
                                }
                            }
                            Err(_) => {
                                // No entries or error: show worker with placeholder data
                                let times = vec![TimesheetDisplay {
                                    checked_in_time: "".to_string(),
                                    checked_out_time: "".to_string(),
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

                let in_progress_worker_items: Vec<WorkerWithTimes> = in_progress_workers_data
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
                            color: t.color,
                            barcode: SharedString::from(w.worker.barcode.clone()),
                            show_name: t.show_name,
                        })
                    })
                    .collect();

                let not_in_progress_worker_items: Vec<WorkerWithTimes> =
                    not_in_progress_workers_data
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
                                color: t.color,
                                barcode: SharedString::from(w.worker.barcode.clone()),
                                show_name: t.show_name,
                            })
                        })
                        .collect();

                ui.set_in_progress_workers(
                    Rc::new(slint::VecModel::from(in_progress_worker_items)).into(),
                );
                ui.set_not_in_progress_workers(
                    Rc::new(slint::VecModel::from(not_in_progress_worker_items)).into(),
                );

                // keep worker_names updated
                let names: Vec<SharedString> = sorted_workers
                    .iter()
                    .map(|w| SharedString::from(w.name.clone()))
                    .collect();
                ui.set_worker_names(Rc::new(slint::VecModel::from(names)).into());

                // 🔧 NEW: also refresh management_workers for the Workers tab
                let management_worker_items: Vec<WorkerInfo> = sorted_workers
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

                for worker in &sorted_workers {
                    let daily =
                        crate::db::get_daily_hours(&conn_ref, worker.id, &today).unwrap_or(0.0);
                    let weekly = crate::db::get_weekly_hours(
                        &conn_ref,
                        worker.id,
                        &week_start_str,
                        &week_end_str,
                    )
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

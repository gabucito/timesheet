use chrono::{Datelike, Timelike};
use chrono_tz::America::Santiago;
use std::cell::RefCell;
use std::rc::Rc;

use crate::{db, reports};
use slint::ComponentHandle;

static LAST_SCAN_TIME: std::sync::Mutex<Option<chrono::DateTime<chrono::Utc>>> =
    std::sync::Mutex::new(None);
static LAST_SCAN_BARCODE: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

pub fn setup_event_handlers(conn: Rc<RefCell<rusqlite::Connection>>, ui: &crate::ui::MainWindow) {
    let ui_handle = ui.as_weak();
    let ui_handle_barcode = ui_handle.clone();
    let ui_handle_add = ui_handle.clone();
    let ui_handle_edit = ui_handle.clone();

    let conn_clone2 = conn.clone();
    let conn_clone3 = conn.clone();
    let conn_clone4 = conn.clone();
    let conn_clone_date = conn.clone();
    let _conn_clone_worker_timer = conn.clone();
    let conn_clone_report = conn.clone();

    ui.on_barcode_scanned(move |barcode_str| {
        println!("Barcode scanned callback triggered with: '{}'", barcode_str);
        let trimmed_barcode = crate::barcode::normalize(&barcode_str);
        // Check if scan should be ignored: too fast AND same barcode as last
        let now = chrono::Utc::now();
        {
            let mut last_scan_time = LAST_SCAN_TIME.lock().unwrap();
            let mut last_scan_barcode = LAST_SCAN_BARCODE.lock().unwrap();
            if let (Some(last_time), Some(last_barcode)) =
                (*last_scan_time, last_scan_barcode.as_ref())
                && now.signed_duration_since(last_time) < chrono::Duration::seconds(2)
                && *last_barcode == trimmed_barcode
            {
                println!("Scan ignored - too soon after last scan and same barcode");
                return;
            }
            *last_scan_time = Some(now);
            *last_scan_barcode = Some(trimmed_barcode.to_string());
        }

        let conn = conn_clone2.borrow();
        println!("Looking up worker with barcode: '{}'", trimmed_barcode);
        let worker_result = db::get_worker_by_barcode(&conn, &trimmed_barcode);
        match worker_result {
            Ok(Some(worker)) => {
                println!("Worker found: {} (ID: {})", worker.name, worker.id);
                let status_result = db::get_current_status(&conn, worker.id);
                match status_result {
                    Ok(Some(_)) => {
                        // Worker is currently clocked in, perform clock out
                        if let Err(e) = db::clock_out(&conn, worker.id) {
                            if let Some(ui) = ui_handle_barcode.upgrade() {
                                ui.set_error_dialog_message(
                                    format!("Error al marcar salida: {}", e).into(),
                                );
                                ui.set_show_error_dialog(true);
                                ui.set_trigger_error_dialog_show(true);
                            }
                            return;
                        }
                        // Show notification
                        if let Some(ui) = ui_handle_barcode.upgrade() {
                            println!("Showing notification dialog for clock out: {}", worker.name);
                            ui.set_confirm_worker_name(worker.name.into());
                            ui.set_confirm_action("Salida registrada".into());
                            ui.set_confirm_is_check_in(false);
                            ui.set_show_confirm_dialog(true);
                            ui.set_trigger_dialog_show(true);
                        }
                    }
                    Ok(None) => {
                        // Worker is not clocked in, perform clock in
                        if let Err(e) = db::clock_in(&conn, worker.id) {
                            if let Some(ui) = ui_handle_barcode.upgrade() {
                                ui.set_error_dialog_message(
                                    format!("Error al marcar entrada: {}", e).into(),
                                );
                                ui.set_show_error_dialog(true);
                                ui.set_trigger_error_dialog_show(true);
                            }
                            return;
                        }
                        // Show notification
                        if let Some(ui) = ui_handle_barcode.upgrade() {
                            println!("Showing notification dialog for clock in: {}", worker.name);
                            ui.set_confirm_worker_name(worker.name.into());
                            ui.set_confirm_action("Entrada registrada".into());
                            ui.set_confirm_is_check_in(true);
                            ui.set_show_confirm_dialog(true);
                            ui.set_trigger_dialog_show(true);
                        }
                    }
                    Err(e) => {
                        if let Some(ui) = ui_handle_barcode.upgrade() {
                            ui.set_error_dialog_message(
                                format!("Error al obtener estado: {}", e).into(),
                            );
                            ui.set_show_error_dialog(true);
                            ui.set_trigger_error_dialog_show(true);
                        }
                        return;
                    }
                }
                if let Some(ui) = ui_handle_barcode.upgrade() {
                    ui.set_show_error_dialog(false);
                }
                crate::worker_display::refresh_workers(&conn_clone2, &ui_handle_barcode);
            }
            Ok(None) => {
                println!("Worker not found for barcode: '{}'", trimmed_barcode);
                if let Some(ui) = ui_handle_barcode.upgrade() {
                    ui.set_error_dialog_message("Trabajador no encontrado".into());
                    ui.set_show_error_dialog(true);
                    ui.set_trigger_error_dialog_show(true);
                }
            }
            Err(e) => {
                println!("Error looking up worker: {}", e);
                if let Some(ui) = ui_handle_barcode.upgrade() {
                    ui.set_error_dialog_message(
                        format!("Error al buscar trabajador: {}", e).into(),
                    );
                    ui.set_show_error_dialog(true);
                    ui.set_trigger_error_dialog_show(true);
                }
            }
        }
    });

    ui.on_add_worker(move |name, barcode| {
        let name = name.trim();
        let barcode = crate::barcode::normalize(&barcode);
        if !name.is_empty() && !barcode.is_empty() {
            let conn = conn_clone3.borrow();
            match db::add_worker(&conn, name, &barcode) {
                Ok(_) => {
                    if let Some(ui) = ui_handle_add.upgrade() {
                        ui.set_show_error_dialog(false);
                    }
                    crate::worker_display::refresh_workers(&conn_clone3, &ui_handle_add);
                }
                Err(e) => {
                    if let Some(ui) = ui_handle_add.upgrade() {
                        ui.set_error_dialog_message(
                            format!("Error al agregar trabajador: {}", e).into(),
                        );
                        ui.set_show_error_dialog(true);
                        ui.set_trigger_error_dialog_show(true);
                    }
                }
            }
        } else if let Some(ui) = ui_handle_add.upgrade() {
            ui.set_error_dialog_message("El nombre y código de barras son obligatorios".into());
            ui.set_show_error_dialog(true);
            ui.set_trigger_error_dialog_show(true);
        }
    });

    ui.on_edit_worker(move |old_name, new_name, new_barcode| {
        let old_name = old_name.trim();
        let new_name = new_name.trim();
        let new_barcode = crate::barcode::normalize(&new_barcode);
        if !old_name.is_empty() && !new_name.is_empty() && !new_barcode.is_empty() {
            let conn = conn_clone4.borrow();
            match db::get_workers(&conn) {
                Ok(workers) => {
                    if let Some(worker) = workers.into_iter().find(|w| w.name == old_name) {
                        match db::update_worker(&conn, worker.id, new_name, &new_barcode) {
                            Ok(_) => {
                                if let Some(ui) = ui_handle_edit.upgrade() {
                                    ui.set_show_error_dialog(false);
                                }
                                crate::worker_display::refresh_workers(
                                    &conn_clone4,
                                    &ui_handle_edit,
                                );
                            }
                            Err(e) => {
                                if let Some(ui) = ui_handle_edit.upgrade() {
                                    ui.set_error_dialog_message(
                                        format!("Error al actualizar trabajador: {}", e).into(),
                                    );
                                    ui.set_show_error_dialog(true);
                                    ui.set_trigger_error_dialog_show(true);
                                }
                            }
                        }
                    } else if let Some(ui) = ui_handle_edit.upgrade() {
                        ui.set_error_dialog_message("Trabajador no encontrado".into());
                        ui.set_show_error_dialog(true);
                        ui.set_trigger_error_dialog_show(true);
                    }
                }
                Err(e) => {
                    if let Some(ui) = ui_handle_edit.upgrade() {
                        ui.set_error_dialog_message(
                            format!("Error al obtener trabajadores: {}", e).into(),
                        );
                        ui.set_show_error_dialog(true);
                        ui.set_trigger_error_dialog_show(true);
                    }
                }
            }
        }
    });

    let ui_handle_confirm = ui_handle.clone();
    ui.on_confirm_check_action(move |_| {
        // Hide dialog
        if let Some(ui) = ui_handle_confirm.upgrade() {
            ui.set_show_confirm_dialog(false);
        }
    });

    let ui_handle_error = ui_handle.clone();
    ui.on_close_error_dialog(move || {
        // Hide dialog
        if let Some(ui) = ui_handle_error.upgrade() {
            ui.set_show_error_dialog(false);
        }
    });

    let ui_handle_time = ui.as_weak();
    ui.on_update_current_time(move || {
        if let Some(ui) = ui_handle_time.upgrade() {
            let now = chrono::Utc::now().with_timezone(&Santiago);
            ui.set_current_time_display(
                format!("{}:{}:{}", now.hour(), now.minute(), now.second()).into(),
            );
        }
    });

    let ui_handle_date = ui_handle.clone();
    ui.on_date_changed(move || {
        crate::worker_display::refresh_workers(&conn_clone_date, &ui_handle_date);
    });

    let ui_handle_test = ui.as_weak();
    let ui_handle_report = ui_handle.clone();
    ui.on_test_printer_connection(move || {
        if let Some(ui) = ui_handle_test.upgrade() {
            // Comprehensive list of common USB thermal printer device paths in Linux
            let printer_devices = [
                "/dev/lp0",
                "/dev/lp1",
                "/dev/lp2",
                "/dev/lp3",
                "/dev/usb/lp0",
                "/dev/usb/lp1",
                "/dev/usb/lp2",
                "/dev/usb/lp3",
                "/dev/ttyACM0",
                "/dev/ttyACM1",
                "/dev/ttyACM2",
                "/dev/ttyACM3",
                "/dev/ttyUSB0",
                "/dev/ttyUSB1",
                "/dev/ttyUSB2",
                "/dev/ttyUSB3",
                "/dev/ttyS0",
                "/dev/ttyS1",
                "/dev/ttyS2",
                "/dev/ttyS3",
            ];
            for device in &printer_devices {
                if let Ok(mut file) = std::fs::File::create(device) {
                    use std::io::Write;
                    let _ = file.write_all(b"\x1b@\nPrinter OK\n\x1dVA\x00");
                    ui.set_printer_status_message("Printer connected".into());
                    return;
                }
            }
            ui.set_printer_status_message("Printer not found".into());
        }
    });

    ui.on_generate_report(move || {
        if let Some(ui) = ui_handle_report.upgrade() {
            ui.set_report_status_message("".into());
            let selected_date_str = ui.get_selected_date().to_string();
            let selected_naive = chrono::NaiveDate::parse_from_str(&selected_date_str, "%Y-%m-%d")
                .unwrap_or_else(|_| chrono::Utc::now().date_naive());
            let month_start = chrono::NaiveDate::from_ymd_opt(
                selected_naive.year(),
                selected_naive.month(),
                1,
            )
            .unwrap_or(selected_naive);
            let month_label = month_start.format("%Y-%m").to_string();
            let output_dir = std::path::PathBuf::from(format!("reports/{}", month_label));

            let result = {
                let conn_ref = conn_clone_report.borrow();
                reports::generate_monthly_reports(&*conn_ref, month_start, &output_dir)
            };

            match result {
                Ok(()) => {
                    ui.set_report_status_message(
                        format!(
                            "Reportes generados para {} en {}",
                            month_label,
                            output_dir.display()
                        )
                        .into(),
                    );
                }
                Err(e) => {
                    ui.set_error_dialog_message(
                        format!("Error al generar reportes: {}", e).into(),
                    );
                    ui.set_show_error_dialog(true);
                    ui.set_trigger_error_dialog_show(true);
                }
            }
        }
    });
}

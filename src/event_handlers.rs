use chrono_tz::America::Santiago;
use slint::SharedString;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Mutex;

use crate::db;
use crate::ui;
use slint::ComponentHandle;

static LAST_SCAN_TIME: std::sync::Mutex<Option<chrono::DateTime<chrono::Utc>>> = std::sync::Mutex::new(None);
static LAST_SCAN_BARCODE: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

pub fn setup_event_handlers(
    conn: Rc<RefCell<rusqlite::Connection>>,
    ui: &crate::ui::MainWindow,
) {
    let ui_handle = ui.as_weak();
    let ui_handle_barcode = ui_handle.clone();
    let ui_handle_add = ui_handle.clone();
    let ui_handle_edit = ui_handle.clone();

    let conn_clone2 = conn.clone();
    let conn_clone3 = conn.clone();
    let conn_clone4 = conn.clone();
    let conn_clone_date = conn.clone();
    let conn_clone_worker_timer = conn.clone();
    let conn_clone_print = conn.clone();

    ui.on_barcode_scanned(move |barcode_str| {
        println!("Barcode scanned callback triggered with: '{}'", barcode_str);
        // Check if scan should be ignored: too fast AND same barcode as last
        let now = chrono::Utc::now();
        {
            let mut last_scan_time = LAST_SCAN_TIME.lock().unwrap();
            let mut last_scan_barcode = LAST_SCAN_BARCODE.lock().unwrap();
            if let (Some(last_time), Some(last_barcode)) =
                (*last_scan_time, last_scan_barcode.as_ref())
            {
                if now.signed_duration_since(last_time) < chrono::Duration::seconds(5)
                    && last_barcode == barcode_str.as_str()
                {
                    println!("Scan ignored - too soon after last scan and same barcode");
                    return;
                }
            }
            *last_scan_time = Some(now);
            *last_scan_barcode = Some(barcode_str.to_string());
        }

        let conn = conn_clone2.borrow();
        println!("Looking up worker with barcode: '{}'", barcode_str);
        let worker_result = db::get_worker_by_barcode(&*conn, &barcode_str);
        match worker_result {
            Ok(Some(worker)) => {
                println!("Worker found: {} (ID: {})", worker.name, worker.id);
                let status_result = db::get_current_status(&*conn, worker.id);
                match status_result {
                    Ok(Some(_)) => {
                        // Worker is currently clocked in, perform clock out
                        if let Err(e) = db::clock_out(&*conn, worker.id) {
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
                            ui.set_show_confirm_dialog(true);
                            ui.set_trigger_dialog_show(true);
                        }
                    }
                    Ok(None) => {
                        // Worker is not clocked in, perform clock in
                        if let Err(e) = db::clock_in(&*conn, worker.id) {
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
                println!("Worker not found for barcode: '{}'", barcode_str);
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
        let barcode = barcode.trim();
        if !name.is_empty() && !barcode.is_empty() {
            let conn = conn_clone3.borrow();
            match db::add_worker(&*conn, name, barcode) {
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
        } else {
            if let Some(ui) = ui_handle_add.upgrade() {
                ui.set_error_dialog_message("El nombre y código de barras son obligatorios".into());
                ui.set_show_error_dialog(true);
                ui.set_trigger_error_dialog_show(true);
            }
        }
    });

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
                                    ui.set_show_error_dialog(false);
                                }
                                crate::worker_display::refresh_workers(&conn_clone4, &ui_handle_edit);
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
                    } else {
                        if let Some(ui) = ui_handle_edit.upgrade() {
                            ui.set_error_dialog_message("Trabajador no encontrado".into());
                            ui.set_show_error_dialog(true);
                            ui.set_trigger_error_dialog_show(true);
                        }
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

    let ui_handle_date = ui_handle.clone();
    ui.on_date_changed(move || {
        crate::worker_display::refresh_workers(&conn_clone_date, &ui_handle_date);
    });

    let ui_handle_test = ui.as_weak();
    let ui_handle_print = ui_handle.clone();
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

    ui.on_print_report(move || {
        println!("Print report button clicked");
        if let Some(ui) = ui_handle_print.upgrade() {
            let conn_ref = conn_clone_print.borrow();
            let selected_date_str = ui.get_selected_date().to_string();
            let selected_naive = chrono::NaiveDate::parse_from_str(&selected_date_str, "%Y-%m-%d")
                .unwrap_or(chrono::Utc::now().date_naive());
            let today = selected_naive.format("%Y-%m-%d").to_string();
            let month = selected_naive.format("%Y-%m").to_string();
            let week = selected_naive.week(chrono::Weekday::Mon);
            let week_start = week.first_day();
            let week_end = week.last_day();
            let week_start_str = week_start.format("%Y-%m-%d").to_string();
            let week_end_str = week_end.format("%Y-%m-%d").to_string();

            let workers = db::get_workers(&*conn_ref).unwrap_or(vec![]);

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
            let mut file = None;
            for device in &printer_devices {
                let f = std::fs::OpenOptions::new().write(true).open(device);
                if let Ok(f) = f {
                    println!("Found printer device: {}", device);
                    file = Some(f);
                    break;
                }
            }
            if let Some(mut file) = file {
                use std::io::Write;

                std::thread::sleep(std::time::Duration::from_millis(500));

                if let Err(e) = file.write_all(b"\x1b@") {
                    println!("Error writing ESC/POS init: {}", e);
                    return;
                }

                let _ = file.write_all(b"Timesheet Report\n");
                let _ = file.write_all(format!("Date: {}\n\n", selected_date_str).as_bytes());
                let _ = file.write_all(b"Name\tDaily\tWeekly\tMonthly\n");
                let _ = file.write_all(b"--------------------------------\n");

                for worker in &workers {
                    let daily = db::get_daily_hours(&*conn_ref, worker.id, &today).unwrap_or(0.0);
                    let weekly =
                        db::get_weekly_hours(&*conn_ref, worker.id, &week_start_str, &week_end_str)
                            .unwrap_or(0.0);
                    let monthly =
                        db::get_monthly_hours(&*conn_ref, worker.id, &month).unwrap_or(0.0);
                    let line = format!(
                        "{}\t{:.2}\t{:.2}\t{:.2}\n",
                        worker.name, daily, weekly, monthly
                    );
                    let _ = file.write_all(line.as_bytes());
                }

                let _ = file.write_all(b"\n\x1dVA\x00");
                let _ = file.flush();
                println!("Report sent to printer");
            }
        }
    });
}
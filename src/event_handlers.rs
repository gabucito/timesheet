use chrono::{Datelike, Timelike};
use chrono_tz::America::Santiago;
use std::cell::RefCell;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

use serde::Deserialize;

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
    let ui_handle_detect_usb = ui_handle.clone();
    let ui_handle_open_dir = ui_handle.clone();

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

    ui.on_detect_usb(move || {
        if let Some(ui) = ui_handle_detect_usb.upgrade() {
            match detect_or_mount_usb() {
                Ok(path) => {
                    let path_str = path.display().to_string();
                    ui.set_report_output_directory(path_str.clone().into());
                    ui.set_report_status_message(format!("USB disponible en {}", path_str).into());
                }
                Err(err) => {
                    ui.set_error_dialog_message(
                        format!("No se pudo detectar o montar el USB: {}", err).into(),
                    );
                    ui.set_show_error_dialog(true);
                    ui.set_trigger_error_dialog_show(true);
                }
            }
        }
    });

    ui.on_open_report_directory(move || {
        if let Some(ui) = ui_handle_open_dir.upgrade() {
            let target_string = {
                let last = ui.get_last_report_directory().to_string();
                if !last.trim().is_empty() {
                    Some(last.trim().to_string())
                } else {
                    let base = ui.get_report_output_directory().to_string();
                    if !base.trim().is_empty() {
                        Some(base.trim().to_string())
                    } else {
                        None
                    }
                }
            };

            let Some(target_string) = target_string else {
                ui.set_error_dialog_message("No hay carpeta seleccionada para abrir".into());
                ui.set_show_error_dialog(true);
                ui.set_trigger_error_dialog_show(true);
                return;
            };

            let target_path = PathBuf::from(&target_string);

            if !target_path.exists() {
                ui.set_error_dialog_message(
                    format!("La carpeta {} no existe", target_path.display()).into(),
                );
                ui.set_show_error_dialog(true);
                ui.set_trigger_error_dialog_show(true);
                return;
            }

            if let Err(err) = open_directory_in_file_manager(&target_path) {
                ui.set_error_dialog_message(
                    format!(
                        "No se pudo abrir la carpeta {}: {}",
                        target_path.display(),
                        err
                    )
                    .into(),
                );
                ui.set_show_error_dialog(true);
                ui.set_trigger_error_dialog_show(true);
            }
        }
    });

    ui.on_generate_report(move || {
        if let Some(ui) = ui_handle_report.upgrade() {
            ui.set_report_status_message("".into());
            ui.set_last_report_directory("".into());
            let selected_date_str = ui.get_selected_date().to_string();
            let selected_naive = chrono::NaiveDate::parse_from_str(&selected_date_str, "%Y-%m-%d")
                .unwrap_or_else(|_| chrono::Utc::now().date_naive());
            let month_start =
                chrono::NaiveDate::from_ymd_opt(selected_naive.year(), selected_naive.month(), 1)
                    .unwrap_or(selected_naive);
            let month_label = month_start.format("%Y-%m").to_string();

            // Use a standard accessible location for reports
            let output_dir = PathBuf::from("/tmp/timesheet_reports").join(&month_label);
            let output_dir_str = output_dir.display().to_string();

            // Ensure the directory exists
            if let Err(e) = fs::create_dir_all(&output_dir) {
                ui.set_error_dialog_message(
                    format!("Error creating report directory: {}", e).into(),
                );
                ui.set_show_error_dialog(true);
                ui.set_trigger_error_dialog_show(true);
                return;
            }

            let result = {
                let conn_ref = conn_clone_report.borrow();
                reports::generate_monthly_reports(
                    &*conn_ref,
                    month_start,
                    selected_naive,
                    &output_dir,
                )
            };

            match result {
                Ok(()) => {
                    ui.set_last_report_directory(output_dir_str.clone().into());
                    ui.set_report_status_message(
                        format!(
                            "Reportes generados para {} en {}",
                            month_label, output_dir_str
                        )
                        .into(),
                    );
                }
                Err(e) => {
                    ui.set_error_dialog_message(format!("Error al generar reportes: {}", e).into());
                    ui.set_show_error_dialog(true);
                    ui.set_trigger_error_dialog_show(true);
                }
            }
        }
    });
}

fn resolve_output_directory(base: &str, month_label: &str) -> PathBuf {
    let trimmed = base.trim();
    if trimmed.is_empty() {
        return PathBuf::from(format!("reports/{}", month_label));
    }

    let base_path = PathBuf::from(trimmed);
    if base_path
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| name == month_label)
        .unwrap_or(false)
    {
        base_path
    } else {
        base_path.join(month_label)
    }
}

fn open_directory_in_file_manager(path: &Path) -> std::io::Result<()> {
    #[cfg(target_os = "windows")]
    {
        Command::new("explorer").arg(path).spawn().map(|_| ())
    }
    #[cfg(target_os = "macos")]
    {
        Command::new("open").arg(path).spawn().map(|_| ())
    }
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        Command::new("xdg-open").arg(path).spawn().map(|_| ())
    }
}

fn detect_or_mount_usb() -> Result<PathBuf, UsbMountError> {
    if let Some(existing) = detect_existing_mount() {
        return Ok(existing);
    }

    let devices = enumerate_usb_devices()?;

    if let Some(mounted) = devices
        .iter()
        .filter_map(|dev| dev.mount_point.as_ref())
        .find(|mount| !mount.is_empty())
    {
        return Ok(PathBuf::from(mounted));
    }

    let device = devices.into_iter().next().ok_or(UsbMountError::NoDevices)?;
    mount_device(&device.device_path)
}

fn detect_existing_mount() -> Option<PathBuf> {
    let roots = [
        Path::new("/run/media"),
        Path::new("/media"),
        Path::new("/mnt"),
    ];
    for root in roots {
        if !root.is_dir() {
            continue;
        }
        if let Some(found) = find_mount_under(root) {
            return Some(found);
        }
    }
    None
}

fn find_mount_under(root: &Path) -> Option<PathBuf> {
    if let Ok(entries) = fs::read_dir(root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            if let Ok(nested) = fs::read_dir(&path) {
                for nested_entry in nested.flatten() {
                    let nested_path = nested_entry.path();
                    if nested_path.is_dir() {
                        return Some(nested_path);
                    }
                }
            }
            // fallback to direct directory if no nested directories found
            return Some(path);
        }
    }
    None
}

fn enumerate_usb_devices() -> Result<Vec<UsbDevice>, UsbMountError> {
    let output = Command::new("lsblk")
        .args(["-J", "-o", "NAME,PATH,TYPE,MOUNTPOINT,RM,HOTPLUG,TRAN"])
        .output()
        .map_err(UsbMountError::Command)?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(UsbMountError::CommandFailed(stderr.trim().to_string()));
    }

    let info: LsblkInfo = serde_json::from_slice(&output.stdout)?;

    let mut devices = Vec::new();
    for device in info.blockdevices {
        collect_usb_candidates(&device, false, &mut devices);
    }

    if devices.is_empty() {
        Err(UsbMountError::NoDevices)
    } else {
        Ok(devices)
    }
}

fn collect_usb_candidates(
    device: &LsblkDevice,
    inherited_candidate: bool,
    out: &mut Vec<UsbDevice>,
) {
    let self_candidate = device.rm.unwrap_or(0) != 0
        || device.hotplug.unwrap_or(0) != 0
        || device.tran.as_deref() == Some("usb");
    let is_candidate = self_candidate || inherited_candidate;
    let mount_point = device.mountpoint.clone();
    let path = device
        .path
        .as_ref()
        .map(|p| p.to_string())
        .or_else(|| Some(format!("/dev/{}", device.name)));

    match device.kind.as_str() {
        "disk" => {
            if device.children.is_empty() {
                if is_candidate {
                    if let Some(dev_path) = path {
                        out.push(UsbDevice {
                            device_path: dev_path,
                            mount_point,
                        });
                    }
                }
            } else {
                for child in &device.children {
                    collect_usb_candidates(child, is_candidate, out);
                }
            }
        }
        "part" => {
            if is_candidate {
                if let Some(dev_path) = path {
                    out.push(UsbDevice {
                        device_path: dev_path,
                        mount_point,
                    });
                }
            }
        }
        _ => {}
    }
}

fn mount_device(device_path: &str) -> Result<PathBuf, UsbMountError> {
    let output = Command::new("udisksctl")
        .arg("mount")
        .arg("-b")
        .arg(device_path)
        .output()
        .map_err(UsbMountError::Command)?;

    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let message = if !stderr.trim().is_empty() {
            stderr.trim().to_string()
        } else {
            stdout.trim().to_string()
        };
        return Err(UsbMountError::MountFailed(message));
    }

    let stdout = String::from_utf8(output.stdout)?;
    parse_mount_point(&stdout).ok_or_else(|| UsbMountError::Parse(stdout))
}

fn parse_mount_point(output: &str) -> Option<PathBuf> {
    // Expect messages like "Mounted /dev/sdb1 at /media/user/LABEL."
    if let Some(pos) = output.find(" at ") {
        let after_at = &output[pos + 4..];
        let path_part = after_at
            .lines()
            .next()
            .unwrap_or("")
            .trim()
            .trim_end_matches('.');
        if !path_part.is_empty() {
            return Some(PathBuf::from(path_part));
        }
    }
    None
}

#[derive(Debug)]
struct UsbDevice {
    device_path: String,
    mount_point: Option<String>,
}

#[derive(Deserialize)]
struct LsblkInfo {
    #[serde(default)]
    blockdevices: Vec<LsblkDevice>,
}

#[derive(Deserialize)]
struct LsblkDevice {
    name: String,
    #[serde(default)]
    path: Option<String>,
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    mountpoint: Option<String>,
    #[serde(default)]
    rm: Option<u8>,
    #[serde(default)]
    hotplug: Option<u8>,
    #[serde(default)]
    tran: Option<String>,
    #[serde(default)]
    children: Vec<LsblkDevice>,
}

#[derive(Debug)]
enum UsbMountError {
    NoDevices,
    Command(io::Error),
    CommandFailed(String),
    MountFailed(String),
    Utf8(std::string::FromUtf8Error),
    Parse(String),
    Json(serde_json::Error),
}

impl fmt::Display for UsbMountError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UsbMountError::NoDevices => write!(f, "no se encontraron dispositivos USB disponibles"),
            UsbMountError::Command(err) => {
                if err.kind() == io::ErrorKind::NotFound {
                    write!(
                        f,
                        "no se encontró el comando requerido (instale 'lsblk' y 'udisksctl')"
                    )
                } else {
                    write!(f, "falló la ejecución del comando: {}", err)
                }
            }
            UsbMountError::CommandFailed(msg) => write!(f, "lsblk devolvió un error: {}", msg),
            UsbMountError::MountFailed(msg) => write!(f, "montaje fallido: {}", msg),
            UsbMountError::Utf8(err) => write!(f, "respuesta inválida: {}", err),
            UsbMountError::Parse(output) => write!(
                f,
                "no se pudo interpretar la ruta de montaje: {}",
                output.trim()
            ),
            UsbMountError::Json(err) => {
                write!(f, "no se pudo interpretar la salida de lsblk: {}", err)
            }
        }
    }
}

impl std::error::Error for UsbMountError {}

impl From<std::string::FromUtf8Error> for UsbMountError {
    fn from(value: std::string::FromUtf8Error) -> Self {
        UsbMountError::Utf8(value)
    }
}

impl From<serde_json::Error> for UsbMountError {
    fn from(value: serde_json::Error) -> Self {
        UsbMountError::Json(value)
    }
}

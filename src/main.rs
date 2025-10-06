mod db;

use slint::SharedString;
use std::cell::RefCell;
use std::rc::Rc;

slint::include_modules!();

#[derive(Clone)]
struct WorkerDisplay {
    id: i64,
    name: String,
    status: String,
    time: String,
    color: slint::Color,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let conn = db::init_db()?;
    let conn = Rc::new(RefCell::new(conn));

    let ui = MainWindow::new()?;
    let conn_clone = conn.clone();
    let ui_handle = ui.as_weak();

    // Load workers and update UI
    let workers = db::get_workers(&conn.borrow())?;
    let mut worker_displays = Vec::new();
    for worker in workers {
        let status = match db::get_current_status(&conn.borrow(), worker.id)? {
            Some(entry) => {
                WorkerDisplay {
                    id: worker.id,
                    name: worker.name,
                    status: "Clocked In".to_string(),
                    time: entry.clock_in.format("%H:%M").to_string(),
                    color: slint::Color::from_rgb_u8(0, 255, 0), // Green
                }
            }
            None => {
                // Check last clock out
                let last_out = get_last_clock_out(&conn.borrow(), worker.id)?;
                WorkerDisplay {
                    id: worker.id,
                    name: worker.name,
                    status: "Clocked Out".to_string(),
                    time: last_out.unwrap_or_else(|| "N/A".to_string()),
                    color: slint::Color::from_rgb_u8(255, 0, 0), // Red
                }
            }
        };
        worker_displays.push(status);
    }

    let worker_items: Vec<WorkerItem> = worker_displays
        .into_iter()
        .map(|w| WorkerItem {
            name: SharedString::from(w.name),
            status: SharedString::from(w.status),
            time: SharedString::from(w.time),
            color: w.color,
        })
        .collect();

    ui.set_workers(Rc::new(slint::VecModel::from(worker_items)).into());

    // Handle barcode input
    let conn_clone2 = conn.clone();
    ui.on_barcode_scanned(move |id_str| {
        if let Ok(id) = id_str.parse::<i64>() {
            let mut conn = conn_clone2.borrow_mut();
            match db::get_current_status(&conn, id) {
                Ok(Some(_)) => {
                    db::clock_out(&mut conn, id).unwrap();
                }
                Ok(None) => {
                    db::clock_in(&mut conn, id).unwrap();
                }
                Err(_) => {}
            }
            // Refresh UI
            if let Some(ui) = ui_handle.upgrade() {
                // Reload workers
                // ... similar code ...
            }
        }
    });

    ui.run()?;
    Ok(())
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
            .with_timezone(&chrono::Utc);
        Ok(Some(dt.format("%H:%M").to_string()))
    } else {
        Ok(None)
    }
}

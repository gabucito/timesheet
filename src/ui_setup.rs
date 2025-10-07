use std::cell::RefCell;
use std::rc::Rc;

use crate::worker_display::refresh_workers;
use chrono_tz::America::Santiago;

pub fn initialize_ui_and_data(
    ui: &crate::ui::MainWindow,
    conn: &Rc<RefCell<rusqlite::Connection>>,
    ui_handle: &slint::Weak<crate::ui::MainWindow>,
) -> Result<(), Box<dyn std::error::Error>> {
    let now = chrono::Utc::now();
    ui.set_selected_date(now.format("%Y-%m-%d").to_string().into());

    // Initialize current time display
    let santiago_time = now.with_timezone(&Santiago);
    ui.set_current_time_display(santiago_time.format("%H:%M:%S").to_string().into());

    // Load initial data using refresh function
    refresh_workers(conn, ui_handle);

    Ok(())
}
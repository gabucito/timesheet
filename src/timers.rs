use chrono_tz::America::Santiago;
use std::cell::RefCell;
use std::rc::Rc;

use crate::ui;

pub fn setup_timers(conn: Rc<RefCell<rusqlite::Connection>>, ui_handle: slint::Weak<crate::ui::MainWindow>) {
    // Set up timer to refresh ongoing hours every 10 seconds
    let conn_clone_worker_timer = conn.clone();
    let ui_handle_worker_timer = ui_handle.clone();
    let worker_timer = slint::Timer::default();
    worker_timer.start(
        slint::TimerMode::Repeated,
        std::time::Duration::from_secs(10),
        move || {
            crate::worker_display::refresh_workers(&conn_clone_worker_timer, &ui_handle_worker_timer);
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
}
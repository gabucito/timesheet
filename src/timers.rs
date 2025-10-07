use std::cell::RefCell;
use std::rc::Rc;

pub fn setup_timers(
    conn: Rc<RefCell<rusqlite::Connection>>,
    ui_handle: slint::Weak<crate::ui::MainWindow>,
) {
    // Set up timer to refresh ongoing hours every 10 seconds
    let conn_clone_worker_timer = conn.clone();
    let ui_handle_worker_timer = ui_handle.clone();
    let worker_timer = slint::Timer::default();
    worker_timer.start(
        slint::TimerMode::Repeated,
        std::time::Duration::from_secs(10),
        move || {
            crate::worker_display::refresh_workers(
                &conn_clone_worker_timer,
                &ui_handle_worker_timer,
            );
        },
    );
}

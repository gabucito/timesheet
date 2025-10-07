mod db;
mod types;
mod utils;
mod worker_display;
mod ui_setup;
mod event_handlers;
mod timers;
mod ui;

use std::cell::RefCell;
use std::rc::Rc;
use slint::ComponentHandle;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let conn = db::init_db()?;
    let conn = Rc::new(RefCell::new(conn));

    let ui = crate::ui::MainWindow::new()?;

    let ui_handle = ui.as_weak();
    ui_setup::initialize_ui_and_data(&ui, &conn, &ui_handle)?;

    event_handlers::setup_event_handlers(conn.clone(), &ui);

    timers::setup_timers(conn, ui_handle);

    ui.run()?;
    Ok(())
}

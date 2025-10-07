mod db;
mod event_handlers;
mod timers;
mod types;
mod ui;
mod ui_setup;
mod utils;
mod worker_display;

use slint::ComponentHandle;
use std::cell::RefCell;
use std::rc::Rc;

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

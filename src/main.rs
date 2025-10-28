use slint::ComponentHandle;
use std::cell::RefCell;
use std::rc::Rc;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let conn = timesheet::db::init_db()?;
    let conn = Rc::new(RefCell::new(conn));

    let ui = timesheet::ui::MainWindow::new()?;

    let ui_handle = ui.as_weak();
    timesheet::ui_setup::initialize_ui_and_data(&ui, &conn, &ui_handle)?;

    timesheet::event_handlers::setup_event_handlers(conn.clone(), &ui);

    timesheet::timers::setup_timers(conn, ui_handle);

    ui.run()?;
    Ok(())
}

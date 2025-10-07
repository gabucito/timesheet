
#[derive(Clone)]
pub struct TimesheetDisplay {
    pub checked_in_time: String,
    pub checked_out_time: String,
    pub current_total_hours: String,
    pub color: slint::Color,
    pub show_name: bool,
}

#[derive(Clone)]
pub struct DataWorker {
    pub worker: crate::db::Worker,
    pub times: Vec<TimesheetDisplay>,
}
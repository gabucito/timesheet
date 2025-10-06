use chrono::{DateTime, Utc};
use rusqlite::{Connection, Result};

pub struct Worker {
    pub id: i64,
    pub name: String,
    pub active: bool,
}

pub struct TimesheetEntry {
    pub id: i64,
    pub worker_id: i64,
    pub clock_in: DateTime<Utc>,
    pub clock_out: Option<DateTime<Utc>>,
}

pub fn init_db() -> Result<Connection> {
    let conn = Connection::open("timesheet.db")?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS workers (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            active BOOLEAN DEFAULT 1
        )",
        [],
    )?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS timesheets (
            id INTEGER PRIMARY KEY,
            worker_id INTEGER NOT NULL,
            clock_in TEXT NOT NULL,
            clock_out TEXT,
            FOREIGN KEY (worker_id) REFERENCES workers(id)
        )",
        [],
    )?;
    Ok(conn)
}

// Worker management
pub fn add_worker(conn: &Connection, name: &str) -> Result<i64> {
    conn.execute(
        "INSERT INTO workers (name) VALUES (?)",
        rusqlite::params![name],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn get_workers(conn: &Connection) -> Result<Vec<Worker>> {
    let mut stmt = conn.prepare("SELECT id, name, active FROM workers WHERE active = 1")?;
    let worker_iter = stmt.query_map([], |row| {
        Ok(Worker {
            id: row.get(0)?,
            name: row.get(1)?,
            active: row.get(2)?,
        })
    })?;
    worker_iter.collect()
}

pub fn update_worker(conn: &Connection, id: i64, name: &str) -> Result<()> {
    conn.execute(
        "UPDATE workers SET name = ? WHERE id = ?",
        rusqlite::params![name, id],
    )?;
    Ok(())
}

pub fn soft_delete_worker(conn: &Connection, id: i64) -> Result<()> {
    conn.execute(
        "UPDATE workers SET active = 0 WHERE id = ?",
        rusqlite::params![id],
    )?;
    Ok(())
}

// Timesheet functions
pub fn clock_in(conn: &Connection, worker_id: i64) -> Result<i64> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO timesheets (worker_id, clock_in) VALUES (?, ?)",
        rusqlite::params![worker_id, now],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn clock_out(conn: &Connection, worker_id: i64) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE timesheets SET clock_out = ? WHERE worker_id = ? AND clock_out IS NULL",
        rusqlite::params![now, worker_id],
    )?;
    Ok(())
}

pub fn get_current_status(conn: &Connection, worker_id: i64) -> Result<Option<TimesheetEntry>> {
    let mut stmt = conn.prepare(
        "SELECT id, worker_id, clock_in, clock_out FROM timesheets WHERE worker_id = ? AND clock_out IS NULL ORDER BY id DESC LIMIT 1"
    )?;
    let mut rows = stmt.query(rusqlite::params![worker_id])?;
    if let Some(row) = rows.next()? {
        Ok(Some(TimesheetEntry {
            id: row.get(0)?,
            worker_id: row.get(1)?,
            clock_in: DateTime::parse_from_rfc3339(&row.get::<_, String>(2)?)
                .expect("Invalid time")
                .with_timezone(&Utc),
            clock_out: None,
        }))
    } else {
        Ok(None)
    }
}

// Reporting functions
pub fn get_daily_hours(conn: &Connection, worker_id: i64, date: &str) -> Result<f64> {
    let mut stmt = conn.prepare(
        "SELECT clock_in, clock_out FROM timesheets WHERE worker_id = ? AND date(clock_in) = ?",
    )?;
    let mut rows = stmt.query(rusqlite::params![worker_id, date])?;
    let mut total_hours = 0.0;
    while let Some(row) = rows.next()? {
        let clock_in: String = row.get(0)?;
        let clock_out: Option<String> = row.get(1)?;
        if let Some(out) = clock_out {
            let in_time = DateTime::parse_from_rfc3339(&clock_in)
                .expect("Invalid time")
                .with_timezone(&Utc);
            let out_time = DateTime::parse_from_rfc3339(&out)
                .expect("Invalid time")
                .with_timezone(&Utc);
            total_hours += (out_time - in_time).num_seconds() as f64 / 3600.0;
        }
    }
    Ok(total_hours)
}

pub fn get_weekly_hours(
    conn: &Connection,
    worker_id: i64,
    start_date: &str,
    end_date: &str,
) -> Result<f64> {
    let mut stmt = conn.prepare(
        "SELECT clock_in, clock_out FROM timesheets WHERE worker_id = ? AND date(clock_in) BETWEEN ? AND ?"
    )?;
    let mut rows = stmt.query(rusqlite::params![worker_id, start_date, end_date])?;
    let mut total_hours = 0.0;
    while let Some(row) = rows.next()? {
        let clock_in: String = row.get(0)?;
        let clock_out: Option<String> = row.get(1)?;
        if let Some(out) = clock_out {
            let in_time = DateTime::parse_from_rfc3339(&clock_in)
                .expect("Invalid time")
                .with_timezone(&Utc);
            let out_time = DateTime::parse_from_rfc3339(&out)
                .expect("Invalid time")
                .with_timezone(&Utc);
            total_hours += (out_time - in_time).num_seconds() as f64 / 3600.0;
        }
    }
    Ok(total_hours)
}

pub fn get_monthly_hours(conn: &Connection, worker_id: i64, month: &str) -> Result<f64> {
    let mut stmt = conn.prepare(
        "SELECT clock_in, clock_out FROM timesheets WHERE worker_id = ? AND strftime('%Y-%m', clock_in) = ?"
    )?;
    let mut rows = stmt.query(rusqlite::params![worker_id, month])?;
    let mut total_hours = 0.0;
    while let Some(row) = rows.next()? {
        let clock_in: String = row.get(0)?;
        let clock_out: Option<String> = row.get(1)?;
        if let Some(out) = clock_out {
            let in_time = DateTime::parse_from_rfc3339(&clock_in)
                .expect("Invalid time")
                .with_timezone(&Utc);
            let out_time = DateTime::parse_from_rfc3339(&out)
                .expect("Invalid time")
                .with_timezone(&Utc);
            total_hours += (out_time - in_time).num_seconds() as f64 / 3600.0;
        }
    }
    Ok(total_hours)
}

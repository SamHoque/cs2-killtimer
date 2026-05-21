//! Tiny timestamped console logger.

use chrono::Local;

fn stamp() -> String {
    let dt = Local::now();
    dt.format("[%-m/%-d/%y %H:%M:%S]").to_string()
}

pub fn info(msg: &str) {
    println!("{} {}", stamp(), msg);
}

pub fn warn(msg: &str) {
    println!("{} [warn] {}", stamp(), msg);
}

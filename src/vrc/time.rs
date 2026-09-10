use std::time::SystemTime;
use chrono::{Datelike, DateTime, Timelike};

pub fn parse_time(time: &str) -> String {
    match chrono::DateTime::parse_from_rfc3339(time)
        .map(|time|format!("{:02}.{:02}.{} {:02}:{:02}", time.day(), time.month(), time.year(), time.hour(), time.minute()))
    {
        Ok(ok) => ok,
        Err(err) => time.to_string(),
    }
}

pub fn date(time:&str) -> String{
    time.rsplit("-").take(3).map(str::to_string).reduce(|item1, item2|format!("{}.{}", item1, item2)).unwrap_or(time.to_string())
}
use chrono::{DateTime, Utc};

pub fn now_utc() -> DateTime<Utc> {
    Utc::now()
}

pub fn format_iso8601(dt: DateTime<Utc>) -> String {
    dt.to_rfc3339()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_time_now() {
        let t1 = now_utc();
        std::thread::sleep(std::time::Duration::from_millis(1));
        let t2 = now_utc();
        assert!(t2 > t1);
    }
}

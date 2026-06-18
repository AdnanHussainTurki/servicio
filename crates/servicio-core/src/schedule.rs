use crate::error::CoreError;
use crate::worker::Schedule;
use chrono::{DateTime, Utc};
use std::str::FromStr;
use std::time::Duration;

/// Delay from `from` until the schedule's next fire.
pub fn next_delay(schedule: &Schedule, from: DateTime<Utc>) -> Result<Duration, CoreError> {
    match schedule {
        Schedule::IntervalSecs(n) => Ok(Duration::from_secs(*n)),
        Schedule::Cron(expr) => {
            let normalized = normalize_cron(expr);
            let sched = cron::Schedule::from_str(&normalized)
                .map_err(|e| CoreError::Spawn(format!("invalid cron '{expr}': {e}")))?;
            let next = sched
                .after(&from)
                .next()
                .ok_or_else(|| CoreError::Spawn(format!("cron '{expr}' has no next time")))?;
            let secs = (next - from).num_seconds().max(0) as u64;
            Ok(Duration::from_secs(secs))
        }
    }
}

/// Turn a 5-field cron expr into the 6-field (seconds-leading) form the crate wants.
fn normalize_cron(expr: &str) -> String {
    let fields = expr.split_whitespace().count();
    if fields == 5 {
        format!("0 {expr}")
    } else {
        expr.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};

    #[test]
    fn interval_delay_is_constant() {
        let d = next_delay(&Schedule::IntervalSecs(30), Utc::now()).unwrap();
        assert_eq!(d, Duration::from_secs(30));
    }

    #[test]
    fn cron_daily_3am_next_delay_is_positive_and_bounded() {
        let from = Utc.with_ymd_and_hms(2026, 1, 1, 0, 30, 0).unwrap();
        let d = next_delay(&Schedule::Cron("0 3 * * *".into()), from).unwrap();
        assert_eq!(d, Duration::from_secs((2 * 60 + 30) * 60));
    }

    #[test]
    fn invalid_cron_is_error() {
        assert!(next_delay(&Schedule::Cron("not a cron".into()), Utc::now()).is_err());
    }
}

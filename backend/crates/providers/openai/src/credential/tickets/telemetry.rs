use super::{ATTEMPT_WINDOW, Attempt};
use serde_json::{Value, json};
use std::collections::BTreeMap;

pub(super) fn summary(attempts: &[Attempt], now: i64) -> Value {
    let recent: Vec<_> = attempts
        .iter()
        .filter(|a| a.at >= now - ATTEMPT_WINDOW && a.at <= now)
        .collect();
    let successes = recent.iter().filter(|a| a.success).count();
    let mut by_ip = BTreeMap::<Option<&str>, Vec<&Attempt>>::new();
    for attempt in &recent {
        by_ip
            .entry(attempt.ip.as_deref().filter(|ip| !ip.is_empty()))
            .or_default()
            .push(attempt);
    }
    let unique_ips = by_ip.keys().filter(|ip| ip.is_some()).count();
    let unknown = by_ip.get(&None).map_or(0, Vec::len);
    let mut ips: Vec<_> = by_ip
        .into_iter()
        .map(|(ip, attempts)| {
            let successes = attempts.iter().filter(|a| a.success).count();
            json!({"ip":ip,"attempts":attempts.len(),"successes":successes,
            "successRate":100.0 * successes as f64 / attempts.len() as f64,
            "lastAttempt":attempts.iter().max_by_key(|a| a.at)})
        })
        .collect();
    ips.sort_by_key(|row| std::cmp::Reverse(row["lastAttempt"]["at"].as_i64().unwrap_or(0)));
    json!({"windowSeconds":ATTEMPT_WINDOW,"maxAttempts":1000,"attempts":recent.len(),"successes":successes,
        "successRate":if recent.is_empty() {None} else {Some(100.0 * successes as f64 / recent.len() as f64)},
        "uniqueIps":unique_ips,"unknownIpAttempts":unknown,"ips":ips,
        "lastAttempt":attempts.iter().filter(|a| a.at <= now).max_by_key(|a| a.at),"recentAttempts":recent})
}

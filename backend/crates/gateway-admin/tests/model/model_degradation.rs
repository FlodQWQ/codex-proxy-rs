use chrono::{Duration, Utc};
use gateway_admin::model::model_degradation::{ModelObservation, account_model_degradations};

fn observation(minutes: i64, response: &str) -> ModelObservation {
    ModelObservation {
        account_id: "account-a".to_owned(),
        request_id: format!("request-{minutes}"),
        routing_scope: "groups".to_owned(),
        group_ids: vec!["group-a".to_owned()],
        sent_model: "gpt-6-astra".to_owned(),
        response_model: response.to_owned(),
        observed_at: "2026-09-19T00:00:00Z"
            .parse::<chrono::DateTime<Utc>>()
            .unwrap()
            + Duration::minutes(minutes),
        succeeded: true,
    }
}

#[test]
fn degradation_recovers_relapses_and_expires_without_extending_on_recovery() {
    let bad = observation(0, "gpt-5.6-luna");
    let good = observation(10, "gpt-6");
    let now = bad.observed_at + Duration::minutes(15);
    let result = account_model_degradations(vec![good.clone(), bad.clone()], now);
    let marker = &result["account-a"][0];
    assert_eq!(marker.recovered_at, Some(good.observed_at));
    assert_eq!(marker.expires_at, bad.observed_at + Duration::hours(3));
    let relapse = observation(12, "gpt-5.6-sol");
    let result = account_model_degradations(vec![good.clone(), relapse.clone(), bad.clone()], now);
    assert!(result["account-a"][0].recovered_at.is_none());
    assert_eq!(result["account-a"][0].detected_at, relapse.observed_at);
    assert!(
        account_model_degradations(
            vec![bad.clone(), good],
            bad.observed_at + Duration::hours(3)
        )
        .is_empty()
    );
}

#[test]
fn degradation_recovery_is_scoped_and_requires_successful_comparable_response() {
    let bad = observation(0, "gpt-5.6-luna");
    let now = bad.observed_at + Duration::minutes(15);
    for kind in 0..7 {
        let mut good = observation(10, "gpt-6-astra");
        match kind {
            0 => good.account_id = "account-b".to_owned(),
            1 => good.group_ids = vec!["group-b".to_owned()],
            2 => good.routing_scope = "all".to_owned(),
            3 => good.sent_model = "gpt-5.6-luna".to_owned(),
            4 => good.succeeded = false,
            5 => good.response_model = "unknown".to_owned(),
            _ => good.observed_at = bad.observed_at,
        }
        let result = account_model_degradations(vec![bad.clone(), good], now);
        assert!(result["account-a"][0].recovered_at.is_none(), "case {kind}");
    }
    assert!(account_model_degradations(vec![observation(10, "unknown")], now).is_empty());
    assert!(account_model_degradations(vec![observation(10, "gpt-5.5")], now).is_empty());
    assert!(account_model_degradations(vec![observation(10, "gpt-6-astra")], now).is_empty());
}

#[test]
fn degradation_normalizes_aliases_dates_and_group_order() {
    let mut bad = observation(0, "openai/gpt-6-luna-latest");
    bad.sent_model = "gpt-6-2026-09-01-xhigh".to_owned();
    bad.group_ids = vec!["b".to_owned(), "a".to_owned()];
    let mut good = observation(10, "gpt-6-astra-20260901");
    good.group_ids = vec!["a".to_owned(), "b".to_owned()];
    let now = good.observed_at;
    let result = account_model_degradations(vec![good, bad], now);
    assert!(result["account-a"][0].recovered_at.is_some());
    let mut tradeoff = observation(0, "gpt-5.5-mini");
    tradeoff.sent_model = "gpt-5.4".to_owned();
    assert!(account_model_degradations(vec![tradeoff], now).is_empty());
}

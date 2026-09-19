use super::*;
use gateway_admin::model::model_degradation::account_model_degradations;

#[tokio::test]
async fn model_degradation_query_filters_and_projects_persisted_usage() {
    let Some(database) = TestDatabase::create("model_degradation").await else {
        return;
    };
    PgProviderAccountRepository::new(database.pool.clone())
        .insert_provider_account(account("acct_model_audit", "audit-user"))
        .await
        .unwrap();
    let now = chrono::DateTime::from_timestamp_micros(Utc::now().timestamp_micros()).unwrap();
    for (id, minutes, response) in [
        ("expired", -181, "gpt-5.6-luna"),
        ("bad-old", -30, "gpt-5.6-sol"),
        ("bad-new", -20, "gpt-5.6-sol"),
        ("recovered", -10, "gpt-6-astra"),
        ("compact", -5, "gpt-5.6-luna"),
        ("prewarm", -4, "gpt-5.6-luna"),
        ("empty", -3, "gpt-5.6-luna"),
        ("missing", -2, "gpt-5.6-luna"),
    ] {
        seed_model_request(
            &database.pool,
            ModelRequestSeed {
                request_id: id,
                account_id: "acct_model_audit",
                provider_kind: "openai",
                model: "gpt-6-astra",
                total_tokens: 10,
                cost_amount: "0",
                started_at: now + TimeDelta::minutes(minutes),
            },
        )
        .await
        .unwrap();
        sqlx::query("update model_requests set upstream_response_model = $2 where id = $1")
            .bind(id)
            .bind(response)
            .execute(&database.pool)
            .await
            .unwrap();
    }
    sqlx::query("update model_requests set compact = true where id = 'compact'")
        .execute(&database.pool)
        .await
        .unwrap();
    sqlx::query("update model_requests set request_kind = 'prewarm' where id = 'prewarm'")
        .execute(&database.pool)
        .await
        .unwrap();
    sqlx::query("update model_requests set total_tokens = 0, input_tokens = 0 where id = 'empty'")
        .execute(&database.pool)
        .await
        .unwrap();
    sqlx::query("update model_requests set upstream_response_model = null where id = 'missing'")
        .execute(&database.pool)
        .await
        .unwrap();
    let store = admin_account_store(&database.pool);
    let ids = vec!["acct_model_audit".to_owned()];
    let observations = store.load_model_observations(&ids, now).await.unwrap();
    assert_eq!(observations.len(), 2);
    let markers = account_model_degradations(observations, now);
    let marker = &markers["acct_model_audit"][0];
    assert_eq!(marker.request_id, "bad-new");
    assert!(marker.recovered_at.is_some());
    assert_eq!(marker.expires_at, now + TimeDelta::minutes(160));
    assert!(
        store
            .load_model_observations(&["other-account".to_owned()], now)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        store
            .load_model_observations(&ids, now + TimeDelta::hours(3))
            .await
            .unwrap()
            .is_empty()
    );
    database.close().await;
}

use gateway_admin::ports::model_fingerprint::{FingerprintAnalysis, FingerprintOutput};

#[test]
fn bridge_dtos_use_modeltrace_snake_case_fields() {
    let output = FingerprintOutput {
        text: "1, 2, 3".to_owned(),
        expected_count: 3,
    };
    assert_eq!(
        serde_json::to_value(output).unwrap(),
        serde_json::json!({ "text": "1, 2, 3", "expected_count": 3 })
    );

    let analysis: FingerprintAnalysis = serde_json::from_value(serde_json::json!({
        "prediction": "gpt-6-astra",
        "probability": 0.8,
        "used_outputs": 3,
        "diagnostics": [{ "accepted": true, "parsed_numbers": 3 }]
    }))
    .unwrap();
    assert_eq!(analysis.used_outputs, 3);
    assert_eq!(analysis.diagnostics[0].parsed_numbers, 3);
}

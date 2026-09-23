//! ModelTrace 指纹分析能力，由 Host 运行本地上游实现。

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FingerprintModel {
    pub id: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct FingerprintChallenge {
    pub id: String,
    pub prompt: String,
    pub expected_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct FingerprintOutput {
    pub text: String,
    pub expected_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct FingerprintAnalysis {
    pub prediction: Option<String>,
    pub probability: Option<f64>,
    pub used_outputs: usize,
    pub diagnostics: Vec<FingerprintDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct FingerprintDiagnostic {
    pub accepted: bool,
    pub parsed_numbers: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("ModelTrace 本地指纹库不可用")]
pub struct FingerprintUnavailable;

#[async_trait]
pub trait ModelFingerprintAnalyzer: Send + Sync {
    async fn models(&self) -> Result<Vec<FingerprintModel>, FingerprintUnavailable>;

    async fn challenges(&self) -> Result<Vec<FingerprintChallenge>, FingerprintUnavailable>;

    async fn analyze(
        &self,
        outputs: &[FingerprintOutput],
    ) -> Result<FingerprintAnalysis, FingerprintUnavailable>;
}

pub struct UnavailableModelFingerprintAnalyzer;

#[async_trait]
impl ModelFingerprintAnalyzer for UnavailableModelFingerprintAnalyzer {
    async fn models(&self) -> Result<Vec<FingerprintModel>, FingerprintUnavailable> {
        Err(FingerprintUnavailable)
    }

    async fn challenges(&self) -> Result<Vec<FingerprintChallenge>, FingerprintUnavailable> {
        Err(FingerprintUnavailable)
    }

    async fn analyze(
        &self,
        _outputs: &[FingerprintOutput],
    ) -> Result<FingerprintAnalysis, FingerprintUnavailable> {
        Err(FingerprintUnavailable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}

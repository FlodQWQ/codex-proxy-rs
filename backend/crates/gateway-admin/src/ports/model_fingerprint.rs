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

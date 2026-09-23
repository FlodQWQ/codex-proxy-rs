//! 管理端主动指纹检测结果；原始回答不持久化。

use chrono::{DateTime, Duration, Utc};

use crate::ports::model_fingerprint::FingerprintAnalysis;

pub const MINIMUM_FINGERPRINT_CONFIDENCE: f64 = 0.70;
pub const FINGERPRINT_VALIDITY: Duration = Duration::hours(3);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FingerprintTestStatus {
    Degraded,
    Passed,
    Inconclusive,
}

impl FingerprintTestStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Degraded => "degraded",
            Self::Passed => "passed",
            Self::Inconclusive => "inconclusive",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AccountFingerprintTestResult {
    pub account_id: String,
    pub sent_model: String,
    pub response_model: Option<String>,
    pub confidence: Option<f64>,
    pub status: FingerprintTestStatus,
    pub attempted: usize,
    pub used_outputs: usize,
    pub expires_at: Option<DateTime<Utc>>,
}

impl AccountFingerprintTestResult {
    #[must_use]
    pub fn inconclusive(
        account_id: String,
        sent_model: String,
        attempted: usize,
        analysis: FingerprintAnalysis,
    ) -> Self {
        Self {
            account_id,
            sent_model,
            response_model: analysis.prediction,
            confidence: analysis.probability,
            status: FingerprintTestStatus::Inconclusive,
            attempted,
            used_outputs: analysis.used_outputs,
            expires_at: None,
        }
    }

    #[must_use]
    pub fn conclusive(
        account_id: String,
        sent_model: String,
        response_model: String,
        confidence: f64,
        degraded: bool,
        attempted: usize,
        used_outputs: usize,
        observed_at: DateTime<Utc>,
    ) -> Self {
        Self {
            account_id,
            sent_model,
            response_model: Some(response_model),
            confidence: Some(confidence),
            status: if degraded {
                FingerprintTestStatus::Degraded
            } else {
                FingerprintTestStatus::Passed
            },
            attempted,
            used_outputs,
            expires_at: Some(observed_at + FINGERPRINT_VALIDITY),
        }
    }
}

//! 通过 VPS 本地 ModelTrace checkout 执行固定题库与原版指纹算法。

use std::{
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
};

use async_trait::async_trait;
use gateway_admin::ports::model_fingerprint::{
    FingerprintAnalysis, FingerprintChallenge, FingerprintModel, FingerprintOutput,
    FingerprintUnavailable, ModelFingerprintAnalyzer,
};
use serde::Deserialize;

const MODELTRACE_BRIDGE: &str = r#"
import json
import math
import sys
from pathlib import Path

from challenge_suite import fingerprint_suite
from fingerprint import analyze_global_outputs, load_bank, parse_numbers

root = Path.cwd()
bank = load_bank(root / "data" / "unified_bank.json")
action = sys.argv[1]

if action == "models":
    print(json.dumps({"models": [
        {"id": item["id"], "label": item.get("display_name") or item["id"]}
        for item in bank["models"] if item.get("family") == "gpt"
    ]}))
elif action == "challenges":
    challenges = []
    for item in fingerprint_suite()[:6]:
        prompt = "\n\n".join(part for part in (
            item.get("system", ""), item.get("user_prefix", ""), item["prompt"]
        ) if part)
        challenges.append({
            "id": item["challenge_id"],
            "prompt": prompt,
            "expected_count": item["expected_count"],
        })
    print(json.dumps({"challenges": challenges}))
elif action == "analyze":
    payload = json.load(sys.stdin)
    outputs = payload["outputs"]
    diagnostics = []
    for index, item in enumerate(outputs):
        expected = int(item.get("expected_count") or 0)
        parsed = len(parse_numbers(str(item.get("text", ""))))
        minimum = max(80, math.ceil(expected * 0.55)) if expected else 80
        diagnostics.append({
            "accepted": parsed >= minimum,
            "parsed_numbers": parsed,
        })
    try:
        result = analyze_global_outputs(outputs, bank)
        print(json.dumps({
            "prediction": result["prediction"],
            "probability": result["probability"],
            "used_outputs": result["used_outputs"],
            "diagnostics": diagnostics,
        }))
    except ValueError:
        print(json.dumps({
            "prediction": None,
            "probability": None,
            "used_outputs": 0,
            "diagnostics": diagnostics,
        }))
else:
    raise SystemExit(2)
"#;

#[derive(Deserialize)]
struct ModelsResponse {
    models: Vec<FingerprintModel>,
}

#[derive(Deserialize)]
struct ChallengesResponse {
    challenges: Vec<FingerprintChallenge>,
}

pub struct LocalModelTrace {
    home: PathBuf,
}

impl LocalModelTrace {
    #[must_use]
    pub fn new(home: PathBuf) -> Self {
        Self { home }
    }

    async fn run(
        &self,
        action: &'static str,
        input: Option<Vec<u8>>,
    ) -> Result<Vec<u8>, FingerprintUnavailable> {
        let home = self.home.clone();
        tokio::task::spawn_blocking(move || {
            let python = if cfg!(windows) {
                home.join(".venv").join("Scripts").join("python.exe")
            } else {
                home.join(".venv").join("bin").join("python")
            };
            let mut child = Command::new(python)
                .arg("-c")
                .arg(MODELTRACE_BRIDGE)
                .arg(action)
                .current_dir(&home)
                .env("PYTHONDONTWRITEBYTECODE", "1")
                .env("PYTHONUNBUFFERED", "1")
                .stdin(if input.is_some() {
                    Stdio::piped()
                } else {
                    Stdio::null()
                })
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
                .map_err(|_| FingerprintUnavailable)?;
            if let Some(input) = input {
                child
                    .stdin
                    .take()
                    .ok_or(FingerprintUnavailable)?
                    .write_all(&input)
                    .map_err(|_| FingerprintUnavailable)?;
            }
            let output = child
                .wait_with_output()
                .map_err(|_| FingerprintUnavailable)?;
            if !output.status.success() {
                return Err(FingerprintUnavailable);
            }
            Ok(output.stdout)
        })
        .await
        .map_err(|_| FingerprintUnavailable)?
    }

    async fn decode<T: for<'de> Deserialize<'de>>(
        &self,
        action: &'static str,
        input: Option<Vec<u8>>,
    ) -> Result<T, FingerprintUnavailable> {
        let output = self.run(action, input).await?;
        serde_json::from_slice(&output).map_err(|_| FingerprintUnavailable)
    }
}

#[async_trait]
impl ModelFingerprintAnalyzer for LocalModelTrace {
    async fn models(&self) -> Result<Vec<FingerprintModel>, FingerprintUnavailable> {
        Ok(self.decode::<ModelsResponse>("models", None).await?.models)
    }

    async fn challenges(&self) -> Result<Vec<FingerprintChallenge>, FingerprintUnavailable> {
        Ok(self
            .decode::<ChallengesResponse>("challenges", None)
            .await?
            .challenges)
    }

    async fn analyze(
        &self,
        outputs: &[FingerprintOutput],
    ) -> Result<FingerprintAnalysis, FingerprintUnavailable> {
        let input = serde_json::to_vec(&serde_json::json!({ "outputs": outputs }))
            .map_err(|_| FingerprintUnavailable)?;
        self.decode("analyze", Some(input)).await
    }
}

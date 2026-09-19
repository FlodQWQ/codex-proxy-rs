use serde::Deserialize;
use serde_json::Value;
use std::{
    path::{Path, PathBuf},
    process::Stdio,
};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

pub(super) fn executable() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("CPR_CODEX_TICKET_PROBE") {
        return Some(path.into());
    }
    let name = if cfg!(windows) {
        "codex-ticket-probe.exe"
    } else {
        "codex-ticket-probe"
    };
    std::env::current_exe()
        .ok()?
        .parent()
        .map(|p| p.join(name))
        .filter(|p| p.is_file())
}

#[derive(Deserialize)]
pub(super) struct ProbeResult {
    pub ticket: String,
    pub status: u16,
    pub ip: String,
    pub result: String,
}

pub(super) async fn probe(path: &Path, input: Value) -> Result<ProbeResult, &'static str> {
    let bytes = serde_json::to_vec(&input).map_err(|_| "input_error")?;
    let mut child = tokio::process::Command::new(path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|_| "helper_error")?;
    let mut stdin = child.stdin.take().ok_or("helper_error")?;
    stdin.write_all(&bytes).await.map_err(|_| "helper_error")?;
    drop(stdin);
    let mut output = Vec::new();
    child
        .stdout
        .take()
        .ok_or("helper_error")?
        .take(65537)
        .read_to_end(&mut output)
        .await
        .map_err(|_| "helper_error")?;
    if output.len() > 65536 || !child.wait().await.map_err(|_| "helper_error")?.success() {
        return Err("helper_error");
    }
    serde_json::from_slice(&output).map_err(|_| "helper_error")
}

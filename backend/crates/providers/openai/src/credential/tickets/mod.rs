//! 按账号与模型获取门票；独立打票出口不改变业务代理，持久文件不对外暴露凭据。
use super::CodexCredentialRepository;
use crate::transport::{headers::build_codex_model_headers, profile::CodexWireProfileState};
use chrono::Utc;
use gateway_admin::ports::provider::{ProviderAdminError, ProviderAdminErrorKind};
use gateway_core::account::{ProviderAccount, ProviderAccountId};
use reqwest::{Client, Proxy, redirect::Policy};
use secrecy::ExposeSecret;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::PathBuf, sync::Mutex, time::Duration};

const MODELS: [&str; 2] = ["gpt-6-astra", "gpt-5.6-sol"];
const TTL: i64 = 3600;

#[derive(Default, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Settings {
    enabled: bool,
    proxy_url: String,
    account_ids: Vec<String>,
    revision: u64,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Attempt {
    at: i64,
    status: u16,
    length: usize,
    success: bool,
    result: String,
}

#[derive(Default, Clone, Deserialize, Serialize)]
struct Record {
    ticket: String,
    expires: i64,
    credential_revision: u64,
    attempts: Vec<Attempt>,
}

#[derive(Default, Clone, Deserialize, Serialize)]
struct State {
    settings: Settings,
    records: BTreeMap<String, Record>,
}

pub(crate) struct CodexTicketService {
    repository: CodexCredentialRepository,
    profile: CodexWireProfileState,
    path: PathBuf,
    state: Mutex<State>,
    writes: tokio::sync::Mutex<()>,
}

fn error(kind: ProviderAdminErrorKind) -> ProviderAdminError {
    ProviderAdminError::new(kind)
}
fn key(account: &str, model: &str) -> String {
    format!("{account}/{model}")
}
fn valid(record: &Record, now: i64, revision: u64) -> bool {
    record.expires > now
        && record.ticket.len() == 292
        && record.ticket.starts_with("gAAAAA")
        && record.credential_revision == revision
}

impl CodexTicketService {
    pub(crate) async fn new(
        repository: CodexCredentialRepository,
        profile: CodexWireProfileState,
        path: PathBuf,
    ) -> Result<Self, std::io::Error> {
        let state = match tokio::fs::read(&path).await {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map_err(|_| std::io::Error::other("invalid ticket state"))?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => State::default(),
            Err(e) => return Err(e),
        };
        Ok(Self {
            repository,
            profile,
            path,
            state: Mutex::new(state),
            writes: tokio::sync::Mutex::new(()),
        })
    }

    fn selected(settings: &Settings, account: &ProviderAccount, model: &str) -> bool {
        settings.enabled
            && account.authentication_kind() == "oauth"
            && MODELS.contains(&model)
            && settings
                .account_ids
                .iter()
                .any(|id| id == account.id().as_str())
    }

    pub(crate) fn blocks(&self, account: &ProviderAccount, model: &str) -> bool {
        let state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        Self::selected(&state.settings, account, model)
            && !state
                .records
                .get(&key(account.id().as_str(), model))
                .is_some_and(|record| {
                    valid(record, Utc::now().timestamp(), account.revision().get())
                })
    }

    pub(crate) fn ticket(&self, account: &ProviderAccount, model: &str) -> Option<String> {
        let state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if !Self::selected(&state.settings, account, model) {
            return None;
        }
        state
            .records
            .get(&key(account.id().as_str(), model))
            .filter(|record| valid(record, Utc::now().timestamp(), account.revision().get()))
            .map(|record| record.ticket.clone())
    }

    pub(crate) async fn view(&self) -> Result<Value, ProviderAdminError> {
        let state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        let mut accounts = Vec::new();
        for id in &state.settings.account_ids {
            let typed = ProviderAccountId::new(id.clone())
                .map_err(|_| error(ProviderAdminErrorKind::Invalid))?;
            let Some(account) = self
                .repository
                .store()
                .get_account(&typed)
                .await
                .map_err(|_| error(ProviderAdminErrorKind::Unavailable))?
            else {
                continue;
            };
            let now = Utc::now().timestamp();
            let models: Vec<Value> = MODELS.iter().map(|model| {
                let record = state.records.get(&key(id, model)).cloned().unwrap_or_default();
                let attempts: Vec<_> = record.attempts.iter().filter(|a| a.at >= now - TTL).collect();
                let success = attempts.iter().filter(|a| a.success).count();
                let ready = valid(&record, now, account.revision().get());
                json!({"model":model,"ready":ready,"remainingSeconds":if ready {record.expires-now} else {0},
                    "blocked":state.settings.enabled && account.enabled() && !ready,
                    "attempts":attempts.len(),"successes":success,
                    "lastAttempt":record.attempts.last(),"recentAttempts":attempts})
            }).collect();
            accounts.push(json!({"accountId":id,"name":account.name(),"enabled":account.enabled(),"models":models}));
        }
        let endpoint = url::Url::parse(&state.settings.proxy_url).ok().map(|u| {
            format!(
                "{}://{}:{}",
                u.scheme(),
                u.host_str().unwrap_or(""),
                u.port_or_known_default().unwrap_or(0)
            )
        });
        Ok(
            json!({"enabled":state.settings.enabled,"revision":state.settings.revision,"accountIds":state.settings.account_ids,
            "proxyConfigured":!state.settings.proxy_url.is_empty(),"proxyEndpoint":endpoint,
            "ttlSeconds":TTL,"refreshBeforeSeconds":600,"intervalSeconds":6,"failClosed":true,"accounts":accounts}),
        )
    }

    pub(crate) async fn update(&self, value: Value) -> Result<Value, ProviderAdminError> {
        let mut next: Settings =
            serde_json::from_value(value).map_err(|_| error(ProviderAdminErrorKind::Invalid))?;
        next.account_ids.sort();
        next.account_ids.dedup();
        if next.account_ids.len() > 200 {
            return Err(error(ProviderAdminErrorKind::Invalid));
        }
        for id in &next.account_ids {
            let id = ProviderAccountId::new(id.clone())
                .map_err(|_| error(ProviderAdminErrorKind::Invalid))?;
            let account = self
                .repository
                .store()
                .get_account(&id)
                .await
                .map_err(|_| error(ProviderAdminErrorKind::Unavailable))?
                .ok_or_else(|| error(ProviderAdminErrorKind::NotFound))?;
            if account.provider().as_str() != "openai" || account.authentication_kind() != "oauth" {
                return Err(error(ProviderAdminErrorKind::Invalid));
            }
        }
        let _write = self.writes.lock().await;
        let mut proposed = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        if next.revision != proposed.settings.revision {
            return Err(error(ProviderAdminErrorKind::Conflict));
        }
        next.proxy_url = next.proxy_url.trim().to_owned();
        if next.proxy_url.is_empty() {
            next.proxy_url.clone_from(&proposed.settings.proxy_url);
        }
        if !next.proxy_url.is_empty() {
            let u = url::Url::parse(&next.proxy_url)
                .map_err(|_| error(ProviderAdminErrorKind::Invalid))?;
            if !matches!(u.scheme(), "http" | "https" | "socks5" | "socks5h")
                || u.host_str().is_none()
                || u.query().is_some()
                || u.fragment().is_some()
                || !matches!(u.path(), "" | "/")
            {
                return Err(error(ProviderAdminErrorKind::Invalid));
            }
        } else if next.enabled {
            return Err(error(ProviderAdminErrorKind::Invalid));
        }
        next.revision += 1;
        proposed.settings = next;
        proposed.records.retain(|record_key, _| {
            proposed
                .settings
                .account_ids
                .iter()
                .any(|id| record_key.starts_with(&format!("{id}/")))
        });
        self.persist(&proposed).await?;
        *self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = proposed;
        drop(_write);
        self.view().await
    }

    async fn persist(&self, state: &State) -> Result<(), ProviderAdminError> {
        let bytes =
            serde_json::to_vec(state).map_err(|_| error(ProviderAdminErrorKind::Internal))?;
        let path = self.path.with_extension("next");
        // 只写运行目录中的私有文件，代理密码和票据不会进入 API 或日志。
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            let _file = std::fs::OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .mode(0o600)
                .open(&path)
                .map_err(|_| error(ProviderAdminErrorKind::Internal))?;
        }
        tokio::fs::write(&path, bytes)
            .await
            .map_err(|_| error(ProviderAdminErrorKind::Internal))?;
        tokio::fs::rename(path, &self.path)
            .await
            .map_err(|_| error(ProviderAdminErrorKind::Internal))
    }

    pub(crate) async fn refresh(&self) -> Result<(), ProviderAdminError> {
        let settings = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .settings
            .clone();
        if !settings.enabled || settings.proxy_url.is_empty() {
            return Ok(());
        }
        for id in &settings.account_ids {
            let typed = ProviderAccountId::new(id.clone())
                .map_err(|_| error(ProviderAdminErrorKind::Invalid))?;
            for model in MODELS {
                let current = self
                    .state
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .settings
                    .clone();
                if !current.enabled || current.revision != settings.revision {
                    return Ok(());
                }
                let Some(account) = self
                    .repository
                    .store()
                    .get_account(&typed)
                    .await
                    .map_err(|_| error(ProviderAdminErrorKind::Unavailable))?
                else {
                    continue;
                };
                if !account.enabled() || account.authentication_kind() != "oauth" {
                    continue;
                }
                let now = Utc::now().timestamp();
                let cached = self
                    .state
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .records
                    .get(&key(id, model))
                    .cloned()
                    .unwrap_or_default();
                if valid(&cached, now + 600, account.revision().get()) {
                    continue;
                }
                let (ticket, status, result) =
                    self.probe(&account, model, &settings.proxy_url).await;
                let success = status == 200 && ticket.len() == 292 && ticket.starts_with("gAAAAA");
                let attempt = Attempt {
                    at: now,
                    status,
                    length: ticket.len(),
                    success,
                    result,
                };
                let Some(fresh) = self
                    .repository
                    .store()
                    .get_account(&typed)
                    .await
                    .map_err(|_| error(ProviderAdminErrorKind::Unavailable))?
                else {
                    continue;
                };
                if !fresh.enabled() || fresh.revision() != account.revision() {
                    continue;
                }
                let _write = self.writes.lock().await;
                let mut proposed = self
                    .state
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .clone();
                if proposed.settings.revision != settings.revision {
                    continue;
                }
                let record = proposed.records.entry(key(id, model)).or_default();
                record.attempts.retain(|a| a.at >= now - TTL);
                if record.attempts.len() >= 1000 {
                    record.attempts.remove(0);
                }
                record.attempts.push(attempt);
                if success {
                    record.ticket = ticket;
                    record.expires = Utc::now().timestamp() + TTL;
                    record.credential_revision = account.revision().get();
                }
                self.persist(&proposed).await?;
                *self
                    .state
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) = proposed;
            }
        }
        Ok(())
    }

    async fn probe(
        &self,
        account: &ProviderAccount,
        model: &str,
        proxy: &str,
    ) -> (String, u16, String) {
        let operation = async {
            let runtime = self
                .repository
                .load_runtime_credential(account)
                .await
                .map_err(|_| "token_error")?;
            let authorization = runtime
                .authentication
                .authorization_header()
                .map_err(|_| "token_error")?;
            let mut profile = self.profile.snapshot();
            profile.client_kind = crate::transport::profile::selection::ClientKind::Cli;
            profile.originator = "codex_cli_rs".to_owned();
            if model.contains("astra")
                && semver::Version::parse(&profile.codex_version)
                    .is_ok_and(|v| v < semver::Version::new(0, 153, 4))
            {
                profile.codex_version = "0.153.4".to_owned();
            }
            let headers = build_codex_model_headers(
                &profile,
                authorization.expose_secret(),
                account.upstream_account_id(),
            )
            .map_err(|_| "headers_error")?;
            let builder = Client::builder()
                .no_proxy()
                .http1_only()
                .pool_max_idle_per_host(0)
                .redirect(Policy::none())
                .connect_timeout(Duration::from_secs(15))
                .timeout(Duration::from_secs(25))
                .proxy(Proxy::all(proxy).map_err(|_| "proxy_error")?);
            let client = crate::transport::tls::build_reqwest_client_with_custom_ca(builder)
                .map_err(|_| "proxy_error")?;
            let response = client.post("https://chatgpt.com/backend-api/codex/responses")
                .headers(headers).header("connection","close").header("accept","text/event-stream")
                .header("openai-beta","responses=experimental").header("session_id",uuid::Uuid::new_v4().to_string())
                .json(&json!({"model":model,"store":false,"stream":true,"instructions":"Reply with exactly: pong",
                    "input":[{"role":"user","content":[{"type":"input_text","text":"ping"}]}]}))
                .send().await.map_err(|_| "network_error")?;
            let status = response.status().as_u16();
            let state = response
                .headers()
                .get("x-codex-turn-state")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("")
                .trim()
                .to_owned();
            Ok::<_, &str>((state, status))
        };
        match tokio::time::timeout(Duration::from_secs(25), operation).await {
            Ok(Ok((ticket, status))) => {
                let result = if status == 200 && ticket.len() == 292 && ticket.starts_with("gAAAAA")
                {
                    "success"
                } else {
                    "miss"
                };
                (ticket, status, result.to_owned())
            }
            Ok(Err(reason)) => (String::new(), 0, reason.to_owned()),
            Err(_) => (String::new(), 0, "timeout".to_owned()),
        }
    }
}

//! 按账号与模型获取门票；独立打票出口不改变业务代理，持久文件不对外暴露凭据。
use super::CodexCredentialRepository;
use crate::transport::{headers::build_codex_model_headers, profile::CodexWireProfileState};
use chrono::Utc;
use futures::{StreamExt as _, TryStreamExt as _};
use gateway_admin::ports::provider::{ProviderAdminError, ProviderAdminErrorKind};
use gateway_core::account::{ProviderAccount, ProviderAccountId};
use reqwest::{Client, Proxy, redirect::Policy};
use secrecy::ExposeSecret;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::PathBuf, sync::Mutex, time::Duration};

const MODELS: [&str; 2] = ["gpt-6-astra", "gpt-5.6-sol"];
const TTL: i64 = 3600;
const RATE_LIMIT_COOLDOWN: i64 = 3600;
mod process;
mod telemetry;
mod trace;

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
    #[serde(default)]
    ip: Option<String>,
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
    // 账号额度由模型共享；独立保存，不能随票据或一小时统计窗口被清掉。
    #[serde(default)]
    rate_limited_until: BTreeMap<String, i64>,
}

pub(crate) struct CodexTicketService {
    helper: Option<PathBuf>,
    base_url: String,
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
fn valid(record: &Record, now: i64) -> bool {
    // Credential revisions also advance for refresh metadata/backoff CAS writes;
    // ticket reuse is governed by the ticket's own lifetime instead.
    record.expires > now
        && record.ticket.len() == 292
        && record.ticket.starts_with("gAAAAA")
}

impl CodexTicketService {
    pub(crate) async fn new(
        repository: CodexCredentialRepository,
        profile: CodexWireProfileState,
        path: PathBuf,
        base_url: String,
    ) -> Result<Self, std::io::Error> {
        let state = match tokio::fs::read(&path).await {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map_err(|_| std::io::Error::other("invalid ticket state"))?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => State::default(),
            Err(e) => return Err(e),
        };
        Ok(Self {
            helper: process::executable(),
            base_url,
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
                    valid(record, Utc::now().timestamp())
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
            .filter(|record| valid(record, Utc::now().timestamp()))
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
                let ready = valid(&record, now);
                let mut view = telemetry::summary(&record.attempts, now);
                let fields = view.as_object_mut().expect("ticket summary object");
                fields.extend(json!({"model":model,"ready":ready,"remainingSeconds":if ready {record.expires-now} else {0},
                    "expiresAt":if ready {Some(record.expires)} else {None},
                    "blocked":state.settings.enabled && account.enabled() && !ready}).as_object().unwrap().clone());
                view
            }).collect();
            accounts.push(json!({"accountId":id,"name":account.name(),"enabled":account.enabled(),"models":models,
                "rateLimitedUntil":state.rate_limited_until.get(id).filter(|until| **until > now)}));
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
            "transport":if self.helper.is_some() {"go-http1"} else {"native-http1"},
            "harvestIdentity":{"version":self.harvest_profile().codex_version,"userAgent":self.harvest_profile().user_agent()},
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
        // 账号之间并行，单账号模型串行，首次 429 后同轮其他模型也必须停止。
        let mut probes = Vec::new();
        for id in &settings.account_ids {
            let settings = &settings;
            probes.push(async move {
                for model in MODELS {
                    self.refresh_one(settings, id, model).await?;
                }
                Ok::<(), ProviderAdminError>(())
            });
        }
        futures::stream::iter(probes)
            .buffer_unordered(8)
            .try_collect::<Vec<_>>()
            .await?;
        Ok(())
    }

    async fn refresh_one(
        &self,
        settings: &Settings,
        id: &str,
        model: &str,
    ) -> Result<(), ProviderAdminError> {
        let typed = ProviderAccountId::new(id.to_owned())
            .map_err(|_| error(ProviderAdminErrorKind::Invalid))?;
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
            return Ok(());
        };
        if !account.enabled() || account.authentication_kind() != "oauth" {
            return Ok(());
        }
        let now = Utc::now().timestamp();
        if self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .rate_limited_until
            .get(id)
            .is_some_and(|until| *until > now)
        {
            return Ok(());
        }
        let cached = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .records
            .get(&key(id, model))
            .cloned()
            .unwrap_or_default();
        if valid(&cached, now + 600) {
            return Ok(());
        }
        let (ticket, status, result, ip) = self.probe(&account, model, &settings.proxy_url).await;
        let success = status == 200 && ticket.len() == 292 && ticket.starts_with("gAAAAA");
        let attempt = Attempt {
            ip,
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
            return Ok(());
        };
        if !fresh.enabled() || fresh.revision() != account.revision() {
            return Ok(());
        }
        let _write = self.writes.lock().await;
        let mut proposed = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        if proposed.settings.revision != settings.revision {
            return Ok(());
        }
        if status == 429 {
            proposed.rate_limited_until.insert(
                id.to_owned(),
                Utc::now().timestamp().saturating_add(RATE_LIMIT_COOLDOWN),
            );
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
        Ok(())
    }

    fn harvest_profile(&self) -> crate::transport::profile::CodexWireProfile {
        use crate::transport::profile::selection::{ClientKind, ClientPlatform};
        let mut profile = self.profile.snapshot();
        profile.client_kind = ClientKind::Cli;
        profile.originator = "codex_cli_rs".to_owned();
        // 独立读取官方 CLI 稳定版本，绝不沿用 Desktop 内嵌的 alpha 版本。
        profile.codex_version = self
            .profile
            .client_release(ClientKind::Cli, ClientPlatform::Linux, "x86_64")
            .map(|release| release.codex_version)
            .unwrap_or_else(|| "0.153.4".to_owned());
        profile.os_type = "Ubuntu".to_owned();
        profile.os_version = "22.4.0".to_owned();
        profile.arch = "x86_64".to_owned();
        profile.terminal = "xterm-256color".to_owned();
        profile.residency = None;
        profile
    }

    async fn probe(
        &self,
        account: &ProviderAccount,
        model: &str,
        proxy: &str,
    ) -> (String, u16, String, Option<String>) {
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
            let mut profile = self.harvest_profile();
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
            let endpoint = crate::transport::endpoint_url(
                &self.base_url,
                crate::transport::CODEX_RESPONSES_PATH,
            );
            let body = json!({"model":model,"store":false,"stream":true,"instructions":"Reply with exactly: pong",
                "input":[{"role":"user","content":[{"type":"input_text","text":"ping"}]}]});
            let session = uuid::Uuid::new_v4().to_string();
            if let Some(helper) = &self.helper {
                let mut fields: BTreeMap<String, String> = headers
                    .iter()
                    .map(|(name, value)| {
                        (name.to_string(), value.to_str().unwrap_or("").to_owned())
                    })
                    .collect();
                fields.insert("Accept".to_owned(), "text/event-stream".to_owned());
                fields.insert("Content-Type".to_owned(), "application/json".to_owned());
                fields.insert(
                    "OpenAI-Beta".to_owned(),
                    "responses=experimental".to_owned(),
                );
                fields.insert("session_id".to_owned(), session);
                let result = process::probe(
                    helper,
                    json!({"endpoint":endpoint,"proxyUrl":proxy,"headers":fields,"body":body}),
                )
                .await?;
                let failure = if result.status == 0 {
                    Some(match result.result.as_str() {
                        "proxy_error" => "proxy_error",
                        "input_error" => "input_error",
                        _ => "network_error",
                    })
                } else {
                    None
                };
                return Ok((
                    result.ticket,
                    result.status,
                    (!result.ip.is_empty()).then_some(result.ip),
                    failure,
                ));
            }
            let builder = Client::builder()
                .no_proxy()
                .http1_only()
                .pool_max_idle_per_host(1)
                .redirect(Policy::none())
                .connect_timeout(Duration::from_secs(15))
                .timeout(Duration::from_secs(25))
                .proxy(Proxy::all(proxy).map_err(|_| "proxy_error")?);
            let client = crate::transport::tls::build_reqwest_client_with_custom_ca(builder)
                .map_err(|_| "proxy_error")?;
            let observed = trace::observe(&client, &endpoint).await;
            let response = client
                .post(&endpoint)
                .headers(headers)
                .header("connection", "close")
                .header("accept", "text/event-stream")
                .header("openai-beta", "responses=experimental")
                .header("session_id", session)
                .json(&body)
                .send()
                .await
                .map_err(|_| "network_error")?;
            let ip = observed.and_then(|trace| trace.confirm(&response));
            let status = response.status().as_u16();
            let state = response
                .headers()
                .get("x-codex-turn-state")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("")
                .trim()
                .to_owned();
            Ok::<_, &str>((state, status, ip, None))
        };
        match tokio::time::timeout(Duration::from_secs(25), operation).await {
            Ok(Ok((ticket, status, ip, failure))) => {
                let result = failure.unwrap_or(
                    if status == 200 && ticket.len() == 292 && ticket.starts_with("gAAAAA") {
                        "success"
                    } else if status != 200 {
                        "http_error"
                    } else {
                        "invalid_ticket"
                    },
                );
                (ticket, status, result.to_owned(), ip)
            }
            Ok(Err(reason)) => (String::new(), 0, reason.to_owned(), None),
            Err(_) => (String::new(), 0, "timeout".to_owned(), None),
        }
    }
}

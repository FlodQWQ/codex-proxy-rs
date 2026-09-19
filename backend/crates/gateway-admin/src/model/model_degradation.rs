//! 根据留存使用记录计算三小时降智标记，不修改账号调度状态。

use std::collections::BTreeMap;

use chrono::{DateTime, Duration, Utc};

#[derive(Debug, Clone, PartialEq)]
pub struct ModelObservation {
    pub account_id: String,
    pub request_id: String,
    pub routing_scope: String,
    pub group_ids: Vec<String>,
    pub sent_model: String,
    pub response_model: String,
    pub observed_at: DateTime<Utc>,
    pub succeeded: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ModelDegradation {
    pub request_id: String,
    pub routing_scope: String,
    pub group_ids: Vec<String>,
    pub sent_model: String,
    pub response_model: String,
    pub detected_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub recovered_at: Option<DateTime<Utc>>,
}

fn normalized(model: &str) -> String {
    let mut model = model.trim().to_ascii_lowercase().replace('_', "-");
    if let Some(value) = model.strip_prefix("openai/") {
        model = value.to_owned();
    }
    loop {
        let before = model.len();
        for suffix in ["-latest", "-thinking", "-xhigh", "-high", "-medium", "-low"] {
            if model.ends_with(suffix) {
                model.truncate(model.len() - suffix.len());
            }
        }
        for length in [11, 9] {
            if let Some(start) = model.len().checked_sub(length)
                && let Some(tail) = model.get(start..)
                && tail.starts_with('-')
                && ((length == 9 && tail[1..].bytes().all(|c| c.is_ascii_digit()))
                    || (length == 11
                        && tail.bytes().enumerate().all(|(i, c)| {
                            if [0, 5, 8].contains(&i) {
                                c == b'-'
                            } else {
                                c.is_ascii_digit()
                            }
                        })))
            {
                model.truncate(start);
            }
        }
        if before == model.len() {
            break;
        }
    }
    match model.as_str() {
        "gpt-6" => "gpt-6-astra".to_owned(),
        "gpt-6-sol" => "gpt-5.6-sol".to_owned(),
        "gpt-6-luna" => "gpt-5.6-luna".to_owned(),
        _ => model,
    }
}

// 继承 sub2api 的保守运维分级，不以价格推断能力；跨系列与未知模型不可比较。
fn grade(model: &str) -> Option<(&'static str, u16, u8)> {
    match model {
        "gpt-6-astra" => return Some(("named", 1, 3)),
        "gpt-5.6-sol" => return Some(("named", 1, 2)),
        "gpt-5.6-luna" => return Some(("named", 1, 1)),
        _ => {}
    }
    let (base, tier) = if let Some(base) = model.strip_suffix("-mini") {
        (base, 2)
    } else if let Some(base) = model.strip_suffix("-nano") {
        (base, 1)
    } else {
        (model, 3)
    };
    let generation = match base {
        "gpt-4.1" => 41,
        "gpt-5" => 50,
        "gpt-5.1" => 51,
        "gpt-5.2" => 52,
        "gpt-5.3" => 53,
        "gpt-5.4" => 54,
        "gpt-5.5" => 55,
        _ => return None,
    };
    Some(("gpt", generation, tier))
}

/// 只保留最近三小时的降智证据；恢复不能延长到期时间。
#[must_use]
pub fn account_model_degradations(
    observations: Vec<ModelObservation>,
    now: DateTime<Utc>,
) -> BTreeMap<String, Vec<ModelDegradation>> {
    let mut scopes = BTreeMap::<_, Vec<ModelObservation>>::new();
    for mut observation in observations {
        if observation.observed_at <= now - Duration::hours(3) || observation.observed_at > now {
            continue;
        }
        observation.group_ids.sort();
        observation.group_ids.dedup();
        scopes
            .entry((
                observation.account_id.clone(),
                observation.routing_scope.clone(),
                observation.group_ids.clone(),
                normalized(&observation.sent_model),
            ))
            .or_default()
            .push(observation);
    }
    let mut result = BTreeMap::<String, Vec<ModelDegradation>>::new();
    for ((account_id, _, _, sent), observations) in scopes {
        let Some((family, generation, tier)) = grade(&sent) else {
            continue;
        };
        let downgrade = observations
            .iter()
            .filter(|item| {
                grade(&normalized(&item.response_model)).is_some_and(|(f, g, t)| {
                    f == family && g <= generation && t <= tier && (g < generation || t < tier)
                })
            })
            .max_by_key(|item| (item.observed_at, &item.request_id));
        let Some(downgrade) = downgrade else {
            continue;
        };
        let recovery = observations
            .iter()
            .filter(|item| {
                // 同一时间的并发请求不能作为“后续恢复”的证据。
                item.succeeded
                    && item.observed_at > downgrade.observed_at
                    && grade(&normalized(&item.response_model))
                        .is_some_and(|(f, g, t)| f == family && g >= generation && t >= tier)
            })
            .max_by_key(|item| item.observed_at);
        result
            .entry(account_id)
            .or_default()
            .push(ModelDegradation {
                request_id: downgrade.request_id.clone(),
                routing_scope: downgrade.routing_scope.clone(),
                group_ids: downgrade.group_ids.clone(),
                sent_model: downgrade.sent_model.clone(),
                response_model: downgrade.response_model.clone(),
                detected_at: downgrade.observed_at,
                expires_at: downgrade.observed_at + Duration::hours(3),
                recovered_at: recovery.map(|item| item.observed_at),
            });
    }
    result
}

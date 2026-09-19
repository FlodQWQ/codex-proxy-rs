use super::ChatOptions;
use bytes::Bytes;
use gateway_core::event::ProviderEvent;
use gateway_protocol::openai::sse::SseEventDecoder;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

fn usage(value: &Value) -> Value {
    if !value.is_object() {
        return Value::Null;
    }
    let mut result = json!({"prompt_tokens":value["input_tokens"],"completion_tokens":value["output_tokens"],"total_tokens":value["total_tokens"]});
    for (from, to) in [
        ("input_tokens_details", "prompt_tokens_details"),
        ("output_tokens_details", "completion_tokens_details"),
    ] {
        if let Some(details) = value.get(from) {
            result[to] = details.clone();
        }
    }
    result
}

fn finish_reason(response: &Value, has_tools: bool) -> &'static str {
    if response["status"] == "incomplete" {
        if response
            .pointer("/incomplete_details/reason")
            .and_then(Value::as_str)
            == Some("content_filter")
        {
            "content_filter"
        } else {
            "length"
        }
    } else if has_tools {
        "tool_calls"
    } else {
        "stop"
    }
}

fn sse(value: Value) -> Bytes {
    Bytes::from(format!("data: {value}\n\n"))
}

pub(in crate::openai) fn complete_response(
    mut response: Value,
    options: &ChatOptions,
    events: &[ProviderEvent],
) -> Value {
    // Codex 的终态可能是薄快照；完整消息已在 output_item.done 中交付。
    if response["output"].as_array().is_none_or(Vec::is_empty) {
        let mut items = BTreeMap::new();
        for event in events {
            if let Some(wire) = event
                .wire_event()
                .filter(|wire| wire.protocol() == "openai")
                && wire.event_type().or_else(|| wire.data()["type"].as_str())
                    == Some("response.output_item.done")
                && let Some(index) = wire.data()["output_index"].as_u64()
                && let Some(item) = wire.data().get("item").filter(|item| item.is_object())
            {
                items.insert(index, item.clone());
            }
        }
        if !items.is_empty() {
            response["output"] = Value::Array(items.into_values().collect());
        }
    }
    let mut content = String::new();
    let mut refusal = String::new();
    let mut reasoning = String::new();
    let mut tools = Vec::new();
    for item in response["output"].as_array().into_iter().flatten() {
        match item["type"].as_str() {
            Some("message") => for part in item["content"].as_array().into_iter().flatten() {
                match part["type"].as_str() {
                    Some("output_text") => content.push_str(part["text"].as_str().unwrap_or("")),
                    Some("refusal") => refusal.push_str(part["refusal"].as_str().unwrap_or("")),
                    _=>{},
                }
            },
            Some("function_call") => tools.push(json!({"id":item["call_id"],"type":"function","function":{"name":item["name"],"arguments":item["arguments"]}})),
            Some("reasoning") => for part in item["summary"].as_array().into_iter().flatten() { reasoning.push_str(part["text"].as_str().unwrap_or("")); },
            _=>{},
        }
    }
    let mut message = json!({"role":"assistant","content":if content.is_empty() {Value::Null} else {json!(content)}});
    if !refusal.is_empty() {
        message["refusal"] = json!(refusal);
    }
    if !reasoning.is_empty() {
        message["reasoning_content"] = json!(reasoning);
    }
    let has_tools = !tools.is_empty();
    if has_tools {
        message["tool_calls"] = json!(tools);
    }
    let mut output = json!({"id":response.get("id").filter(|v|v.is_string()).cloned().unwrap_or_else(||json!(format!("chatcmpl-{}",uuid::Uuid::now_v7()))),"object":"chat.completion","created":response["created_at"].as_i64().unwrap_or_else(||chrono::Utc::now().timestamp()),
        "model":response.get("model").filter(|v| v.is_string()).cloned().unwrap_or_else(||json!(options.model)),
        "choices":[{"index":0,"message":message,"finish_reason":finish_reason(&response,has_tools),"logprobs":null}]});
    if response["usage"].is_object() {
        output["usage"] = usage(&response["usage"]);
    }
    if let Some(tier) = response.get("service_tier") {
        output["service_tier"] = tier.clone();
    }
    output
}

struct Tool {
    index: usize,
    arguments_seen: bool,
}

pub(in crate::openai) struct ChatStream {
    decoder: SseEventDecoder,
    options: ChatOptions,
    id: String,
    model: String,
    created: i64,
    started: bool,
    terminal: bool,
    text_seen: BTreeSet<(u64, u64)>,
    refusal_seen: BTreeSet<(u64, u64)>,
    reasoning_seen: BTreeSet<(u64, u64)>,
    tools: BTreeMap<u64, Tool>,
    pub failed: bool,
}

impl ChatStream {
    pub(in crate::openai) fn new(options: ChatOptions) -> Self {
        Self {
            decoder: SseEventDecoder::default(),
            id: format!("chatcmpl-{}", uuid::Uuid::now_v7()),
            model: options.model.clone(),
            options,
            created: chrono::Utc::now().timestamp(),
            started: false,
            terminal: false,
            text_seen: BTreeSet::new(),
            refusal_seen: BTreeSet::new(),
            reasoning_seen: BTreeSet::new(),
            tools: BTreeMap::new(),
            failed: false,
        }
    }

    fn chunk(&self, delta: Value, reason: Option<&str>) -> Bytes {
        let mut chunk = json!({"id":self.id,"object":"chat.completion.chunk","created":self.created,"model":self.model,
            "choices":[{"index":0,"delta":delta,"finish_reason":reason,"logprobs":null}]});
        if self.options.include_usage {
            chunk["usage"] = Value::Null;
        }
        sse(chunk)
    }

    fn begin(&mut self, response: &Value, fallback_id: Option<&str>, out: &mut Vec<Bytes>) {
        if self.started {
            return;
        }
        if let Some(id) = response["id"].as_str().or(fallback_id) {
            self.id = id.to_owned();
        }
        if let Some(model) = response["model"].as_str() {
            self.model = model.to_owned();
        }
        if let Some(created) = response["created_at"].as_i64() {
            self.created = created;
        }
        self.started = true;
        out.push(self.chunk(json!({"role":"assistant","content":""}), None));
    }

    fn item(&mut self, index: u64, item: &Value, complete: bool, out: &mut Vec<Bytes>) {
        match item["type"].as_str() {
            Some("function_call") => {
                let tool_index = self.tools.len();
                if let std::collections::btree_map::Entry::Vacant(entry) = self.tools.entry(index) {
                    entry.insert(Tool {
                        index: tool_index,
                        arguments_seen: false,
                    });
                    out.push(self.chunk(json!({"tool_calls":[{"index":tool_index,"id":item["call_id"],"type":"function","function":{"name":item["name"],"arguments":""}}]}),None));
                }
                if complete
                    && let Some(tool) = self.tools.get_mut(&index)
                    && !tool.arguments_seen
                {
                    let tool_index = tool.index;
                    tool.arguments_seen = true;
                    out.push(self.chunk(json!({"tool_calls":[{"index":tool_index,"function":{"arguments":item["arguments"].as_str().unwrap_or("")}}]}),None));
                }
            }
            Some("message") if complete => {
                for (part_index, part) in
                    item["content"].as_array().into_iter().flatten().enumerate()
                {
                    let key = (index, part_index as u64);
                    if part["type"] == "output_text" && self.text_seen.insert(key) {
                        out.push(self.chunk(json!({"content":part["text"]}), None));
                    } else if part["type"] == "refusal" && self.refusal_seen.insert(key) {
                        out.push(self.chunk(json!({"refusal":part["refusal"]}), None));
                    }
                }
            }
            Some("reasoning") if complete => {
                for (part_index, part) in
                    item["summary"].as_array().into_iter().flatten().enumerate()
                {
                    if self.reasoning_seen.insert((index, part_index as u64)) {
                        out.push(self.chunk(json!({"reasoning_content":part["text"]}), None));
                    }
                }
            }
            _ => {}
        }
    }

    pub(in crate::openai) fn convert(
        &mut self,
        frames: Vec<Bytes>,
        fallback_id: Option<&str>,
    ) -> Vec<Bytes> {
        let mut out = Vec::new();
        let had_frames = !frames.is_empty();
        for frame in frames {
            let events = match self.decoder.push(&frame) {
                Ok(events) => events,
                Err(_) => {
                    self.failed = true;
                    return out;
                }
            };
            for event in events {
                if event.data == "[DONE]" {
                    continue;
                }
                let data: Value = match serde_json::from_str(&event.data) {
                    Ok(data) => data,
                    Err(_) => {
                        self.failed = true;
                        return out;
                    }
                };
                if self.terminal {
                    continue;
                }
                self.begin(&data["response"], fallback_id, &mut out);
                let index = data["output_index"].as_u64().unwrap_or(0);
                let part = data["content_index"].as_u64().unwrap_or(0);
                match data["type"]
                    .as_str()
                    .or(event.event.as_deref())
                    .unwrap_or("")
                {
                    "response.output_text.delta" => {
                        self.text_seen.insert((index, part));
                        out.push(self.chunk(json!({"content":data["delta"]}), None));
                    }
                    "response.refusal.delta" => {
                        self.refusal_seen.insert((index, part));
                        out.push(self.chunk(json!({"refusal":data["delta"]}), None));
                    }
                    "response.reasoning_summary_text.delta" => {
                        self.reasoning_seen
                            .insert((index, data["summary_index"].as_u64().unwrap_or(0)));
                        out.push(self.chunk(json!({"reasoning_content":data["delta"]}), None));
                    }
                    "response.output_item.added" => {
                        self.item(index, &data["item"], false, &mut out)
                    }
                    "response.output_item.done" => self.item(index, &data["item"], true, &mut out),
                    "response.function_call_arguments.delta" => {
                        if let Some(tool) = self.tools.get_mut(&index) {
                            let tool_index = tool.index;
                            tool.arguments_seen = true;
                            out.push(self.chunk(json!({"tool_calls":[{"index":tool_index,"function":{"arguments":data["delta"]}}]}),None));
                        } else {
                            self.failed = true;
                            return out;
                        }
                    }
                    "response.completed" | "response.incomplete" => {
                        let response = &data["response"];
                        if !response.is_object() {
                            self.failed = true;
                            return out;
                        }
                        for (index, item) in response["output"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .enumerate()
                        {
                            self.item(index as u64, item, true, &mut out);
                        }
                        out.push(self.chunk(
                            json!({}),
                            Some(finish_reason(response, !self.tools.is_empty())),
                        ));
                        if self.options.include_usage {
                            out.push(sse(json!({"id":self.id,"object":"chat.completion.chunk","created":self.created,"model":self.model,"choices":[],"usage":usage(&response["usage"])})));
                        }
                        self.terminal = true;
                    }
                    "response.failed" | "error" => {
                        let error=data.pointer("/response/error").or_else(||data.get("error")).cloned().unwrap_or_else(||json!({"type":"api_error","message":"Upstream response failed."}));
                        out.push(sse(json!({"error":error})));
                        self.terminal = true;
                    }
                    _ => {}
                }
            }
        }
        // 首个 wire 片段可能不是完整 SSE 帧，先交付角色，仍由共享流程负责取消与最终结算。
        if !self.started && had_frames {
            self.begin(&Value::Null, fallback_id, &mut out);
        }
        out
    }
}

use super::ChatError;
use serde_json::{Map, Value, json};

#[derive(Clone)]
pub(in crate::openai) struct ChatOptions {
    pub model: String,
    pub include_usage: bool,
}

fn text<'a>(value: &'a Value, field: &'static str) -> Result<&'a str, ChatError> {
    value
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or(ChatError(field, "Expected a non-empty string."))
}

pub(in crate::openai) fn convert_request(value: Value) -> Result<(Value, ChatOptions), ChatError> {
    let object = value
        .as_object()
        .ok_or(ChatError("body", "Expected a JSON object."))?;
    let model = text(&value["model"], "model")?.to_owned();
    let messages = value["messages"]
        .as_array()
        .filter(|v| !v.is_empty())
        .ok_or(ChatError(
            "messages",
            "Expected a non-empty messages array.",
        ))?;
    for (key, val) in object {
        if val.is_null() {
            continue;
        }
        match key.as_str() {
            "model"
            | "messages"
            | "stream"
            | "stream_options"
            | "max_tokens"
            | "max_completion_tokens"
            | "temperature"
            | "top_p"
            | "tools"
            | "tool_choice"
            | "parallel_tool_calls"
            | "reasoning_effort"
            | "response_format"
            | "service_tier"
            | "metadata"
            | "user"
            | "store"
            | "safety_identifier"
            | "prompt_cache_key"
            | "prompt_cache_retention"
            | "verbosity" => {}
            "n" if val.as_u64() == Some(1) => {}
            "frequency_penalty" | "presence_penalty" if val.as_f64() == Some(0.0) => {}
            "logprobs" if val == false => {}
            "logit_bias" if val.as_object().is_some_and(Map::is_empty) => {}
            _ => {
                return Err(ChatError(
                    "parameters",
                    "Unsupported Chat parameter. Only n=1, neutral penalties and text output are supported; stop, seed, logprobs, audio and legacy functions are not supported.",
                ));
            }
        }
    }
    let streaming = match object.get("stream").filter(|v| !v.is_null()) {
        None => false,
        Some(value) => value
            .as_bool()
            .ok_or(ChatError("stream", "Expected a boolean."))?,
    };
    let include_usage = if let Some(options) = object.get("stream_options").filter(|v| !v.is_null())
    {
        let options = options
            .as_object()
            .ok_or(ChatError("stream_options", "Expected an object."))?;
        if options.keys().any(|key| key != "include_usage") {
            return Err(ChatError(
                "stream_options",
                "Only include_usage is supported.",
            ));
        }
        options
            .get("include_usage")
            .map(|v| {
                v.as_bool().ok_or(ChatError(
                    "stream_options.include_usage",
                    "Expected a boolean.",
                ))
            })
            .transpose()?
            .unwrap_or(false)
    } else {
        false
    };
    let mut input = Vec::new();
    for message in messages {
        let role = text(&message["role"], "messages.role")?;
        if message
            .get("function_call")
            .is_some_and(|value| !value.is_null())
        {
            return Err(ChatError(
                "messages.function_call",
                "Use tool_calls instead of legacy function_call.",
            ));
        }
        if !matches!(role, "system" | "developer" | "user" | "assistant" | "tool") {
            return Err(ChatError("messages.role", "Unsupported message role."));
        }
        if role == "tool" {
            let call_id = text(&message["tool_call_id"], "messages.tool_call_id")?;
            let output = message["content"].as_str().ok_or(ChatError(
                "messages.content",
                "Tool output must be a string.",
            ))?;
            input.push(json!({"type":"function_call_output","call_id":call_id,"output":output}));
            continue;
        }
        if message.get("name").is_some_and(|v| !v.is_null()) {
            return Err(ChatError(
                "messages.name",
                "Named message participants cannot be represented losslessly.",
            ));
        }
        let content = message
            .get("content")
            .filter(|value| !value.is_null())
            .or_else(|| {
                if role == "assistant" {
                    message.get("refusal").filter(|value| !value.is_null())
                } else {
                    None
                }
            });
        if let Some(content) = content {
            let content = if content.is_string() {
                content.clone()
            } else {
                let parts = content.as_array().ok_or(ChatError(
                    "messages.content",
                    "Expected a string or content array.",
                ))?;
                let mut out = Vec::new();
                for part in parts {
                    match part["type"].as_str() {
                        Some("text") => out.push(json!({"type":"input_text","text":part["text"].as_str().ok_or(ChatError("messages.content.text", "Expected text."))?})),
                        Some("image_url") if role == "user" => {
                            let url = text(&part["image_url"]["url"], "messages.content.image_url.url")?;
                            let mut image = json!({"type":"input_image","image_url":url});
                            if let Some(detail) = part["image_url"].get("detail") { image["detail"] = detail.clone(); }
                            out.push(image);
                        },
                        _ => return Err(ChatError("messages.content.type", "Only text and user image_url parts are supported.")),
                    }
                }
                Value::Array(out)
            };
            input.push(json!({"role":role,"content":content}));
        } else if role != "assistant" {
            return Err(ChatError(
                "messages.content",
                "Message content is required.",
            ));
        }
        if let Some(calls) = message.get("tool_calls").filter(|v| !v.is_null()) {
            if role != "assistant" {
                return Err(ChatError(
                    "messages.tool_calls",
                    "Tool calls require the assistant role.",
                ));
            }
            for call in calls
                .as_array()
                .ok_or(ChatError("messages.tool_calls", "Expected an array."))?
            {
                if call["type"] != "function" {
                    return Err(ChatError(
                        "messages.tool_calls.type",
                        "Only function tools are supported.",
                    ));
                }
                input.push(json!({"type":"function_call","call_id":text(&call["id"],"messages.tool_calls.id")?,
                    "name":text(&call["function"]["name"],"messages.tool_calls.function.name")?,
                    "arguments":call["function"]["arguments"].as_str().ok_or(ChatError("messages.tool_calls.function.arguments", "Arguments must be a JSON string."))?}));
            }
        }
    }
    let mut output = json!({"model":model,"input":input,"stream":streaming,"store":false});
    for field in [
        "temperature",
        "top_p",
        "parallel_tool_calls",
        "service_tier",
        "metadata",
        "user",
        "store",
        "safety_identifier",
        "prompt_cache_key",
        "prompt_cache_retention",
    ] {
        if let Some(value) = object.get(field).filter(|v| !v.is_null()) {
            output[field] = value.clone();
        }
    }
    if let Some(tokens) = object
        .get("max_completion_tokens")
        .or_else(|| object.get("max_tokens"))
        .filter(|v| !v.is_null())
    {
        if tokens.as_u64().is_none_or(|n| n == 0) {
            return Err(ChatError(
                "max_completion_tokens",
                "Expected a positive integer.",
            ));
        }
        output["max_output_tokens"] = tokens.clone();
    }
    if let Some(effort) = object.get("reasoning_effort").filter(|v| !v.is_null()) {
        if !effort.is_string() {
            return Err(ChatError("reasoning_effort", "Expected a string."));
        }
        output["reasoning"] = json!({"effort":effort});
    }
    if let Some(tools) = object.get("tools").filter(|v| !v.is_null()) {
        let mut mapped = Vec::new();
        for tool in tools
            .as_array()
            .ok_or(ChatError("tools", "Expected an array."))?
        {
            if tool["type"] != "function" {
                return Err(ChatError(
                    "tools.type",
                    "Only function tools are supported.",
                ));
            }
            let function = tool["function"]
                .as_object()
                .ok_or(ChatError("tools.function", "Expected a function object."))?;
            text(&tool["function"]["name"], "tools.function.name")?;
            let mut function = function.clone();
            function.insert("type".to_owned(), json!("function"));
            mapped.push(Value::Object(function));
        }
        output["tools"] = Value::Array(mapped);
    }
    if let Some(choice) = object.get("tool_choice").filter(|v| !v.is_null()) {
        output["tool_choice"] = if choice.is_string() {
            choice.clone()
        } else {
            if choice["type"] != "function" {
                return Err(ChatError("tool_choice", "Expected a function choice."));
            }
            json!({"type":"function","name":text(&choice["function"]["name"], "tool_choice.function.name")?})
        };
    }
    if let Some(format) = object.get("response_format").filter(|v| !v.is_null()) {
        output["text"] = json!({"format":match format["type"].as_str() {
            Some("text" | "json_object") => format.clone(),
            Some("json_schema") => {
                let mut schema=format["json_schema"].as_object().ok_or(ChatError("response_format.json_schema", "Expected a schema object."))?.clone();
                schema.insert("type".to_owned(),json!("json_schema")); Value::Object(schema)
            },
            _ => return Err(ChatError("response_format", "Unsupported response format.")),
        }});
    }
    if let Some(verbosity) = object.get("verbosity").filter(|v| !v.is_null()) {
        if output.get("text").is_none() {
            output["text"] = json!({});
        }
        output["text"]["verbosity"] = verbosity.clone();
    }
    Ok((
        output,
        ChatOptions {
            model,
            include_usage,
        },
    ))
}

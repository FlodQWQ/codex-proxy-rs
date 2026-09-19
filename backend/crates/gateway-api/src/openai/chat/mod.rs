//! Chat Completions wire 兼容投影；调度、计量与交付生命周期仍由 Responses adapter 承担。
mod request;
mod response;
pub(super) use request::{ChatOptions, convert_request};
pub(super) use response::{ChatStream, complete_response};

pub(super) struct ChatError(pub &'static str, pub &'static str);

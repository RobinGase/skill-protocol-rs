//! Common tool status / request / response shapes shared across skills.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Compact "is this tool wired up?" payload returned from a `status`
/// command. Useful for orchestrators that render a list of tools with a
/// connection state badge.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeToolStatus {
    pub id: String,
    pub label: String,
    pub category: String,
    pub available: bool,
    pub connected: bool,
    pub status: String,
    pub message: String,
}

/// Action invocation for skills that namespace multiple operations
/// inside a single CLI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolRunRequest {
    pub action: String,
    #[serde(default)]
    pub args: Value,
}

/// Generic outcome envelope. Skills are free to populate `data` with
/// any JSON; the orchestrator inspects `ok` to decide whether to bubble
/// the message to the user as success or failure.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolRunResponse {
    pub tool_id: String,
    pub action: String,
    pub ok: bool,
    pub status: String,
    pub message: String,
    #[serde(default)]
    pub artifact_paths: Vec<String>,
    #[serde(default)]
    pub data: Value,
}

/// Compose payload accepted by Gmail-style send/draft skills.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GmailComposeRequest {
    pub to: Vec<String>,
    #[serde(default)]
    pub cc: Vec<String>,
    #[serde(default)]
    pub bcc: Vec<String>,
    pub subject: String,
    pub body: String,
    /// Absolute or workspace-relative paths to files to attach. Empty
    /// list → plain text/plain message. Non-empty → multipart/mixed
    /// with the body as part 1 and one binary part per attachment.
    #[serde(default)]
    pub attachments: Vec<String>,
}

/// Brief metadata returned from Gmail search/list operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GmailMessageSummary {
    pub id: String,
    #[serde(default)]
    pub thread_id: Option<String>,
    #[serde(default)]
    pub snippet: Option<String>,
    #[serde(default)]
    pub subject: Option<String>,
    #[serde(default)]
    pub from: Option<String>,
    #[serde(default)]
    pub to: Option<String>,
    #[serde(default)]
    pub date: Option<String>,
}

//! JSON request / response contract for skill CLIs.
//!
//! A skill CLI that opts into the protocol reads a [`SkillCliRequest`] from
//! stdin and writes a [`SkillCliResponse`] to stdout. Every field is
//! optional except `command`, so callers are free to send a minimal
//! `{"command": "status"}` and let the skill apply its defaults.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{Read, Write};

/// Request envelope sent by an orchestrator to a skill CLI on stdin.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillCliRequest {
    /// Top-level command, e.g. `status`, `plan`, `run`.
    pub command: String,
    /// Optional skill-specific operation name (e.g. `discover`, `send`).
    #[serde(default)]
    pub operation: Option<String>,
    /// Free-form arguments for the operation. Skills define their own
    /// schema and validate as needed.
    #[serde(default)]
    pub args: Value,
    /// Free-form context the orchestrator wants the skill to see.
    #[serde(default)]
    pub context: Value,
    /// Absolute path the skill should treat as the workspace root for
    /// resolving relative input/output paths.
    #[serde(default, alias = "workspace_root")]
    pub workspace_root: Option<String>,
    /// Absolute path of the skill's own folder (typically
    /// `<workspace_root>/skills/<name>` or `~/.claude/skills/<name>`).
    #[serde(default, alias = "skill_root")]
    pub skill_root: Option<String>,
    /// Skill-specific settings the orchestrator forwards. Use
    /// [`parse_settings`] to deserialize into your own struct.
    #[serde(default)]
    pub settings: Value,
}

/// Response envelope written by a skill CLI to stdout.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillCliResponse {
    pub ok: bool,
    pub status: String,
    pub message: String,
    #[serde(default)]
    pub artifacts: Vec<String>,
    #[serde(default)]
    pub data: Value,
}

impl SkillCliResponse {
    pub fn ok(status: &str, message: impl Into<String>, data: Value) -> Self {
        Self {
            ok: true,
            status: status.to_string(),
            message: message.into(),
            artifacts: Vec::new(),
            data,
        }
    }

    pub fn blocked(message: impl Into<String>, data: Value) -> Self {
        Self {
            ok: false,
            status: "blocked".to_string(),
            message: message.into(),
            artifacts: Vec::new(),
            data,
        }
    }

    pub fn with_artifacts(mut self, artifacts: Vec<String>) -> Self {
        self.artifacts = artifacts;
        self
    }
}

/// Read a [`SkillCliRequest`] from stdin. Errors include the underlying
/// IO or JSON message so the caller can surface them in a response.
pub fn read_request() -> Result<SkillCliRequest, String> {
    let mut input = String::new();
    std::io::stdin()
        .read_to_string(&mut input)
        .map_err(|error| format!("Failed to read skill request from stdin: {error}"))?;
    serde_json::from_str(&input)
        .map_err(|error| format!("Failed to parse skill request JSON: {error}"))
}

/// Write a [`SkillCliResponse`] as JSON to stdout (no trailing newline).
pub fn write_response(response: &SkillCliResponse) -> Result<(), String> {
    let serialized = serde_json::to_string(response)
        .map_err(|error| format!("Failed to serialize skill response: {error}"))?;
    std::io::stdout()
        .write_all(serialized.as_bytes())
        .map_err(|error| format!("Failed to write skill response: {error}"))?;
    Ok(())
}

/// Deserialize the [`SkillCliRequest::settings`] payload into a typed
/// struct. The error message preserves the underlying serde_json
/// diagnostic so misconfigured callers get a useful hint.
pub fn parse_settings<T>(value: &Value) -> Result<T, String>
where
    T: serde::de::DeserializeOwned,
{
    serde_json::from_value(value.clone())
        .map_err(|error| format!("Failed to parse settings payload: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ok_helper_sets_truthy_status() {
        let response = SkillCliResponse::ok("ready", "all good", Value::Null)
            .with_artifacts(vec!["data/example.csv".to_string()]);
        assert!(response.ok);
        assert_eq!(response.status, "ready");
        assert_eq!(response.artifacts, vec!["data/example.csv".to_string()]);
    }

    #[test]
    fn blocked_helper_emits_falsy_status() {
        let response = SkillCliResponse::blocked("missing api key", Value::Null);
        assert!(!response.ok);
        assert_eq!(response.status, "blocked");
        assert_eq!(response.message, "missing api key");
    }

    #[test]
    fn request_round_trip_through_serde() {
        let raw = r#"{
            "command": "run",
            "operation": "discover",
            "args": {"query": "coffee shops in Amsterdam"},
            "settings": {"reportExportDir": "/tmp/reports"}
        }"#;
        let parsed: SkillCliRequest = serde_json::from_str(raw).expect("parse");
        assert_eq!(parsed.command, "run");
        assert_eq!(parsed.operation.as_deref(), Some("discover"));
        assert_eq!(
            parsed
                .args
                .get("query")
                .and_then(Value::as_str)
                .unwrap_or_default(),
            "coffee shops in Amsterdam"
        );
    }
}

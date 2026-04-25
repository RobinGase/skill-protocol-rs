//! Workspace and skill-root path resolution, plus a tiny `.env` loader.
//!
//! Skills frequently want to:
//! - Resolve relative input/output paths against a single "workspace root"
//!   chosen by the orchestrator.
//! - Locate their own skill folder (for reading bundled assets, writing a
//!   token cache, etc.).
//! - Hydrate environment variables from a `.env` file at the workspace
//!   root before running, without pulling in a heavy dotenv dep.
//!
//! All env-var manipulation here is wrapped in `unsafe` because Rust 2024
//! marks [`std::env::set_var`] as unsafe to call. Skill CLIs are
//! short-lived, single-process programs that hydrate env early and don't
//! race with other threads, so the contract is honored.

use crate::protocol::SkillCliRequest;
use std::{
    env, fs,
    path::{Path, PathBuf},
};

/// Primary env var the skill protocol uses to surface the workspace root
/// to nested processes (e.g. attachment loaders that need to resolve
/// `data/leads.csv`). Falls back to `KAIZEN_WORKSPACE_ROOT` for
/// compatibility with skills migrated from the KaizenMAX runtime.
pub const SKILL_WORKSPACE_ROOT_ENV: &str = "SKILL_WORKSPACE_ROOT";

/// Backwards-compatible env var honored if [`SKILL_WORKSPACE_ROOT_ENV`]
/// is unset.
pub const LEGACY_WORKSPACE_ROOT_ENV: &str = "KAIZEN_WORKSPACE_ROOT";

/// Returns the explicit workspace root from the request, or the current
/// working directory as a sensible fallback.
pub fn optional_workspace_root(request: &SkillCliRequest) -> Option<PathBuf> {
    request
        .workspace_root
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
        .or_else(|| env::current_dir().ok())
}

/// Returns the explicit skill root from the request, or `None`.
pub fn optional_skill_root(request: &SkillCliRequest) -> Option<PathBuf> {
    request
        .skill_root
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
}

/// Compute a skill root by trying, in order:
/// 1. Explicit `request.skill_root`.
/// 2. `<workspace_root>/skills/<skill_name>`.
///
/// Errors only when both inputs are missing.
pub fn resolve_skill_root(request: &SkillCliRequest, skill_name: &str) -> Result<PathBuf, String> {
    if let Some(root) = optional_skill_root(request) {
        return Ok(root);
    }
    if let Some(workspace_root) = optional_workspace_root(request) {
        return Ok(workspace_root.join("skills").join(skill_name));
    }
    Err(format!(
        "skillRoot is required for '{}' when workspaceRoot cannot be resolved.",
        skill_name
    ))
}

/// Like [`optional_workspace_root`] but errors instead of returning
/// `None`. Use when the skill genuinely cannot proceed without a root.
pub fn required_workspace_root(request: &SkillCliRequest) -> Result<String, String> {
    optional_workspace_root(request)
        .map(|path| path.display().to_string())
        .ok_or_else(|| "workspaceRoot is required for this skill.".to_string())
}

/// Validate that the request carries a non-empty `operation` field.
pub fn required_operation(request: &SkillCliRequest) -> Result<&str, String> {
    request
        .operation
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "operation is required for this skill command.".to_string())
}

/// Surface the workspace root via [`SKILL_WORKSPACE_ROOT_ENV`] (and the
/// legacy `KAIZEN_WORKSPACE_ROOT` alias) and load any `.env` file at the
/// workspace root.
///
/// Skills downstream may resolve relative paths from the env var without
/// having to thread the workspace root through every call site.
pub fn hydrate_env_from_workspace(request: &SkillCliRequest) -> Result<(), String> {
    let Some(workspace_root) = optional_workspace_root(request) else {
        return Ok(());
    };
    if env::var_os(SKILL_WORKSPACE_ROOT_ENV).is_none() {
        // SAFETY: skill CLIs are single-threaded at hydration time.
        unsafe {
            env::set_var(SKILL_WORKSPACE_ROOT_ENV, workspace_root.as_os_str());
        }
    }
    if env::var_os(LEGACY_WORKSPACE_ROOT_ENV).is_none() {
        // SAFETY: see above.
        unsafe {
            env::set_var(LEGACY_WORKSPACE_ROOT_ENV, workspace_root.as_os_str());
        }
    }
    let env_path = workspace_root.join(".env");
    if !env_path.exists() {
        return Ok(());
    }
    load_dotenv(env_path.as_path())
}

/// Resolve the workspace root from the environment. Useful for processes
/// that did not receive a [`SkillCliRequest`] (for example, skill CLIs
/// invoked directly from a shell).
pub fn workspace_root_from_env() -> Option<PathBuf> {
    for key in [SKILL_WORKSPACE_ROOT_ENV, LEGACY_WORKSPACE_ROOT_ENV] {
        if let Ok(value) = env::var(key) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Some(PathBuf::from(trimmed));
            }
        }
    }
    env::current_dir().ok()
}

fn load_dotenv(path: &Path) -> Result<(), String> {
    let contents = fs::read_to_string(path)
        .map_err(|error| format!("Failed to read {}: {error}", path.display()))?;
    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let Some((key, value)) = trimmed.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if key.is_empty() || env::var_os(key).is_some() {
            continue;
        }
        let value = value.trim().trim_matches('"').trim_matches('\'');
        // SAFETY: see hydrate_env_from_workspace.
        unsafe {
            env::set_var(key, value);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn req() -> SkillCliRequest {
        SkillCliRequest {
            command: "status".to_string(),
            operation: None,
            args: Value::Null,
            context: Value::Null,
            workspace_root: None,
            skill_root: None,
            settings: Value::Null,
        }
    }

    #[test]
    fn workspace_root_falls_back_to_current_dir() {
        let request = req();
        let resolved = optional_workspace_root(&request);
        assert!(resolved.is_some(), "expected current_dir() fallback");
    }

    #[test]
    fn workspace_root_uses_explicit_field() {
        let mut request = req();
        request.workspace_root = Some("/tmp/workspace".to_string());
        let resolved = optional_workspace_root(&request).expect("workspace root");
        assert_eq!(resolved, PathBuf::from("/tmp/workspace"));
    }

    #[test]
    fn required_operation_errors_on_blank() {
        let mut request = req();
        request.operation = Some("   ".to_string());
        assert!(required_operation(&request).is_err());
        request.operation = Some("send".to_string());
        assert_eq!(required_operation(&request).unwrap(), "send");
    }

    #[test]
    fn resolve_skill_root_appends_skills_segment_when_workspace_known() {
        let mut request = req();
        request.workspace_root = Some("/srv/agent".to_string());
        let root = resolve_skill_root(&request, "leads").expect("skill root");
        assert_eq!(root, PathBuf::from("/srv/agent/skills/leads"));
    }
}

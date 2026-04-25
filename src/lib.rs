//! Reusable building blocks for Rust-backed Claude Code skills.
//!
//! This crate gives skill authors a small, opinionated foundation:
//!
//! - [`protocol`] — JSON request/response types and stdin/stdout helpers for
//!   skills that want to be invoked by an orchestrator over a structured
//!   contract instead of (or in addition to) ad-hoc CLI flags.
//! - [`paths`] — workspace and skill-root resolution, plus a tiny `.env`
//!   loader so skills can hydrate their environment without a heavy
//!   dotenv crate dependency.
//! - [`config`] — the [`config::SkillConfig`] trait. Replace whatever
//!   settings system your host uses with a small adapter that implements
//!   this trait, and your skill stays portable.
//! - [`report`] — a self-contained CSV + XLSX exporter for skills that
//!   want to drop structured artefacts on disk.
//! - [`tool`] — common tool status / request / response shapes.
//!
//! The crate has no async runtime dependency. Skills decide whether to use
//! `tokio`, `async-std`, or stay sync.

pub mod config;
pub mod paths;
pub mod protocol;
pub mod report;
pub mod tool;

pub use config::{BasicSkillConfig, SkillConfig};
pub use paths::{
    hydrate_env_from_workspace, optional_skill_root, optional_workspace_root, required_operation,
    required_workspace_root, resolve_skill_root,
};
pub use protocol::{
    SkillCliRequest, SkillCliResponse, parse_settings, read_request, write_response,
};
pub use report::{ReportExportResult, display_artifact_path, export_report_artifacts};
pub use tool::{
    GmailComposeRequest, GmailMessageSummary, NativeToolStatus, ToolRunRequest, ToolRunResponse,
};

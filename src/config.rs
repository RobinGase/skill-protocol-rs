//! Skill-side configuration trait.
//!
//! Skills should accept any host configuration that can be projected into
//! a [`SkillConfig`]. The two methods on the trait deliberately stay
//! narrow — they cover the only knobs the bundled [`crate::report`]
//! exporter needs. Skills may extend the trait or layer their own traits
//! on top for specific tools.

use std::{env, path::PathBuf};

/// Public surface a skill needs from its host's configuration system.
///
/// Implementations are expected to be cheap (no IO). If a host has to
/// load and parse a settings file, do that once during bootstrap and
/// cache the result behind this trait.
pub trait SkillConfig {
    /// Override the directory where report artefacts are written.
    /// `None` means "use the workspace default".
    fn report_export_dir(&self) -> Option<PathBuf> {
        None
    }

    /// Default report format hint. Recognised values are `"csv"` and
    /// `"xlsx"`. `None` means "use the exporter's default".
    fn report_default_format(&self) -> Option<String> {
        None
    }
}

/// A drop-in [`SkillConfig`] that reads its values from environment
/// variables and otherwise yields `None`. Skills that don't need a
/// host-supplied config can default to this.
#[derive(Debug, Clone, Default)]
pub struct BasicSkillConfig {
    pub report_export_dir: Option<PathBuf>,
    pub report_default_format: Option<String>,
}

impl BasicSkillConfig {
    /// Build a config from `SKILL_REPORT_EXPORT_DIR` and
    /// `SKILL_REPORT_DEFAULT_FORMAT` environment variables. Empty values
    /// are treated as unset.
    pub fn from_env() -> Self {
        let report_export_dir = env::var("SKILL_REPORT_EXPORT_DIR")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .map(PathBuf::from);
        let report_default_format = env::var("SKILL_REPORT_DEFAULT_FORMAT")
            .ok()
            .map(|value| value.trim().to_ascii_lowercase())
            .filter(|value| !value.is_empty());
        Self {
            report_export_dir,
            report_default_format,
        }
    }
}

impl SkillConfig for BasicSkillConfig {
    fn report_export_dir(&self) -> Option<PathBuf> {
        self.report_export_dir.clone()
    }

    fn report_default_format(&self) -> Option<String> {
        self.report_default_format.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_returns_nones() {
        let cfg = BasicSkillConfig::default();
        assert!(cfg.report_export_dir().is_none());
        assert!(cfg.report_default_format().is_none());
    }

    #[test]
    fn explicit_construction_round_trips() {
        let cfg = BasicSkillConfig {
            report_export_dir: Some(PathBuf::from("/tmp/reports")),
            report_default_format: Some("csv".to_string()),
        };
        assert_eq!(cfg.report_export_dir(), Some(PathBuf::from("/tmp/reports")));
        assert_eq!(cfg.report_default_format(), Some("csv".to_string()));
    }
}

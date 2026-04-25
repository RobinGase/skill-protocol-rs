//! Self-contained CSV and XLSX writer for skills that emit tabular
//! artefacts.
//!
//! `export_report_artifacts` writes two files (a `.csv` and an `.xlsx`)
//! into a configurable directory and returns a [`ReportExportResult`]
//! with the relative paths the orchestrator can show the user.

use crate::config::SkillConfig;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::{
    collections::BTreeSet,
    fs,
    io::Write,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

const DEFAULT_REPORT_FORMAT: &str = "xlsx";
const DEFAULT_REPORT_EXPORT_DIR: &str = "data/worker_artifacts";

/// Outcome of a successful report export.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportExportResult {
    pub row_count: usize,
    pub columns: Vec<String>,
    /// Workspace-relative paths to the CSV and XLSX artefacts (in that
    /// order). Falls back to absolute paths when the artefact lives
    /// outside the workspace.
    pub artifact_paths: Vec<String>,
}

/// Resolve the export directory using the host config's hint, falling
/// back to `<workspace_root>/data/worker_artifacts`.
pub fn report_export_dir(cfg: &dyn SkillConfig, workspace_root: &Path) -> PathBuf {
    match cfg.report_export_dir() {
        Some(candidate) if candidate.is_absolute() => candidate,
        Some(candidate) => workspace_root.join(candidate),
        None => workspace_root.join(DEFAULT_REPORT_EXPORT_DIR),
    }
}

/// Resolve the preferred default format. Falls back to `xlsx`.
pub fn report_default_format(cfg: &dyn SkillConfig) -> String {
    match cfg.report_default_format() {
        Some(value) => {
            let lower = value.trim().to_ascii_lowercase();
            if matches!(lower.as_str(), "csv" | "xlsx") {
                lower
            } else {
                DEFAULT_REPORT_FORMAT.to_string()
            }
        }
        None => DEFAULT_REPORT_FORMAT.to_string(),
    }
}

/// Render a path as workspace-relative when possible, otherwise return
/// its absolute display form.
pub fn display_artifact_path(path: &Path, workspace_root: &Path) -> String {
    path.strip_prefix(workspace_root)
        .ok()
        .map(|relative| {
            let text = relative.display().to_string();
            if text.is_empty() {
                ".".to_string()
            } else {
                text
            }
        })
        .unwrap_or_else(|| path.display().to_string())
}

/// Write rows out as both CSV and XLSX inside the resolved export
/// directory and return a [`ReportExportResult`] describing what landed
/// on disk.
pub fn export_report_artifacts(
    cfg: &dyn SkillConfig,
    workspace_root: &Path,
    file_stem: &str,
    rows: &[Map<String, Value>],
) -> Result<ReportExportResult, String> {
    let export_dir = report_export_dir(cfg, workspace_root);
    fs::create_dir_all(&export_dir).map_err(|error| {
        format!(
            "Failed to create report export directory '{}': {error}",
            export_dir.display()
        )
    })?;

    let stem = sanitize_file_stem(file_stem);
    let csv_path = export_dir.join(format!("{}.csv", stem));
    let xlsx_path = export_dir.join(format!("{}.xlsx", stem));
    let columns = collect_columns(rows);

    write_csv_report(&csv_path, &columns, rows)?;
    write_xlsx_report(&xlsx_path, &columns, rows)?;

    Ok(ReportExportResult {
        row_count: rows.len(),
        columns,
        artifact_paths: vec![
            display_artifact_path(&csv_path, workspace_root),
            display_artifact_path(&xlsx_path, workspace_root),
        ],
    })
}

fn collect_columns(rows: &[Map<String, Value>]) -> Vec<String> {
    let mut set = BTreeSet::new();
    for row in rows {
        for key in row.keys() {
            set.insert(key.clone());
        }
    }
    set.into_iter().collect()
}

fn cell_to_string(value: Option<&Value>) -> String {
    match value {
        Some(Value::Null) | None => String::new(),
        Some(Value::String(text)) => text.clone(),
        Some(other) => other.to_string(),
    }
}

fn write_csv_report(
    path: &Path,
    columns: &[String],
    rows: &[Map<String, Value>],
) -> Result<(), String> {
    let mut out = String::new();
    out.push_str(
        &columns
            .iter()
            .map(|item| csv_escape(item))
            .collect::<Vec<_>>()
            .join(","),
    );
    out.push('\n');
    for row in rows {
        let line = columns
            .iter()
            .map(|column| csv_escape(&cell_to_string(row.get(column))))
            .collect::<Vec<_>>()
            .join(",");
        out.push_str(&line);
        out.push('\n');
    }
    fs::write(path, out)
        .map_err(|error| format!("Failed to write CSV report '{}': {error}", path.display()))
}

fn csv_escape(value: &str) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') || value.contains('\r') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

fn write_xlsx_report(
    path: &Path,
    columns: &[String],
    rows: &[Map<String, Value>],
) -> Result<(), String> {
    let file = fs::File::create(path)
        .map_err(|error| format!("Failed to create XLSX report '{}': {error}", path.display()))?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    zip.start_file("[Content_Types].xml", options)
        .map_err(|error| format!("Failed to start XLSX content types: {error}"))?;
    zip.write_all(content_types_xml().as_bytes())
        .map_err(|error| format!("Failed to write XLSX content types: {error}"))?;

    zip.add_directory("_rels/", options)
        .map_err(|error| format!("Failed to add XLSX rels directory: {error}"))?;
    zip.start_file("_rels/.rels", options)
        .map_err(|error| format!("Failed to start XLSX rels: {error}"))?;
    zip.write_all(root_rels_xml().as_bytes())
        .map_err(|error| format!("Failed to write XLSX rels: {error}"))?;

    zip.add_directory("xl/", options)
        .map_err(|error| format!("Failed to add XLSX xl directory: {error}"))?;
    zip.start_file("xl/workbook.xml", options)
        .map_err(|error| format!("Failed to start XLSX workbook: {error}"))?;
    zip.write_all(workbook_xml().as_bytes())
        .map_err(|error| format!("Failed to write XLSX workbook: {error}"))?;

    zip.add_directory("xl/_rels/", options)
        .map_err(|error| format!("Failed to add XLSX workbook rels directory: {error}"))?;
    zip.start_file("xl/_rels/workbook.xml.rels", options)
        .map_err(|error| format!("Failed to start XLSX workbook rels: {error}"))?;
    zip.write_all(workbook_rels_xml().as_bytes())
        .map_err(|error| format!("Failed to write XLSX workbook rels: {error}"))?;

    zip.add_directory("xl/worksheets/", options)
        .map_err(|error| format!("Failed to add XLSX worksheet directory: {error}"))?;
    zip.start_file("xl/worksheets/sheet1.xml", options)
        .map_err(|error| format!("Failed to start XLSX worksheet: {error}"))?;
    zip.write_all(sheet_xml(columns, rows).as_bytes())
        .map_err(|error| format!("Failed to write XLSX worksheet: {error}"))?;

    zip.start_file("xl/styles.xml", options)
        .map_err(|error| format!("Failed to start XLSX styles: {error}"))?;
    zip.write_all(styles_xml().as_bytes())
        .map_err(|error| format!("Failed to write XLSX styles: {error}"))?;

    zip.finish().map_err(|error| {
        format!(
            "Failed to finalize XLSX report '{}': {error}",
            path.display()
        )
    })?;
    Ok(())
}

fn content_types_xml() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>
  <Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>
  <Override PartName="/xl/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"/>
</Types>"#
}

fn root_rels_xml() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/>
</Relationships>"#
}

fn workbook_xml() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <sheets>
    <sheet name="Report" sheetId="1" r:id="rId1"/>
  </sheets>
</workbook>"#
}

fn workbook_rels_xml() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>
</Relationships>"#
}

fn styles_xml() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <fonts count="1"><font><sz val="11"/><name val="Aptos"/></font></fonts>
  <fills count="1"><fill><patternFill patternType="none"/></fill></fills>
  <borders count="1"><border/></borders>
  <cellStyleXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/></cellStyleXfs>
  <cellXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0" xfId="0"/></cellXfs>
</styleSheet>"#
}

fn sheet_xml(columns: &[String], rows: &[Map<String, Value>]) -> String {
    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<worksheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\"><sheetData>",
    );
    xml.push_str(&sheet_row_xml(
        1,
        columns
            .iter()
            .map(|column| column.as_str())
            .collect::<Vec<_>>(),
    ));
    for (idx, row) in rows.iter().enumerate() {
        let values = columns
            .iter()
            .map(|column| cell_to_string(row.get(column)))
            .collect::<Vec<_>>();
        let refs = values
            .iter()
            .map(|value| value.as_str())
            .collect::<Vec<_>>();
        xml.push_str(&sheet_row_xml((idx + 2) as u32, refs));
    }
    xml.push_str("</sheetData></worksheet>");
    xml
}

fn sheet_row_xml(row_number: u32, values: Vec<&str>) -> String {
    let mut row = format!("<row r=\"{}\">", row_number);
    for (idx, value) in values.into_iter().enumerate() {
        let cell_ref = format!("{}{}", excel_column_name(idx), row_number);
        row.push_str(&format!(
            "<c r=\"{}\" t=\"inlineStr\"><is><t xml:space=\"preserve\">{}</t></is></c>",
            cell_ref,
            xml_escape(value)
        ));
    }
    row.push_str("</row>");
    row
}

fn excel_column_name(index: usize) -> String {
    let mut n = index + 1;
    let mut out = String::new();
    while n > 0 {
        let rem = (n - 1) % 26;
        out.insert(0, (b'A' + rem as u8) as char);
        n = (n - 1) / 26;
    }
    out
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn sanitize_file_stem(value: &str) -> String {
    let cleaned = value
        .trim()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();

    if cleaned.is_empty() {
        format!("report-{}", unix_epoch_secs())
    } else {
        cleaned
    }
}

fn unix_epoch_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::BasicSkillConfig;
    use tempfile::TempDir;

    #[test]
    fn report_export_writes_csv_and_xlsx() {
        let temp = TempDir::new().expect("temp dir");
        let cfg = BasicSkillConfig {
            report_export_dir: Some(temp.path().to_path_buf()),
            report_default_format: None,
        };
        let rows = vec![
            Map::from_iter(vec![
                ("name".to_string(), Value::String("Ada".to_string())),
                (
                    "email".to_string(),
                    Value::String("ada@example.com".to_string()),
                ),
            ]),
            Map::from_iter(vec![
                ("name".to_string(), Value::String("Linus".to_string())),
                (
                    "email".to_string(),
                    Value::String("linus@example.com".to_string()),
                ),
            ]),
        ];

        let result =
            export_report_artifacts(&cfg, temp.path(), "prospects", &rows).expect("export");
        assert_eq!(result.row_count, 2);
        assert_eq!(result.artifact_paths.len(), 2);
        assert!(temp.path().join(&result.artifact_paths[0]).exists());
        assert!(temp.path().join(&result.artifact_paths[1]).exists());
    }

    #[test]
    fn default_format_recognises_csv_and_xlsx_only() {
        let csv_cfg = BasicSkillConfig {
            report_default_format: Some("CSV".to_string()),
            ..BasicSkillConfig::default()
        };
        assert_eq!(report_default_format(&csv_cfg), "csv");

        let bogus_cfg = BasicSkillConfig {
            report_default_format: Some("docx".to_string()),
            ..BasicSkillConfig::default()
        };
        assert_eq!(report_default_format(&bogus_cfg), "xlsx");
    }

    #[test]
    fn export_dir_falls_back_to_default_when_unset() {
        let cfg = BasicSkillConfig::default();
        let dir = report_export_dir(&cfg, Path::new("/srv/work"));
        assert_eq!(dir, PathBuf::from("/srv/work/data/worker_artifacts"));
    }

    #[test]
    fn export_dir_treats_relative_override_as_workspace_relative() {
        let cfg = BasicSkillConfig {
            report_export_dir: Some(PathBuf::from("out/reports")),
            ..BasicSkillConfig::default()
        };
        let dir = report_export_dir(&cfg, Path::new("/srv/work"));
        assert_eq!(dir, PathBuf::from("/srv/work/out/reports"));
    }

    #[test]
    fn excel_column_name_handles_double_letter_columns() {
        assert_eq!(excel_column_name(0), "A");
        assert_eq!(excel_column_name(25), "Z");
        assert_eq!(excel_column_name(26), "AA");
        assert_eq!(excel_column_name(27), "AB");
    }
}

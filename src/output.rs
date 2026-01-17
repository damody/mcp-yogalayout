use crate::layout::review::Report;
use crate::layout::LayoutOutput;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum OutputError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),
}

/// 寫入 layout.json
pub fn write_layout_json(output: &LayoutOutput, path: &Path) -> Result<(), OutputError> {
    let json = serde_json::to_string_pretty(output)?;
    std::fs::write(path, json)?;
    Ok(())
}

/// 寫入 report.json
pub fn write_report_json(report: &Report, path: &Path) -> Result<(), OutputError> {
    let json = serde_json::to_string_pretty(report)?;
    std::fs::write(path, json)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::{BoundingBox, ElementKind, LayoutElement, SlideSize};
    use tempfile::tempdir;

    #[test]
    fn test_write_layout_json() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("layout.json");

        let output = LayoutOutput {
            slide: SlideSize {
                w_pt: 960.0,
                h_pt: 540.0,
            },
            elements: vec![LayoutElement {
                id: "title".to_string(),
                kind: ElementKind::Text,
                role: "title".to_string(),
                bounding_box: BoundingBox {
                    x: 24.0,
                    y: 24.0,
                    w: 912.0,
                    h: 44.0,
                },
                ratio: None,
                alt: None,
                source_ref: None,
            }],
        };

        write_layout_json(&output, &path).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("\"w_pt\": 960.0"));
        assert!(content.contains("\"title\""));
    }

    #[test]
    fn test_write_report_json() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("report.json");

        let report = Report {
            overflow_elements: vec![],
            clipped_text: vec![],
            fallbacks: vec![],
            warnings: vec!["Test warning".to_string()],
        };

        write_report_json(&report, &path).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("Test warning"));
    }
}

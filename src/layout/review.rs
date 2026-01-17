use super::{LayoutElement, SlideSize};
use serde::{Deserialize, Serialize};

/// 審查報告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Report {
    pub overflow_elements: Vec<OverflowInfo>,
    pub clipped_text: Vec<ClippedTextInfo>,
    pub fallbacks: Vec<FallbackInfo>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverflowInfo {
    pub element_id: String,
    pub overflow_x: f32,
    pub overflow_y: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClippedTextInfo {
    pub element_id: String,
    pub estimated_height: f32,
    pub allocated_height: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FallbackInfo {
    pub from: String,
    pub to: String,
    pub reason: String,
}

impl Report {
    pub fn new() -> Self {
        Self {
            overflow_elements: vec![],
            clipped_text: vec![],
            fallbacks: vec![],
            warnings: vec![],
        }
    }

    pub fn has_issues(&self) -> bool {
        !self.overflow_elements.is_empty()
            || !self.clipped_text.is_empty()
            || !self.warnings.is_empty()
    }
}

impl Default for Report {
    fn default() -> Self {
        Self::new()
    }
}

/// 審查佈局結果
pub fn review_layout(
    elements: &[LayoutElement],
    slide: &SlideSize,
) -> Report {
    let mut report = Report::new();

    for element in elements {
        let bbox = &element.bounding_box;

        // 檢查 overflow
        let overflow_x = (bbox.x + bbox.w) - slide.w_pt;
        let overflow_y = (bbox.y + bbox.h) - slide.h_pt;

        if overflow_x > 0.1 || overflow_y > 0.1 {
            report.overflow_elements.push(OverflowInfo {
                element_id: element.id.clone(),
                overflow_x: overflow_x.max(0.0),
                overflow_y: overflow_y.max(0.0),
            });
        }

        // 檢查元素是否超出左/上邊界
        if bbox.x < -0.1 || bbox.y < -0.1 {
            report.warnings.push(format!(
                "Element '{}' has negative position: x={}, y={}",
                element.id, bbox.x, bbox.y
            ));
        }

        // 檢查元素尺寸是否過小
        if bbox.w < 10.0 || bbox.h < 10.0 {
            report.warnings.push(format!(
                "Element '{}' is very small: {}x{}pt",
                element.id, bbox.w, bbox.h
            ));
        }
    }

    // 檢查元素數量
    let figure_count = elements.iter().filter(|e| matches!(e.kind, super::ElementKind::Figure)).count();
    if figure_count > 4 {
        report.warnings.push(format!(
            "Too many figures ({}). Consider reducing to 4 or fewer.",
            figure_count
        ));
    }

    report
}

/// 決定是否需要降級
pub fn should_fallback(report: &Report) -> Option<FallbackStrategy> {
    if report.overflow_elements.is_empty() {
        return None;
    }

    // 如果有嚴重 overflow，建議降級
    let max_overflow_y = report.overflow_elements.iter()
        .map(|o| o.overflow_y)
        .max_by(|a, b| a.partial_cmp(b).unwrap())
        .unwrap_or(0.0);

    if max_overflow_y > 100.0 {
        Some(FallbackStrategy::ChangeTemplate)
    } else if max_overflow_y > 50.0 {
        Some(FallbackStrategy::CompactDensity)
    } else {
        Some(FallbackStrategy::WarnOnly)
    }
}

#[derive(Debug, Clone, Copy)]
pub enum FallbackStrategy {
    ChangeTemplate,
    CompactDensity,
    WarnOnly,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::{BoundingBox, ElementKind};

    #[test]
    fn test_detect_overflow() {
        let slide = SlideSize { w_pt: 960.0, h_pt: 540.0 };
        let elements = vec![
            LayoutElement {
                id: "test".to_string(),
                kind: ElementKind::Text,
                role: "body".to_string(),
                bounding_box: BoundingBox { x: 900.0, y: 500.0, w: 100.0, h: 100.0 },
                ratio: None,
                alt: None,
                source_ref: None,
            },
        ];

        let report = review_layout(&elements, &slide);
        assert_eq!(report.overflow_elements.len(), 1);
        assert!((report.overflow_elements[0].overflow_x - 40.0).abs() < 0.1);
        assert!((report.overflow_elements[0].overflow_y - 60.0).abs() < 0.1);
    }

    #[test]
    fn test_no_overflow() {
        let slide = SlideSize { w_pt: 960.0, h_pt: 540.0 };
        let elements = vec![
            LayoutElement {
                id: "test".to_string(),
                kind: ElementKind::Text,
                role: "body".to_string(),
                bounding_box: BoundingBox { x: 24.0, y: 24.0, w: 200.0, h: 50.0 },
                ratio: None,
                alt: None,
                source_ref: None,
            },
        ];

        let report = review_layout(&elements, &slide);
        assert!(report.overflow_elements.is_empty());
    }
}

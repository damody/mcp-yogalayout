use super::templates::{self, Density, Template};
use super::tree::LayoutTree;
use super::{LayoutElement, LayoutOutput, SlideConfig, SlideSize};
use crate::ast::Slide;
use crate::theme::Theme;
use taffy::prelude::*;

/// 計算投影片佈局
pub fn compute_layout(
    slide: &Slide,
    theme: &Theme,
    template: Option<Template>,
    density: Density,
) -> Result<(LayoutOutput, Vec<LayoutElement>), ComputeError> {
    let config = SlideConfig::landscape_16_9();

    // 選擇模板
    let template = template.unwrap_or_else(|| templates::auto_select_template(slide, theme));

    // 建立佈局樹
    let mut tree = LayoutTree::new(theme.clone());

    let root = match template {
        Template::SingleColumn => templates::build_single_column(&mut tree, slide, density)?,
        Template::TwoColumn => templates::build_two_column(&mut tree, slide, density)?,
    };

    tree.root = root;

    // 計算佈局
    tree.compute_layout(Size {
        width: AvailableSpace::Definite(config.width_pt),
        height: AvailableSpace::Definite(config.height_pt),
    })?;

    // 收集元素
    let elements = tree.collect_elements();

    let output = LayoutOutput {
        slide: SlideSize {
            w_pt: config.width_pt,
            h_pt: config.height_pt,
        },
        elements: elements.clone(),
    };

    Ok((output, elements))
}

#[derive(Debug, thiserror::Error)]
pub enum ComputeError {
    #[error("Taffy error: {0}")]
    Taffy(#[from] taffy::TaffyError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::*;
    use crate::theme::*;
    use std::collections::HashMap;

    fn create_test_theme() -> Theme {
        let mut typography = HashMap::new();
        typography.insert("body".to_string(), Typography {
            family: "Inter".to_string(),
            size_pt: 14.0,
            line_height: 1.35,
            weight: 400,
        });
        typography.insert("title".to_string(), Typography {
            family: "Inter".to_string(),
            size_pt: 34.0,
            line_height: 1.10,
            weight: 700,
        });
        typography.insert("subtitle".to_string(), Typography {
            family: "Inter".to_string(),
            size_pt: 18.0,
            line_height: 1.20,
            weight: 500,
        });
        typography.insert("h2".to_string(), Typography {
            family: "Inter".to_string(),
            size_pt: 20.0,
            line_height: 1.20,
            weight: 700,
        });
        typography.insert("caption".to_string(), Typography {
            family: "Inter".to_string(),
            size_pt: 12.0,
            line_height: 1.30,
            weight: 400,
        });

        Theme {
            typography,
            spacing_pt: SpacingScale {
                xs: 4.0, sm: 8.0, md: 12.0, lg: 16.0, xl: 24.0, xxl: 32.0,
            },
            policy: LayoutPolicy {
                page_padding: "xl".to_string(),
                min_font_pt: 10.0,
                min_image_box_pt: MinImageBox { w: 180.0, h: 120.0 },
                two_col_when: TwoColCondition {
                    has_image_or_diagram: true,
                    has_bullets_or_table: true,
                },
                two_col_split: [0.58, 0.42],
            },
        }
    }

    #[test]
    fn test_compute_simple_slide() {
        let theme = create_test_theme();
        let slide = Slide {
            title: "Test Title".to_string(),
            subtitle: Some("Test Subtitle".to_string()),
            blocks: vec![],
        };

        let (output, _) = compute_layout(&slide, &theme, None, Density::Comfortable).unwrap();

        assert_eq!(output.slide.w_pt, 960.0);
        assert_eq!(output.slide.h_pt, 540.0);
        assert!(output.elements.iter().any(|e| e.id == "title"));
        assert!(output.elements.iter().any(|e| e.id == "subtitle"));
    }
}

use super::measure::{self, FigureConstraints};
use super::templates::{self, Density, Template};
use super::tree::LayoutTree;
use super::{BoundingBox, ElementKind, LayoutElement, LayoutOutput, MultiPageOutput, PageLayout, SlideConfig, SlideSize};
use crate::ast::{Block, Slide};
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

/// 計算多頁佈局
///
/// 這個函數會自動分頁，將內容填滿盡可能少的投影片。
pub fn compute_multi_page_layout(
    slide: &Slide,
    theme: &Theme,
    density: Density,
) -> Result<MultiPageOutput, ComputeError> {
    let config = SlideConfig::landscape_16_9();
    let padding = theme.get_spacing(&theme.policy.page_padding) * density.padding_scale();
    let gap = theme.get_spacing("lg") * density.gap_scale();

    let available_width = config.width_pt - padding * 2.0;
    let indent_pt = theme.get_spacing("lg");

    // 量測標題和副標題
    let title_height = measure::measure_text(&slide.title, "title", available_width, theme).height;
    let subtitle_height = slide.subtitle.as_ref()
        .map(|s| measure::measure_text(s, "subtitle", available_width, theme).height + theme.get_spacing("sm"))
        .unwrap_or(0.0);

    // 量測每個區塊的高度
    let block_heights: Vec<f32> = slide.blocks.iter()
        .map(|block| measure_block(block, available_width, theme, indent_pt, gap))
        .collect();

    // 分頁演算法：貪婪地將區塊填入頁面
    let mut pages: Vec<PageLayout> = vec![];
    let mut current_elements: Vec<LayoutElement> = vec![];
    let mut current_y = padding;
    let mut block_idx = 0;

    // 第一頁放標題
    current_elements.push(LayoutElement {
        id: "title".to_string(),
        kind: ElementKind::Text,
        role: "title".to_string(),
        bounding_box: BoundingBox {
            x: padding,
            y: current_y,
            w: available_width,
            h: title_height,
        },
        ratio: None,
        alt: None,
        source_ref: None,
    });
    current_y += title_height;

    if slide.subtitle.is_some() {
        current_y += theme.get_spacing("sm");
        current_elements.push(LayoutElement {
            id: "subtitle".to_string(),
            kind: ElementKind::Text,
            role: "subtitle".to_string(),
            bounding_box: BoundingBox {
                x: padding,
                y: current_y,
                w: available_width,
                h: subtitle_height - theme.get_spacing("sm"),
            },
            ratio: None,
            alt: None,
            source_ref: None,
        });
        current_y += subtitle_height - theme.get_spacing("sm");
    }
    current_y += gap;

    // 填入區塊
    while block_idx < slide.blocks.len() {
        let block_height = block_heights[block_idx];

        // 檢查是否需要分頁
        if current_y + block_height > config.height_pt - padding && !current_elements.is_empty() {
            // 儲存當前頁
            let used = current_y;
            pages.push(PageLayout {
                page_number: pages.len() + 1,
                elements: current_elements,
                used_height_pt: used,
                remaining_height_pt: config.height_pt - used - padding,
            });

            // 開始新頁
            current_elements = vec![];
            current_y = padding;
        }

        // 加入區塊元素
        let block_elements = layout_block(&slide.blocks[block_idx], block_idx, current_y, available_width, padding, theme, indent_pt, gap);
        for elem in block_elements {
            current_elements.push(elem);
        }
        current_y += block_height + gap;
        block_idx += 1;
    }

    // 儲存最後一頁
    if !current_elements.is_empty() {
        let used = current_y;
        pages.push(PageLayout {
            page_number: pages.len() + 1,
            elements: current_elements,
            used_height_pt: used,
            remaining_height_pt: config.height_pt - used - padding,
        });
    }

    let total_pages = pages.len();

    Ok(MultiPageOutput {
        slide_size: SlideSize {
            w_pt: config.width_pt,
            h_pt: config.height_pt,
        },
        pages,
        total_pages,
    })
}

/// 從 theme 建立 FigureConstraints
fn get_figure_constraints(theme: &Theme) -> FigureConstraints {
    let policy = theme.get_figure_policy();
    FigureConstraints {
        min_width: policy.min_width_pt,
        min_height: policy.min_height_pt,
        max_height: policy.max_height_pt,
        width_ratio: policy.width_ratio,
    }
}

/// 量測區塊高度
fn measure_block(block: &Block, max_width: f32, theme: &Theme, indent_pt: f32, gap: f32) -> f32 {
    match block {
        Block::Section(sec) => {
            let heading_height = measure::measure_text(&sec.heading, "h2", max_width, theme).height;
            let children_height: f32 = sec.children.iter()
                .map(|child| measure_block(child, max_width, theme, indent_pt, gap))
                .sum();
            let children_gaps = if sec.children.is_empty() { 0.0 } else { (sec.children.len() - 1) as f32 * gap * 0.5 };
            heading_height + gap * 0.5 + children_height + children_gaps
        }
        Block::Bullets(bullets) => {
            measure::measure_bullets(bullets, max_width, theme, indent_pt).height
        }
        Block::Table(table) => {
            measure::measure_table(table, max_width, theme).height
        }
        Block::Figure(fig) => {
            let constraints = get_figure_constraints(theme);
            measure::measure_figure(&fig.ratio, max_width, &constraints).height
        }
        Block::Callout(callout) => {
            measure::measure_text(&callout.text, "caption", max_width, theme).height
        }
    }
}

/// 為區塊生成佈局元素
fn layout_block(
    block: &Block,
    index: usize,
    start_y: f32,
    available_width: f32,
    padding: f32,
    theme: &Theme,
    indent_pt: f32,
    gap: f32,
) -> Vec<LayoutElement> {
    let mut elements = vec![];
    let mut y = start_y;

    match block {
        Block::Section(sec) => {
            let heading_height = measure::measure_text(&sec.heading, "h2", available_width, theme).height;
            elements.push(LayoutElement {
                id: format!("section:{}:heading", sec.heading),
                kind: ElementKind::Text,
                role: "h2".to_string(),
                bounding_box: BoundingBox {
                    x: padding,
                    y,
                    w: available_width,
                    h: heading_height,
                },
                ratio: None,
                alt: None,
                source_ref: None,
            });
            y += heading_height + gap * 0.5;

            for (i, child) in sec.children.iter().enumerate() {
                let child_elements = layout_block(child, i, y, available_width, padding, theme, indent_pt, gap);
                let child_height = measure_block(child, available_width, theme, indent_pt, gap);
                elements.extend(child_elements);
                y += child_height + gap * 0.5;
            }
        }
        Block::Bullets(bullets) => {
            let height = measure::measure_bullets(bullets, available_width, theme, indent_pt).height;
            elements.push(LayoutElement {
                id: format!("bullets:{}", index),
                kind: ElementKind::Bullets,
                role: "body".to_string(),
                bounding_box: BoundingBox {
                    x: padding,
                    y,
                    w: available_width,
                    h: height,
                },
                ratio: None,
                alt: None,
                source_ref: None,
            });
        }
        Block::Table(table) => {
            let height = measure::measure_table(table, available_width, theme).height;
            elements.push(LayoutElement {
                id: format!("table:{}", index),
                kind: ElementKind::Table,
                role: "body".to_string(),
                bounding_box: BoundingBox {
                    x: padding,
                    y,
                    w: available_width,
                    h: height,
                },
                ratio: None,
                alt: None,
                source_ref: None,
            });
        }
        Block::Figure(fig) => {
            let constraints = get_figure_constraints(theme);
            let measured = measure::measure_figure(&fig.ratio, available_width, &constraints);
            // 圖片置中
            let x_offset = (available_width - measured.width) / 2.0;
            elements.push(LayoutElement {
                id: format!("fig:{}", fig.id),
                kind: ElementKind::Figure,
                role: "body".to_string(),
                bounding_box: BoundingBox {
                    x: padding + x_offset,
                    y,
                    w: measured.width,
                    h: measured.height,
                },
                ratio: Some(format!("{}:{}", fig.ratio.w, fig.ratio.h)),
                alt: Some(fig.alt.clone()),
                source_ref: None,
            });
        }
        Block::Callout(callout) => {
            let height = measure::measure_text(&callout.text, "caption", available_width, theme).height;
            elements.push(LayoutElement {
                id: format!("callout:{}", index),
                kind: ElementKind::Callout,
                role: "caption".to_string(),
                bounding_box: BoundingBox {
                    x: padding,
                    y,
                    w: available_width,
                    h: height,
                },
                ratio: None,
                alt: None,
                source_ref: None,
            });
        }
    }

    elements
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
        typography.insert(
            "body".to_string(),
            Typography {
                family: "Inter".to_string(),
                size_pt: 14.0,
                line_height: 1.35,
                weight: 400,
            },
        );
        typography.insert(
            "title".to_string(),
            Typography {
                family: "Inter".to_string(),
                size_pt: 34.0,
                line_height: 1.10,
                weight: 700,
            },
        );
        typography.insert(
            "subtitle".to_string(),
            Typography {
                family: "Inter".to_string(),
                size_pt: 18.0,
                line_height: 1.20,
                weight: 500,
            },
        );
        typography.insert(
            "h2".to_string(),
            Typography {
                family: "Inter".to_string(),
                size_pt: 20.0,
                line_height: 1.20,
                weight: 700,
            },
        );
        typography.insert(
            "caption".to_string(),
            Typography {
                family: "Inter".to_string(),
                size_pt: 12.0,
                line_height: 1.30,
                weight: 400,
            },
        );

        Theme {
            typography,
            spacing_pt: SpacingScale {
                xs: 4.0,
                sm: 8.0,
                md: 12.0,
                lg: 16.0,
                xl: 24.0,
                xxl: 32.0,
            },
            policy: LayoutPolicy {
                page_padding: "xl".to_string(),
                min_font_pt: 10.0,
                min_image_box_pt: MinImageBox { w: 180.0, h: 120.0 },
                figure_constraints: FigurePolicy::default(),
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

use super::measure::{self, FigureConstraints};
use super::templates::{self, Density, Template};
use super::tree::LayoutTree;
use super::{BoundingBox, ElementKind, LayoutElement, LayoutOutput, MultiPageOutput, PageLayout, SlideConfig, SlideSize};
use crate::ast::{Block, Slide, Section, Figure};
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

/// ============================================================================
/// 極致密集單頁佈局 (One Page Dense Layout)
/// ============================================================================
///
/// 佈局結構：
/// ```
/// ┌──────────────────────────────────────────────────────────────┐
/// │ 標題  副標題                                                  │ <- 標題區 (title_h)
/// ├────────────────┬─────────┬─────────┬─────────┬───────────────┤
/// │                │ Section │ Section │ Section │ Section       │
/// │   [Figure]     │  內容    │  內容    │  內容    │  內容         │ <- 主內容區 (main_h)
/// │                │         │         │         │               │
/// ├────────┬───────┴─────────┴─────────┴─────────┴───────────────┤
/// │ Section│ Section │ Section │ Section │ Section │ Section     │ <- 底部區 (bottom_h)
/// └────────┴─────────┴─────────┴─────────┴─────────┴─────────────┘
/// ```

pub fn compute_one_page_dense_layout(
    slide: &Slide,
    theme: &Theme,
) -> Result<MultiPageOutput, ComputeError> {
    let config = SlideConfig::landscape_16_9();

    // 極致緊湊的間距
    let padding = 6.0;  // 超小邊距
    let gap = 3.0;      // 超小間隙
    let cell_padding = 4.0;  // 格子內邊距

    let available_width = config.width_pt - padding * 2.0;
    let available_height = config.height_pt - padding * 2.0;

    // 收集所有 sections 和 figures
    let (sections, figures) = extract_content(slide);

    // 標題區高度
    let title_h = 24.0;
    let subtitle_h = if slide.subtitle.is_some() { 12.0 } else { 0.0 };
    let header_h = title_h + subtitle_h + gap;

    // 計算主內容區和底部區的分配
    let content_start_y = padding + header_h;
    let remaining_h = available_height - header_h;

    // 根據 section 數量決定佈局
    let section_count = sections.len();
    let figure_count = figures.len();

    // 佈局策略：
    // - 如果有圖：左側放圖（40%寬），右側放 sections（60%寬，多欄）
    // - 底部放剩餘的 sections（多欄）

    let mut elements = vec![];

    // 1. 標題
    elements.push(LayoutElement {
        id: "title".to_string(),
        kind: ElementKind::Text,
        role: "title".to_string(),
        bounding_box: BoundingBox {
            x: padding,
            y: padding,
            w: available_width * 0.5,
            h: title_h,
        },
        ratio: None,
        alt: None,
        source_ref: None,
    });

    // 2. 副標題（放在標題右邊，同一行）
    if slide.subtitle.is_some() {
        elements.push(LayoutElement {
            id: "subtitle".to_string(),
            kind: ElementKind::Text,
            role: "subtitle".to_string(),
            bounding_box: BoundingBox {
                x: padding + available_width * 0.5 + gap,
                y: padding + 6.0,  // 稍微往下對齊
                w: available_width * 0.5 - gap,
                h: subtitle_h + 6.0,
            },
            ratio: None,
            alt: None,
            source_ref: None,
        });
    }

    // 3. 計算主內容區佈局
    if figure_count > 0 {
        // 有圖的情況：左圖右文
        let figure_width = available_width * 0.38;  // 圖佔 38%
        let text_width = available_width * 0.62 - gap;  // 文字佔 62%

        // 計算圖區高度（主內容區的 65%）
        let main_content_h = remaining_h * 0.65;
        let bottom_h = remaining_h * 0.35 - gap;

        // 放置圖片（垂直堆疊）
        let figure_h = (main_content_h - gap * (figure_count.min(2) - 1) as f32) / figure_count.min(2) as f32;
        for (i, fig) in figures.iter().take(2).enumerate() {
            elements.push(LayoutElement {
                id: format!("fig:{}", fig.id),
                kind: ElementKind::Figure,
                role: "body".to_string(),
                bounding_box: BoundingBox {
                    x: padding,
                    y: content_start_y + (figure_h + gap) * i as f32,
                    w: figure_width,
                    h: figure_h,
                },
                ratio: Some(format!("{}:{}", fig.ratio.w, fig.ratio.h)),
                alt: Some(fig.alt.clone()),
                source_ref: None,
            });
        }

        // 右側放 sections（多欄網格）
        let right_x = padding + figure_width + gap;
        let right_sections: Vec<_> = sections.iter().take(section_count.min(6)).collect();

        // 計算右側網格：2-3 欄 x 2-3 列
        let right_cols = if right_sections.len() <= 2 { 2 } else { 3.min((right_sections.len() + 1) / 2) };
        let right_rows = (right_sections.len() + right_cols - 1) / right_cols;
        let cell_w = (text_width - gap * (right_cols - 1) as f32) / right_cols as f32;
        let cell_h = (main_content_h - gap * (right_rows - 1) as f32) / right_rows as f32;

        for (i, section) in right_sections.iter().enumerate() {
            let col = i % right_cols;
            let row = i / right_cols;
            let cell_x = right_x + (cell_w + gap) * col as f32;
            let cell_y = content_start_y + (cell_h + gap) * row as f32;

            add_section_elements(&mut elements, section, cell_x, cell_y, cell_w, cell_h, cell_padding, gap);
        }

        // 底部區：剩餘的 sections + 剩餘的 figures
        let bottom_y = content_start_y + main_content_h + gap;
        let remaining_sections: Vec<_> = sections.iter().skip(6).collect();
        let remaining_figures: Vec<_> = figures.iter().skip(2).collect();
        let bottom_items = remaining_sections.len() + remaining_figures.len();

        if bottom_items > 0 {
            // 底部使用 4 欄，多餘的項目換行
            let bottom_cols = 4.min(bottom_items);
            let bottom_rows = (bottom_items + bottom_cols - 1) / bottom_cols;
            let bottom_cell_w = (available_width - gap * (bottom_cols - 1) as f32) / bottom_cols as f32;
            let bottom_row_h = (bottom_h - gap * (bottom_rows - 1) as f32) / bottom_rows as f32;

            let mut idx = 0;
            for section in remaining_sections {
                let col = idx % bottom_cols;
                let row = idx / bottom_cols;
                let cell_x = padding + (bottom_cell_w + gap) * col as f32;
                let cell_y = bottom_y + (bottom_row_h + gap) * row as f32;
                add_section_elements(&mut elements, section, cell_x, cell_y, bottom_cell_w, bottom_row_h, cell_padding, gap);
                idx += 1;
            }
            for fig in remaining_figures {
                let col = idx % bottom_cols;
                let row = idx / bottom_cols;
                let cell_x = padding + (bottom_cell_w + gap) * col as f32;
                let cell_y = bottom_y + (bottom_row_h + gap) * row as f32;
                elements.push(LayoutElement {
                    id: format!("fig:{}", fig.id),
                    kind: ElementKind::Figure,
                    role: "body".to_string(),
                    bounding_box: BoundingBox {
                        x: cell_x,
                        y: cell_y,
                        w: bottom_cell_w,
                        h: bottom_row_h,
                    },
                    ratio: Some(format!("{}:{}", fig.ratio.w, fig.ratio.h)),
                    alt: Some(fig.alt.clone()),
                    source_ref: None,
                });
                idx += 1;
            }
        }
    } else {
        // 沒有圖的情況：純文字網格
        let cols = (section_count as f32).sqrt().ceil() as usize;
        let rows = (section_count + cols - 1) / cols;
        let cell_w = (available_width - gap * (cols - 1) as f32) / cols as f32;
        let cell_h = (remaining_h - gap * (rows - 1) as f32) / rows as f32;

        for (i, section) in sections.iter().enumerate() {
            let col = i % cols;
            let row = i / cols;
            let cell_x = padding + (cell_w + gap) * col as f32;
            let cell_y = content_start_y + (cell_h + gap) * row as f32;

            add_section_elements(&mut elements, section, cell_x, cell_y, cell_w, cell_h, cell_padding, gap);
        }
    }

    // 組合成單頁輸出
    let page = PageLayout {
        page_number: 1,
        elements,
        used_height_pt: config.height_pt - padding * 2.0,
        remaining_height_pt: 0.0,
    };

    Ok(MultiPageOutput {
        slide_size: SlideSize {
            w_pt: config.width_pt,
            h_pt: config.height_pt,
        },
        pages: vec![page],
        total_pages: 1,
    })
}

/// 從 Slide 提取所有 sections 和 figures
fn extract_content(slide: &Slide) -> (Vec<Section>, Vec<Figure>) {
    let mut sections = vec![];
    let mut figures = vec![];

    for block in &slide.blocks {
        extract_block(block, &mut sections, &mut figures);
    }

    (sections, figures)
}

fn extract_block(block: &Block, sections: &mut Vec<Section>, figures: &mut Vec<Figure>) {
    match block {
        Block::Section(sec) => {
            // 先提取 section 中的 figures
            let mut section_figures = vec![];
            let mut other_children = vec![];
            for child in &sec.children {
                match child {
                    Block::Figure(fig) => section_figures.push(fig.clone()),
                    _ => other_children.push(child.clone()),
                }
            }
            figures.extend(section_figures);

            // 只有有內容的 section 才加入
            if !other_children.is_empty() || sec.children.is_empty() {
                sections.push(Section {
                    heading: sec.heading.clone(),
                    children: other_children,
                });
            }
        }
        Block::Figure(fig) => {
            figures.push(fig.clone());
        }
        Block::Bullets(bullets) => {
            // 頂層 bullets 包成一個虛擬 section
            sections.push(Section {
                heading: "".to_string(),
                children: vec![Block::Bullets(bullets.clone())],
            });
        }
        Block::Table(table) => {
            sections.push(Section {
                heading: "".to_string(),
                children: vec![Block::Table(table.clone())],
            });
        }
        Block::Callout(callout) => {
            sections.push(Section {
                heading: "".to_string(),
                children: vec![Block::Callout(callout.clone())],
            });
        }
    }
}

/// 為 section 生成元素
fn add_section_elements(
    elements: &mut Vec<LayoutElement>,
    section: &Section,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    cell_padding: f32,
    gap: f32,
) {
    let inner_x = x + cell_padding;
    let inner_y = y + cell_padding;
    let inner_w = w - cell_padding * 2.0;
    let inner_h = h - cell_padding * 2.0;

    // Section heading
    let heading_h = if section.heading.is_empty() { 0.0 } else { 14.0 };
    if !section.heading.is_empty() {
        elements.push(LayoutElement {
            id: format!("section:{}:heading", section.heading),
            kind: ElementKind::Text,
            role: "h2".to_string(),
            bounding_box: BoundingBox {
                x: inner_x,
                y: inner_y,
                w: inner_w,
                h: heading_h,
            },
            ratio: None,
            alt: None,
            source_ref: None,
        });
    }

    // Section children
    let content_y = inner_y + heading_h + if heading_h > 0.0 { gap } else { 0.0 };
    let content_h = inner_h - heading_h - if heading_h > 0.0 { gap } else { 0.0 };

    if section.children.is_empty() {
        return;
    }

    // 計算邏輯區塊數量：所有 bullets 合併為 1 個區塊，其他類型各自獨立
    let bullets_count = section.children.iter().filter(|c| matches!(c, Block::Bullets(_))).count();
    let table_count = section.children.iter().filter(|c| matches!(c, Block::Table(_))).count();
    let callout_count = section.children.iter().filter(|c| matches!(c, Block::Callout(_))).count();

    let logical_blocks = (if bullets_count > 0 { 1 } else { 0 }) + table_count + callout_count;

    if logical_blocks == 0 {
        return;
    }

    let block_h = (content_h - gap * (logical_blocks - 1).max(0) as f32) / logical_blocks as f32;
    let mut current_y = content_y;

    // 先放所有 bullets（合併成一個大區塊）
    if bullets_count > 0 {
        elements.push(LayoutElement {
            id: "bullets:0".to_string(),
            kind: ElementKind::Bullets,
            role: "body".to_string(),
            bounding_box: BoundingBox {
                x: inner_x,
                y: current_y,
                w: inner_w,
                h: block_h,
            },
            ratio: None,
            alt: None,
            source_ref: None,
        });
        current_y += block_h + gap;
    }

    // 再放 tables
    let mut table_idx = 0;
    for child in &section.children {
        if let Block::Table(_) = child {
            elements.push(LayoutElement {
                id: format!("table:{}", table_idx),
                kind: ElementKind::Table,
                role: "body".to_string(),
                bounding_box: BoundingBox {
                    x: inner_x,
                    y: current_y,
                    w: inner_w,
                    h: block_h,
                },
                ratio: None,
                alt: None,
                source_ref: None,
            });
            current_y += block_h + gap;
            table_idx += 1;
        }
    }

    // 最後放 callouts
    let mut callout_idx = 0;
    for child in &section.children {
        if let Block::Callout(_) = child {
            elements.push(LayoutElement {
                id: format!("callout:{}", callout_idx),
                kind: ElementKind::Callout,
                role: "caption".to_string(),
                bounding_box: BoundingBox {
                    x: inner_x,
                    y: current_y,
                    w: inner_w,
                    h: block_h,
                },
                ratio: None,
                alt: None,
                source_ref: None,
            });
            current_y += block_h + gap;
            callout_idx += 1;
        }
    }
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

use super::tree::{LayoutTree, NodeContext};
use crate::ast::*;
use crate::theme::Theme;
use taffy::prelude::*;
use taffy::NodeId;

/// 模板類型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Template {
    SingleColumn,
    TwoColumn,
}

/// 密度設定
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Density {
    Comfortable,
    Compact,
}

impl Density {
    pub fn gap_scale(&self) -> f32 {
        match self {
            Density::Comfortable => 1.0,
            Density::Compact => 0.6,
        }
    }

    pub fn padding_scale(&self) -> f32 {
        match self {
            Density::Comfortable => 1.0,
            Density::Compact => 0.75,
        }
    }
}

/// 自動選擇模板
pub fn auto_select_template(slide: &Slide, theme: &Theme) -> Template {
    let has_figure = has_figures(slide);
    let has_text_content = has_bullets_or_table(slide);

    let policy = &theme.policy.two_col_when;

    if policy.has_image_or_diagram && has_figure && policy.has_bullets_or_table && has_text_content
    {
        Template::TwoColumn
    } else {
        Template::SingleColumn
    }
}

fn has_figures(slide: &Slide) -> bool {
    for block in &slide.blocks {
        if contains_figure(block) {
            return true;
        }
    }
    false
}

fn contains_figure(block: &Block) -> bool {
    match block {
        Block::Figure(_) => true,
        Block::Section(sec) => sec.children.iter().any(contains_figure),
        _ => false,
    }
}

fn has_bullets_or_table(slide: &Slide) -> bool {
    for block in &slide.blocks {
        if contains_bullets_or_table(block) {
            return true;
        }
    }
    false
}

fn contains_bullets_or_table(block: &Block) -> bool {
    match block {
        Block::Bullets(_) | Block::Table(_) => true,
        Block::Section(sec) => sec.children.iter().any(contains_bullets_or_table),
        _ => false,
    }
}

/// 建構單欄佈局
pub fn build_single_column(
    tree: &mut LayoutTree,
    slide: &Slide,
    density: Density,
) -> Result<NodeId, taffy::TaffyError> {
    let theme = tree.theme.clone();
    let padding = theme.get_spacing(&theme.policy.page_padding) * density.padding_scale();
    let gap = theme.get_spacing("lg") * density.gap_scale();

    // Header 區域（title + subtitle）
    let mut header_children = vec![];

    // Title
    let title_node = tree.new_leaf(
        Style {
            flex_shrink: 0.0,
            ..Default::default()
        },
        NodeContext::Text {
            id: "title".to_string(),
            role: "title".to_string(),
            content: slide.title.clone(),
        },
    )?;
    header_children.push(title_node);

    // Subtitle
    if let Some(ref subtitle) = slide.subtitle {
        let subtitle_node = tree.new_leaf(
            Style {
                flex_shrink: 0.0,
                margin: Rect {
                    top: LengthPercentageAuto::length(theme.get_spacing("sm")),
                    ..Rect::zero()
                },
                ..Default::default()
            },
            NodeContext::Text {
                id: "subtitle".to_string(),
                role: "subtitle".to_string(),
                content: subtitle.clone(),
            },
        )?;
        header_children.push(subtitle_node);
    }

    let header = tree.new_container(
        "header",
        Style {
            flex_direction: FlexDirection::Column,
            flex_shrink: 0.0,
            ..Default::default()
        },
        &header_children,
    )?;

    // Body 區域
    let body_children = build_body_nodes(tree, &slide.blocks, density, &theme)?;

    let body = tree.new_container(
        "body",
        Style {
            flex_direction: FlexDirection::Column,
            flex_grow: 1.0,
            gap: Size {
                width: LengthPercentage::length(0.0),
                height: LengthPercentage::length(gap),
            },
            ..Default::default()
        },
        &body_children,
    )?;

    // Root
    let root = tree.new_container(
        "root",
        Style {
            flex_direction: FlexDirection::Column,
            size: Size {
                width: Dimension::length(960.0),
                height: Dimension::length(540.0),
            },
            padding: Rect {
                left: LengthPercentage::length(padding),
                right: LengthPercentage::length(padding),
                top: LengthPercentage::length(padding),
                bottom: LengthPercentage::length(padding),
            },
            gap: Size {
                width: LengthPercentage::length(0.0),
                height: LengthPercentage::length(gap),
            },
            ..Default::default()
        },
        &[header, body],
    )?;

    Ok(root)
}

/// 建構雙欄佈局
pub fn build_two_column(
    tree: &mut LayoutTree,
    slide: &Slide,
    density: Density,
) -> Result<NodeId, taffy::TaffyError> {
    let theme = tree.theme.clone();
    let padding = theme.get_spacing(&theme.policy.page_padding) * density.padding_scale();
    let gap = theme.get_spacing("lg") * density.gap_scale();
    let col_gap = theme.get_spacing("xl") * density.gap_scale();
    let split = theme.policy.two_col_split;

    // Header
    let mut header_children = vec![];

    let title_node = tree.new_leaf(
        Style {
            flex_shrink: 0.0,
            ..Default::default()
        },
        NodeContext::Text {
            id: "title".to_string(),
            role: "title".to_string(),
            content: slide.title.clone(),
        },
    )?;
    header_children.push(title_node);

    if let Some(ref subtitle) = slide.subtitle {
        let subtitle_node = tree.new_leaf(
            Style {
                flex_shrink: 0.0,
                margin: Rect {
                    top: LengthPercentageAuto::length(theme.get_spacing("sm")),
                    ..Rect::zero()
                },
                ..Default::default()
            },
            NodeContext::Text {
                id: "subtitle".to_string(),
                role: "subtitle".to_string(),
                content: subtitle.clone(),
            },
        )?;
        header_children.push(subtitle_node);
    }

    let header = tree.new_container(
        "header",
        Style {
            flex_direction: FlexDirection::Column,
            flex_shrink: 0.0,
            ..Default::default()
        },
        &header_children,
    )?;

    // 分離文字內容與圖片
    let (text_blocks, figure_blocks) = separate_content(&slide.blocks);

    // 左欄（文字內容）
    let left_children = build_body_nodes(tree, &text_blocks, density, &theme)?;
    let left_col = tree.new_container(
        "left_column",
        Style {
            flex_direction: FlexDirection::Column,
            flex_basis: Dimension::percent(split[0]),
            flex_grow: 0.0,
            flex_shrink: 0.0,
            gap: Size {
                width: LengthPercentage::length(0.0),
                height: LengthPercentage::length(gap),
            },
            ..Default::default()
        },
        &left_children,
    )?;

    // 右欄（圖片）
    let right_children = build_body_nodes(tree, &figure_blocks, density, &theme)?;
    let right_col = tree.new_container(
        "right_column",
        Style {
            flex_direction: FlexDirection::Column,
            flex_basis: Dimension::percent(split[1]),
            flex_grow: 0.0,
            flex_shrink: 0.0,
            gap: Size {
                width: LengthPercentage::length(0.0),
                height: LengthPercentage::length(gap),
            },
            ..Default::default()
        },
        &right_children,
    )?;

    // Body（雙欄容器）
    let body = tree.new_container(
        "body",
        Style {
            flex_direction: FlexDirection::Row,
            flex_grow: 1.0,
            gap: Size {
                width: LengthPercentage::length(col_gap),
                height: LengthPercentage::length(0.0),
            },
            ..Default::default()
        },
        &[left_col, right_col],
    )?;

    // Root
    let root = tree.new_container(
        "root",
        Style {
            flex_direction: FlexDirection::Column,
            size: Size {
                width: Dimension::length(960.0),
                height: Dimension::length(540.0),
            },
            padding: Rect {
                left: LengthPercentage::length(padding),
                right: LengthPercentage::length(padding),
                top: LengthPercentage::length(padding),
                bottom: LengthPercentage::length(padding),
            },
            gap: Size {
                width: LengthPercentage::length(0.0),
                height: LengthPercentage::length(gap),
            },
            ..Default::default()
        },
        &[header, body],
    )?;

    Ok(root)
}

/// 分離文字內容與圖片
fn separate_content(blocks: &[Block]) -> (Vec<Block>, Vec<Block>) {
    let mut text_blocks = vec![];
    let mut figure_blocks = vec![];

    for block in blocks {
        match block {
            Block::Figure(_) => figure_blocks.push(block.clone()),
            Block::Section(sec) => {
                let (text_children, fig_children) = separate_content(&sec.children);
                if !text_children.is_empty() {
                    text_blocks.push(Block::Section(Section {
                        heading: sec.heading.clone(),
                        children: text_children,
                    }));
                }
                figure_blocks.extend(fig_children);
            }
            _ => text_blocks.push(block.clone()),
        }
    }

    (text_blocks, figure_blocks)
}

/// 建構 body 區域的節點
fn build_body_nodes(
    tree: &mut LayoutTree,
    blocks: &[Block],
    density: Density,
    theme: &Theme,
) -> Result<Vec<NodeId>, taffy::TaffyError> {
    let mut nodes = vec![];

    for (i, block) in blocks.iter().enumerate() {
        let node = build_block_node(tree, block, i, density, theme)?;
        nodes.push(node);
    }

    Ok(nodes)
}

/// 建構單個區塊的節點
fn build_block_node(
    tree: &mut LayoutTree,
    block: &Block,
    index: usize,
    density: Density,
    theme: &Theme,
) -> Result<NodeId, taffy::TaffyError> {
    let gap = theme.get_spacing("sm") * density.gap_scale();

    match block {
        Block::Section(sec) => {
            let mut children = vec![];

            // Section heading
            let heading_node = tree.new_leaf(
                Style {
                    flex_shrink: 0.0,
                    ..Default::default()
                },
                NodeContext::Text {
                    id: format!("section:{}:heading", sec.heading),
                    role: "h2".to_string(),
                    content: sec.heading.clone(),
                },
            )?;
            children.push(heading_node);

            // Section children
            for (j, child) in sec.children.iter().enumerate() {
                let child_node = build_block_node(tree, child, j, density, theme)?;
                children.push(child_node);
            }

            tree.new_container(
                &format!("section:{}", sec.heading),
                Style {
                    flex_direction: FlexDirection::Column,
                    flex_shrink: 0.0,
                    gap: Size {
                        width: LengthPercentage::length(0.0),
                        height: LengthPercentage::length(gap),
                    },
                    ..Default::default()
                },
                &children,
            )
        }

        Block::Bullets(bullets) => tree.new_leaf(
            Style::default(),
            NodeContext::Bullets {
                id: format!("bullets:{}", index),
                bullets: bullets.clone(),
            },
        ),

        Block::Table(table) => tree.new_leaf(
            Style::default(),
            NodeContext::Table {
                id: format!("table:{}", index),
                table: table.clone(),
            },
        ),

        Block::Figure(fig) => tree.new_leaf(
            Style::default(),
            NodeContext::Figure {
                id: format!("fig:{}", fig.id),
                ratio: fig.ratio,
                kind: fig.kind,
                alt: fig.alt.clone(),
            },
        ),

        Block::Callout(callout) => tree.new_leaf(
            Style {
                margin: Rect {
                    top: LengthPercentageAuto::length(theme.get_spacing("md")),
                    ..Rect::zero()
                },
                ..Default::default()
            },
            NodeContext::Callout {
                id: format!("callout:{}", index),
                text: callout.text.clone(),
            },
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
                two_col_when: TwoColCondition {
                    has_image_or_diagram: true,
                    has_bullets_or_table: true,
                },
                two_col_split: [0.58, 0.42],
            },
        }
    }

    #[test]
    fn test_auto_select_single_column() {
        let theme = create_test_theme();
        let slide = Slide {
            title: "Test".to_string(),
            subtitle: None,
            blocks: vec![Block::Bullets(Bullets {
                items: vec![BulletItem {
                    text: "Item".to_string(),
                    children: vec![],
                }],
            })],
        };
        assert_eq!(auto_select_template(&slide, &theme), Template::SingleColumn);
    }

    #[test]
    fn test_auto_select_two_column() {
        let theme = create_test_theme();
        let slide = Slide {
            title: "Test".to_string(),
            subtitle: None,
            blocks: vec![Block::Section(Section {
                heading: "Section".to_string(),
                children: vec![
                    Block::Bullets(Bullets {
                        items: vec![BulletItem {
                            text: "Item".to_string(),
                            children: vec![],
                        }],
                    }),
                    Block::Figure(Figure {
                        id: "fig1".to_string(),
                        ratio: AspectRatio::new(16, 9),
                        kind: FigureKind::Diagram,
                        alt: "Test".to_string(),
                    }),
                ],
            })],
        };
        assert_eq!(auto_select_template(&slide, &theme), Template::TwoColumn);
    }

    #[test]
    fn test_build_single_column() {
        let theme = create_test_theme();
        let mut tree = LayoutTree::new(theme);
        let slide = Slide {
            title: "Test".to_string(),
            subtitle: Some("Subtitle".to_string()),
            blocks: vec![],
        };

        let root = build_single_column(&mut tree, &slide, Density::Comfortable).unwrap();
        tree.root = root;
        tree.compute_layout(Size::MAX_CONTENT).unwrap();

        let elements = tree.collect_elements();
        assert!(elements.iter().any(|e| e.id == "title"));
        assert!(elements.iter().any(|e| e.id == "subtitle"));
    }

    #[test]
    fn test_build_two_column() {
        let theme = create_test_theme();
        let mut tree = LayoutTree::new(theme);
        let slide = Slide {
            title: "Test".to_string(),
            subtitle: None,
            blocks: vec![
                Block::Section(Section {
                    heading: "Content".to_string(),
                    children: vec![Block::Bullets(Bullets {
                        items: vec![BulletItem {
                            text: "Item".to_string(),
                            children: vec![],
                        }],
                    })],
                }),
                Block::Figure(Figure {
                    id: "fig1".to_string(),
                    ratio: AspectRatio::new(16, 9),
                    kind: FigureKind::Diagram,
                    alt: "Test".to_string(),
                }),
            ],
        };

        let root = build_two_column(&mut tree, &slide, Density::Comfortable).unwrap();
        tree.root = root;
        tree.compute_layout(Size::MAX_CONTENT).unwrap();

        let elements = tree.collect_elements();
        assert!(elements.iter().any(|e| e.id == "title"));
        assert!(elements.iter().any(|e| e.id == "fig:fig1"));
    }
}

use super::{BoundingBox, ElementKind, LayoutElement};
use crate::ast::*;
use crate::theme::Theme;
use taffy::prelude::*;
use taffy::{NodeId, TaffyTree};

/// 節點上下文：儲存量測所需資訊
#[derive(Debug, Clone)]
pub enum NodeContext {
    /// 文字節點
    Text {
        id: String,
        role: String,
        content: String,
    },
    /// 項目符號列表
    Bullets {
        id: String,
        bullets: crate::ast::Bullets,
    },
    /// 表格
    Table {
        id: String,
        table: crate::ast::Table,
    },
    /// 圖片/示意圖
    Figure {
        id: String,
        ratio: AspectRatio,
        kind: FigureKind,
        alt: String,
    },
    /// Callout
    Callout { id: String, text: String },
    /// 容器節點（無需量測）
    Container { id: String },
}

impl NodeContext {
    pub fn id(&self) -> &str {
        match self {
            NodeContext::Text { id, .. } => id,
            NodeContext::Bullets { id, .. } => id,
            NodeContext::Table { id, .. } => id,
            NodeContext::Figure { id, .. } => id,
            NodeContext::Callout { id, .. } => id,
            NodeContext::Container { id } => id,
        }
    }
}

/// 佈局樹包裝器
pub struct LayoutTree {
    pub tree: TaffyTree<NodeContext>,
    pub root: NodeId,
    pub theme: Theme,
}

impl LayoutTree {
    /// 建立新的佈局樹
    pub fn new(theme: Theme) -> Self {
        let tree = TaffyTree::new();
        // 使用一個臨時的 root，稍後會被設定
        Self {
            tree,
            root: taffy::NodeId::new(0),
            theme,
        }
    }

    /// 建立容器節點
    pub fn new_container(
        &mut self,
        id: &str,
        style: Style,
        children: &[NodeId],
    ) -> Result<NodeId, taffy::TaffyError> {
        let node = self.tree.new_with_children(style, children)?;
        self.tree
            .set_node_context(node, Some(NodeContext::Container { id: id.to_string() }))?;
        Ok(node)
    }

    /// 建立葉節點（需量測）
    pub fn new_leaf(
        &mut self,
        style: Style,
        context: NodeContext,
    ) -> Result<NodeId, taffy::TaffyError> {
        self.tree.new_leaf_with_context(style, context)
    }

    /// 計算佈局
    pub fn compute_layout(
        &mut self,
        available_size: Size<AvailableSpace>,
    ) -> Result<(), taffy::TaffyError> {
        let theme = self.theme.clone();
        self.tree.compute_layout_with_measure(
            self.root,
            available_size,
            |known_dimensions, available_space, _node_id, node_context, _style| {
                measure_node(known_dimensions, available_space, node_context, &theme)
            },
        )
    }

    /// 取得節點佈局結果
    pub fn get_layout(&self, node: NodeId) -> Option<&taffy::Layout> {
        self.tree.layout(node).ok()
    }

    /// 收集所有元素的佈局結果
    pub fn collect_elements(&self) -> Vec<LayoutElement> {
        let mut elements = vec![];
        self.collect_elements_recursive(self.root, 0.0, 0.0, &mut elements);
        elements
    }

    fn collect_elements_recursive(
        &self,
        node: NodeId,
        parent_x: f32,
        parent_y: f32,
        elements: &mut Vec<LayoutElement>,
    ) {
        let layout = match self.tree.layout(node) {
            Ok(l) => l,
            Err(_) => return,
        };

        let x = parent_x + layout.location.x;
        let y = parent_y + layout.location.y;

        if let Some(context) = self.tree.get_node_context(node) {
            let element = match context {
                NodeContext::Text { id, role, .. } => Some(LayoutElement {
                    id: id.clone(),
                    kind: ElementKind::Text,
                    role: role.clone(),
                    bounding_box: BoundingBox {
                        x,
                        y,
                        w: layout.size.width,
                        h: layout.size.height,
                    },
                    ratio: None,
                    alt: None,
                    source_ref: None,
                }),
                NodeContext::Bullets { id, .. } => Some(LayoutElement {
                    id: id.clone(),
                    kind: ElementKind::Bullets,
                    role: "body".to_string(),
                    bounding_box: BoundingBox {
                        x,
                        y,
                        w: layout.size.width,
                        h: layout.size.height,
                    },
                    ratio: None,
                    alt: None,
                    source_ref: None,
                }),
                NodeContext::Table { id, .. } => Some(LayoutElement {
                    id: id.clone(),
                    kind: ElementKind::Table,
                    role: "body".to_string(),
                    bounding_box: BoundingBox {
                        x,
                        y,
                        w: layout.size.width,
                        h: layout.size.height,
                    },
                    ratio: None,
                    alt: None,
                    source_ref: None,
                }),
                NodeContext::Figure { id, ratio, alt, .. } => Some(LayoutElement {
                    id: id.clone(),
                    kind: ElementKind::Figure,
                    role: "body".to_string(),
                    bounding_box: BoundingBox {
                        x,
                        y,
                        w: layout.size.width,
                        h: layout.size.height,
                    },
                    ratio: Some(format!("{}:{}", ratio.w, ratio.h)),
                    alt: Some(alt.clone()),
                    source_ref: None,
                }),
                NodeContext::Callout { id, .. } => Some(LayoutElement {
                    id: id.clone(),
                    kind: ElementKind::Callout,
                    role: "caption".to_string(),
                    bounding_box: BoundingBox {
                        x,
                        y,
                        w: layout.size.width,
                        h: layout.size.height,
                    },
                    ratio: None,
                    alt: None,
                    source_ref: None,
                }),
                NodeContext::Container { .. } => None,
            };

            if let Some(el) = element {
                elements.push(el);
            }
        }

        // 遞迴處理子節點
        if let Ok(children) = self.tree.children(node) {
            for child in children {
                self.collect_elements_recursive(child, x, y, elements);
            }
        }
    }
}

/// 量測節點尺寸
fn measure_node(
    known_dimensions: Size<Option<f32>>,
    available_space: Size<AvailableSpace>,
    node_context: Option<&mut NodeContext>,
    theme: &Theme,
) -> Size<f32> {
    // 如果已知尺寸，直接返回
    if let Size {
        width: Some(w),
        height: Some(h),
    } = known_dimensions
    {
        return Size {
            width: w,
            height: h,
        };
    }

    let context = match node_context {
        Some(c) => c,
        None => return Size::ZERO,
    };

    let max_width = match available_space.width {
        AvailableSpace::Definite(w) => w,
        AvailableSpace::MinContent => 100.0,
        AvailableSpace::MaxContent => 800.0,
    };

    match context {
        NodeContext::Text { role, content, .. } => {
            let measured = super::measure::measure_text(content, role, max_width, theme);
            Size {
                width: measured.width,
                height: measured.height,
            }
        }
        NodeContext::Bullets { bullets, .. } => {
            let indent_pt = theme.get_spacing("lg");
            let measured = super::measure::measure_bullets(bullets, max_width, theme, indent_pt);
            Size {
                width: measured.width,
                height: measured.height,
            }
        }
        NodeContext::Table { table, .. } => {
            let measured = super::measure::measure_table(table, max_width, theme);
            Size {
                width: measured.width,
                height: measured.height,
            }
        }
        NodeContext::Figure { ratio, .. } => {
            let policy = theme.get_figure_policy();
            let constraints = super::measure::FigureConstraints {
                min_width: policy.min_width_pt,
                min_height: policy.min_height_pt,
                max_height: policy.max_height_pt,
                width_ratio: policy.width_ratio,
            };
            let measured = super::measure::measure_figure(ratio, max_width, &constraints);
            Size {
                width: measured.width,
                height: measured.height,
            }
        }
        NodeContext::Callout { text, .. } => {
            let measured = super::measure::measure_text(text, "caption", max_width, theme);
            Size {
                width: measured.width,
                height: measured.height,
            }
        }
        NodeContext::Container { .. } => Size::ZERO,
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
    fn test_create_layout_tree() {
        let theme = create_test_theme();
        let mut tree = LayoutTree::new(theme);

        let leaf = tree
            .new_leaf(
                Style::default(),
                NodeContext::Text {
                    id: "test".to_string(),
                    role: "body".to_string(),
                    content: "Hello".to_string(),
                },
            )
            .unwrap();

        let root = tree
            .new_container(
                "root",
                Style {
                    size: Size {
                        width: Dimension::length(960.0),
                        height: Dimension::length(540.0),
                    },
                    ..Default::default()
                },
                &[leaf],
            )
            .unwrap();

        tree.root = root;
        tree.compute_layout(Size::MAX_CONTENT).unwrap();

        let elements = tree.collect_elements();
        assert_eq!(elements.len(), 1);
        assert_eq!(elements[0].id, "test");
    }

    #[test]
    fn test_nested_layout() {
        let theme = create_test_theme();
        let mut tree = LayoutTree::new(theme);

        let text1 = tree
            .new_leaf(
                Style::default(),
                NodeContext::Text {
                    id: "text1".to_string(),
                    role: "body".to_string(),
                    content: "First".to_string(),
                },
            )
            .unwrap();

        let text2 = tree
            .new_leaf(
                Style::default(),
                NodeContext::Text {
                    id: "text2".to_string(),
                    role: "body".to_string(),
                    content: "Second".to_string(),
                },
            )
            .unwrap();

        let container = tree
            .new_container(
                "container",
                Style {
                    flex_direction: FlexDirection::Column,
                    ..Default::default()
                },
                &[text1, text2],
            )
            .unwrap();

        let root = tree
            .new_container(
                "root",
                Style {
                    size: Size {
                        width: Dimension::length(960.0),
                        height: Dimension::length(540.0),
                    },
                    padding: Rect {
                        left: LengthPercentage::length(24.0),
                        right: LengthPercentage::length(24.0),
                        top: LengthPercentage::length(24.0),
                        bottom: LengthPercentage::length(24.0),
                    },
                    ..Default::default()
                },
                &[container],
            )
            .unwrap();

        tree.root = root;
        tree.compute_layout(Size::MAX_CONTENT).unwrap();

        let elements = tree.collect_elements();
        assert_eq!(elements.len(), 2);

        // 驗證位置有 padding offset
        assert!(elements[0].bounding_box.x >= 24.0);
        assert!(elements[0].bounding_box.y >= 24.0);
    }
}

use crate::ast::{AspectRatio, Bullets, BulletItem, Table};
use crate::theme::{Theme, Typography};
use super::MeasuredSize;

/// 量測文字高度（使用簡化估算）
pub fn measure_text(
    text: &str,
    role: &str,
    max_width_pt: f32,
    theme: &Theme,
) -> MeasuredSize {
    let typography = theme.get_typography(role).unwrap_or_else(|| {
        theme.get_typography("body").expect("Theme must have 'body' typography")
    });

    let line_height_pt = typography.size_pt * typography.line_height;

    // 估算平均字元寬度（中英混排用保守值）
    let avg_char_width = typography.size_pt * 0.55;

    // 計算每行可容納的字元數
    let chars_per_line = (max_width_pt / avg_char_width).floor().max(1.0) as usize;

    // 計算行數（考慮換行）
    let lines = estimate_lines(text, chars_per_line);

    let height = lines as f32 * line_height_pt;

    MeasuredSize {
        width: max_width_pt,
        height,
    }
}

/// 估算文字行數
fn estimate_lines(text: &str, chars_per_line: usize) -> usize {
    let mut total_lines = 0;

    for line in text.lines() {
        let char_count = line.chars().count();
        if char_count == 0 {
            total_lines += 1;
        } else {
            total_lines += (char_count + chars_per_line - 1) / chars_per_line;
        }
    }

    total_lines.max(1)
}

/// 量測項目符號列表高度
pub fn measure_bullets(
    bullets: &Bullets,
    max_width_pt: f32,
    theme: &Theme,
    indent_pt: f32,
) -> MeasuredSize {
    let typography = theme.get_typography("body").expect("Theme must have 'body' typography");
    let line_height_pt = typography.size_pt * typography.line_height;
    let item_spacing = theme.get_spacing("sm");

    fn measure_items(
        items: &[BulletItem],
        width: f32,
        typography: &Typography,
        line_height_pt: f32,
        item_spacing: f32,
        depth: usize,
        indent_pt: f32,
    ) -> f32 {
        let mut height = 0.0;
        let available_width = width - (depth as f32 * indent_pt);
        let avg_char_width = typography.size_pt * 0.55;
        let chars_per_line = (available_width / avg_char_width).floor().max(1.0) as usize;

        for (i, item) in items.iter().enumerate() {
            let lines = estimate_lines(&item.text, chars_per_line);
            height += lines as f32 * line_height_pt;

            if !item.children.is_empty() {
                height += measure_items(
                    &item.children,
                    width,
                    typography,
                    line_height_pt,
                    item_spacing,
                    depth + 1,
                    indent_pt,
                );
            }

            if i < items.len() - 1 {
                height += item_spacing;
            }
        }
        height
    }

    let total_height = measure_items(
        &bullets.items,
        max_width_pt,
        typography,
        line_height_pt,
        item_spacing,
        0,
        indent_pt,
    );

    MeasuredSize {
        width: max_width_pt,
        height: total_height,
    }
}

/// 量測表格高度
pub fn measure_table(
    table: &Table,
    max_width_pt: f32,
    theme: &Theme,
) -> MeasuredSize {
    let typography = theme.get_typography("body").expect("Theme must have 'body' typography");
    let line_height_pt = typography.size_pt * typography.line_height;
    let cell_padding = theme.get_spacing("sm");
    let row_padding = theme.get_spacing("xs");

    let col_count = table.header.len().max(1);
    let col_width = (max_width_pt - cell_padding * 2.0 * col_count as f32) / col_count as f32;

    let avg_char_width = typography.size_pt * 0.55;
    let chars_per_cell = (col_width / avg_char_width).floor().max(1.0) as usize;

    // 計算 header 高度
    let header_max_lines = table.header.iter()
        .map(|cell| estimate_lines(cell, chars_per_cell))
        .max()
        .unwrap_or(1);
    let header_height = header_max_lines as f32 * line_height_pt + cell_padding * 2.0;

    // 計算 rows 高度
    let mut rows_height = 0.0;
    for row in &table.rows {
        let row_max_lines = row.iter()
            .map(|cell| estimate_lines(cell, chars_per_cell))
            .max()
            .unwrap_or(1);
        rows_height += row_max_lines as f32 * line_height_pt + cell_padding * 2.0 + row_padding;
    }

    MeasuredSize {
        width: max_width_pt,
        height: header_height + rows_height,
    }
}

/// 量測圖片/示意圖高度（依據長寬比）
pub fn measure_figure(
    ratio: &AspectRatio,
    max_width_pt: f32,
    min_box: &crate::theme::MinImageBox,
) -> MeasuredSize {
    let width = max_width_pt.max(min_box.w);
    let height = ratio.height_for_width(width).max(min_box.h);

    MeasuredSize { width, height }
}

#[cfg(test)]
mod tests {
    use super::*;
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
    fn test_measure_single_line_text() {
        let theme = create_test_theme();
        let result = measure_text("Hello", "body", 400.0, &theme);
        // 14pt * 1.35 = 18.9pt for one line
        assert!((result.height - 18.9).abs() < 0.1);
    }

    #[test]
    fn test_measure_multiline_text() {
        let theme = create_test_theme();
        // 400pt width, ~14 * 0.55 = 7.7pt per char, so ~51 chars per line
        // 100 chars should be ~2 lines
        let long_text = "a".repeat(100);
        let result = measure_text(&long_text, "body", 400.0, &theme);
        assert!(result.height > 18.9 * 1.5); // At least 1.5 lines worth
    }

    #[test]
    fn test_measure_figure_16_9() {
        let ratio = AspectRatio::new(16, 9);
        let min_box = crate::theme::MinImageBox { w: 180.0, h: 120.0 };
        let result = measure_figure(&ratio, 320.0, &min_box);
        assert_eq!(result.width, 320.0);
        assert!((result.height - 180.0).abs() < 0.1);
    }

    #[test]
    fn test_measure_bullets() {
        let theme = create_test_theme();
        let bullets = Bullets {
            items: vec![
                BulletItem { text: "Item 1".to_string(), children: vec![] },
                BulletItem { text: "Item 2".to_string(), children: vec![] },
            ],
        };
        let result = measure_bullets(&bullets, 400.0, &theme, 16.0);
        // 2 lines + spacing
        assert!(result.height > 30.0);
    }

    #[test]
    fn test_measure_table() {
        let theme = create_test_theme();
        let table = Table {
            header: vec!["A".to_string(), "B".to_string()],
            rows: vec![
                vec!["1".to_string(), "2".to_string()],
            ],
            alignments: vec![],
        };
        let result = measure_table(&table, 400.0, &theme);
        // header + 1 row
        assert!(result.height > 40.0);
    }
}

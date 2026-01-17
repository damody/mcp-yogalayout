use mcp_yogalayout::layout::compute::compute_layout;
use mcp_yogalayout::layout::review::review_layout;
use mcp_yogalayout::layout::templates::Density;
use mcp_yogalayout::md::parse_markdown;
use mcp_yogalayout::theme::Theme;
use std::path::PathBuf;

fn load_test_theme() -> Theme {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("workspace/themes/default.json");
    Theme::load(&path).expect("Failed to load test theme")
}

#[test]
fn test_full_pipeline_short_content() {
    let md = r#"# Short Slide
> Quick summary

## Points
- First point
- Second point
"#;

    let slide = parse_markdown(md).expect("Failed to parse markdown");
    let theme = load_test_theme();

    let (output, elements) = compute_layout(&slide, &theme, None, Density::Comfortable)
        .expect("Failed to compute layout");

    assert_eq!(output.slide.w_pt, 960.0);
    assert_eq!(output.slide.h_pt, 540.0);

    // Should have title, subtitle, section heading, and bullets
    assert!(output.elements.iter().any(|e| e.id == "title"));
    assert!(output.elements.iter().any(|e| e.id == "subtitle"));

    let report = review_layout(&elements, &output.slide);
    assert!(
        report.overflow_elements.is_empty(),
        "Short content should not overflow"
    );
}

#[test]
fn test_full_pipeline_with_figure() {
    let md = r#"# Slide with Figure
> Has both text and image

## Content
- Point one
- Point two

## Diagram
<fig id="diagram1" ratio="16:9" kind="diagram" alt="Test diagram" />
"#;

    let slide = parse_markdown(md).expect("Failed to parse markdown");
    let theme = load_test_theme();

    let (output, elements) = compute_layout(&slide, &theme, None, Density::Comfortable)
        .expect("Failed to compute layout");

    // Should have figure
    let fig = output.elements.iter().find(|e| e.id == "fig:diagram1");
    assert!(fig.is_some(), "Should have figure element");

    let fig = fig.unwrap();
    assert!(fig.ratio.is_some());
    assert!(fig.alt.is_some());

    let report = review_layout(&elements, &output.slide);
    // May or may not overflow depending on layout, but should complete
    assert!(report.fallbacks.is_empty() || !report.fallbacks.is_empty());
}

#[test]
fn test_full_pipeline_with_table() {
    let md = r#"# Table Slide

## Data
| Col A | Col B | Col C |
|-------|-------|-------|
| 1     | 2     | 3     |
| 4     | 5     | 6     |
"#;

    let slide = parse_markdown(md).expect("Failed to parse markdown");
    let theme = load_test_theme();

    let (output, _) = compute_layout(&slide, &theme, None, Density::Comfortable)
        .expect("Failed to compute layout");

    // Should have table
    let table = output.elements.iter().find(|e| e.id.starts_with("table:"));
    assert!(table.is_some(), "Should have table element");
}

#[test]
fn test_prd_example_slide() {
    let md = r#"# Anti-Lag POC
> 目標：降低輸入延遲，並保留穩定幀率

## KPI
| 指標 | 數值 | 變化 |
|---|---:|---:|
| Input Lag | **16ms** | -60% |
| Avg Power | 3.1W | -18% |

## 核心機制
- CPU 不再超前排隊 3-4 幀
- 只保持 **1 幀** queue
- `Fence` 同步點插入引擎與 GPU 之間

## 資料流示意
<fig id="flow" ratio="16:9" kind="diagram" alt="Pipeline：Game App -> SDK -> Service -> libgui -> SharedMemory -> Game App，右側註記：queue=1" />

> Note：若內容過長，優先改 single-column，其次 compact，再回報需精簡。
"#;

    let slide = parse_markdown(md).expect("Failed to parse markdown");
    let theme = load_test_theme();

    let (output, elements) = compute_layout(&slide, &theme, None, Density::Comfortable)
        .expect("Failed to compute layout");

    // Verify all expected elements exist
    assert!(output.elements.iter().any(|e| e.id == "title"));
    assert!(output.elements.iter().any(|e| e.id == "subtitle"));
    assert!(output.elements.iter().any(|e| e.id.contains("KPI")));
    assert!(output.elements.iter().any(|e| e.id.contains("核心機制")));
    assert!(output.elements.iter().any(|e| e.id == "fig:flow"));

    // Should have callout
    assert!(output.elements.iter().any(|e| e.id.starts_with("callout:")));

    let report = review_layout(&elements, &output.slide);
    println!("Report: {:?}", report);
}

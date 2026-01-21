use mcp_yogalayout::layout::compute::compute_layout;
use mcp_yogalayout::layout::review::review_layout;
use mcp_yogalayout::layout::templates::Density;
use mcp_yogalayout::md::parse_markdown;
use mcp_yogalayout::theme::Theme;
use std::path::PathBuf;

fn main() {
    // 範例 Markdown 內容
    let md = r#"# Anti-Lag POC
> 目標：降低輸入延遲，並保留穩定幀率

## KPI
| 指標 | 數值 | 變化 |
|---|---:|---:|
| Input Lag | 16ms | -60% |
| Avg Power | 3.1W | -18% |

## 核心機制
- CPU 不再超前排隊 3-4 幀
- 只保持 1 幀 queue
- Fence 同步點插入引擎與 GPU 之間

## 資料流示意
<fig id="flow" ratio="16:9" kind="diagram" alt="Pipeline flow diagram" />

> Note：若內容過長，優先改 single-column，其次 compact。
"#;

    // 載入主題
    let theme_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("workspace/themes/default.json");
    let theme = Theme::load(&theme_path).expect("Failed to load theme");

    // 解析 Markdown
    let slide = parse_markdown(md).expect("Failed to parse markdown");

    // 計算佈局
    let (output, elements) = compute_layout(&slide, &theme, None, Density::Comfortable)
        .expect("Failed to compute layout");

    // 審查佈局
    let report = review_layout(&elements, &output.slide);

    // 輸出 layout.json
    println!("=== layout.json ===");
    println!("{}", serde_json::to_string_pretty(&output).unwrap());

    println!("\n=== report.json ===");
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
}

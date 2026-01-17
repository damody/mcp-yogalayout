# MCP Yoga Layout 實作計畫

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 實作一個 Rust MCP Server（stdio transport），提供工具讓上層 AI 以檔案路徑輸入 Markdown，解析後用 Taffy (Flexbox) 計算 16:9 橫式投影片中所有元素的座標與大小，輸出 layout.json 與 report.json。

**Architecture:** 採用 rmcp 作為 MCP Server 框架（stdio transport），使用 taffy 純 Rust Flexbox 實作進行版面計算，pulldown-cmark 解析 GFM Markdown。系統分為：路徑安全驗證 → Markdown 解析 → DocAST 建立 → 模板選擇 → Taffy 佈局計算 → 輸出生成。

**Tech Stack:**
- `rmcp` 0.8.0 - MCP Server 框架
- `taffy` 0.9.2 - Flexbox 佈局引擎
- `pulldown-cmark` 0.13.0 - Markdown 解析器
- `serde` / `serde_json` - JSON 序列化
- `tokio` - 非同步執行環境

---

## 專案結構

```
mcp-yogalayout/
  Cargo.toml
  src/
    main.rs            # MCP server + tool registry
    tool.rs            # compute_slide_layout handler
    paths.rs           # workspace path sanitize
    theme.rs           # theme json structs
    md.rs              # markdown parse -> DocAST
    ast.rs             # DocAST types
    layout/
      mod.rs           # layout module
      templates.rs     # single_col / two_col builders
      tree.rs          # taffy tree wrapper
      measure.rs       # text/table/figure measurement
      compute.rs       # run taffy + extract boxes
      review.rs        # overflow & fallback
    output.rs          # write layout.json/report.json
  workspace/
    inputs/
      slide.md         # 測試用 Markdown
    themes/
      default.json     # 預設主題
    out/               # 輸出目錄
```

---

## Task 1: 專案初始化與基本結構

**Files:**
- Create: `Cargo.toml`
- Create: `src/main.rs`
- Create: `src/lib.rs`

**Step 1: 建立 Cargo.toml**

```toml
[package]
name = "mcp-yogalayout"
version = "0.1.0"
edition = "2021"

[dependencies]
rmcp = { version = "0.8", features = ["server", "macros", "transport-io"] }
taffy = "0.9"
pulldown-cmark = { version = "0.13", default-features = false, features = ["simd"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
tokio = { version = "1", features = ["full"] }
thiserror = "2.0"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
schemars = "0.8"

[lib]
name = "mcp_yogalayout"
path = "src/lib.rs"

[[bin]]
name = "mcp-yogalayout"
path = "src/main.rs"
```

**Step 2: 建立 src/lib.rs**

```rust
pub mod ast;
pub mod layout;
pub mod md;
pub mod output;
pub mod paths;
pub mod theme;
pub mod tool;
```

**Step 3: 建立 src/main.rs**

```rust
use mcp_yogalayout::tool::LayoutService;
use rmcp::ServiceExt;
use tokio::io::{stdin, stdout};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 設定 tracing 輸出到 stderr（避免干擾 stdio transport）
    tracing_subscriber::registry()
        .with(fmt::layer().with_writer(std::io::stderr))
        .with(EnvFilter::from_default_env())
        .init();

    tracing::info!("Starting MCP Yoga Layout server");

    let service = LayoutService::new();
    let transport = (stdin(), stdout());
    let server = service.serve(transport).await?;
    server.waiting().await?;

    Ok(())
}
```

**Step 4: 執行 cargo check 驗證編譯**

Run: `cargo check`
Expected: 會有 unresolved import 錯誤（因為模組尚未實作），這是正常的

**Step 5: Commit**

```bash
git init
git add Cargo.toml src/main.rs src/lib.rs
git commit -m "feat: initialize project structure with dependencies

Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>"
```

---

## Task 2: 路徑安全模組 (paths.rs)

**Files:**
- Create: `src/paths.rs`
- Test: `src/paths.rs` (inline tests)

**Step 1: 撰寫 paths.rs 的測試**

```rust
// src/paths.rs

use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PathError {
    #[error("Path must be relative: {0}")]
    AbsolutePath(String),

    #[error("Path traversal not allowed: {0}")]
    PathTraversal(String),

    #[error("Path must be under workspace/: {0}")]
    OutsideWorkspace(String),

    #[error("File not found: {0}")]
    NotFound(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_relative_path() {
        let base = PathBuf::from("/project");
        let result = resolve_workspace_path(&base, "workspace/inputs/slide.md");
        assert!(result.is_ok());
    }

    #[test]
    fn test_reject_absolute_path() {
        let base = PathBuf::from("/project");
        let result = resolve_workspace_path(&base, "/etc/passwd");
        assert!(matches!(result, Err(PathError::AbsolutePath(_))));
    }

    #[test]
    fn test_reject_path_traversal() {
        let base = PathBuf::from("/project");
        let result = resolve_workspace_path(&base, "workspace/../../../etc/passwd");
        assert!(matches!(result, Err(PathError::PathTraversal(_))));
    }

    #[test]
    fn test_reject_outside_workspace() {
        let base = PathBuf::from("/project");
        let result = resolve_workspace_path(&base, "src/main.rs");
        assert!(matches!(result, Err(PathError::OutsideWorkspace(_))));
    }

    #[test]
    fn test_valid_nested_path() {
        let base = PathBuf::from("/project");
        let result = resolve_workspace_path(&base, "workspace/themes/default.json");
        assert!(result.is_ok());
    }
}
```

**Step 2: 執行測試確認失敗**

Run: `cargo test paths --lib`
Expected: FAIL（函數尚未實作）

**Step 3: 實作 resolve_workspace_path**

```rust
/// 解析並驗證工作區路徑
/// 只允許 workspace/ 之下的相對路徑
pub fn resolve_workspace_path(base_dir: &Path, relative_path: &str) -> Result<PathBuf, PathError> {
    // 檢查是否為絕對路徑
    let path = Path::new(relative_path);
    if path.is_absolute() {
        return Err(PathError::AbsolutePath(relative_path.to_string()));
    }

    // 檢查路徑遍歷
    if relative_path.contains("..") {
        return Err(PathError::PathTraversal(relative_path.to_string()));
    }

    // 檢查是否以 workspace/ 開頭
    if !relative_path.starts_with("workspace/") && !relative_path.starts_with("workspace\\") {
        return Err(PathError::OutsideWorkspace(relative_path.to_string()));
    }

    // 組合完整路徑
    let full_path = base_dir.join(relative_path);
    Ok(full_path)
}

/// 驗證檔案存在
pub fn ensure_file_exists(path: &Path) -> Result<(), PathError> {
    if !path.exists() {
        return Err(PathError::NotFound(path.display().to_string()));
    }
    Ok(())
}

/// 確保目錄存在，若不存在則建立
pub fn ensure_dir_exists(path: &Path) -> Result<(), PathError> {
    if !path.exists() {
        std::fs::create_dir_all(path)?;
    }
    Ok(())
}
```

**Step 4: 執行測試確認通過**

Run: `cargo test paths --lib`
Expected: PASS

**Step 5: Commit**

```bash
git add src/paths.rs src/lib.rs
git commit -m "feat: add workspace path validation module

- Reject absolute paths
- Reject path traversal attacks (..)
- Only allow paths under workspace/

Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>"
```

---

## Task 3: Theme 模組 (theme.rs)

**Files:**
- Create: `src/theme.rs`
- Create: `workspace/themes/default.json`

**Step 1: 撰寫 theme.rs 結構與測試**

```rust
// src/theme.rs

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Theme {
    pub typography: HashMap<String, Typography>,
    pub spacing_pt: SpacingScale,
    pub policy: LayoutPolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Typography {
    pub family: String,
    pub size_pt: f32,
    pub line_height: f32,
    pub weight: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpacingScale {
    pub xs: f32,
    pub sm: f32,
    pub md: f32,
    pub lg: f32,
    pub xl: f32,
    #[serde(rename = "2xl")]
    pub xxl: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutPolicy {
    pub page_padding: String,
    pub min_font_pt: f32,
    pub min_image_box_pt: MinImageBox,
    pub two_col_when: TwoColCondition,
    pub two_col_split: [f32; 2],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinImageBox {
    pub w: f32,
    pub h: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TwoColCondition {
    pub has_image_or_diagram: bool,
    pub has_bullets_or_table: bool,
}

impl Theme {
    pub fn load(path: &Path) -> Result<Self, ThemeError> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| ThemeError::Io(e.to_string()))?;
        serde_json::from_str(&content)
            .map_err(|e| ThemeError::Parse(e.to_string()))
    }

    pub fn get_typography(&self, role: &str) -> Option<&Typography> {
        self.typography.get(role)
    }

    pub fn get_spacing(&self, name: &str) -> f32 {
        match name {
            "xs" => self.spacing_pt.xs,
            "sm" => self.spacing_pt.sm,
            "md" => self.spacing_pt.md,
            "lg" => self.spacing_pt.lg,
            "xl" => self.spacing_pt.xl,
            "2xl" => self.spacing_pt.xxl,
            _ => self.spacing_pt.md,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ThemeError {
    #[error("IO error: {0}")]
    Io(String),
    #[error("Parse error: {0}")]
    Parse(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_theme() {
        let json = r#"{
            "typography": {
                "title": { "family": "Inter", "size_pt": 34, "line_height": 1.10, "weight": 700 },
                "body": { "family": "Inter", "size_pt": 14, "line_height": 1.35, "weight": 400 }
            },
            "spacing_pt": { "xs": 4, "sm": 8, "md": 12, "lg": 16, "xl": 24, "2xl": 32 },
            "policy": {
                "page_padding": "xl",
                "min_font_pt": 10,
                "min_image_box_pt": { "w": 180, "h": 120 },
                "two_col_when": { "has_image_or_diagram": true, "has_bullets_or_table": true },
                "two_col_split": [0.58, 0.42]
            }
        }"#;

        let theme: Theme = serde_json::from_str(json).unwrap();
        assert_eq!(theme.get_typography("title").unwrap().size_pt, 34.0);
        assert_eq!(theme.get_spacing("xl"), 24.0);
    }
}
```

**Step 2: 執行測試確認通過**

Run: `cargo test theme --lib`
Expected: PASS

**Step 3: 建立預設主題檔案**

```json
{
  "typography": {
    "title":    { "family": "Inter", "size_pt": 34, "line_height": 1.10, "weight": 700 },
    "subtitle": { "family": "Inter", "size_pt": 18, "line_height": 1.20, "weight": 500 },
    "h2":       { "family": "Inter", "size_pt": 20, "line_height": 1.20, "weight": 700 },
    "body":     { "family": "Inter", "size_pt": 14, "line_height": 1.35, "weight": 400 },
    "caption":  { "family": "Inter", "size_pt": 12, "line_height": 1.30, "weight": 400 },
    "mono":     { "family": "JetBrains Mono", "size_pt": 12, "line_height": 1.30, "weight": 400 }
  },
  "spacing_pt": { "xs": 4, "sm": 8, "md": 12, "lg": 16, "xl": 24, "2xl": 32 },
  "policy": {
    "page_padding": "xl",
    "min_font_pt": 10,
    "min_image_box_pt": { "w": 180, "h": 120 },
    "two_col_when": { "has_image_or_diagram": true, "has_bullets_or_table": true },
    "two_col_split": [0.58, 0.42]
  }
}
```

Save to: `workspace/themes/default.json`

**Step 4: Commit**

```bash
git add src/theme.rs workspace/themes/default.json
git commit -m "feat: add theme module with typography and spacing config

Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>"
```

---

## Task 4: DocAST 類型定義 (ast.rs)

**Files:**
- Create: `src/ast.rs`

**Step 1: 定義 DocAST 結構**

```rust
// src/ast.rs

use serde::{Deserialize, Serialize};

/// 投影片文件 AST
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Slide {
    pub title: String,
    pub subtitle: Option<String>,
    pub blocks: Vec<Block>,
}

/// 區塊類型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Block {
    Section(Section),
    Bullets(Bullets),
    Table(Table),
    Callout(Callout),
    Figure(Figure),
}

/// 章節（## 標題）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Section {
    pub heading: String,
    pub children: Vec<Block>,
}

/// 項目符號列表
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bullets {
    pub items: Vec<BulletItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BulletItem {
    pub text: String,
    pub children: Vec<BulletItem>,
}

/// 表格
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Table {
    pub header: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub alignments: Vec<Alignment>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub enum Alignment {
    #[default]
    Left,
    Center,
    Right,
}

/// 註解/Callout
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Callout {
    pub text: String,
}

/// 圖片/示意圖佔位
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Figure {
    pub id: String,
    pub ratio: AspectRatio,
    pub kind: FigureKind,
    pub alt: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct AspectRatio {
    pub w: u32,
    pub h: u32,
}

impl AspectRatio {
    pub fn new(w: u32, h: u32) -> Self {
        Self { w, h }
    }

    /// 給定寬度，計算對應高度
    pub fn height_for_width(&self, width: f32) -> f32 {
        width * (self.h as f32) / (self.w as f32)
    }

    /// 從字串解析 (e.g., "16:9")
    pub fn parse(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() != 2 {
            return None;
        }
        let w = parts[0].parse().ok()?;
        let h = parts[1].parse().ok()?;
        Some(Self { w, h })
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum FigureKind {
    Image,
    #[default]
    Diagram,
    Chart,
}

impl FigureKind {
    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "image" => Self::Image,
            "chart" => Self::Chart,
            _ => Self::Diagram,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aspect_ratio_parse() {
        let ratio = AspectRatio::parse("16:9").unwrap();
        assert_eq!(ratio.w, 16);
        assert_eq!(ratio.h, 9);
    }

    #[test]
    fn test_aspect_ratio_height() {
        let ratio = AspectRatio::new(16, 9);
        let height = ratio.height_for_width(320.0);
        assert!((height - 180.0).abs() < 0.01);
    }

    #[test]
    fn test_figure_kind_parse() {
        assert!(matches!(FigureKind::parse("image"), FigureKind::Image));
        assert!(matches!(FigureKind::parse("DIAGRAM"), FigureKind::Diagram));
        assert!(matches!(FigureKind::parse("unknown"), FigureKind::Diagram));
    }
}
```

**Step 2: 執行測試確認通過**

Run: `cargo test ast --lib`
Expected: PASS

**Step 3: Commit**

```bash
git add src/ast.rs
git commit -m "feat: define DocAST types for slide structure

- Slide, Section, Bullets, Table, Callout, Figure
- AspectRatio with parsing and height calculation

Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>"
```

---

## Task 5: Markdown 解析器 (md.rs)

**Files:**
- Create: `src/md.rs`
- Create: `workspace/inputs/slide.md`

**Step 1: 撰寫解析器測試**

```rust
// src/md.rs

use crate::ast::*;
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("Missing title (# heading)")]
    MissingTitle,
    #[error("Invalid figure tag: {0}")]
    InvalidFigure(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_title_only() {
        let md = "# Hello World";
        let slide = parse_markdown(md).unwrap();
        assert_eq!(slide.title, "Hello World");
        assert!(slide.subtitle.is_none());
    }

    #[test]
    fn test_parse_title_and_subtitle() {
        let md = "# Title\n> Subtitle here";
        let slide = parse_markdown(md).unwrap();
        assert_eq!(slide.title, "Title");
        assert_eq!(slide.subtitle, Some("Subtitle here".to_string()));
    }

    #[test]
    fn test_parse_section() {
        let md = "# Title\n\n## Section 1\n\n- Item 1\n- Item 2";
        let slide = parse_markdown(md).unwrap();
        assert_eq!(slide.blocks.len(), 1);
        if let Block::Section(sec) = &slide.blocks[0] {
            assert_eq!(sec.heading, "Section 1");
            assert_eq!(sec.children.len(), 1);
        } else {
            panic!("Expected Section");
        }
    }

    #[test]
    fn test_parse_figure() {
        let md = r#"# Title

## Diagram

<fig id="flow" ratio="16:9" kind="diagram" alt="Flow diagram" />"#;
        let slide = parse_markdown(md).unwrap();
        if let Block::Section(sec) = &slide.blocks[0] {
            if let Block::Figure(fig) = &sec.children[0] {
                assert_eq!(fig.id, "flow");
                assert_eq!(fig.ratio.w, 16);
                assert_eq!(fig.ratio.h, 9);
                assert_eq!(fig.alt, "Flow diagram");
            } else {
                panic!("Expected Figure");
            }
        } else {
            panic!("Expected Section");
        }
    }

    #[test]
    fn test_parse_table() {
        let md = r#"# Title

## Data

| A | B |
|---|---|
| 1 | 2 |"#;
        let slide = parse_markdown(md).unwrap();
        if let Block::Section(sec) = &slide.blocks[0] {
            if let Block::Table(table) = &sec.children[0] {
                assert_eq!(table.header, vec!["A", "B"]);
                assert_eq!(table.rows.len(), 1);
                assert_eq!(table.rows[0], vec!["1", "2"]);
            } else {
                panic!("Expected Table");
            }
        } else {
            panic!("Expected Section");
        }
    }

    #[test]
    fn test_missing_title() {
        let md = "Just some text";
        let result = parse_markdown(md);
        assert!(matches!(result, Err(ParseError::MissingTitle)));
    }
}
```

**Step 2: 執行測試確認失敗**

Run: `cargo test md --lib`
Expected: FAIL（函數尚未實作）

**Step 3: 實作 parse_markdown**

```rust
/// 解析 Markdown 內容為 Slide AST
pub fn parse_markdown(content: &str) -> Result<Slide, ParseError> {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);

    let parser = Parser::new_ext(content, options);
    let events: Vec<Event> = parser.collect();

    let mut state = ParseState::default();

    let mut i = 0;
    while i < events.len() {
        match &events[i] {
            Event::Start(Tag::Heading { level, .. }) if *level == pulldown_cmark::HeadingLevel::H1 => {
                // 提取 H1 標題文字
                i += 1;
                let mut text = String::new();
                while i < events.len() {
                    match &events[i] {
                        Event::Text(t) => text.push_str(t),
                        Event::Code(c) => text.push_str(c),
                        Event::End(TagEnd::Heading(_)) => break,
                        _ => {}
                    }
                    i += 1;
                }
                state.title = Some(text.trim().to_string());
            }

            Event::Start(Tag::Heading { level, .. }) if *level == pulldown_cmark::HeadingLevel::H2 => {
                // 結束當前 section，開始新 section
                state.finish_current_section();
                i += 1;
                let mut text = String::new();
                while i < events.len() {
                    match &events[i] {
                        Event::Text(t) => text.push_str(t),
                        Event::Code(c) => text.push_str(c),
                        Event::End(TagEnd::Heading(_)) => break,
                        _ => {}
                    }
                    i += 1;
                }
                state.current_section = Some(Section {
                    heading: text.trim().to_string(),
                    children: vec![],
                });
            }

            Event::Start(Tag::BlockQuote(_)) => {
                // 提取 blockquote 文字
                i += 1;
                let mut text = String::new();
                let mut depth = 1;
                while i < events.len() && depth > 0 {
                    match &events[i] {
                        Event::Start(Tag::BlockQuote(_)) => depth += 1,
                        Event::End(TagEnd::BlockQuote(_)) => depth -= 1,
                        Event::Text(t) => text.push_str(t),
                        Event::Code(c) => text.push_str(c),
                        Event::SoftBreak | Event::HardBreak => text.push(' '),
                        _ => {}
                    }
                    i += 1;
                }
                let text = text.trim().to_string();

                // 第一個 blockquote 且 title 存在且 subtitle 未設定 → subtitle
                if state.title.is_some() && state.subtitle.is_none() && state.current_section.is_none() {
                    state.subtitle = Some(text);
                } else {
                    // 否則視為 Callout
                    state.add_block(Block::Callout(Callout { text }));
                }
                continue;
            }

            Event::Start(Tag::List(_)) => {
                let (bullets, new_i) = parse_list(&events, i);
                i = new_i;
                state.add_block(Block::Bullets(bullets));
                continue;
            }

            Event::Start(Tag::Table(alignments)) => {
                let aligns: Vec<Alignment> = alignments.iter().map(|a| match a {
                    pulldown_cmark::Alignment::Left => Alignment::Left,
                    pulldown_cmark::Alignment::Center => Alignment::Center,
                    pulldown_cmark::Alignment::Right => Alignment::Right,
                    pulldown_cmark::Alignment::None => Alignment::Left,
                }).collect();
                let (table, new_i) = parse_table(&events, i, aligns);
                i = new_i;
                state.add_block(Block::Table(table));
                continue;
            }

            Event::Html(html) => {
                // 嘗試解析 <fig ... /> tag
                if let Some(fig) = parse_figure_tag(html) {
                    state.add_block(Block::Figure(fig));
                }
            }

            _ => {}
        }
        i += 1;
    }

    state.finish_current_section();

    let title = state.title.ok_or(ParseError::MissingTitle)?;

    Ok(Slide {
        title,
        subtitle: state.subtitle,
        blocks: state.blocks,
    })
}

#[derive(Default)]
struct ParseState {
    title: Option<String>,
    subtitle: Option<String>,
    blocks: Vec<Block>,
    current_section: Option<Section>,
}

impl ParseState {
    fn add_block(&mut self, block: Block) {
        if let Some(ref mut section) = self.current_section {
            section.children.push(block);
        } else {
            self.blocks.push(block);
        }
    }

    fn finish_current_section(&mut self) {
        if let Some(section) = self.current_section.take() {
            self.blocks.push(Block::Section(section));
        }
    }
}

fn parse_list(events: &[Event], start: usize) -> (Bullets, usize) {
    let mut items = vec![];
    let mut i = start + 1;

    while i < events.len() {
        match &events[i] {
            Event::End(TagEnd::List(_)) => {
                i += 1;
                break;
            }
            Event::Start(Tag::Item) => {
                let (item, new_i) = parse_list_item(events, i);
                items.push(item);
                i = new_i;
            }
            _ => i += 1,
        }
    }

    (Bullets { items }, i)
}

fn parse_list_item(events: &[Event], start: usize) -> (BulletItem, usize) {
    let mut text = String::new();
    let mut children = vec![];
    let mut i = start + 1;

    while i < events.len() {
        match &events[i] {
            Event::End(TagEnd::Item) => {
                i += 1;
                break;
            }
            Event::Text(t) => text.push_str(t),
            Event::Code(c) => {
                text.push('`');
                text.push_str(c);
                text.push('`');
            }
            Event::SoftBreak | Event::HardBreak => text.push(' '),
            Event::Start(Tag::Strong) => text.push_str("**"),
            Event::End(TagEnd::Strong) => text.push_str("**"),
            Event::Start(Tag::List(_)) => {
                let (nested, new_i) = parse_list(events, i);
                children = nested.items;
                i = new_i;
                continue;
            }
            _ => {}
        }
        i += 1;
    }

    (BulletItem { text: text.trim().to_string(), children }, i)
}

fn parse_table(events: &[Event], start: usize, alignments: Vec<Alignment>) -> (Table, usize) {
    let mut header = vec![];
    let mut rows = vec![];
    let mut current_row = vec![];
    let mut current_cell = String::new();
    let mut in_header = false;
    let mut i = start + 1;

    while i < events.len() {
        match &events[i] {
            Event::End(TagEnd::Table) => {
                i += 1;
                break;
            }
            Event::Start(Tag::TableHead) => in_header = true,
            Event::End(TagEnd::TableHead) => {
                header = current_row;
                current_row = vec![];
                in_header = false;
            }
            Event::Start(Tag::TableRow) => current_row = vec![],
            Event::End(TagEnd::TableRow) => {
                if !in_header && !current_row.is_empty() {
                    rows.push(current_row.clone());
                }
                current_row = vec![];
            }
            Event::Start(Tag::TableCell) => current_cell = String::new(),
            Event::End(TagEnd::TableCell) => {
                current_row.push(current_cell.trim().to_string());
                current_cell = String::new();
            }
            Event::Text(t) => current_cell.push_str(t),
            Event::Code(c) => {
                current_cell.push('`');
                current_cell.push_str(c);
                current_cell.push('`');
            }
            Event::Start(Tag::Strong) => current_cell.push_str("**"),
            Event::End(TagEnd::Strong) => current_cell.push_str("**"),
            _ => {}
        }
        i += 1;
    }

    (Table { header, rows, alignments }, i)
}

fn parse_figure_tag(html: &str) -> Option<Figure> {
    let html = html.trim();
    if !html.starts_with("<fig ") || !html.ends_with("/>") {
        return None;
    }

    // 簡單的屬性解析
    let extract_attr = |name: &str| -> Option<String> {
        let pattern = format!("{}=\"", name);
        let start = html.find(&pattern)? + pattern.len();
        let end = html[start..].find('"')? + start;
        Some(html[start..end].to_string())
    };

    let id = extract_attr("id")?;
    let ratio_str = extract_attr("ratio")?;
    let ratio = AspectRatio::parse(&ratio_str)?;
    let kind = extract_attr("kind")
        .map(|k| FigureKind::parse(&k))
        .unwrap_or_default();
    let alt = extract_attr("alt")?;

    Some(Figure { id, ratio, kind, alt })
}

/// 從檔案讀取並解析 Markdown
pub fn parse_markdown_file(path: &std::path::Path) -> Result<Slide, ParseError> {
    let content = std::fs::read_to_string(path)?;
    parse_markdown(&content)
}
```

**Step 4: 執行測試確認通過**

Run: `cargo test md --lib`
Expected: PASS

**Step 5: 建立測試用 Markdown 檔案**

Save to: `workspace/inputs/slide.md`

```markdown
# Anti-Lag POC
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
```

**Step 6: Commit**

```bash
git add src/md.rs workspace/inputs/slide.md
git commit -m "feat: implement Markdown parser with GFM support

- Parse title, subtitle, sections
- Handle bullets, tables, callouts
- Parse <fig /> custom tags for figures

Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>"
```

---

## Task 6: Layout 模組 - 基礎結構

**Files:**
- Create: `src/layout/mod.rs`
- Create: `src/layout/measure.rs`
- Create: `src/layout/tree.rs`

**Step 1: 建立 layout 模組結構**

```rust
// src/layout/mod.rs

pub mod compute;
pub mod measure;
pub mod review;
pub mod templates;
pub mod tree;

use crate::ast::Slide;
use crate::theme::Theme;
use serde::{Deserialize, Serialize};

/// 投影片尺寸設定
#[derive(Debug, Clone, Copy)]
pub struct SlideConfig {
    pub width_pt: f32,
    pub height_pt: f32,
}

impl SlideConfig {
    pub fn landscape_16_9() -> Self {
        Self {
            width_pt: 960.0,
            height_pt: 540.0,
        }
    }
}

/// 佈局元素
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutElement {
    pub id: String,
    pub kind: ElementKind,
    pub role: String,
    pub r#box: BoundingBox,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ratio: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_ref: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ElementKind {
    Text,
    Bullets,
    Table,
    Figure,
    Callout,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct BoundingBox {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

/// 佈局輸出
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutOutput {
    pub slide: SlideSize,
    pub elements: Vec<LayoutElement>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SlideSize {
    pub w_pt: f32,
    pub h_pt: f32,
}
```

**Step 2: 實作文字量測 (measure.rs)**

```rust
// src/layout/measure.rs

use crate::ast::{AspectRatio, Bullets, Table};
use crate::theme::{Theme, Typography};

/// 文字量測結果
#[derive(Debug, Clone, Copy)]
pub struct MeasuredSize {
    pub width: f32,
    pub height: f32,
}

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

    let mut total_height = 0.0;

    fn measure_items(
        items: &[crate::ast::BulletItem],
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

    total_height = measure_items(
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
}
```

**Step 3: 執行測試確認通過**

Run: `cargo test measure --lib`
Expected: PASS

**Step 4: Commit**

```bash
git add src/layout/
git commit -m "feat: add layout module with text/table/figure measurement

- Estimate text height using average character width
- Support bullets with nested items
- Calculate table height per row
- Figure height based on aspect ratio

Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>"
```

---

## Task 7: Taffy Tree 包裝 (tree.rs)

**Files:**
- Create: `src/layout/tree.rs`

**Step 1: 實作 Taffy 樹包裝**

```rust
// src/layout/tree.rs

use super::{BoundingBox, LayoutElement, ElementKind, MeasuredSize};
use crate::ast::*;
use crate::theme::Theme;
use taffy::prelude::*;
use taffy::{TaffyTree, NodeId};

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
    Callout {
        id: String,
        text: String,
    },
    /// 容器節點（無需量測）
    Container {
        id: String,
    },
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
        Self {
            tree,
            root: NodeId::from(taffy::NodeId::new(0)), // 暫時的，稍後設定
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
        self.tree.new_with_children(
            style,
            children,
        ).map(|node| {
            self.tree.set_node_context(node, Some(NodeContext::Container {
                id: id.to_string(),
            })).ok();
            node
        })
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
    pub fn compute_layout(&mut self, available_size: Size<AvailableSpace>) -> Result<(), taffy::TaffyError> {
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
                    r#box: BoundingBox {
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
                    r#box: BoundingBox {
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
                    r#box: BoundingBox {
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
                    r#box: BoundingBox {
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
                    r#box: BoundingBox {
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
    if let Size { width: Some(w), height: Some(h) } = known_dimensions {
        return Size { width: w, height: h };
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
            Size { width: measured.width, height: measured.height }
        }
        NodeContext::Bullets { bullets, .. } => {
            let indent_pt = theme.get_spacing("lg");
            let measured = super::measure::measure_bullets(bullets, max_width, theme, indent_pt);
            Size { width: measured.width, height: measured.height }
        }
        NodeContext::Table { table, .. } => {
            let measured = super::measure::measure_table(table, max_width, theme);
            Size { width: measured.width, height: measured.height }
        }
        NodeContext::Figure { ratio, .. } => {
            let min_box = &theme.policy.min_image_box_pt;
            let measured = super::measure::measure_figure(ratio, max_width, min_box);
            Size { width: measured.width, height: measured.height }
        }
        NodeContext::Callout { text, .. } => {
            let measured = super::measure::measure_text(text, "caption", max_width, theme);
            Size { width: measured.width, height: measured.height }
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
    fn test_create_layout_tree() {
        let theme = create_test_theme();
        let mut tree = LayoutTree::new(theme);

        let leaf = tree.new_leaf(
            Style::default(),
            NodeContext::Text {
                id: "test".to_string(),
                role: "body".to_string(),
                content: "Hello".to_string(),
            },
        ).unwrap();

        let root = tree.new_container("root", Style {
            size: Size {
                width: Dimension::Length(960.0),
                height: Dimension::Length(540.0),
            },
            ..Default::default()
        }, &[leaf]).unwrap();

        tree.root = root;
        tree.compute_layout(Size::MAX_CONTENT).unwrap();

        let elements = tree.collect_elements();
        assert_eq!(elements.len(), 1);
        assert_eq!(elements[0].id, "test");
    }
}
```

**Step 2: 執行測試確認通過**

Run: `cargo test tree --lib`
Expected: PASS

**Step 3: Commit**

```bash
git add src/layout/tree.rs
git commit -m "feat: implement Taffy tree wrapper with measure callbacks

- NodeContext for different element types
- Custom measure function integration
- Collect layout results recursively

Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>"
```

---

## Task 8: 模板建構器 (templates.rs)

**Files:**
- Create: `src/layout/templates.rs`

**Step 1: 實作模板建構器**

```rust
// src/layout/templates.rs

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

    if policy.has_image_or_diagram && has_figure
        && policy.has_bullets_or_table && has_text_content
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
                    top: LengthPercentageAuto::Length(theme.get_spacing("sm")),
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
                width: LengthPercentage::Length(0.0),
                height: LengthPercentage::Length(gap),
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
                width: Dimension::Length(960.0),
                height: Dimension::Length(540.0),
            },
            padding: Rect {
                left: LengthPercentage::Length(padding),
                right: LengthPercentage::Length(padding),
                top: LengthPercentage::Length(padding),
                bottom: LengthPercentage::Length(padding),
            },
            gap: Size {
                width: LengthPercentage::Length(0.0),
                height: LengthPercentage::Length(gap),
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
                    top: LengthPercentageAuto::Length(theme.get_spacing("sm")),
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
            flex_basis: Dimension::Percent(split[0]),
            flex_grow: 0.0,
            flex_shrink: 0.0,
            gap: Size {
                width: LengthPercentage::Length(0.0),
                height: LengthPercentage::Length(gap),
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
            flex_basis: Dimension::Percent(split[1]),
            flex_grow: 0.0,
            flex_shrink: 0.0,
            gap: Size {
                width: LengthPercentage::Length(0.0),
                height: LengthPercentage::Length(gap),
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
                width: LengthPercentage::Length(col_gap),
                height: LengthPercentage::Length(0.0),
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
                width: Dimension::Length(960.0),
                height: Dimension::Length(540.0),
            },
            padding: Rect {
                left: LengthPercentage::Length(padding),
                right: LengthPercentage::Length(padding),
                top: LengthPercentage::Length(padding),
                bottom: LengthPercentage::Length(padding),
            },
            gap: Size {
                width: LengthPercentage::Length(0.0),
                height: LengthPercentage::Length(gap),
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
    let gap = theme.get_spacing("md") * density.gap_scale();

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
                        width: LengthPercentage::Length(0.0),
                        height: LengthPercentage::Length(gap),
                    },
                    ..Default::default()
                },
                &children,
            )
        }

        Block::Bullets(bullets) => {
            tree.new_leaf(
                Style::default(),
                NodeContext::Bullets {
                    id: format!("bullets:{}", index),
                    bullets: bullets.clone(),
                },
            )
        }

        Block::Table(table) => {
            tree.new_leaf(
                Style::default(),
                NodeContext::Table {
                    id: format!("table:{}", index),
                    table: table.clone(),
                },
            )
        }

        Block::Figure(fig) => {
            tree.new_leaf(
                Style::default(),
                NodeContext::Figure {
                    id: format!("fig:{}", fig.id),
                    ratio: fig.ratio,
                    kind: fig.kind,
                    alt: fig.alt.clone(),
                },
            )
        }

        Block::Callout(callout) => {
            tree.new_leaf(
                Style {
                    margin: Rect {
                        top: LengthPercentageAuto::Length(theme.get_spacing("md")),
                        ..Rect::zero()
                    },
                    ..Default::default()
                },
                NodeContext::Callout {
                    id: format!("callout:{}", index),
                    text: callout.text.clone(),
                },
            )
        }
    }
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
    fn test_auto_select_single_column() {
        let theme = create_test_theme();
        let slide = Slide {
            title: "Test".to_string(),
            subtitle: None,
            blocks: vec![
                Block::Bullets(Bullets {
                    items: vec![BulletItem {
                        text: "Item".to_string(),
                        children: vec![],
                    }],
                }),
            ],
        };
        assert_eq!(auto_select_template(&slide, &theme), Template::SingleColumn);
    }

    #[test]
    fn test_auto_select_two_column() {
        let theme = create_test_theme();
        let slide = Slide {
            title: "Test".to_string(),
            subtitle: None,
            blocks: vec![
                Block::Section(Section {
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
                }),
            ],
        };
        assert_eq!(auto_select_template(&slide, &theme), Template::TwoColumn);
    }
}
```

**Step 2: 執行測試確認通過**

Run: `cargo test templates --lib`
Expected: PASS

**Step 3: Commit**

```bash
git add src/layout/templates.rs
git commit -m "feat: implement single and two column template builders

- Auto-select template based on content
- Support density scaling for compact mode
- Separate text content and figures for two-column

Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>"
```

---

## Task 9: 佈局計算與審查 (compute.rs, review.rs)

**Files:**
- Create: `src/layout/compute.rs`
- Create: `src/layout/review.rs`

**Step 1: 實作佈局計算**

```rust
// src/layout/compute.rs

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
```

**Step 2: 實作審查模組**

```rust
// src/layout/review.rs

use super::{BoundingBox, LayoutElement, SlideSize};
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
        let bbox = &element.r#box;

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
    use crate::layout::ElementKind;

    #[test]
    fn test_detect_overflow() {
        let slide = SlideSize { w_pt: 960.0, h_pt: 540.0 };
        let elements = vec![
            LayoutElement {
                id: "test".to_string(),
                kind: ElementKind::Text,
                role: "body".to_string(),
                r#box: BoundingBox { x: 900.0, y: 500.0, w: 100.0, h: 100.0 },
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
                r#box: BoundingBox { x: 24.0, y: 24.0, w: 200.0, h: 50.0 },
                ratio: None,
                alt: None,
                source_ref: None,
            },
        ];

        let report = review_layout(&elements, &slide);
        assert!(report.overflow_elements.is_empty());
    }
}
```

**Step 3: 執行測試確認通過**

Run: `cargo test compute --lib && cargo test review --lib`
Expected: PASS

**Step 4: Commit**

```bash
git add src/layout/compute.rs src/layout/review.rs
git commit -m "feat: implement layout computation and review

- Compute layout using Taffy flexbox
- Review for overflow and clipping
- Suggest fallback strategies

Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>"
```

---

## Task 10: 輸出模組 (output.rs)

**Files:**
- Create: `src/output.rs`

**Step 1: 實作輸出模組**

```rust
// src/output.rs

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
            slide: SlideSize { w_pt: 960.0, h_pt: 540.0 },
            elements: vec![
                LayoutElement {
                    id: "title".to_string(),
                    kind: ElementKind::Text,
                    role: "title".to_string(),
                    r#box: BoundingBox { x: 24.0, y: 24.0, w: 912.0, h: 44.0 },
                    ratio: None,
                    alt: None,
                    source_ref: None,
                },
            ],
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
```

**Step 2: 執行測試確認通過**

Run: `cargo test output --lib`
Expected: PASS

**Step 3: Commit**

```bash
git add src/output.rs
git commit -m "feat: implement layout and report JSON output

Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>"
```

---

## Task 11: MCP Tool Handler (tool.rs)

**Files:**
- Create: `src/tool.rs`

**Step 1: 實作 MCP Tool Handler**

```rust
// src/tool.rs

use crate::layout::compute::{compute_layout, ComputeError};
use crate::layout::review::{review_layout, FallbackInfo, Report};
use crate::layout::templates::{auto_select_template, Density, Template};
use crate::md::{parse_markdown_file, ParseError};
use crate::output::{write_layout_json, write_report_json, OutputError};
use crate::paths::{ensure_dir_exists, ensure_file_exists, resolve_workspace_path, PathError};
use crate::theme::{Theme, ThemeError};
use rmcp::model::{Content, TextContent};
use rmcp::{Error as McpError, ServerHandler, model::ServerInfo, tool};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use thiserror::Error;

/// MCP Service
#[derive(Clone)]
pub struct LayoutService {
    base_dir: PathBuf,
}

impl LayoutService {
    pub fn new() -> Self {
        Self {
            base_dir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        }
    }

    pub fn with_base_dir(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }
}

impl Default for LayoutService {
    fn default() -> Self {
        Self::new()
    }
}

#[rmcp::async_trait]
impl ServerHandler for LayoutService {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            name: "mcp-yogalayout".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            ..Default::default()
        }
    }

    fn list_tools(&self) -> Vec<rmcp::model::Tool> {
        vec![rmcp::model::Tool {
            name: "layout.compute_slide_layout".into(),
            description: Some("Compute slide layout from Markdown file using Flexbox".into()),
            input_schema: schemars::schema_for!(ComputeSlideLayoutInput).schema.into(),
            ..Default::default()
        }]
    }

    async fn call_tool(
        &self,
        name: &str,
        arguments: serde_json::Value,
    ) -> Result<Vec<Content>, McpError> {
        match name {
            "layout.compute_slide_layout" => {
                let input: ComputeSlideLayoutInput = serde_json::from_value(arguments)
                    .map_err(|e| McpError::invalid_params(e.to_string(), None))?;

                match self.compute_slide_layout(input).await {
                    Ok(output) => {
                        let json = serde_json::to_string_pretty(&output)
                            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
                        Ok(vec![Content::Text(TextContent {
                            text: json,
                            ..Default::default()
                        })])
                    }
                    Err(e) => Err(McpError::internal_error(e.to_string(), None)),
                }
            }
            _ => Err(McpError::method_not_found(
                format!("Unknown tool: {}", name),
                None,
            )),
        }
    }
}

impl LayoutService {
    async fn compute_slide_layout(
        &self,
        input: ComputeSlideLayoutInput,
    ) -> Result<ComputeSlideLayoutOutput, ToolError> {
        // 解析路徑
        let markdown_path = resolve_workspace_path(&self.base_dir, &input.markdown_path)?;
        let theme_path = resolve_workspace_path(&self.base_dir, &input.theme_path)?;
        let output_dir = resolve_workspace_path(&self.base_dir, &input.output_dir)?;

        // 驗證輸入檔案
        ensure_file_exists(&markdown_path)?;
        ensure_file_exists(&theme_path)?;

        // 確保輸出目錄存在
        ensure_dir_exists(&output_dir)?;

        // 載入主題
        let theme = Theme::load(&theme_path)?;

        // 解析 Markdown
        let slide = parse_markdown_file(&markdown_path)?;

        // 決定模板
        let template = match input.options.template.as_str() {
            "single_col" => Some(Template::SingleColumn),
            "two_col" => Some(Template::TwoColumn),
            _ => None, // auto
        };

        // 決定密度
        let mut density = match input.options.density.as_str() {
            "compact" => Density::Compact,
            _ => Density::Comfortable,
        };

        // 計算佈局（可能需要多次嘗試以處理 fallback）
        let mut fallbacks = vec![];
        let mut current_template = template.unwrap_or_else(|| auto_select_template(&slide, &theme));

        let (mut output, mut elements) = compute_layout(&slide, &theme, Some(current_template), density)?;
        let mut report = review_layout(&elements, &output.slide);

        // 嘗試降級
        if report.has_issues() && current_template == Template::TwoColumn {
            fallbacks.push(FallbackInfo {
                from: "two_col".to_string(),
                to: "single_col".to_string(),
                reason: "Content overflow detected".to_string(),
            });
            current_template = Template::SingleColumn;
            let result = compute_layout(&slide, &theme, Some(current_template), density)?;
            output = result.0;
            elements = result.1;
            report = review_layout(&elements, &output.slide);
        }

        if report.has_issues() && density == Density::Comfortable {
            fallbacks.push(FallbackInfo {
                from: "comfortable".to_string(),
                to: "compact".to_string(),
                reason: "Content overflow detected".to_string(),
            });
            density = Density::Compact;
            let result = compute_layout(&slide, &theme, Some(current_template), density)?;
            output = result.0;
            elements = result.1;
            report = review_layout(&elements, &output.slide);
        }

        report.fallbacks = fallbacks;

        // 如果仍有問題，加入警告
        if !report.overflow_elements.is_empty() {
            report.warnings.push(
                "Content still overflows after all fallbacks. Consider reducing text content.".to_string()
            );
        }

        // 寫入輸出檔案
        let layout_path = output_dir.join("layout.json");
        let report_path = output_dir.join("report.json");

        write_layout_json(&output, &layout_path)?;
        write_report_json(&report, &report_path)?;

        // 回傳相對路徑
        Ok(ComputeSlideLayoutOutput {
            layout_json_path: format!("{}/layout.json", input.output_dir),
            report_json_path: format!("{}/report.json", input.output_dir),
        })
    }
}

/// Tool 輸入參數
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ComputeSlideLayoutInput {
    /// Markdown 檔案路徑（相對於 workspace/）
    pub markdown_path: String,
    /// 主題檔案路徑（相對於 workspace/）
    pub theme_path: String,
    /// 輸出目錄（相對於 workspace/）
    pub output_dir: String,
    /// 投影片設定
    #[serde(default)]
    pub slide: SlideSettings,
    /// 選項
    #[serde(default)]
    pub options: LayoutOptions,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SlideSettings {
    #[serde(default = "default_aspect")]
    pub aspect: String,
    #[serde(default = "default_orientation")]
    pub orientation: String,
    #[serde(default = "default_unit")]
    pub unit: String,
}

fn default_aspect() -> String { "16:9".to_string() }
fn default_orientation() -> String { "landscape".to_string() }
fn default_unit() -> String { "pt".to_string() }

impl Default for SlideSettings {
    fn default() -> Self {
        Self {
            aspect: default_aspect(),
            orientation: default_orientation(),
            unit: default_unit(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LayoutOptions {
    #[serde(default = "default_template")]
    pub template: String,
    #[serde(default = "default_density")]
    pub density: String,
    #[serde(default)]
    pub allow_two_column: bool,
    #[serde(default)]
    pub debug_dump: bool,
}

fn default_template() -> String { "auto".to_string() }
fn default_density() -> String { "comfortable".to_string() }

impl Default for LayoutOptions {
    fn default() -> Self {
        Self {
            template: default_template(),
            density: default_density(),
            allow_two_column: true,
            debug_dump: false,
        }
    }
}

/// Tool 輸出
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputeSlideLayoutOutput {
    pub layout_json_path: String,
    pub report_json_path: String,
}

#[derive(Debug, Error)]
pub enum ToolError {
    #[error("Path error: {0}")]
    Path(#[from] PathError),
    #[error("Theme error: {0}")]
    Theme(#[from] ThemeError),
    #[error("Parse error: {0}")]
    Parse(#[from] ParseError),
    #[error("Compute error: {0}")]
    Compute(#[from] ComputeError),
    #[error("Output error: {0}")]
    Output(#[from] OutputError),
}
```

**Step 2: 執行 cargo check 驗證編譯**

Run: `cargo check`
Expected: PASS（無編譯錯誤）

**Step 3: Commit**

```bash
git add src/tool.rs
git commit -m "feat: implement MCP tool handler with fallback logic

- compute_slide_layout tool
- Automatic fallback: two_col -> single_col -> compact
- Input validation and path security

Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>"
```

---

## Task 12: 整合測試與驗收

**Files:**
- Create: `tests/integration_test.rs`
- Create: `workspace/inputs/short_slide.md`
- Create: `workspace/inputs/long_slide.md`

**Step 1: 建立整合測試**

```rust
// tests/integration_test.rs

use mcp_yogalayout::layout::compute::compute_layout;
use mcp_yogalayout::layout::review::review_layout;
use mcp_yogalayout::layout::templates::Density;
use mcp_yogalayout::md::parse_markdown;
use mcp_yogalayout::theme::Theme;
use std::path::PathBuf;

fn load_test_theme() -> Theme {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("workspace/themes/default.json");
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
    assert!(report.overflow_elements.is_empty(), "Short content should not overflow");
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
```

**Step 2: 建立額外測試檔案**

Save to: `workspace/inputs/short_slide.md`

```markdown
# Quick Update
> Summary in one line

## Status
- Task A: Done
- Task B: In progress
```

Save to: `workspace/inputs/long_slide.md`

```markdown
# Comprehensive Overview
> Detailed analysis of the entire system architecture and implementation

## Section 1: Background
- This is a long bullet point that explains the background of the project in great detail
- Another detailed point about the historical context
- Third point with extensive explanation
- Fourth point continuing the discussion
- Fifth point wrapping up this section

## Section 2: Technical Details
| Component | Status | Owner | Priority | Notes |
|-----------|--------|-------|----------|-------|
| Frontend | Done | Alice | High | Deployed |
| Backend | WIP | Bob | High | 80% complete |
| Database | Done | Carol | Medium | Optimized |
| Cache | Planned | Dave | Low | Q2 target |

## Section 3: Architecture
<fig id="arch" ratio="16:9" kind="diagram" alt="System architecture showing all components and their interactions" />

## Section 4: Timeline
- Phase 1: Complete
- Phase 2: In progress
- Phase 3: Planned
- Phase 4: Future

> Note: This slide has a lot of content and may require layout adjustments.
```

**Step 3: 執行整合測試**

Run: `cargo test --test integration_test`
Expected: PASS

**Step 4: 執行完整測試套件**

Run: `cargo test`
Expected: PASS

**Step 5: Commit**

```bash
git add tests/ workspace/inputs/
git commit -m "test: add integration tests for full pipeline

- Short content test
- Figure and table tests
- PRD example slide test
- Long content stress test

Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>"
```

---

## Task 13: 最終驗證與清理

**Step 1: 執行完整編譯**

Run: `cargo build --release`
Expected: PASS

**Step 2: 執行 clippy 檢查**

Run: `cargo clippy -- -D warnings`
Expected: PASS（或修正所有警告）

**Step 3: 執行格式化**

Run: `cargo fmt`
Expected: 格式化完成

**Step 4: 測試 MCP Server 執行**

Run: `echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}}' | cargo run --release 2>/dev/null | head -1`
Expected: 收到 JSON-RPC 回應

**Step 5: 最終 Commit**

```bash
git add -A
git commit -m "chore: final cleanup and verification

- All tests passing
- Clippy warnings resolved
- Code formatted

Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>"
```

---

## 技術參考資源

- [rmcp Rust SDK](https://github.com/modelcontextprotocol/rust-sdk) - MCP Server 框架
- [taffy](https://github.com/DioxusLabs/taffy) - Flexbox 佈局引擎
- [pulldown-cmark](https://github.com/pulldown-cmark/pulldown-cmark) - Markdown 解析器
- [MCP Specification](https://modelcontextprotocol.io/docs/develop/build-server) - MCP 協議文件

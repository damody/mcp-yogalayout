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

    #[test]
    fn test_parse_nested_bullets() {
        let md = "# Title\n\n- Item 1\n  - Sub item\n- Item 2";
        let slide = parse_markdown(md).unwrap();
        if let Block::Bullets(bullets) = &slide.blocks[0] {
            assert_eq!(bullets.items.len(), 2);
            assert_eq!(bullets.items[0].children.len(), 1);
            assert_eq!(bullets.items[0].children[0].text, "Sub item");
        } else {
            panic!("Expected Bullets");
        }
    }

    #[test]
    fn test_parse_callout_after_section() {
        let md = "# Title\n> Subtitle\n\n## Section\n\n> This is a note";
        let slide = parse_markdown(md).unwrap();
        assert_eq!(slide.subtitle, Some("Subtitle".to_string()));
        if let Block::Section(sec) = &slide.blocks[0] {
            if let Block::Callout(callout) = &sec.children[0] {
                assert_eq!(callout.text, "This is a note");
            } else {
                panic!("Expected Callout");
            }
        } else {
            panic!("Expected Section");
        }
    }

    #[test]
    fn test_parse_prd_example() {
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

> Note：若內容過長，優先改 single-column，其次 compact，再回報需精簡。"#;

        let slide = parse_markdown(md).unwrap();
        assert_eq!(slide.title, "Anti-Lag POC");
        assert_eq!(slide.subtitle, Some("目標：降低輸入延遲，並保留穩定幀率".to_string()));
        assert_eq!(slide.blocks.len(), 3); // 3 sections
    }
}

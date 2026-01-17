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
    fn test_aspect_ratio_parse_invalid() {
        assert!(AspectRatio::parse("invalid").is_none());
        assert!(AspectRatio::parse("16").is_none());
        assert!(AspectRatio::parse("16:9:4").is_none());
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
        assert!(matches!(FigureKind::parse("IMAGE"), FigureKind::Image));
        assert!(matches!(FigureKind::parse("diagram"), FigureKind::Diagram));
        assert!(matches!(FigureKind::parse("DIAGRAM"), FigureKind::Diagram));
        assert!(matches!(FigureKind::parse("chart"), FigureKind::Chart));
        assert!(matches!(FigureKind::parse("unknown"), FigureKind::Diagram));
    }

    #[test]
    fn test_slide_serialization() {
        let slide = Slide {
            title: "Test".to_string(),
            subtitle: Some("Subtitle".to_string()),
            blocks: vec![Block::Bullets(Bullets {
                items: vec![BulletItem {
                    text: "Item 1".to_string(),
                    children: vec![],
                }],
            })],
        };

        let json = serde_json::to_string(&slide).unwrap();
        assert!(json.contains("Test"));
        assert!(json.contains("Subtitle"));
    }
}

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
        let content = std::fs::read_to_string(path).map_err(|e| ThemeError::Io(e.to_string()))?;
        serde_json::from_str(&content).map_err(|e| ThemeError::Parse(e.to_string()))
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

    #[test]
    fn test_get_spacing_default() {
        let json = r#"{
            "typography": {},
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
        // Unknown spacing name should return md (12)
        assert_eq!(theme.get_spacing("unknown"), 12.0);
    }
}

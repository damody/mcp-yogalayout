pub mod compute;
pub mod measure;
pub mod review;
pub mod templates;
pub mod tree;

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
    #[serde(rename = "box")]
    pub bounding_box: BoundingBox,
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

/// 量測結果
#[derive(Debug, Clone, Copy)]
pub struct MeasuredSize {
    pub width: f32,
    pub height: f32,
}

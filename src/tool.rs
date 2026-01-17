use crate::layout::compute::{compute_layout, ComputeError};
use crate::layout::review::{review_layout, FallbackInfo};
use crate::layout::templates::{auto_select_template, Density, Template};
use crate::md::{parse_markdown_file, ParseError};
use crate::output::{write_layout_json, write_report_json, OutputError};
use crate::paths::{ensure_dir_exists, ensure_file_exists, resolve_workspace_path, PathError};
use crate::theme::{Theme, ThemeError};
use rmcp::model::{CallToolRequestParam, Content, Implementation, ListToolsResult, ProtocolVersion, ServerCapabilities};
use rmcp::service::RequestContext;
use rmcp::{ErrorData as McpError, RoleServer, ServerHandler};
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

impl ServerHandler for LayoutService {
    fn get_info(&self) -> rmcp::model::InitializeResult {
        rmcp::model::InitializeResult {
            protocol_version: ProtocolVersion::LATEST,
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .build(),
            server_info: Implementation {
                name: "mcp-yogalayout".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                title: None,
                icons: None,
                website_url: None,
            },
            instructions: None,
        }
    }

    async fn list_tools(
        &self,
        _request: Option<rmcp::model::PaginatedRequestParam>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        let input_schema = schemars::schema_for!(ComputeSlideLayoutInput);
        let schema_value = serde_json::to_value(&input_schema.schema)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        let schema_map = match schema_value {
            serde_json::Value::Object(map) => map,
            _ => serde_json::Map::new(),
        };

        Ok(ListToolsResult {
            tools: vec![rmcp::model::Tool {
                name: "layout.compute_slide_layout".into(),
                description: Some("Compute slide layout from Markdown file using Flexbox".into()),
                input_schema: std::sync::Arc::new(schema_map),
                annotations: None,
                icons: None,
                output_schema: None,
                title: None,
            }],
            next_cursor: None,
        })
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> Result<rmcp::model::CallToolResult, McpError> {
        let name: &str = &request.name;
        match name {
            "layout.compute_slide_layout" => {
                let args_value = serde_json::Value::Object(request.arguments.unwrap_or_default());
                let input: ComputeSlideLayoutInput = serde_json::from_value(args_value)
                    .map_err(|e| McpError::invalid_params(e.to_string(), None))?;

                match self.compute_slide_layout(input).await {
                    Ok(output) => {
                        let json = serde_json::to_string_pretty(&output)
                            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
                        Ok(rmcp::model::CallToolResult {
                            content: vec![Content::text(json)],
                            is_error: None,
                            meta: None,
                            structured_content: None,
                        })
                    }
                    Err(e) => {
                        Ok(rmcp::model::CallToolResult {
                            content: vec![Content::text(e.to_string())],
                            is_error: Some(true),
                            meta: None,
                            structured_content: None,
                        })
                    }
                }
            }
            _ => Err(McpError::invalid_request("Unknown tool", None)),
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

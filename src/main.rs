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

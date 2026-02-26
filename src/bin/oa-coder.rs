//! oa-coder -- standalone MCP programming sub-agent.
//!
//! Usage: oa-coder --workspace <path>

fn main() -> anyhow::Result<()> {
    // Initialize tracing to stderr so it does not interfere with MCP stdio.
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .with_writer(std::io::stderr)
        .init();

    let workspace = std::env::args()
        .skip_while(|a| a != "--workspace")
        .nth(1)
        .unwrap_or_else(|| ".".to_string());

    let workspace = std::path::Path::new(&workspace).canonicalize()?;

    let config = oa_coder::server::McpServerConfig {
        workspace,
        sandboxed: false,
    };

    oa_coder::run_mcp_server(config)
}

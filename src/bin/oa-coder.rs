//! oa-coder -- standalone MCP programming sub-agent.
//!
//! Usage: oa-coder --workspace <path> [--sandboxed]

fn main() -> anyhow::Result<()> {
    // Initialize tracing to stderr so it does not interfere with MCP stdio.
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .with_writer(std::io::stderr)
        .init();

    let args: Vec<String> = std::env::args().collect();

    let workspace = args
        .iter()
        .position(|a| a == "--workspace")
        .and_then(|i| args.get(i + 1))
        .cloned()
        .unwrap_or_else(|| ".".to_string());

    let sandboxed = args.iter().any(|a| a == "--sandboxed");

    let workspace = std::path::Path::new(&workspace).canonicalize()?;

    let config = oa_coder::server::McpServerConfig {
        workspace,
        sandboxed,
    };

    oa_coder::run_mcp_server(config)
}

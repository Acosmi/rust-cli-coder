# oa-coder

> OpenAcosmi 编程子智能体 | Programming Sub-Agent for OpenAcosmi

---

## 中文

### 简介

**oa-coder** 是一个独立的 MCP（Model Context Protocol）编程子智能体。它通过 stdin/stdout 上的 JSON-RPC 2.0 协议暴露编码工具，可被任何 MCP 兼容客户端作为子进程调用。

核心特性：

- **9 层模糊编辑引擎** — 从精确匹配到 Levenshtein 距离，逐层降级，容忍缩进差异
- **安全路径限制** — 所有文件操作限定在 workspace 目录内，防止路径穿越
- **ripgrep 集成** — 代码搜索直接调用 `rg`，毫秒级响应
- **零 panic 策略** — 全面使用 `?` + `anyhow::Context`，无 `unwrap()`

### 安装

```bash
cargo install oa-coder
```

或从源码构建：

```bash
git clone https://github.com/Acosmi/rust-cli-coder.git
cd oa-coder
cargo build --release
```

### 使用

启动 MCP 服务器：

```bash
oa-coder --workspace /path/to/project
```

服务器从 stdin 读取 JSON-RPC 2.0 请求，从 stdout 输出响应。设置 `RUST_LOG=debug` 可在 stderr 查看详细日志。

#### MCP 客户端配置示例

在你的 MCP 客户端配置中添加：

```json
{
  "mcpServers": {
    "coder": {
      "command": "oa-coder",
      "args": ["--workspace", "/path/to/project"]
    }
  }
}
```

### 工具列表

| 工具 | 功能 | 说明 |
|------|------|------|
| `edit` | 模糊编辑 | 9 层匹配引擎（精确 → 忽略空白 → Levenshtein → 块锚定 → ...），返回 unified diff |
| `read` | 读取文件 | 支持行号、offset/limit 分页、二进制检测、UTF-8 安全截断 |
| `write` | 写入文件 | 原子写入（tempfile + rename），自动创建目录 |
| `grep` | 代码搜索 | ripgrep (`rg --json`) 子进程封装，支持正则、glob 过滤 |
| `glob` | 文件发现 | globset 模式匹配，递归扫描 workspace |
| `bash` | 执行命令 | 在 workspace 目录下执行 shell 命令，带超时控制 |

### 编辑引擎 9 层匹配

1. **SimpleReplacer** — 精确子串匹配
2. **LineTrimmedReplacer** — 逐行 trim 后比较
3. **BlockAnchorReplacer** — 首尾行锚定 + Levenshtein 距离
4. **WhitespaceNormalizedReplacer** — 折叠空白字符
5. **IndentationFlexibleReplacer** — 标准化缩进
6. **EscapeNormalizedReplacer** — 标准化转义序列
7. **TrimmedBoundaryReplacer** — 去除边界空行
8. **ContextAwareReplacer** — 上下文行锚定 + 相似度
9. **MultiOccurrenceReplacer** — 多次出现全部替换

### 作为 Rust 库使用

```rust
use oa_coder::server::McpServerConfig;

fn main() -> anyhow::Result<()> {
    let config = McpServerConfig {
        workspace: std::env::current_dir()?,
        sandboxed: false,
    };
    oa_coder::run_mcp_server(config)
}
```

### 协议示例

请求（stdin）：

```json
{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"read","arguments":{"filePath":"src/main.rs"}}}
```

响应（stdout）：

```json
{"jsonrpc":"2.0","id":1,"result":{"content":[{"type":"text","text":"1\tfn main() {\n2\t    println!(\"hello\");\n3\t}\n"}]}}
```

### 系统要求

- Rust >= 1.85（edition 2024）
- `rg`（ripgrep）— grep 工具需要，可通过 `cargo install ripgrep` 安装

---

## English

### Introduction

**oa-coder** is a standalone MCP (Model Context Protocol) programming sub-agent. It exposes coding tools over JSON-RPC 2.0 on stdin/stdout, making it usable as a subprocess by any MCP-compatible client.

Key features:

- **9-layer fuzzy edit engine** — from exact match to Levenshtein distance, progressive fallback tolerates indentation differences
- **Secure path restriction** — all file operations confined to workspace directory, prevents path traversal
- **ripgrep integration** — code search via `rg` subprocess, millisecond response times
- **Zero-panic policy** — uses `?` + `anyhow::Context` throughout, no `unwrap()`

### Installation

```bash
cargo install oa-coder
```

Or build from source:

```bash
git clone https://github.com/Acosmi/rust-cli-coder.git
cd oa-coder
cargo build --release
```

### Usage

Start the MCP server:

```bash
oa-coder --workspace /path/to/project
```

The server reads JSON-RPC 2.0 requests from stdin and writes responses to stdout. Set `RUST_LOG=debug` for verbose logging on stderr.

#### MCP Client Configuration Example

Add to your MCP client config:

```json
{
  "mcpServers": {
    "coder": {
      "command": "oa-coder",
      "args": ["--workspace", "/path/to/project"]
    }
  }
}
```

### Tools

| Tool | Function | Description |
|------|----------|-------------|
| `edit` | Fuzzy edit | 9-layer matching engine (exact → whitespace-normalized → Levenshtein → block-anchor → ...), returns unified diff |
| `read` | Read file | Line numbers, offset/limit pagination, binary detection, UTF-8 safe truncation |
| `write` | Write file | Atomic writes (tempfile + rename), auto-creates directories |
| `grep` | Code search | ripgrep (`rg --json`) subprocess wrapper, supports regex and glob filters |
| `glob` | File discovery | globset pattern matching, recursive workspace scan |
| `bash` | Execute command | Shell command execution in workspace directory with timeout control |

### 9-Layer Edit Engine

1. **SimpleReplacer** — exact substring match
2. **LineTrimmedReplacer** — trim each line before comparing
3. **BlockAnchorReplacer** — anchor on first/last lines + Levenshtein distance
4. **WhitespaceNormalizedReplacer** — collapse whitespace
5. **IndentationFlexibleReplacer** — normalize indentation
6. **EscapeNormalizedReplacer** — normalize escape sequences
7. **TrimmedBoundaryReplacer** — trim boundary blank lines
8. **ContextAwareReplacer** — context-line anchoring + similarity scoring
9. **MultiOccurrenceReplacer** — replace all occurrences for `replace_all` mode

### Library Usage

```rust
use oa_coder::server::McpServerConfig;

fn main() -> anyhow::Result<()> {
    let config = McpServerConfig {
        workspace: std::env::current_dir()?,
        sandboxed: false,
    };
    oa_coder::run_mcp_server(config)
}
```

### Protocol Example

Request (stdin):

```json
{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"read","arguments":{"filePath":"src/main.rs"}}}
```

Response (stdout):

```json
{"jsonrpc":"2.0","id":1,"result":{"content":[{"type":"text","text":"1\tfn main() {\n2\t    println!(\"hello\");\n3\t}\n"}]}}
```

### System Requirements

- Rust >= 1.85 (edition 2024)
- `rg` (ripgrep) — required by the grep tool, install via `cargo install ripgrep`

---

## License | 许可证

MIT License. See [LICENSE](LICENSE) for details.

Copyright (c) 2026 OpenAcosmi

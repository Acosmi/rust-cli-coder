# oa-coder

> OpenAcosmi 编程子智能体 | Programming Sub-Agent for OpenAcosmi

---

## 中文

### 简介

**oa-coder** 是一个独立的 MCP（Model Context Protocol）编程子智能体。它通过 stdin/stdout 上的 JSON-RPC 2.0 协议暴露编码工具，可被任何 MCP 兼容客户端作为子进程调用。

核心特性：

- **三平台原生沙箱** — macOS Seatbelt / Linux Landlock+Seccomp / Windows Job Object，Docker 自动降级
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

### 沙箱执行

oa-coder 的 `bash` 工具支持沙箱模式，通过 `oa-sandbox` 提供安全隔离的命令执行环境。

#### ✨ 三平台原生沙箱支持

oa-sandbox 是 **少数支持三大操作系统原生沙箱** 的开源方案之一，每个平台均使用操作系统提供的最优安全机制：

| 平台 | 原生后端 | 隔离机制 | 冷启动 |
|------|----------|----------|--------|
| **macOS** | Seatbelt (`sandbox_init_with_parameters`) | SBPL 沙箱配置文件 | ~65ms |
| **Linux** | Landlock + Seccomp-BPF + 用户命名空间 | 内核级 LSM + 系统调用过滤 | — |
| **Windows** | 受限令牌 + Job Object + ACL | Win32 进程安全模型 | — |
| **Docker** | Docker 容器 | 自动降级后端（始终可用） | ~215ms |

当原生后端不可用时，会 **自动无缝降级到 Docker**，确保在任何环境下都能提供沙箱能力。

#### 三级安全模型

| 级别 | 名称 | 网络 | 文件系统 | 适用场景 |
|------|------|------|----------|----------|
| **L0** | deny | 全部拒绝 | 最小只读 | 不受信任的代码、沙箱测试 |
| **L1** | sandbox | 仅公网 TCP | workspace 读写 | MCP 插件、AI 工具调用（默认） |
| **L2** | full | 完全访问 | 完全访问 | 可信代码 + dry-run 预览 |

#### 架构

```text
stdin (JSON-RPC) → McpServer → ToolRouter → bash tool
                                                ↓
                                    sandboxed=true?  ──→  oa-sandbox (隔离执行)
                                    sandboxed=false? ──→  sh -c (直接执行)
stdout (JSON-RPC) ←─────────────────────────────────────────┘
```

#### 运行模式

| 模式 | sandboxed | 说明 |
|------|-----------|------|
| **独立模式** | `false`（默认） | 直接通过 `sh -c` 执行命令，适用于本地开发 |
| **托管模式** | `true` | 通过 `oa-sandbox` 执行，提供进程隔离、文件系统限制、网络控制 |

#### 配置方式

**作为库使用时启用沙箱：**

```rust
let config = McpServerConfig {
    workspace: std::env::current_dir()?,
    sandboxed: true,  // 启用沙箱
};
oa_coder::run_mcp_server(config)
```

**独立 CLI 模式：** 当前默认关闭沙箱，命令在 workspace 目录下直接执行，带有超时保护（默认 120 秒）。

#### 安全特性

- **路径限制** — 所有文件操作（read/write/edit/glob）限定在 workspace 目录内
- **超时控制** — bash 命令默认 120 秒超时，超时自动 kill 进程
- **沙箱隔离**（托管模式）— 通过 oa-sandbox 提供进程级隔离
- **零 unsafe** — `Cargo.toml` 配置 `unsafe_code = "forbid"`

#### ⚠️ 注意事项

> **平台依赖**
>
> - **macOS**：无额外依赖，Seatbelt 为 macOS 系统内置
> - **Linux**：需要安装 `libseccomp-dev` >= 2.5.0（`sudo apt install libseccomp-dev`）
> - **Windows**：需要 Windows 10 或更高版本
> - **Docker 降级**：需要 Docker Engine 已安装并运行

> **当前限制**
>
> - 独立 CLI 模式（`oa-coder`）默认 **不启用沙箱**，需通过库调用或宿主系统设置 `sandboxed: true`
> - 沙箱模式下的 `bash` 工具依赖 `oa-sandbox` 二进制已安装到 `$PATH`
> - Docker 降级模式冷启动约 ~215ms，原生模式约 ~65ms，性能存在差异
> - `oa-sandbox` 中使用了 `unsafe` 代码用于平台 FFI 调用（macOS Seatbelt FFI、Linux libseccomp、Windows Win32 API），所有 `unsafe` 块均附带 `// SAFETY:` 注释

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

- **Three-platform native sandbox** — macOS Seatbelt / Linux Landlock+Seccomp / Windows Job Object, with Docker auto-fallback
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

### Sandbox Execution

The `bash` tool in oa-coder supports sandbox mode, providing secure isolated command execution via `oa-sandbox`.

#### ✨ Three-Platform Native Sandbox Support

oa-sandbox is **one of the few open-source solutions with native sandbox support across all three major operating systems**, using the optimal OS-level security mechanisms on each platform:

| Platform | Native Backend | Isolation Mechanism | Cold Start |
|----------|---------------|---------------------|------------|
| **macOS** | Seatbelt (`sandbox_init_with_parameters`) | SBPL sandbox profiles | ~65ms |
| **Linux** | Landlock + Seccomp-BPF + User Namespaces | Kernel LSM + syscall filtering | — |
| **Windows** | Restricted Token + Job Object + ACL | Win32 process security model | — |
| **Docker** | Docker container | Automatic fallback (always available) | ~215ms |

When native backends are unavailable, the system **automatically and seamlessly falls back to Docker**, ensuring sandbox capabilities in any environment.

#### Three-Tier Security Model

| Level | Name | Network | Filesystem | Use Case |
|-------|------|---------|------------|----------|
| **L0** | deny | All denied | Minimal read-only | Untrusted code, sandbox testing |
| **L1** | sandbox | Public TCP only | Workspace read/write | MCP plugins, AI tool calls (default) |
| **L2** | full | Full access | Full access | Trusted code + dry-run preview |

#### Architecture

```text
stdin (JSON-RPC) → McpServer → ToolRouter → bash tool
                                                ↓
                                    sandboxed=true?  ──→  oa-sandbox (isolated)
                                    sandboxed=false? ──→  sh -c (direct exec)
stdout (JSON-RPC) ←─────────────────────────────────────────┘
```

#### Execution Modes

| Mode | sandboxed | Description |
|------|-----------|-------------|
| **Standalone** | `false` (default) | Executes via `sh -c` directly, suitable for local development |
| **Managed** | `true` | Executes via `oa-sandbox`, providing process isolation, filesystem restrictions, network control |

#### Configuration

**Enable sandbox as a library:**

```rust
let config = McpServerConfig {
    workspace: std::env::current_dir()?,
    sandboxed: true,  // enable sandbox
};
oa_coder::run_mcp_server(config)
```

**Standalone CLI mode:** Sandbox is disabled by default. Commands execute directly in the workspace directory with timeout protection (default 120 seconds).

#### Security Features

- **Path restriction** — all file operations (read/write/edit/glob) confined to workspace directory
- **Timeout control** — bash commands have a 120s default timeout, auto-kills on expiry
- **Sandbox isolation** (managed mode) — process-level isolation via oa-sandbox
- **Zero unsafe** — `Cargo.toml` enforces `unsafe_code = "forbid"`

#### ⚠️ Notes & Caveats

> **Platform Dependencies**
>
> - **macOS**: No additional dependencies; Seatbelt is built into macOS
> - **Linux**: Requires `libseccomp-dev` >= 2.5.0 (`sudo apt install libseccomp-dev`)
> - **Windows**: Requires Windows 10 or later
> - **Docker fallback**: Requires Docker Engine installed and running

> **Current Limitations**
>
> - Standalone CLI mode (`oa-coder`) does **not enable sandbox by default**; set `sandboxed: true` via library usage or host system configuration
> - The sandboxed `bash` tool requires the `oa-sandbox` binary to be installed and available in `$PATH`
> - Docker fallback cold start (~215ms) is slower than native mode (~65ms)
> - `oa-sandbox` uses `unsafe` code for platform FFI calls (macOS Seatbelt FFI, Linux libseccomp, Windows Win32 API); all `unsafe` blocks include `// SAFETY:` comments

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

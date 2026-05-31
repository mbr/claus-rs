//! CLI wrapper for spawning the `claude` command.
//!
//! Provides a builder for constructing [`std::process::Command`] instances. The builder
//! configures command-line arguments for session management, permissions, MCP servers, and I/O
//! formats.

pub mod protocol;

use std::{collections::HashMap, path::PathBuf, process::Command};

/// Joins an iterator of string-like items with commas.
fn join_args<I, T>(args: I) -> String
where
    I: IntoIterator<Item = T>,
    T: AsRef<str>,
{
    args.into_iter()
        .map(|s| s.as_ref().to_owned())
        .collect::<Vec<_>>()
        .join(",")
}

/// Permission mode for tool execution.
///
/// The CLI also reads permission settings from `~/.claude/settings.json` and project-level
/// `.claude/settings.json` files.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum PermissionMode {
    /// Standard permission behavior with prompts.
    ///
    /// Unsuitable for non-interactive use since the CLI may hang waiting for user input.
    /// Use [`DontAsk`](Self::DontAsk) for headless/CI environments.
    #[default]
    Default,
    /// Auto-accept file edits, prompt for other tools.
    ///
    /// Auto-approves reads, file edits, and common filesystem Bash commands (`mkdir`, `touch`,
    /// `rm`, `rmdir`, `mv`, `cp`, `sed`). Other Bash commands and network requests still prompt.
    AcceptEdits,
    /// Allow all tools without prompts.
    ///
    /// Equivalent to `--dangerously-skip-permissions`. Only use in isolated environments
    /// (containers, VMs) without internet access.
    BypassPermissions,
    /// Delegate permission decisions to an MCP tool.
    ///
    /// Routes permission prompts to the MCP tool specified by
    /// [`CliBuilder::permission_prompt_tool`]. The tool receives permission requests and must
    /// return `{"behavior": "allow"}` or `{"behavior": "deny", "message": "..."}`. Useful for
    /// implementing custom approval logic or organizational security policies.
    Delegate,
    /// Deny tools unless pre-approved via allowed tools, settings, or hooks.
    ///
    /// Recommended for CI pipelines and non-interactive use. Auto-denies anything that would
    /// prompt; only pre-approved tools and read-only Bash commands execute.
    DontAsk,
    /// Exploration mode for research without edits.
    ///
    /// Claude can read files and run shell commands to explore, but cannot edit source files.
    /// Permission prompts still apply as in [`Default`](Self::Default) mode.
    Plan,
}

impl PermissionMode {
    /// Returns the CLI argument value.
    fn as_str(self) -> &'static str {
        match self {
            PermissionMode::Default => "default",
            PermissionMode::AcceptEdits => "acceptEdits",
            PermissionMode::BypassPermissions => "bypassPermissions",
            PermissionMode::Delegate => "delegate",
            PermissionMode::DontAsk => "dontAsk",
            PermissionMode::Plan => "plan",
        }
    }
}

/// Output format for the CLI.
///
/// Only applies when using [`CliBuilder::print`] mode.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum OutputFormat {
    /// Plain text output.
    ///
    /// Returns the final response as plain text. Best for simple scripting where you just need
    /// the result.
    #[default]
    Text,
    /// Single JSON result.
    ///
    /// Returns the final `result` message as a single JSON object - the same structure as the
    /// last message in [`StreamJson`](Self::StreamJson).
    Json,
    /// Newline-delimited JSON stream.
    ///
    /// Emits one JSON object per line as events occur. Required [`CliBuilder::verbose`]
    /// to be set.
    // TODO: Explain how to parse the format here!
    StreamJson,
}

impl OutputFormat {
    /// Returns the CLI argument value.
    fn as_str(self) -> &'static str {
        match self {
            OutputFormat::Text => "text",
            OutputFormat::Json => "json",
            OutputFormat::StreamJson => "stream-json",
        }
    }
}

/// Input format for the CLI.
///
/// Only applies when using [`CliBuilder::print`] mode.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum InputFormat {
    /// Plain text input via `-p` flag or stdin.
    #[default]
    Text,
    /// Newline-delimited JSON stream on stdin.
    ///
    /// Enables bidirectional communication for interactive sessions.
    // TODO: Explain how to use input format.
    StreamJson,
}

impl InputFormat {
    /// Returns the CLI argument value.
    fn as_str(self) -> &'static str {
        match self {
            InputFormat::Text => "text",
            InputFormat::StreamJson => "stream-json",
        }
    }
}

/// MCP server definition.
#[derive(Clone, Debug)]
pub enum McpServer {
    /// Stdio-based MCP server (spawns a subprocess).
    Stdio(StdioMcpServer),
    /// HTTP-based MCP server.
    Http(HttpMcpServer),
}

impl McpServer {
    /// Returns the server name.
    pub fn name(&self) -> &str {
        match self {
            McpServer::Stdio(s) => &s.name,
            McpServer::Http(h) => &h.name,
        }
    }

    /// Returns the server definition as a JSON value for `--mcp-config`.
    fn to_value(&self) -> serde_json::Value {
        match self {
            McpServer::Stdio(s) => s.to_value(),
            McpServer::Http(h) => h.to_value(),
        }
    }
}

impl From<StdioMcpServer> for McpServer {
    fn from(server: StdioMcpServer) -> Self {
        McpServer::Stdio(server)
    }
}

impl From<HttpMcpServer> for McpServer {
    fn from(server: HttpMcpServer) -> Self {
        McpServer::Http(server)
    }
}

/// Stdio-based MCP server configuration.
#[derive(Clone, Debug)]
pub struct StdioMcpServer {
    /// Server name (key in `mcpServers` object).
    name: String,
    /// Command to execute.
    command: String,
    /// Command arguments.
    args: Vec<String>,
    /// Environment variables.
    env: HashMap<String, String>,
}

impl StdioMcpServer {
    /// Creates a new stdio MCP server with the given name and command.
    pub fn new(name: impl Into<String>, command: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            command: command.into(),
            args: Vec::new(),
            env: HashMap::new(),
        }
    }

    /// Sets all command arguments.
    pub fn args<I, T>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<String>,
    {
        self.args = args.into_iter().map(Into::into).collect();
        self
    }

    /// Adds a single command argument.
    pub fn arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }

    /// Adds an environment variable.
    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.insert(key.into(), value.into());
        self
    }

    /// Returns the server definition as a JSON value.
    fn to_value(&self) -> serde_json::Value {
        let mut obj = serde_json::json!({
            "command": self.command
        });
        if !self.args.is_empty() {
            obj["args"] = serde_json::json!(self.args);
        }
        if !self.env.is_empty() {
            obj["env"] = serde_json::json!(self.env);
        }
        obj
    }
}

/// HTTP-based MCP server configuration.
#[derive(Clone, Debug)]
pub struct HttpMcpServer {
    /// Server name (key in mcpServers object).
    name: String,
    /// Server URL.
    url: String,
    /// HTTP headers.
    headers: HashMap<String, String>,
}

impl HttpMcpServer {
    /// Creates a new HTTP MCP server with the given name and URL.
    pub fn new(name: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            url: url.into(),
            headers: HashMap::new(),
        }
    }

    /// Adds an HTTP header.
    pub fn header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(key.into(), value.into());
        self
    }

    /// Returns the server definition as a JSON value.
    fn to_value(&self) -> serde_json::Value {
        let mut obj = serde_json::json!({
            "type": "http",
            "url": self.url
        });
        if !self.headers.is_empty() {
            obj["headers"] = serde_json::json!(self.headers);
        }
        obj
    }
}

/// Builder for constructing a `claude` CLI command.
#[derive(Clone, Debug, Default)]
pub struct CliBuilder {
    /// Resume a previous session.
    resume: bool,
    /// Session ID for the conversation.
    #[cfg(feature = "uuid")]
    session_id: Option<uuid::Uuid>,
    /// MCP servers to configure.
    mcp_servers: Vec<McpServer>,
    /// Whether to ignore MCP servers from other sources.
    strict_mcp_config: bool,
    /// Permission mode for tool execution.
    permission_mode: PermissionMode,
    /// MCP tool name for permission prompts (used with delegate mode).
    permission_prompt_tool: Option<String>,
    /// Output format.
    output_format: OutputFormat,
    /// Input format.
    input_format: InputFormat,
    /// Whether to enable verbose output.
    verbose: bool,
    /// Initial prompt (for non-interactive mode).
    prompt: Option<String>,
    /// System prompt (replaces default).
    system_prompt: Option<String>,
    /// System prompt to append to default.
    append_system_prompt: Option<String>,
    /// Model to use.
    model: Option<String>,
    /// Fallback model when primary is overloaded.
    fallback_model: Option<String>,
    /// Maximum agentic turns.
    max_turns: Option<u32>,
    /// Maximum budget in USD.
    max_budget_usd: Option<f64>,
    /// Allowed tools (filter on top of available tools), comma-separated.
    allowed_tools: Option<String>,
    /// Available built-in tools, comma-separated.
    tools: Option<String>,
    /// Additional directories to allow tool access.
    add_dirs: Vec<PathBuf>,
    /// Print mode (non-interactive).
    print: bool,
    /// Include partial message chunks in streaming output.
    include_partial_messages: bool,
    /// Disable session persistence (ephemeral session).
    no_session_persistence: bool,
    /// Working directory for the process.
    current_dir: Option<PathBuf>,
}

impl CliBuilder {
    /// Creates a new builder with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Resumes the most recent session.
    ///
    /// Note: Cannot be combined with [`session_id`](Self::session_id) to resume a specific
    /// session. Use `--session-id` only for new sessions.
    pub fn resume(mut self, enabled: bool) -> Self {
        self.resume = enabled;
        self
    }

    /// Creates a builder configured for non-interactive use.
    ///
    /// Pre-configured with:
    /// - [`PermissionMode::DontAsk`] to avoid hanging on permission prompts
    /// - [`print`](Self::print) mode enabled for non-interactive output
    /// - [`OutputFormat::StreamJson`] for structured, parseable output
    /// - [`verbose`](Self::verbose) enabled (required for `StreamJson`)
    /// - [`strict_mcp_config`](Self::strict_mcp_config) to ignore external MCP servers
    /// - [`max_turns`](Self::max_turns) set to 1 (single response, no agentic loops)
    /// - [`no_session_persistence`](Self::no_session_persistence) to avoid writing session files
    /// - All built-in tools disabled (caller must explicitly enable via [`tools`](Self::tools))
    ///
    /// Suitable for CI pipelines, scripts, and automation. Caller must set
    /// [`prompt`](Self::prompt) before building.
    pub fn headless() -> Self {
        Self {
            permission_mode: PermissionMode::DontAsk,
            print: true,
            output_format: OutputFormat::StreamJson,
            verbose: true,
            strict_mcp_config: true,
            max_turns: Some(1),
            no_session_persistence: true,
            tools: Some(String::new()), // Empty string → --tools "" disables all built-in tools
            ..Self::default()
        }
    }

    /// Sets the session ID for a new conversation.
    ///
    /// Predetermines the session ID for new sessions. Useful for generating deterministic IDs
    /// (e.g., `Uuid::new_v5` from a room/channel identifier). Fails if a session with this ID
    /// already exists.
    ///
    /// Note: Cannot be combined with [`resume`](Self::resume) to resume a specific session.
    /// The CLI only supports resuming the most recent session.
    ///
    /// Sessions are stored in `~/.claude/projects/<encoded-path>/<session-id>.jsonl`.
    #[cfg(feature = "uuid")]
    pub fn session_id(mut self, id: impl Into<uuid::Uuid>) -> Self {
        self.session_id = Some(id.into());
        self
    }

    /// Adds an MCP server to the configuration.
    pub fn mcp_server(mut self, server: impl Into<McpServer>) -> Self {
        self.mcp_servers.push(server.into());
        self
    }

    /// Sets whether to ignore MCP servers from other sources.
    pub fn strict_mcp_config(mut self, strict: bool) -> Self {
        self.strict_mcp_config = strict;
        self
    }

    /// Sets the permission mode for tool execution.
    pub fn permission_mode(mut self, mode: PermissionMode) -> Self {
        self.permission_mode = mode;
        self
    }

    /// Sets the MCP tool name for permission prompts.
    ///
    /// Used with [`PermissionMode::Delegate`] to specify which MCP tool handles permission
    /// decisions. The tool name follows MCP convention: `mcp__<server>__<tool>`.
    pub fn permission_prompt_tool(mut self, tool_name: impl Into<String>) -> Self {
        self.permission_prompt_tool = Some(tool_name.into());
        self
    }

    /// Sets the output format.
    pub fn output_format(mut self, format: OutputFormat) -> Self {
        self.output_format = format;
        self
    }

    /// Sets the input format.
    pub fn input_format(mut self, format: InputFormat) -> Self {
        self.input_format = format;
        self
    }

    /// Enables or disables verbose output.
    pub fn verbose(mut self, enabled: bool) -> Self {
        self.verbose = enabled;
        self
    }

    /// Sets the initial prompt for non-interactive mode.
    pub fn prompt(mut self, prompt: impl Into<String>) -> Self {
        self.prompt = Some(prompt.into());
        self
    }

    /// Sets the system prompt (replaces default).
    pub fn system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = Some(prompt.into());
        self
    }

    /// Appends to the default system prompt.
    pub fn append_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.append_system_prompt = Some(prompt.into());
        self
    }

    /// Sets the model to use.
    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Sets the fallback model when primary is overloaded.
    ///
    /// Only works with [`print`](Self::print) mode.
    pub fn fallback_model(mut self, model: impl Into<String>) -> Self {
        self.fallback_model = Some(model.into());
        self
    }

    /// Sets the maximum number of agentic turns.
    pub fn max_turns(mut self, turns: u32) -> Self {
        self.max_turns = Some(turns);
        self
    }

    /// Sets the maximum budget in USD.
    ///
    /// Only works with [`print`](Self::print) mode.
    pub fn max_budget_usd(mut self, amount: f64) -> Self {
        self.max_budget_usd = Some(amount);
        self
    }

    /// Sets allowed tools.
    ///
    /// Filters which tools are permitted from the available set. Use `"Bash(git:*) Edit"` syntax
    /// to allow specific patterns.
    ///
    /// - `Some([])` — no tools allowed
    /// - `Some(["Read", "Bash(git:*)"])` — only specified tools allowed
    /// - `None` — use default (all available tools allowed)
    pub fn allowed_tools<I, T>(mut self, tools: Option<I>) -> Self
    where
        I: IntoIterator<Item = T>,
        T: AsRef<str>,
    {
        self.allowed_tools = tools.map(|t| join_args(t));
        self
    }

    /// Sets the available built-in tools.
    ///
    /// - `Some([])` — disable all tools
    /// - `Some(["Read", "Bash"])` — only specified tools available
    /// - `None` — use default tool set
    ///
    /// Built-in tools include:
    /// - `Bash` — run shell commands
    /// - `Read` — read file contents
    /// - `Write` — create/overwrite files
    /// - `Edit` — edit existing files
    /// - `Glob` — find files by pattern
    /// - `Grep` — search file contents
    /// - `Task` — spawn subagents
    /// - `WebFetch` — fetch web content
    /// - `WebSearch` — search the web
    /// - `NotebookEdit` — edit Jupyter notebooks
    /// - `AskUserQuestion` — prompt the user for input
    ///
    /// Different from [`allowed_tools`](Self::allowed_tools) which filters on top of available
    /// tools.
    pub fn tools<I, T>(mut self, tools: Option<I>) -> Self
    where
        I: IntoIterator<Item = T>,
        T: AsRef<str>,
    {
        self.tools = tools.map(|t| join_args(t));
        self
    }

    /// Adds a directory for tool access.
    pub fn add_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.add_dirs.push(dir.into());
        self
    }

    /// Sets the working directory for the process.
    pub fn current_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.current_dir = Some(dir.into());
        self
    }

    /// Enables print mode (non-interactive, outputs result and exits).
    pub fn print(mut self, enabled: bool) -> Self {
        self.print = enabled;
        self
    }

    /// Includes partial message chunks in streaming output.
    ///
    /// Only works with [`print`](Self::print) and [`OutputFormat::StreamJson`].
    pub fn include_partial_messages(mut self, enabled: bool) -> Self {
        self.include_partial_messages = enabled;
        self
    }

    /// Disables session persistence (ephemeral session).
    ///
    /// Sessions will not be saved to disk and cannot be resumed.
    /// Only works with [`print`](Self::print) mode.
    pub fn no_session_persistence(mut self, enabled: bool) -> Self {
        self.no_session_persistence = enabled;
        self
    }

    /// Builds the configured command.
    ///
    /// Returns a [`Command`] with all arguments configured. The caller is responsible for setting
    /// up stdio and spawning.
    pub fn build(&self) -> Command {
        let mut cmd = Command::new("claude");

        if self.verbose {
            cmd.arg("--verbose");
        }

        if self.input_format != InputFormat::Text {
            cmd.arg("--input-format").arg(self.input_format.as_str());
        }

        if self.output_format != OutputFormat::Text {
            cmd.arg("--output-format").arg(self.output_format.as_str());
        }

        if self.resume {
            cmd.arg("--resume");
        }

        #[cfg(feature = "uuid")]
        if let Some(session_id) = &self.session_id {
            cmd.arg("--session-id").arg(session_id.to_string());
        }

        for server in &self.mcp_servers {
            let config = serde_json::json!({
                "mcpServers": {
                    server.name(): server.to_value()
                }
            });
            cmd.arg("--mcp-config").arg(config.to_string());
        }

        if self.strict_mcp_config {
            cmd.arg("--strict-mcp-config");
        }

        if self.permission_mode != PermissionMode::Default {
            cmd.arg("--permission-mode")
                .arg(self.permission_mode.as_str());
        }

        if let Some(tool_name) = &self.permission_prompt_tool {
            cmd.arg("--permission-prompt-tool").arg(tool_name);
        }

        if let Some(system_prompt) = &self.system_prompt {
            cmd.arg("--system-prompt").arg(system_prompt);
        }

        if let Some(append_system_prompt) = &self.append_system_prompt {
            cmd.arg("--append-system-prompt").arg(append_system_prompt);
        }

        if let Some(model) = &self.model {
            cmd.arg("--model").arg(model);
        }

        if let Some(fallback_model) = &self.fallback_model {
            cmd.arg("--fallback-model").arg(fallback_model);
        }

        if let Some(max_turns) = self.max_turns {
            cmd.arg("--max-turns").arg(max_turns.to_string());
        }

        if let Some(max_budget_usd) = self.max_budget_usd {
            cmd.arg("--max-budget-usd").arg(max_budget_usd.to_string());
        }

        if let Some(allowed_tools) = &self.allowed_tools {
            cmd.arg("--allowedTools").arg(allowed_tools);
        }

        if let Some(tools) = &self.tools {
            cmd.arg("--tools").arg(tools);
        }

        for dir in &self.add_dirs {
            cmd.arg("--add-dir").arg(dir);
        }

        if self.print {
            cmd.arg("-p");
        }

        if self.include_partial_messages {
            cmd.arg("--include-partial-messages");
        }

        if self.no_session_persistence {
            cmd.arg("--no-session-persistence");
        }

        // Prompt is a positional argument, must come last
        if let Some(prompt) = &self.prompt {
            cmd.arg(prompt);
        }

        if let Some(dir) = &self.current_dir {
            cmd.current_dir(dir);
        }

        cmd
    }
}

#[cfg(test)]
mod tests {
    use super::{CliBuilder, InputFormat, OutputFormat, PermissionMode, StdioMcpServer};

    #[test]
    fn minimal_command_has_no_args() {
        let cmd = CliBuilder::new().build();
        let args: Vec<_> = cmd.get_args().collect();
        assert!(args.is_empty());
    }

    #[test]
    fn headless_preset() {
        let cmd = CliBuilder::headless().build();
        let args: Vec<_> = cmd.get_args().map(|s| s.to_str().unwrap()).collect();
        assert!(args.contains(&"--permission-mode"));
        assert!(args.contains(&"dontAsk"));
        assert!(args.contains(&"-p"));
        assert!(args.contains(&"--output-format"));
        assert!(args.contains(&"stream-json"));
        assert!(args.contains(&"--verbose"));
        assert!(args.contains(&"--strict-mcp-config"));
        assert!(args.contains(&"--max-turns"));
        assert!(args.contains(&"1"));
        assert!(args.contains(&"--no-session-persistence"));
        // --tools "" disables all built-in tools
        assert!(args.contains(&"--tools"));
        assert!(args.contains(&""));
    }

    #[test]
    fn interactive_session_command() {
        let cmd = CliBuilder::new()
            .verbose(true)
            .input_format(InputFormat::StreamJson)
            .output_format(OutputFormat::StreamJson)
            .permission_mode(PermissionMode::DontAsk)
            .build();

        let args: Vec<_> = cmd.get_args().map(|s| s.to_str().unwrap()).collect();
        assert!(args.contains(&"--verbose"));
        assert!(args.contains(&"--input-format"));
        assert!(args.contains(&"stream-json"));
        assert!(args.contains(&"--permission-mode"));
        assert!(args.contains(&"dontAsk"));
    }

    #[test]
    #[cfg(feature = "uuid")]
    fn session_id_requires_uuid() {
        let session_id =
            uuid::Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").expect("valid uuid");
        let cmd = CliBuilder::new().session_id(session_id).build();

        let args: Vec<_> = cmd.get_args().map(|s| s.to_str().unwrap()).collect();
        assert!(args.contains(&"--session-id"));
        assert!(args.contains(&"550e8400-e29b-41d4-a716-446655440000"));
    }

    #[test]
    fn one_shot_command() {
        let cmd = CliBuilder::new()
            .prompt("Hello, world!")
            .print(true)
            .output_format(OutputFormat::StreamJson)
            .max_turns(5)
            .allowed_tools(Some(["Read"]))
            .build();

        let args: Vec<_> = cmd.get_args().map(|s| s.to_str().unwrap()).collect();
        assert!(args.contains(&"-p"));
        assert!(args.contains(&"Hello, world!")); // positional, at end
        assert!(args.contains(&"--max-turns"));
        assert!(args.contains(&"5"));
        assert!(args.contains(&"--allowedTools"));
        assert!(args.contains(&"Read"));
        // Prompt should be last argument
        assert_eq!(args.last(), Some(&"Hello, world!"));
    }

    #[test]
    fn mcp_server_config() {
        let cmd = CliBuilder::new()
            .mcp_server(
                StdioMcpServer::new("myserver", "mycmd")
                    .arg("--flag")
                    .env("KEY", "value"),
            )
            .strict_mcp_config(true)
            .build();

        let args: Vec<_> = cmd.get_args().map(|s| s.to_str().unwrap()).collect();
        assert!(args.contains(&"--mcp-config"));
        assert!(args.contains(&"--strict-mcp-config"));

        let config_idx = args
            .iter()
            .position(|&a| a == "--mcp-config")
            .expect("--mcp-config should be present");
        let config = args[config_idx + 1];
        assert!(config.contains("myserver"));
        assert!(config.contains("mycmd"));
    }

    /// Integration test that actually runs `claude` CLI.
    ///
    /// Run with: `cargo test --features uuid -- --ignored`
    #[test]
    #[ignore]
    fn integration_headless_run() {
        let output = CliBuilder::headless()
            .prompt("Reply with exactly: PONG")
            .build()
            .output()
            .expect("failed to execute claude");

        assert!(output.status.success(), "claude exited with error");

        let stdout = String::from_utf8_lossy(&output.stdout);

        // Parse the last line as JSON (the result message)
        let last_line = stdout.lines().last().expect("no output from claude");
        let result: serde_json::Value =
            serde_json::from_str(last_line).expect("failed to parse result JSON");

        assert_eq!(result["type"], "result");
        assert_eq!(result["subtype"], "success");
        assert!(
            result["result"]
                .as_str()
                .unwrap_or("")
                .to_uppercase()
                .contains("PONG"),
            "expected PONG in result, got: {}",
            result["result"]
        );
    }
}

//! CLI wrapper for spawning the `claude` command.
//!
//! Provides a builder for constructing [`std::process::Command`] instances. The builder
//! configures command-line arguments for session management, permissions, MCP servers, and I/O
//! formats.

use std::{collections::HashMap, path::PathBuf, process::Command};

/// Permission mode for tool execution.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum PermissionMode {
    /// Standard permission behavior with prompts.
    #[default]
    Default,
    /// Auto-accept file edits, prompt for other tools.
    AcceptEdits,
    /// Allow all tools without prompts.
    BypassPermissions,
    /// Delegate permission decisions to an MCP tool.
    Delegate,
    /// Deny tools unless pre-approved via allowed tools, settings, or hooks.
    DontAsk,
    /// Exploration mode with no tool execution.
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
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum OutputFormat {
    /// Plain text output.
    #[default]
    Text,
    /// JSON output.
    Json,
    /// Newline-delimited JSON stream.
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
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum InputFormat {
    /// Plain text input.
    #[default]
    Text,
    /// Newline-delimited JSON stream.
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
    /// Server name (key in mcpServers object).
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
    /// Working directory for the process.
    workdir: Option<PathBuf>,
    /// Session ID for the conversation.
    session_id: Option<String>,
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
    /// System prompt.
    system_prompt: Option<String>,
    /// Model to use.
    model: Option<String>,
    /// Maximum agentic turns.
    max_turns: Option<u32>,
    /// Allowed tools (comma-separated list).
    allowed_tools: Vec<String>,
    /// Additional directories to allow tool access.
    add_dirs: Vec<PathBuf>,
    /// Print mode (non-interactive).
    print: bool,
    /// Skip permission prompts entirely.
    dangerously_skip_permissions: bool,
}

impl CliBuilder {
    /// Creates a new builder with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the working directory for the process.
    pub fn workdir(mut self, path: impl Into<PathBuf>) -> Self {
        self.workdir = Some(path.into());
        self
    }

    /// Sets the session ID for the conversation.
    pub fn session_id(mut self, id: impl Into<String>) -> Self {
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

    /// Sets the system prompt.
    pub fn system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = Some(prompt.into());
        self
    }

    /// Sets the model to use.
    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Sets the maximum number of agentic turns.
    pub fn max_turns(mut self, turns: u32) -> Self {
        self.max_turns = Some(turns);
        self
    }

    /// Adds an allowed tool.
    pub fn allow_tool(mut self, tool: impl Into<String>) -> Self {
        self.allowed_tools.push(tool.into());
        self
    }

    /// Sets all allowed tools.
    pub fn allowed_tools<I, T>(mut self, tools: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<String>,
    {
        self.allowed_tools = tools.into_iter().map(Into::into).collect();
        self
    }

    /// Adds a directory for tool access.
    pub fn add_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.add_dirs.push(dir.into());
        self
    }

    /// Enables print mode (non-interactive, outputs result and exits).
    pub fn print(mut self, enabled: bool) -> Self {
        self.print = enabled;
        self
    }

    /// Skips all permission prompts.
    ///
    /// Use with caution. This bypasses all safety checks for tool execution.
    pub fn dangerously_skip_permissions(mut self, enabled: bool) -> Self {
        self.dangerously_skip_permissions = enabled;
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

        if let Some(session_id) = &self.session_id {
            cmd.arg("--session-id").arg(session_id);
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

        if let Some(model) = &self.model {
            cmd.arg("--model").arg(model);
        }

        if let Some(max_turns) = self.max_turns {
            cmd.arg("--max-turns").arg(max_turns.to_string());
        }

        if !self.allowed_tools.is_empty() {
            cmd.arg("--allowedTools").arg(self.allowed_tools.join(","));
        }

        for dir in &self.add_dirs {
            cmd.arg("--add-dir").arg(dir);
        }

        if self.print {
            cmd.arg("--print");
        }

        if self.dangerously_skip_permissions {
            cmd.arg("--dangerously-skip-permissions");
        }

        if let Some(prompt) = &self.prompt {
            cmd.arg("-p").arg(prompt);
        }

        if let Some(workdir) = &self.workdir {
            cmd.current_dir(workdir);
        }

        cmd
    }
}

#[cfg(test)]
mod tests {
    use super::{CliBuilder, InputFormat, OutputFormat, PermissionMode, StdioMcpServer};

    #[test]
    fn minimal_command() {
        let cmd = CliBuilder::new().build();
        let args: Vec<_> = cmd.get_args().collect();
        assert!(args.is_empty());
    }

    #[test]
    fn interactive_session_command() {
        let cmd = CliBuilder::new()
            .verbose(true)
            .input_format(InputFormat::StreamJson)
            .output_format(OutputFormat::StreamJson)
            .session_id("test-session")
            .permission_mode(PermissionMode::DontAsk)
            .build();

        let args: Vec<_> = cmd.get_args().map(|s| s.to_str().unwrap()).collect();
        assert!(args.contains(&"--verbose"));
        assert!(args.contains(&"--input-format"));
        assert!(args.contains(&"stream-json"));
        assert!(args.contains(&"--session-id"));
        assert!(args.contains(&"test-session"));
        assert!(args.contains(&"--permission-mode"));
        assert!(args.contains(&"dontAsk"));
    }

    #[test]
    fn one_shot_command() {
        let cmd = CliBuilder::new()
            .prompt("Hello, world!")
            .print(true)
            .output_format(OutputFormat::StreamJson)
            .max_turns(5)
            .allow_tool("Read")
            .build();

        let args: Vec<_> = cmd.get_args().map(|s| s.to_str().unwrap()).collect();
        assert!(args.contains(&"-p"));
        assert!(args.contains(&"Hello, world!"));
        assert!(args.contains(&"--print"));
        assert!(args.contains(&"--max-turns"));
        assert!(args.contains(&"5"));
        assert!(args.contains(&"--allowedTools"));
        assert!(args.contains(&"Read"));
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
}

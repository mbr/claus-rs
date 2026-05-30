//! Claude Code stream-json protocol types.
//!
//! Defines envelope types for Claude Code's `--output-format stream-json` protocol. The protocol
//! wraps Anthropic API types (from [`crate::anthropic`]) with session metadata.
//!
//! # Protocol Overview
//!
//! Claude Code emits newline-delimited JSON (NDJSON) on stdout. Each line is a JSON object with a
//! `type` field that determines the message variant:
//!
//! - `system` — Session initialization with available tools, model, and configuration
//! - `stream_event` — Wrapped Anthropic API streaming events (only with --include-partial-messages)
//! - `assistant` — Complete assistant message after streaming finishes
//! - `user` — Echoed user messages or tool results
//! - `result` — Final result with statistics (cost, duration, token usage)

use std::io::{self, BufRead};

use serde::{Deserialize, Serialize};

use crate::anthropic::{Content, Message, Role, ServerToolUsage, StreamEvent, StreamingMessage};

/// Error parsing Claude Code output.
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    /// Failed to read a line.
    #[error("failed to read line")]
    Io(#[from] io::Error),
    /// Failed to parse JSON.
    #[error("failed to parse JSON: {line:?}")]
    Json {
        /// The line that failed to parse.
        line: String,
        /// The parse error.
        #[source]
        source: serde_json::Error,
    },
}

/// Parses lines of Claude Code stdout into messages.
///
/// Takes any [`BufRead`] (e.g., from [`std::process::Output::stdout`]) and returns an iterator
/// yielding parsed [`OutputMessage`]s. Empty lines are skipped.
///
/// # Example
///
/// ```no_run
/// use claus::claudio::{CliBuilder, protocol::{parse_output, OutputMessage}};
///
/// let output = CliBuilder::headless()
///     .prompt("Hello")
///     .build()
///     .output()
///     .expect("failed to run");
///
/// for msg in parse_output(&output.stdout[..]) {
///     match msg {
///         Ok(OutputMessage::Assistant(a)) => println!("Assistant: {:?}", a.message.content),
///         Ok(OutputMessage::Result(r)) => println!("Done: ${:.4}", r.total_cost_usd),
///         Ok(_) => {}
///         Err(e) => eprintln!("Error: {e}"),
///     }
/// }
/// ```
pub fn parse_output(
    reader: impl BufRead,
) -> impl Iterator<Item = Result<OutputMessage, ParseError>> {
    reader.lines().filter_map(|line_result| match line_result {
        Ok(line) if line.is_empty() => None,
        Ok(line) => {
            Some(serde_json::from_str(&line).map_err(|source| ParseError::Json { line, source }))
        }
        Err(e) => Some(Err(ParseError::Io(e))),
    })
}

/// Common envelope fields for Claude Code messages.
///
/// Most message types include session metadata. Use `#[serde(flatten)]` to embed these fields.
#[derive(Clone, Debug, Deserialize)]
pub struct Envelope {
    /// Session identifier.
    pub session_id: String,
    /// Parent tool use ID if this message is part of a tool execution.
    #[serde(default)]
    pub parent_tool_use_id: Option<String>,
    /// Message UUID.
    pub uuid: String,
}

/// Message from Claude Code stdout.
///
/// Each line of stdout is a JSON object representing a message, with a
/// `type` field that determines the variant.
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OutputMessage {
    /// Session initialization.
    System(SystemMessage),
    /// Wrapped Anthropic API streaming event.
    StreamEvent(StreamEventMessage),
    /// Complete assistant message.
    Assistant(AssistantMessage),
    /// Echoed user message or tool result.
    User(UserMessage),
    /// Final result of a conversation turn.
    Result(ResultMessage),
}

/// System initialization message.
///
/// Sent at the start of a session with configuration details.
#[derive(Clone, Debug, Deserialize)]
pub struct SystemMessage {
    /// Message subtype (e.g., `"init"`).
    pub subtype: String,
    /// Current working directory.
    pub cwd: String,
    /// Available tools.
    pub tools: Vec<String>,
    /// Configured MCP servers.
    #[serde(default)]
    pub mcp_servers: Vec<McpServerStatus>,
    /// Model identifier.
    pub model: String,
    /// Permission mode.
    #[serde(rename = "permissionMode")]
    pub permission_mode: String,
    /// Available slash commands.
    #[serde(default)]
    pub slash_commands: Vec<String>,
    /// API key source.
    #[serde(rename = "apiKeySource")]
    pub api_key_source: String,
    /// Claude Code version.
    pub claude_code_version: String,
    /// Output style.
    pub output_style: String,
    /// Available agents.
    #[serde(default)]
    pub agents: Vec<String>,
    /// Available skills.
    #[serde(default)]
    pub skills: Vec<String>,
    /// Loaded plugins.
    #[serde(default)]
    pub plugins: Vec<String>,
    /// Session metadata.
    #[serde(flatten)]
    pub envelope: Envelope,
}

/// MCP server status in system init.
#[derive(Clone, Debug, Deserialize)]
pub struct McpServerStatus {
    /// Server name.
    pub name: String,
    /// Connection status.
    pub status: String,
}

/// Wrapped Anthropic streaming event.
///
/// Contains a [`StreamEvent`] from the Anthropic API along with session metadata.
#[derive(Debug, Deserialize)]
pub struct StreamEventMessage {
    /// The Anthropic streaming event.
    pub event: StreamEvent,
    /// Session metadata.
    #[serde(flatten)]
    pub envelope: Envelope,
}

/// Complete assistant message.
///
/// Contains a full message from the Anthropic API with response metadata. Sent after all
/// streaming events for a message have been delivered.
#[derive(Debug, Deserialize)]
pub struct AssistantMessage {
    /// The complete Anthropic message.
    pub message: StreamingMessage,
    /// Session metadata.
    #[serde(flatten)]
    pub envelope: Envelope,
}

/// Echoed user message or tool result.
#[derive(Clone, Debug, Deserialize)]
pub struct UserMessage {
    /// The user message content.
    pub message: Message,
    /// Session metadata.
    #[serde(flatten)]
    pub envelope: Envelope,
    /// Structured metadata about tool result (present when message contains a tool result).
    #[serde(default)]
    pub tool_use_result: Option<serde_json::Value>,
}

/// Final result of a conversation turn.
#[derive(Clone, Debug, Deserialize)]
pub struct ResultMessage {
    /// Result subtype (`"success"`, `"error_max_turns"`, etc.).
    pub subtype: String,
    /// Whether this represents an error.
    pub is_error: bool,
    /// Total duration in milliseconds.
    pub duration_ms: u64,
    /// API call duration in milliseconds.
    #[serde(default)]
    pub duration_api_ms: u64,
    /// Number of conversation turns.
    #[serde(default)]
    pub num_turns: u32,
    /// Final text result.
    pub result: Option<String>,
    /// Total cost in USD.
    #[serde(default)]
    pub total_cost_usd: f64,
    /// Token usage statistics.
    pub usage: ResultUsage,
    /// Per-model usage breakdown.
    #[serde(default, rename = "modelUsage")]
    pub model_usage: serde_json::Map<String, serde_json::Value>,
    /// Permission denials during this turn.
    #[serde(default)]
    pub permission_denials: Vec<serde_json::Value>,
    /// Session metadata.
    #[serde(flatten)]
    pub envelope: Envelope,
}

/// Token usage statistics in result message.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct ResultUsage {
    /// Input tokens.
    #[serde(default)]
    pub input_tokens: u32,
    /// Cache creation input tokens.
    #[serde(default)]
    pub cache_creation_input_tokens: u32,
    /// Cache read input tokens.
    #[serde(default)]
    pub cache_read_input_tokens: u32,
    /// Output tokens.
    #[serde(default)]
    pub output_tokens: u32,
    /// Server tool use statistics.
    #[serde(default)]
    pub server_tool_use: ServerToolUsage,
    /// Service tier.
    #[serde(default)]
    pub service_tier: String,
    /// Cache creation breakdown.
    #[serde(default)]
    pub cache_creation: CacheCreation,
}

/// Cache creation breakdown.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct CacheCreation {
    /// Tokens in 1-hour ephemeral cache.
    #[serde(default)]
    pub ephemeral_1h_input_tokens: u32,
    /// Tokens in 5-minute ephemeral cache.
    #[serde(default)]
    pub ephemeral_5m_input_tokens: u32,
}

// --- Input types ---

/// Input message to Claude Code stdin.
///
/// Send as newline-delimited JSON when using `--input-format stream-json`.
#[derive(Clone, Debug, Serialize)]
pub struct InputMessage {
    /// Message type (always `"user"`).
    #[serde(rename = "type")]
    message_type: &'static str,
    /// The message content.
    pub message: Message,
}

impl InputMessage {
    /// Creates a text input message.
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            message_type: "user",
            message: Message::from_text(Role::User, text),
        }
    }

    /// Creates an input message with custom content blocks.
    pub fn with_content(content: Vec<Content>) -> Self {
        Self {
            message_type: "user",
            message: Message {
                role: Role::User,
                content,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_system_init() {
        let json = r#"{
            "type": "system",
            "subtype": "init",
            "cwd": "/home/user/project",
            "session_id": "6484002d-24fe-4f95-ad4b-6bf7130f1fcb",
            "tools": ["Bash", "Read", "Write"],
            "mcp_servers": [],
            "model": "claude-opus-4-5-20251101",
            "permissionMode": "default",
            "slash_commands": ["commit"],
            "apiKeySource": "none",
            "claude_code_version": "2.1.17",
            "output_style": "default",
            "agents": [],
            "skills": [],
            "plugins": [],
            "uuid": "f34a0e91-06ae-426c-9e5c-317a7572ff29"
        }"#;

        let msg: OutputMessage = serde_json::from_str(json).expect("parse");
        match msg {
            OutputMessage::System(sys) => {
                assert_eq!(sys.subtype, "init");
                assert_eq!(sys.cwd, "/home/user/project");
                assert_eq!(sys.tools, vec!["Bash", "Read", "Write"]);
                assert_eq!(sys.model, "claude-opus-4-5-20251101");
                assert_eq!(sys.permission_mode, "default");
            }
            _ => panic!("expected System variant"),
        }
    }

    #[test]
    fn parse_result_success() {
        let json = r#"{
            "type": "result",
            "subtype": "success",
            "is_error": false,
            "duration_ms": 2633,
            "duration_api_ms": 2600,
            "num_turns": 1,
            "result": "hello",
            "session_id": "6484002d-24fe-4f95-ad4b-6bf7130f1fcb",
            "total_cost_usd": 0.12779625,
            "usage": {
                "input_tokens": 3,
                "cache_creation_input_tokens": 20429,
                "cache_read_input_tokens": 0,
                "output_tokens": 4,
                "server_tool_use": {"web_search_requests": 0, "web_fetch_requests": 0},
                "service_tier": "standard",
                "cache_creation": {"ephemeral_1h_input_tokens": 0, "ephemeral_5m_input_tokens": 20429}
            },
            "modelUsage": {},
            "permission_denials": [],
            "uuid": "4e5d6b84-6129-47b3-bba6-fdb375aa7b3d"
        }"#;

        let msg: OutputMessage = serde_json::from_str(json).expect("parse");
        match msg {
            OutputMessage::Result(res) => {
                assert_eq!(res.subtype, "success");
                assert!(!res.is_error);
                assert_eq!(res.duration_ms, 2633);
                assert_eq!(res.result, Some("hello".to_string()));
                assert_eq!(res.usage.input_tokens, 3);
                assert_eq!(res.usage.cache_creation_input_tokens, 20429);
            }
            _ => panic!("expected Result variant"),
        }
    }

    #[test]
    fn parse_assistant() {
        let json = r#"{
            "type": "assistant",
            "message": {
                "model": "claude-opus-4-5-20251101",
                "id": "msg_016erzjGS5oTB6Q8uohJEpAs",
                "type": "message",
                "role": "assistant",
                "content": [{"type": "text", "text": "hello"}],
                "stop_reason": null,
                "stop_sequence": null,
                "usage": {
                    "input_tokens": 3,
                    "output_tokens": 1
                }
            },
            "parent_tool_use_id": null,
            "session_id": "6484002d-24fe-4f95-ad4b-6bf7130f1fcb",
            "uuid": "9e40ea6e-9f3e-43e1-a6c0-59e9de6c347f"
        }"#;

        let msg: OutputMessage = serde_json::from_str(json).expect("parse");
        match msg {
            OutputMessage::Assistant(asst) => {
                assert_eq!(asst.message.id, "msg_016erzjGS5oTB6Q8uohJEpAs");
                assert_eq!(asst.message.content.len(), 1);
                assert!(asst.envelope.parent_tool_use_id.is_none());
            }
            _ => panic!("expected Assistant variant"),
        }
    }

    #[test]
    fn serialize_input_text() {
        let input = InputMessage::text("hello world");
        let json = serde_json::to_value(&input).expect("serialize");

        assert_eq!(json["type"], "user");
        assert_eq!(json["message"]["role"], "user");
        assert_eq!(json["message"]["content"][0]["type"], "text");
        assert_eq!(json["message"]["content"][0]["text"], "hello world");
    }

    #[test]
    fn parse_user_tool_result() {
        use crate::anthropic::Role;

        let json = r#"{"type":"user","message":{"role":"user","content":[{"tool_use_id":"toolu_01TV2WdLXaSZwBGgKGPvLEmy","type":"tool_result","content":"hello"}]},"parent_tool_use_id":null,"session_id":"bf7004a5-4781-4c4e-bd35-6f4516db86fd","uuid":"dfc99bb7-55dc-4829-87a8-e9fd6333f970","tool_use_result":{"type":"text","file":{"filePath":"/tmp/hello.txt"}}}"#;

        let msg: OutputMessage = serde_json::from_str(json).expect("parse");
        match msg {
            OutputMessage::User(user) => {
                assert_eq!(user.message.role, Role::User);
                assert_eq!(user.message.content.len(), 1);
                assert!(user.tool_use_result.is_some());
            }
            _ => panic!("expected User variant"),
        }
    }

    /// Test parsing actual Claude CLI output.
    #[test]
    fn parse_real_assistant_with_tool_use() {
        let json = r#"{"type":"assistant","message":{"model":"claude-opus-4-5-20251101","id":"msg_01UQFX7fDMP5CKAWQWTgtodQ","type":"message","role":"assistant","content":[{"type":"tool_use","id":"toolu_01TV2WdLXaSZwBGgKGPvLEmy","name":"Read","input":{"file_path":"/tmp/hello.txt"},"caller":{"type":"direct"}}],"stop_reason":null,"stop_sequence":null,"stop_details":null,"usage":{"input_tokens":3,"cache_creation_input_tokens":22175,"cache_read_input_tokens":0,"cache_creation":{"ephemeral_5m_input_tokens":22175,"ephemeral_1h_input_tokens":0},"output_tokens":1,"service_tier":"standard","inference_geo":"not_available"},"context_management":null},"parent_tool_use_id":null,"session_id":"bf7004a5-4781-4c4e-bd35-6f4516db86fd","uuid":"a3c66f24-58f3-4727-b052-961d2205958f"}"#;

        let msg: OutputMessage = serde_json::from_str(json).expect("parse");
        match msg {
            OutputMessage::Assistant(asst) => {
                assert_eq!(asst.message.model, "claude-opus-4-5-20251101");
                assert_eq!(asst.message.content.len(), 1);
            }
            _ => panic!("expected Assistant variant"),
        }
    }
}

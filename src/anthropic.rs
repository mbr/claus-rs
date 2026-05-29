//! Anthropic API related types.
//!
//! This module contains types that match the implemented Anthropic API.

use std::{fmt, fmt::Display};

use schemars::{JsonSchema, schema_for};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// API version that is compatible with this module.
pub const ANTHROPIC_VERSION: &str = "2023-06-01";

/// Default API endpoint host to use.
pub const DEFAULT_ENDPOINT_HOST: &str = "api.anthropic.com";

/// Default model to use for requests.
pub const DEFAULT_MODEL: &str = "claude-sonnet-4-20250514";

/// The body of a request to the messages endpoint.
///
/// This type can be used to construct a [`crate::http_request::HttpRequest`] for the `messages`
/// endpoint. Usually it is better to use [`crate::MessagesRequestBuilder`] instead.
#[derive(Debug, Serialize)]
pub struct MessagesBody<'a> {
    /// The model to use for the request.
    pub model: &'a str,
    /// The maximum number of tokens for the response.
    pub max_tokens: u32,
    /// The system prompt for the conversation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<&'a str>,
    /// The messages to include in the request.
    ///
    /// Uses [`im::Vector`] for efficient sharing and cloning of conversation history.
    pub messages: &'a im::Vector<Message>,
    /// Tools available for the model to use.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<&'a im::Vector<Tool>>,
    /// Whether to stream the response.
    #[serde(skip_serializing_if = "is_false")]
    pub stream: bool,
}

/// Helper function to check if a boolean is false, used with `serde(skip_serializing_if)`.
fn is_false(value: &bool) -> bool {
    !value
}

/// A role in a conversation.
///
/// The currrent API specification only supports `user` and `assistant` roles.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// Message from the user.
    User,
    /// Message from the model.
    Assistant,
}

/// A message in a conversation.
///
/// Iterating over a message will yield its `content`.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Message {
    /// The role of the message.
    pub role: Role,
    /// The contents of the message.
    ///
    /// Messages are allowed to be multipart and can contain mixed inputs.
    pub content: Vec<Content>,
}

impl Message {
    /// Convenience function to construct a message containt a single piece of text.
    pub fn from_text<S: Into<String>>(role: Role, text: S) -> Self {
        Self {
            role,
            content: vec![Content::from_text(text.into())],
        }
    }
}

impl IntoIterator for Message {
    type Item = Content;
    type IntoIter = std::vec::IntoIter<Content>;

    fn into_iter(self) -> Self::IntoIter {
        self.content.into_iter()
    }
}

impl<'a> IntoIterator for &'a Message {
    type Item = &'a Content;
    type IntoIter = std::slice::Iter<'a, Content>;

    fn into_iter(self) -> Self::IntoIter {
        self.content.iter()
    }
}

/// A tool that can be used by the model.
///
/// Tools allow the model to perform actions beyond text generation, such as calling functions
/// or retrieving external data.
///
/// # Example
///
/// ```
/// use claus::anthropic::Tool;
/// use schemars::JsonSchema;
/// use serde_json;
///
/// #[derive(JsonSchema)]
/// struct GetStockPriceInput {
///     /// The stock ticker symbol, e.g. AAPL for Apple Inc.
///     ticker: String,
/// }
///
/// let tool = Tool::new::<GetStockPriceInput, _, _>(
///     "get_stock_price",
///     "Get the current stock price for a given ticker symbol.",
/// );
///# let serialized = serde_json::to_value(&tool).expect("Should serialize");
///#
///# assert_eq!(serialized["name"], "get_stock_price");
///# assert_eq!(serialized["description"], "Get the current stock price for a given ticker symbol.");
///# assert_eq!(serialized["input_schema"]["type"], "object");
///# assert_eq!(serialized["input_schema"]["properties"]["ticker"]["type"], "string");
///# assert_eq!(serialized["input_schema"]["required"][0], "ticker");
/// ```
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Tool {
    /// The name of the tool.
    ///
    /// Must be a valid identifier that can be referenced by the model.
    pub name: String,
    /// A description of what the tool does.
    ///
    /// This helps the model understand when and how to use the tool.
    pub description: String,
    /// JSON schema defining the tool's input parameters.
    ///
    /// This schema describes the structure and types of the parameters
    /// that the tool expects to receive.
    pub input_schema: Value,
}

impl Tool {
    /// Creates a new tool with the given name and description.
    ///
    /// The input schema is automatically generated from the type parameter T.
    pub fn new<T, N, D>(name: N, description: D) -> Self
    where
        T: JsonSchema,
        N: Into<String>,
        D: Into<String>,
    {
        let schema = schema_for!(T);
        let input_schema =
            serde_json::to_value(schema).expect("Schema serialization should not fail");

        Self {
            name: name.into(),
            description: description.into(),
            input_schema,
        }
    }
}

/// A tool use request from the model.
///
/// Represents the model invoking a tool with specific input parameters.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ToolUse {
    /// Unique identifier for this tool use.
    pub id: String,
    /// The name of the tool being invoked.
    pub name: String,
    /// The input parameters for the tool.
    pub input: Value,
}

impl Display for ToolUse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}({}) with {:?}", self.name, self.id, self.input)
    }
}

/// A tool result response to a tool use.
///
/// Represents the result of executing a tool that was requested by the model.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ToolResult {
    /// The ID of the tool use this result corresponds to.
    pub tool_use_id: String,
    /// The result content.
    pub content: ToolResultContent,
    /// Whether this result represents an error.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
}

/// Content that can be included in a tool result.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum ToolResultContent {
    /// Potentially multiple pieces of content.
    Content(Vec<Content>),
    /// A single string of content.
    String(String),
}

impl Display for ToolResultContent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ToolResultContent::Content(contents) => {
                for (idx, content) in contents.iter().enumerate() {
                    if idx > 0 {
                        write!(f, " ")?;
                    }
                    content.fmt(f)?;
                }
                Ok(())
            }
            ToolResultContent::String(string) => f.write_str(string),
        }
    }
}

impl From<String> for ToolResultContent {
    fn from(s: String) -> Self {
        ToolResultContent::String(s)
    }
}

impl From<&str> for ToolResultContent {
    fn from(s: &str) -> Self {
        ToolResultContent::String(s.to_string())
    }
}

impl From<Vec<Content>> for ToolResultContent {
    fn from(content: Vec<Content>) -> Self {
        ToolResultContent::Content(content)
    }
}

impl ToolResult {
    /// Creates a successful tool result.
    pub fn success<T: Into<ToolResultContent>>(tool_use_id: String, content: T) -> Self {
        Self {
            tool_use_id,
            content: content.into(),
            is_error: None,
        }
    }

    /// Creates an error tool result.
    pub fn error<T: Into<ToolResultContent>>(tool_use_id: String, error_content: T) -> Self {
        Self {
            tool_use_id,
            content: error_content.into(),
            is_error: Some(true),
        }
    }

    /// Creates an error tool result for an unknown tool.
    pub fn unknown_tool<S: AsRef<str>>(tool_use_id: String, tool_name: S) -> Self {
        Self::error(tool_use_id, format!("Unknown tool: {}", tool_name.as_ref()))
    }
}

impl Display for ToolResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_error == Some(true) {
            write!(
                f,
                "Tool result error for {}: {}",
                self.tool_use_id, self.content
            )
        } else {
            write!(f, "Tool result for {}: {}", self.tool_use_id, self.content)
        }
    }
}

/// Individual result from a web search.
///
/// When Claude uses the web search tool, Anthropic's servers perform a search and return
/// matching results. Each result contains metadata about a webpage that Claude can use to
/// answer questions requiring current information.
///
/// The `encrypted_content` field is opaque to client applications; applications typically use
/// only `title` and `url` for displaying citations to users.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WebSearchResult {
    /// Title of the webpage.
    pub title: String,
    /// URL of the webpage.
    ///
    /// Applications should display this as a citation link when showing search results.
    pub url: String,
    /// Encrypted page content for Claude's internal processing.
    ///
    /// Opaque to client applications.
    pub encrypted_content: String,
    /// How recently the page was published or updated, if available.
    pub page_age: Option<String>,
}

/// Content pieces that make up a message.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Content {
    /// Text content.
    Text {
        /// The text content.
        text: String,
    },
    /// Image content.
    ///
    /// TODO: At the moment, images are not supported.
    Image,
    /// Tool use content.
    ToolUse(ToolUse),
    /// Tool result content.
    ToolResult(ToolResult),
    /// Server-side tool invocation.
    ///
    /// Unlike [`Content::ToolUse`] which the client must execute, server tool uses are handled
    /// by Anthropic's infrastructure. The client receives this block to observe that a server
    /// tool was called, but no action is required—the result appears as a subsequent content
    /// block in the same response.
    ///
    /// See <https://docs.anthropic.com/en/docs/build-with-claude/tool-use/web-search-tool>.
    ServerToolUse {
        /// Unique identifier linking this invocation to its result block.
        id: String,
        /// Name of the server tool (e.g., `"web_search"`).
        name: String,
        /// Parameters passed to the tool.
        input: Value,
    },
    /// Result from a server-side web search.
    ///
    /// Appears after the corresponding [`Content::ServerToolUse`] block and contains the
    /// webpages Claude found relevant to the query. Applications should display source URLs
    /// as citations.
    WebSearchToolResult {
        /// ID of the [`Content::ServerToolUse`] this result corresponds to.
        tool_use_id: String,
        /// Search results, ordered by relevance.
        content: Vec<WebSearchResult>,
    },
    /// Catch-all for unrecognized content types.
    ///
    /// Ensures deserialization doesn't fail for unknown types added in future API versions,
    /// allowing applications to gracefully skip content they don't understand.
    #[serde(other)]
    Unknown,
}

impl Display for Content {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Content::Text { text } => f.write_str(text),
            Content::Image => f.write_str("<image>"),
            Content::ToolUse(tool_use) => tool_use.fmt(f),
            Content::ToolResult(tool_result) => tool_result.fmt(f),
            Content::ServerToolUse { id, name, .. } => write!(f, "<server_tool_use:{name}({id})>"),
            Content::WebSearchToolResult { tool_use_id, .. } => {
                write!(f, "<web_search_result:{tool_use_id}>")
            }
            Content::Unknown => f.write_str("<unknown>"),
        }
    }
}

impl Content {
    /// Convenience function to construct a text content piece.
    pub fn from_text<S: Into<String>>(text: S) -> Self {
        Content::Text { text: text.into() }
    }

    /// Returns the text content of the content piece, if it is a text piece.
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Content::Text { text } => Some(text.as_str()),
            _ => None,
        }
    }
}

/// Anthropic API error.
///
/// Errors defined in the Anthropic API specification, do not include parsing or transport errors.
#[derive(Clone, Debug, thiserror::Error, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ApiError {
    /// HTTP 400, invalid request
    #[error("Invalid request")]
    InvalidRequestError,
    /// HTTP 401, authentication error
    #[error("Authentication error")]
    AuthenticationError,
    /// HTTP 403, your API key does not have permission to use the specified resource
    #[error("Permission error")]
    PermissionError,
    /// HTTP 404, the requested resource was not found
    #[error("Not found")]
    NotFoundError,
    /// HTTP 413, request exceeds the maximum allowed number of bytes
    #[error("Request too large")]
    RequestTooLarge,
    /// HTTP 429, your account has hit a rate limit
    #[error("Rate limit exceeded")]
    RateLimitError,
    /// HTTP 500, an unexpected error has occurred internal to Anthropic's systems
    #[error("API error")]
    #[allow(clippy::enum_variant_names)]
    ApiError,
    /// HTTP 529, Anthropic's API is temporarily overloaded
    #[error("API overloaded")]
    OverloadedError,
}

/// A response from the Anthropic API.
///
/// This is the catch-all type for all valid responses from the API.
#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ApiResponse {
    /// A response to a messages request.
    Message(MessagesResponse),
    /// An error response from the API.
    Error { error: ApiError },
}

impl ApiResponse {
    /// Returns a string describing the type of the response.
    pub fn kind(&self) -> &'static str {
        match self {
            ApiResponse::Message(_) => "message",
            ApiResponse::Error { .. } => "error",
        }
    }
}

impl TryFrom<ApiResponse> for MessagesResponse {
    type Error = ();

    fn try_from(helper: ApiResponse) -> Result<Self, Self::Error> {
        match helper {
            ApiResponse::Message(response) => Ok(response),
            ApiResponse::Error { error: _ } => Err(()),
        }
    }
}

/// The reason that the model stopped generating.
///
/// See [the Anthropic API documentation](https://docs.anthropic.com/en/api/handling-stop-reasons)
/// for more information.
#[derive(Copy, Clone, Debug, Eq, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    /// The model reached a natural stopping point.
    EndTurn,
    /// We exceeded the requested `max_tokens` or the model's maximum.
    MaxTokens,
    /// One of the provided custom `stop_sequences` was generated.
    StopSequence,
    /// The model invoked one or more tools.
    ToolUse,
    /// The model paused its turn.
    PauseTurn,
    /// The model refused to provide a response.
    Refusal,
}

/// The response to a messages request.
#[derive(Debug, Deserialize, Serialize)]
pub struct MessagesResponse {
    /// The ID of the response.
    pub id: String,
    /// The model used to generate the response.
    pub model: String,
    /// The reason the response was stopped.
    pub stop_reason: StopReason,
    /// The sequence that caused the response to be stopped.
    ///
    /// This will only be set if a custom stop sequence was provided, and it was hit.
    pub stop_sequence: Option<String>,
    /// The usage statistics for the request.
    pub usage: Usage,
    // TODO: missing `container`
    #[serde(flatten)]
    pub message: Message,
}

/// Token usage statistics for a request.
///
/// Tracks how many tokens were consumed for input and output, including prompt caching metrics
/// when caching is enabled. Prompt caching allows the API to reuse previously processed prompt
/// prefixes, reducing both latency and costs for repeated prompts.
///
/// See <https://docs.anthropic.com/en/docs/build-with-claude/prompt-caching> for details.
#[derive(Debug, Deserialize, Serialize)]
pub struct Usage {
    /// Tokens sent to the model that were not served from cache.
    pub input_tokens: u32,
    /// Tokens generated by the model in its response.
    pub output_tokens: u32,
    /// Tokens written to the prompt cache during this request.
    ///
    /// When you mark content with `cache_control: {"type": "ephemeral"}` and the cache doesn't
    /// already contain it, the API caches the processed representation for subsequent requests.
    /// Cache writes incur a premium over standard input tokens. Zero if caching is not used or
    /// the cache already contained the content.
    #[serde(default)]
    pub cache_creation_input_tokens: u32,
    /// Tokens read from the prompt cache during this request.
    ///
    /// When a request's prompt prefix matches cached content, these tokens are served from cache
    /// instead of being reprocessed. Cache reads are cheaper than standard input tokens and also
    /// reduce latency. Zero if caching is not used or there was a cache miss.
    #[serde(default)]
    pub cache_read_input_tokens: u32,
}

/// Usage statistics for server-side tools.
///
/// Server-side tools (like web search) run on Anthropic's infrastructure. This struct tracks
/// how many times each server tool was invoked during a request, which affects billing since
/// server tools have per-use pricing in addition to token costs.
///
/// See <https://docs.anthropic.com/en/docs/build-with-claude/tool-use/web-search-tool> for
/// details.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ServerToolUsage {
    /// Number of web searches performed during this request.
    ///
    /// Bounded by the `max_uses` parameter if specified in the tool configuration.
    pub web_search_requests: u32,
}

/// Token usage statistics for streaming responses.
///
/// Unlike [`Usage`], fields are optional because streaming events may provide partial usage
/// information. The `message_start` event typically provides `input_tokens`, while
/// `message_delta` provides `output_tokens` as the response completes.
#[derive(Debug, Deserialize)]
pub struct StreamingUsage {
    /// Tokens sent to the model that were not served from cache.
    pub input_tokens: Option<u32>,
    /// Tokens generated by the model so far.
    pub output_tokens: Option<u32>,
    /// Tokens written to the prompt cache during this request.
    ///
    /// See [`Usage::cache_creation_input_tokens`] for details.
    pub cache_creation_input_tokens: Option<u32>,
    /// Tokens read from the prompt cache during this request.
    ///
    /// See [`Usage::cache_read_input_tokens`] for details.
    pub cache_read_input_tokens: Option<u32>,
    /// Usage statistics for server-side tools like web search.
    ///
    /// Present when server-side tools were invoked during the request.
    pub server_tool_use: Option<ServerToolUsage>,
}

/// A message from the streaming API.
///
/// This has a different structure from MessagesResponse since the streaming API
/// has nullable fields that get filled in during the stream.
#[derive(Debug, Deserialize)]
pub struct StreamingMessage {
    /// The ID of the response.
    pub id: String,
    /// The model used to generate the response.
    pub model: String,
    /// The reason the response was stopped (initially null).
    pub stop_reason: Option<StopReason>,
    /// The sequence that caused the response to be stopped (initially null).
    pub stop_sequence: Option<String>,
    /// Additional details when `stop_reason` is `Refusal`.
    ///
    /// Contains the refusal category and explanation.
    #[serde(default)]
    pub stop_details: Option<Value>,
    /// The usage statistics for the request.
    pub usage: Usage,
    /// The role of the message.
    pub role: Role,
    /// The contents of the message (initially empty).
    pub content: Vec<Content>,
    /// Context management information (beta feature).
    ///
    /// Present when context editing strategies are applied in long conversations.
    #[serde(default)]
    pub context_management: Option<Value>,
}

impl StreamingMessage {
    /// Update this message with a delta from a MessageDelta event.
    pub fn update(&mut self, delta: MessageDelta) {
        if let Some(stop_reason) = delta.stop_reason {
            self.stop_reason = Some(stop_reason);
        }
        if let Some(stop_sequence) = delta.stop_sequence {
            self.stop_sequence = Some(stop_sequence);
        }
    }
}

/// Decoded event from the streaming API.
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamEvent {
    /// Start of a message.
    ///
    /// The content of the message will be empty.
    MessageStart { message: StreamingMessage },
    /// Start of a content block.
    ContentBlockStart { index: u32, content_block: Content },
    /// Delta update to a content block.
    ContentBlockDelta { index: u32, delta: Delta },
    /// End of a content block.
    ContentBlockStop { index: u32 },
    /// Delta update to the message.
    MessageDelta {
        delta: MessageDelta,
        usage: Option<StreamingUsage>,
    },
    /// End of the message.
    MessageStop,
    /// Ping event (no data).
    Ping,
    /// Error event.
    Error { error: ApiError },
    /// Unknown event type that should be handled gracefully.
    ///
    /// It had a valid `type` tag, but nothing more is known about it.
    #[serde(skip)]
    Unknown {
        event_type: Vec<u8>,
        contents: serde_json::Value,
    },
}

/// Delta types for content block updates.
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Delta {
    TextDelta { text: String },
    InputJsonDelta { partial_json: String },
    ThinkingDelta { thinking: String },
    SignatureDelta { signature: String },
}

/// Delta updates to the message itself.
///
/// # Example
///
/// ```
/// use claus::anthropic::{StreamingMessage, MessageDelta, StopReason, Role, Usage};
///
/// let mut message = StreamingMessage {
///     id: "msg_123".to_string(),
///     model: "claude-test".to_string(),
///     stop_reason: None,
///     stop_sequence: None,
///     usage: Usage {
///         input_tokens: 10,
///         output_tokens: 5,
///         cache_creation_input_tokens: 0,
///         cache_read_input_tokens: 0,
///     },
///     role: Role::Assistant,
///     content: vec![],
/// };
///
/// let delta = MessageDelta {
///     stop_reason: Some(StopReason::EndTurn),
///     stop_sequence: None,
/// };
///
/// message.update(delta);
/// assert_eq!(message.stop_reason, Some(StopReason::EndTurn));
/// ```
#[derive(Debug, Deserialize, Clone)]
pub struct MessageDelta {
    pub stop_reason: Option<StopReason>,
    pub stop_sequence: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::{Content, Delta, StopReason, StreamEvent, Usage};

    #[test]
    fn test_deserialize_content_block_delta_text() {
        let data = br#"{"type": "content_block_delta", "index": 0, "delta": {"type": "text_delta", "text": "Hello"}}"#;

        let result: StreamEvent = serde_json::from_slice(data).unwrap();
        match result {
            StreamEvent::ContentBlockDelta { index, delta } => {
                assert_eq!(index, 0);
                match delta {
                    Delta::TextDelta { text } => assert_eq!(text, "Hello"),
                    _ => panic!("Expected TextDelta"),
                }
            }
            _ => panic!("Expected ContentBlockDelta"),
        }
    }

    #[test]
    fn test_deserialize_message_start() {
        let data = br#"{"type": "message_start", "message": {"id": "msg_1nZdL29xx5MUA1yADyHTEsnR8uuvGzszyY", "type": "message", "role": "assistant", "content": [], "model": "claude-opus-4-20250514", "stop_reason": null, "stop_sequence": null, "usage": {"input_tokens": 25, "output_tokens": 1}}}"#;

        let result = crate::deserialize_event(data).unwrap();
        match result {
            StreamEvent::MessageStart { message } => {
                // Now properly deserializes with the new StreamingMessage type
                assert_eq!(message.id, "msg_1nZdL29xx5MUA1yADyHTEsnR8uuvGzszyY");
                assert_eq!(message.model, "claude-opus-4-20250514");
                assert_eq!(message.stop_reason, None); // nullable in streaming
                assert_eq!(message.stop_sequence, None); // nullable in streaming
                assert_eq!(message.usage.input_tokens, 25);
                assert_eq!(message.usage.output_tokens, 1);
                assert!(message.content.is_empty());
            }
            other => {
                panic!("Expected MessageStart event, but got: {:?}", other);
            }
        }
    }

    #[test]
    fn test_deserialize_message_delta() {
        let data = br#"{"type": "message_delta", "delta": {"stop_reason": "end_turn", "stop_sequence": null}, "usage": {"output_tokens": 38}}"#;

        let result = crate::deserialize_event(data).unwrap();
        match result {
            StreamEvent::MessageDelta { delta, usage } => {
                assert_eq!(delta.stop_reason, Some(StopReason::EndTurn));
                assert_eq!(delta.stop_sequence, None);
                assert_eq!(usage.unwrap().output_tokens, Some(38));
            }
            StreamEvent::Unknown {
                event_type,
                contents,
            } => {
                eprintln!(
                    "Got Unknown: {:?}, contents: {:?}",
                    String::from_utf8_lossy(&event_type),
                    contents
                );
                panic!("Expected MessageDelta but got Unknown");
            }
            other => {
                panic!("Expected MessageDelta event, but got: {:?}", other);
            }
        }
    }

    #[test]
    fn test_usage_with_cache_fields() {
        let data = br#"{"input_tokens": 100, "output_tokens": 50, "cache_creation_input_tokens": 1000, "cache_read_input_tokens": 500}"#;
        let usage: Usage = serde_json::from_slice(data).expect("should deserialize");
        assert_eq!(usage.input_tokens, 100);
        assert_eq!(usage.output_tokens, 50);
        assert_eq!(usage.cache_creation_input_tokens, 1000);
        assert_eq!(usage.cache_read_input_tokens, 500);
    }

    #[test]
    fn test_usage_without_cache_fields() {
        let data = br#"{"input_tokens": 100, "output_tokens": 50}"#;
        let usage: Usage = serde_json::from_slice(data).expect("should deserialize");
        assert_eq!(usage.cache_creation_input_tokens, 0);
        assert_eq!(usage.cache_read_input_tokens, 0);
    }

    #[test]
    fn test_deserialize_server_tool_use() {
        let data = br#"{"type": "content_block_start", "index": 0, "content_block": {"type": "server_tool_use", "id": "srvtoolu_xxx", "name": "web_search", "input": {"query": "rust programming"}}}"#;
        let event: StreamEvent = serde_json::from_slice(data).expect("should deserialize");
        match event {
            StreamEvent::ContentBlockStart {
                content_block: Content::ServerToolUse { id, name, input },
                ..
            } => {
                assert_eq!(id, "srvtoolu_xxx");
                assert_eq!(name, "web_search");
                assert_eq!(input["query"], "rust programming");
            }
            _ => panic!("expected ContentBlockStart with ServerToolUse"),
        }
    }

    #[test]
    fn test_deserialize_web_search_result() {
        let data = br#"{"type": "content_block_start", "index": 1, "content_block": {"type": "web_search_tool_result", "tool_use_id": "srvtoolu_xxx", "content": [{"type": "web_search_result", "title": "Rust Programming Language", "url": "https://www.rust-lang.org/", "encrypted_content": "...", "page_age": "2 days ago"}]}}"#;
        let event: StreamEvent = serde_json::from_slice(data).expect("should deserialize");
        match event {
            StreamEvent::ContentBlockStart {
                content_block:
                    Content::WebSearchToolResult {
                        tool_use_id,
                        content,
                    },
                ..
            } => {
                assert_eq!(tool_use_id, "srvtoolu_xxx");
                assert_eq!(content.len(), 1);
                assert_eq!(content[0].title, "Rust Programming Language");
                assert_eq!(content[0].url, "https://www.rust-lang.org/");
                assert_eq!(content[0].page_age, Some("2 days ago".to_string()));
            }
            _ => panic!("expected ContentBlockStart with WebSearchToolResult"),
        }
    }

    #[test]
    fn test_deserialize_unknown_content() {
        let data = br#"{"type": "content_block_start", "index": 0, "content_block": {"type": "future_content_type", "some_field": "value"}}"#;
        let event: StreamEvent = serde_json::from_slice(data).expect("should deserialize");
        match event {
            StreamEvent::ContentBlockStart {
                content_block: Content::Unknown,
                ..
            } => {}
            _ => panic!("expected ContentBlockStart with Unknown"),
        }
    }
}

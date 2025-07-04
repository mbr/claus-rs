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

/// Content pieces that make up a message.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Content {
    Text {
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
}

impl Display for Content {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Content::Text { text } => f.write_str(text),
            Content::Image => f.write_str("<image>"),
            Content::ToolUse(tool_use) => tool_use.fmt(f),
            Content::ToolResult(tool_result) => tool_result.fmt(f),
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

/// The usage statistics for a request.
#[derive(Debug, Deserialize, Serialize)]
pub struct Usage {
    /// The number of tokens that were sent to the model.
    pub input_tokens: u32,
    /// The number of tokens that were generated by the model.
    pub output_tokens: u32,
}

/// The usage statistics for streaming events.
///
/// Unlike the regular [`Usage`], this allows for partial usage information
/// where some fields might be missing.
#[derive(Debug, Deserialize)]
pub struct StreamingUsage {
    /// The number of tokens that were sent to the model.
    pub input_tokens: Option<u32>,
    /// The number of tokens that were generated by the model.
    pub output_tokens: Option<u32>,
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
    /// The usage statistics for the request.
    pub usage: Usage,
    /// The role of the message.
    pub role: Role,
    /// The contents of the message (initially empty).
    pub content: Vec<Content>,
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
///     usage: Usage { input_tokens: 10, output_tokens: 5 },
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
    use super::{Delta, StopReason, StreamEvent};

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
}

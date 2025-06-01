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
}

impl Display for Content {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Content::Text { text } => f.write_str(text),
            Content::Image => f.write_str("<image>"),
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

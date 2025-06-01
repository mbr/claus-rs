//! Anthropic API related types.
//!
//! This module contains types that match the implemented Anthropic API.

use std::{fmt, fmt::Display, sync::Arc};

use serde::{Deserialize, Serialize};

use crate::Error;

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
    /// Note that `MessagesBody` uses `Arc`s to allow for cheaply copying conversations.
    #[serde(serialize_with = "serialize_arc_vec")]
    pub messages: &'a Vec<Arc<Message>>,
}

/// A role in a conversation.
///
/// The currrent API specification only supports `user` and `assistant` roles.
#[derive(Clone, Debug, Serialize, Deserialize)]
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
/// This is the "top-level" type for a response from the API.
#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ApiResponse {
    /// A response to a messages request.
    Message(MessagesResponse),
    /// An error response from the API.
    Error { error: ApiError },
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

/// Deserializes an Anthropic API response from JSON.
pub fn deserialize_response<T>(json: &str) -> Result<T, Error>
where
    T: TryFrom<ApiResponse>,
{
    let api_response: ApiResponse = serde_json::from_str(json)?;

    // Handle API errors explicitly
    if let ApiResponse::Error { error } = &api_response {
        return Err(Error::Api(error.clone()));
    }

    // Try conversion, handle failure case
    match T::try_from(api_response) {
        Ok(response) => Ok(response),
        Err(_) => Err(Error::UnexpectedResponseType),
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct MessagesResponse {
    pub id: String,
    pub model: String,
    pub stop_reason: String,
    pub stop_sequence: Option<String>,
    pub usage: Usage,
    #[serde(flatten)]
    pub message: Message,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Usage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}

fn serialize_arc_vec<S>(messages: &Vec<Arc<Message>>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    use serde::ser::SerializeSeq;
    let mut seq = serializer.serialize_seq(Some(messages.len()))?;
    for message in messages {
        seq.serialize_element(&**message)?;
    }
    seq.end()
}

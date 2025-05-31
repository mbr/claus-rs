//! Anthropic API related types.
//!
//! This module contains types that match the implemented Anthropic API.

use std::{fmt, fmt::Display, sync::Arc};

use serde::{Deserialize, Serialize};

use crate::{Error, serialize_arc_vec};

/// API version that is compatible with this module.
pub const ANTHROPIC_VERSION: &str = "2023-06-01";

/// Default API endpoint host to use.
pub const DEFAULT_ENDPOINT_HOST: &str = "api.anthropic.com";

/// Default model to use for requests.
pub const DEFAULT_MODEL: &str = "claude-sonnet-4-20250514";

#[derive(Debug, Serialize)]
pub struct MessagesBody<'a> {
    pub model: &'a str,
    pub max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<&'a str>,
    #[serde(serialize_with = "serialize_arc_vec")]
    pub messages: &'a Vec<Arc<Message>>,
}

#[derive(Clone, Debug)]
pub enum Role {
    User,
    Assistant,
    Other(String),
}

impl serde::Serialize for Role {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Role::User => serializer.serialize_str("user"),
            Role::Assistant => serializer.serialize_str("assistant"),
            Role::Other(s) => serializer.serialize_str(s),
        }
    }
}

impl<'de> serde::Deserialize<'de> for Role {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            "user" => Ok(Role::User),
            "assistant" => Ok(Role::Assistant),
            _ => Ok(Role::Other(s)),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Message {
    pub role: Role,
    pub content: Vec<Content>,
}

impl Message {
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

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Content {
    Text { text: String },
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
    pub fn from_text<S: Into<String>>(text: S) -> Self {
        Content::Text { text: text.into() }
    }

    pub fn as_text(&self) -> Option<&str> {
        if let Content::Text { text } = self {
            return Some(text.as_str());
        }
        None
    }
}

/// Anthropic API error.
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
#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ApiResponse {
    Message(MessagesResponse),
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

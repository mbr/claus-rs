//! Klaus. The AI client, Claude's German second-degree cousin.
//!
//! The crate separates IO from the protocol, thus it can be run with a variety of backends.
//!
//! At its core sits the [`Api`] struct, which holds common information for all requests. A
//! typical interaction is through the [`MessageRequestBuilder`]:
//!
//! ```
//! use klaus::{Api, HttpRequest, MessagesRequestBuilder, Role};
//!
//! let api = Api::new("sk-ant-api03-...");
//!
//! let http_request: HttpRequest = MessagesRequestBuilder::new()
//!     .push_message(Role::User, "Hello, world!")
//!     .build(&api);
//!
//! assert_eq!(http_request.url.as_ref(), "https://api.anthropic.com/v1/messages");
//! assert_eq!(http_request.method, "POST");
//!
//! assert_eq!(
//!     http_request.render_headers(),
//!     "content-type: application/json\n\
//!      anthropic-version: 2023-06-01\n\
//!      x-api-key: sk-ant-api03-...\n\
//!      anthropic-model: claude-sonnet-4-20250514\n\
//!      max-tokens: 1024"
//! );
//!
//! assert_eq!(
//!     &http_request.body,
//!     r#"{"messages":[{"role":"user","content":{"type":"text","text":"Hello, world!"}}]}"#
//! );
//!
//! // now the request can be sent with any HTTP client
//! ```

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json;

/// API version that is compatible with this module.
pub const ANTHROPIC_VERSION: &str = "2023-06-01";

/// Default API endpoint host to use.
pub const DEFAULT_ENDPOINT_HOST: &str = "api.anthropic.com";

/// Default model to use for requests.
pub const DEFAULT_MODEL: &str = "claude-sonnet-4-20250514";

/// An Anthropic API configuration.
#[derive(Debug)]
pub struct Api {
    /// The Anthropic API key.
    api_key: Arc<str>,
    /// The default model to use for requests.
    default_model: Arc<str>,
    /// The default maximum number of tokens for responses.
    default_max_tokens: u32,
    /// The API endpoint host (without protocol or path).
    endpoint_host: Arc<str>,
}

impl Api {
    /// Creates a new Anthropic API instance.
    ///
    /// Requires a valid Anthropic API key.
    pub fn new<S: Into<Arc<str>>>(api_key: S) -> Self {
        Self {
            api_key: api_key.into(),
            default_model: Arc::from(DEFAULT_MODEL),
            default_max_tokens: 1024,
            endpoint_host: Arc::from(DEFAULT_ENDPOINT_HOST),
        }
    }

    /// Sets the default model for requests.
    ///
    /// If not set, [`DEFAULT_MODEL`] will be used.
    pub fn default_model<S: Into<Arc<str>>>(mut self, model: S) -> Self {
        self.default_model = model.into();
        self
    }

    /// Sets the default maximum tokens for responses.
    ///
    /// If not set, the default is 1024.
    pub fn default_max_tokens(mut self, max_tokens: u32) -> Self {
        self.default_max_tokens = max_tokens;
        self
    }

    /// Sets the API endpoint host.
    ///
    /// If not set, [`DEFAULT_ENDPOINT_HOST`] will be used.
    pub fn endpoint_host<S: Into<Arc<str>>>(mut self, endpoint_host: S) -> Self {
        self.endpoint_host = endpoint_host.into();
        self
    }

    /// Creates the required headers for any API request.
    fn create_default_headers(&self) -> Vec<(&'static str, Arc<str>)> {
        vec![
            ("content-type", Arc::from("application/json")),
            ("anthropic-version", Arc::from(ANTHROPIC_VERSION)),
            ("x-api-key", self.api_key.clone()),
        ]
    }
}

/// HTTP request encapsulation.
///
/// This type represents an HTTP request that can be sent to the Anthropic API.
#[derive(Debug)]
pub struct HttpRequest {
    /// Request host.
    pub host: String,
    /// Request path.
    pub path: String,
    /// HTTP method.
    pub method: &'static str,
    /// Request headers.
    pub headers: Vec<(&'static str, Arc<str>)>,
    /// Request body.
    pub body: String,
}

impl HttpRequest {
    pub fn render_headers(&self) -> String {
        self.headers
            .iter()
            .map(|(k, v)| format!("{}: {}", k, v.as_ref()))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

impl std::fmt::Display for HttpRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Write request line
        writeln!(f, "{} {} HTTP/1.1", self.method, self.path)?;
        
        // Write Host header first
        writeln!(f, "Host: {}", self.host)?;

        // Write other headers
        for (key, value) in &self.headers {
            writeln!(f, "{}: {}", key, value.as_ref())?;
        }

        // Empty line between headers and body
        writeln!(f)?;

        // Write body
        write!(f, "{}", self.body)?;

        Ok(())
    }
}

#[derive(Debug)]
pub struct MessagesRequestBuilder {
    /// The model to use for the request.
    ///
    /// If none is provided, the default model will be used.
    model: Option<String>,
    /// The maximum number of tokens for the response.
    ///
    /// If none is provided, the default max tokens will be used.
    max_tokens: Option<u32>,
    /// The messages to send.
    messages: Vec<Arc<Message>>,
    // Note: Missing: container, mcp_servers, metadata, service_tier,
    //                stop_sequences, stream, system, temperature, thinking,
    //                tool_choice, tools, top_k, top_p
}

#[derive(Debug, Serialize)]
struct MessagesBody<'a> {
    #[serde(serialize_with = "serialize_arc_vec")]
    messages: &'a Vec<Arc<Message>>,
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

impl MessagesRequestBuilder {
    /// Creates a new message request builder.
    pub fn new() -> Self {
        Self {
            model: None,
            max_tokens: None,
            messages: Vec::new(),
        }
    }

    /// Sets the model for the request.
    pub fn model<S: Into<String>>(mut self, model: S) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Sets the maximum tokens for the request.
    pub fn max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }

    /// Appends a message to the request.
    pub fn push<M: Into<Arc<Message>>>(mut self, message: M) -> Self {
        self.messages.push(message.into());
        self
    }

    /// Constructs and appends a message to the request.
    pub fn push_message<S: Into<String>>(self, role: Role, content: S) -> Self {
        let message = Message {
            role,
            content: Content::Text {
                text: content.into(),
            },
        };
        self.push(message)
    }

    /// Builds the HTTP request.
    pub fn build(&self, api: &Api) -> HttpRequest {
        let mut headers = api.create_default_headers();

        if let Some(model) = &self.model {
            headers.push(("anthropic-model", Arc::from(model.as_str())));
        } else {
            headers.push(("anthropic-model", api.default_model.clone()));
        }

        if let Some(max_tokens) = self.max_tokens {
            headers.push(("max-tokens", Arc::from(max_tokens.to_string())));
        } else {
            headers.push(("max-tokens", Arc::from(api.default_max_tokens.to_string())));
        }

        let body = MessagesBody {
            messages: &self.messages,
        };

        let body = serde_json::to_string(&body).expect("failed to serialize messages");

        HttpRequest {
            host: api.endpoint_host.to_string(),
            path: "/v1/messages".to_string(),
            method: "POST",
            headers,
            body,
        }
    }
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
        let s = <&str>::deserialize(deserializer)?;
        match s {
            "user" => Ok(Role::User),
            "assistant" => Ok(Role::Assistant),
            _ => Ok(Role::Other(s.to_string())),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Message {
    role: Role,
    content: Content,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum Content {
    Text { text: String },
    // not supported: Image
}

/// Anthropic API error.
#[derive(Debug, thiserror::Error)]

enum Error {}

// Below are features that may be feature-gated later.

#[cfg(feature = "reqwest")]
impl HttpRequest {
    /// Converts this HttpRequest into a reqwest::Request.
    pub fn try_into_reqwest(self) -> Result<reqwest::Request, Box<dyn std::error::Error>> {
        let method = reqwest::Method::from_bytes(self.method.as_bytes())?;

        let url_string = format!("https://{}{}", self.host, self.path);
        let url = reqwest::Url::parse(&url_string)?;
        let mut request = reqwest::Request::new(method, url);

        // Set body
        *request.body_mut() = Some(self.body.into());

        // Add headers
        let headers = request.headers_mut();
        for (key, value) in self.headers {
            let header_name = reqwest::header::HeaderName::from_bytes(key.as_bytes())?;
            let header_value = reqwest::header::HeaderValue::from_str(&value)?;
            headers.insert(header_name, header_value);
        }

        Ok(request)
    }
}

#[cfg(feature = "reqwest-blocking")]
impl HttpRequest {
    /// Converts this HttpRequest into a reqwest::blocking::Request.
    pub fn try_into_reqwest_blocking(self) -> Result<reqwest::blocking::Request, Box<dyn std::error::Error>> {
        let method = reqwest::Method::from_bytes(self.method.as_bytes())?;

        let url_string = format!("https://{}{}", self.host, self.path);
        let url = reqwest::Url::parse(&url_string)?;
        let mut request = reqwest::blocking::Request::new(method, url);

        // Set body
        *request.body_mut() = Some(self.body.into());

        // Add headers
        let headers = request.headers_mut();
        for (key, value) in self.headers {
            let header_name = reqwest::header::HeaderName::from_bytes(key.as_bytes())?;
            let header_value = reqwest::header::HeaderValue::from_str(&value)?;
            headers.insert(header_name, header_value);
        }

        Ok(request)
    }
}

#[cfg(feature = "reqwest")]
impl From<HttpRequest> for reqwest::Request {
    fn from(http_request: HttpRequest) -> Self {
        http_request.try_into_reqwest().expect("failed to convert to reqwest::Request")
    }
}

#[cfg(feature = "reqwest-blocking")]
impl From<HttpRequest> for reqwest::blocking::Request {
    fn from(http_request: HttpRequest) -> Self {
        http_request.try_into_reqwest_blocking().expect("failed to convert to reqwest::blocking::Request")
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::HttpRequest;

    #[cfg(feature = "reqwest")]
    #[test]
    fn test_http_request_to_reqwest_conversion() {
        let http_request = HttpRequest {
            host: "api.anthropic.com".to_string(),
            path: "/v1/messages".to_string(),
            method: "POST",
            headers: vec![
                ("content-type", Arc::from("application/json")),
                ("anthropic-version", Arc::from("2023-06-01")),
                ("x-api-key", Arc::from("test-key")),
                ("anthropic-model", Arc::from("claude-3-sonnet-20240229")),
                ("max-tokens", Arc::from("1024")),
            ],
            body:
                r#"{"messages":[{"role":"user","content":{"type":"text","text":"Hello, world!"}}]}"#
                    .to_string(),
        };

        // Convert to reqwest::Request
        let reqwest_request: reqwest::Request = http_request
            .try_into()
            .expect("should convert successfully");

        assert_eq!(reqwest_request.method(), &reqwest::Method::POST);
        assert_eq!(
            reqwest_request.url().as_str(),
            "https://api.anthropic.com/v1/messages"
        );

        let headers = reqwest_request.headers();
        assert_eq!(headers.get("content-type").unwrap(), "application/json");
        assert_eq!(headers.get("anthropic-version").unwrap(), "2023-06-01");
        assert_eq!(headers.get("x-api-key").unwrap(), "test-key");

        let body = reqwest_request.body().unwrap();
        let body_bytes = body.as_bytes().unwrap();
        let body_str = std::str::from_utf8(body_bytes).unwrap();
        assert!(body_str.contains("Hello, world!"));
        assert!(body_str.contains("\"type\":\"text\""));
    }

    #[cfg(feature = "reqwest-blocking")]
    #[test]
    fn test_http_request_to_reqwest_blocking_conversion() {
        let http_request = HttpRequest {
            host: "api.anthropic.com".to_string(),
            path: "/v1/messages".to_string(),
            method: "POST",
            headers: vec![
                ("content-type", Arc::from("application/json")),
                ("anthropic-version", Arc::from("2023-06-01")),
                ("x-api-key", Arc::from("test-key")),
                ("anthropic-model", Arc::from("claude-3-sonnet-20240229")),
                ("max-tokens", Arc::from("1024")),
            ],
            body:
                r#"{"messages":[{"role":"user","content":{"type":"text","text":"Hello, world!"}}]}"#
                    .to_string(),
        };

        // Convert to reqwest::blocking::Request
        let reqwest_request: reqwest::blocking::Request = http_request
            .try_into()
            .expect("should convert successfully");

        assert_eq!(reqwest_request.method(), &reqwest::Method::POST);
        assert_eq!(
            reqwest_request.url().as_str(),
            "https://api.anthropic.com/v1/messages"
        );

        let headers = reqwest_request.headers();
        assert_eq!(headers.get("content-type").unwrap(), "application/json");
        assert_eq!(headers.get("anthropic-version").unwrap(), "2023-06-01");
        assert_eq!(headers.get("x-api-key").unwrap(), "test-key");

        let body = reqwest_request.body().unwrap();
        let body_bytes = body.as_bytes().unwrap();
        let body_str = std::str::from_utf8(body_bytes).unwrap();
        assert!(body_str.contains("Hello, world!"));
        assert!(body_str.contains("\"type\":\"text\""));
    }
}

#![doc = include_str!("../README.md")]

pub mod anthropic;
pub mod conversation;

use std::{fmt, sync::Arc};

use anthropic::MessagesBody;

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
            default_model: Arc::from(anthropic::DEFAULT_MODEL),
            default_max_tokens: 1024,
            endpoint_host: Arc::from(anthropic::DEFAULT_ENDPOINT_HOST),
        }
    }

    /// Sets the default model for requests.
    ///
    /// If not set, [`anthropic::DEFAULT_MODEL`] will be used.
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
    /// If not set, [`anthropic::DEFAULT_ENDPOINT_HOST`] will be used.
    pub fn endpoint_host<S: Into<Arc<str>>>(mut self, endpoint_host: S) -> Self {
        self.endpoint_host = endpoint_host.into();
        self
    }

    /// Creates the required headers for any API request.
    fn create_default_headers(&self) -> Vec<(&'static str, Arc<str>)> {
        vec![
            ("content-type", Arc::from("application/json")),
            ("anthropic-version", Arc::from(anthropic::ANTHROPIC_VERSION)),
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

impl fmt::Display for HttpRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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
    /// The system prompt for the conversation.
    system: Option<String>,
    /// The messages to send.
    messages: Vec<Arc<anthropic::Message>>,
    // Note: Missing: container, mcp_servers, metadata, service_tier,
    //                stop_sequences, stream, temperature, thinking,
    //                tool_choice, tools, top_k, top_p
}

fn serialize_arc_vec<S>(
    messages: &Vec<Arc<anthropic::Message>>,
    serializer: S,
) -> Result<S::Ok, S::Error>
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

impl Default for MessagesRequestBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl MessagesRequestBuilder {
    /// Creates a new message request builder.
    pub fn new() -> Self {
        Self {
            model: None,
            max_tokens: None,
            system: None,
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

    /// Sets the system prompt for the request.
    pub fn system<S: Into<String>>(mut self, system: S) -> Self {
        self.system = Some(system.into());
        self
    }

    /// Appends a message to the request.
    pub fn push<M: Into<Arc<anthropic::Message>>>(mut self, message: M) -> Self {
        self.messages.push(message.into());
        self
    }

    /// Constructs and appends a message to the request.
    pub fn push_message<S: Into<String>>(self, role: anthropic::Role, text: S) -> Self {
        let message = anthropic::Message::from_text(role, text);
        self.push(message)
    }

    /// Replace all messages in the request with given messages.
    pub fn set_messages(mut self, messages: Vec<Arc<anthropic::Message>>) -> Self {
        self.messages = messages;
        self
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

        let body = {
            let model = if let Some(ref model) = self.model {
                model.as_str()
            } else {
                &api.default_model
            };

            let system = self.system.as_deref();

            let body = MessagesBody {
                model,
                max_tokens: self.max_tokens.unwrap_or(api.default_max_tokens),
                system,
                messages: &self.messages,
            };

            serde_json::to_string(&body).expect("failed to serialize messages")
        };

        HttpRequest {
            host: api.endpoint_host.to_string(),
            path: "/v1/messages".to_string(),
            method: "POST",
            headers,
            body,
        }
    }
}

/// An Anthropic API error.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Deserialization error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("API error: {0}")]
    Api(#[from] anthropic::ApiError),
    #[error("Unexpected response type")]
    UnexpectedResponseType,
}

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
    pub fn try_into_reqwest_blocking(
        self,
    ) -> Result<reqwest::blocking::Request, Box<dyn std::error::Error>> {
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
        http_request
            .try_into_reqwest()
            .expect("failed to convert to reqwest::Request")
    }
}

#[cfg(feature = "reqwest-blocking")]
impl From<HttpRequest> for reqwest::blocking::Request {
    fn from(http_request: HttpRequest) -> Self {
        http_request
            .try_into_reqwest_blocking()
            .expect("failed to convert to reqwest::blocking::Request")
    }
}

#[cfg(test)]
mod tests {
    use super::anthropic::{ApiError, Content, MessagesResponse, Role, deserialize_response};

    #[cfg(feature = "reqwest")]
    #[test]
    fn test_http_request_to_reqwest_conversion() {
        let http_request = super::HttpRequest {
            host: "api.anthropic.com".to_string(),
            path: "/v1/messages".to_string(),
            method: "POST",
            headers: vec![
                ("content-type", std::sync::Arc::from("application/json")),
                ("anthropic-version", std::sync::Arc::from("2023-06-01")),
                ("x-api-key", std::sync::Arc::from("test-key")),
                (
                    "anthropic-model",
                    std::sync::Arc::from("claude-3-sonnet-20240229"),
                ),
                ("max-tokens", std::sync::Arc::from("1024")),
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
        let http_request = super::HttpRequest {
            host: "api.anthropic.com".to_string(),
            path: "/v1/messages".to_string(),
            method: "POST",
            headers: vec![
                ("content-type", std::sync::Arc::from("application/json")),
                ("anthropic-version", std::sync::Arc::from("2023-06-01")),
                ("x-api-key", std::sync::Arc::from("test-key")),
                (
                    "anthropic-model",
                    std::sync::Arc::from("claude-3-sonnet-20240229"),
                ),
                ("max-tokens", std::sync::Arc::from("1024")),
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

    #[test]
    fn test_api_response_error_deserialization() {
        let json = r#"{
  "type": "error",
  "error": {
    "type": "not_found_error",
    "message": "The requested resource could not be found."
  }
}"#;

        let result: Result<MessagesResponse, _> = deserialize_response(json);

        assert!(result.is_err());
        if let Err(super::Error::Api(api_error)) = result {
            assert!(matches!(api_error, ApiError::NotFoundError));
        } else {
            panic!("Expected Api error");
        }
    }

    #[test]
    fn test_api_response_invalid_request_deserialization() {
        let json = r#"{
  "error": {
    "message": "Invalid request",
    "type": "invalid_request_error"
  },
  "type": "error"
}"#;

        let result: Result<MessagesResponse, _> = deserialize_response(json);

        assert!(result.is_err());
        if let Err(super::Error::Api(api_error)) = result {
            assert!(matches!(api_error, ApiError::InvalidRequestError));
        } else {
            panic!("Expected Api error");
        }
    }

    #[test]
    fn test_api_response_message_deserialization() {
        let json = r#"{
  "content": [
    {
      "text": "Hi! My name is Claude.",
      "type": "text"
    }
  ],
  "id": "msg_013Zva2CMHLNnXjNJJKqJ2EF",
  "model": "claude-3-7-sonnet-20250219",
  "role": "assistant",
  "stop_reason": "end_turn",
  "stop_sequence": null,
  "type": "message",
  "usage": {
    "input_tokens": 2095,
    "output_tokens": 503
  }
}"#;

        let response: MessagesResponse =
            deserialize_response(json).expect("should deserialize API message response");

        assert_eq!(response.id, "msg_013Zva2CMHLNnXjNJJKqJ2EF");
        assert_eq!(response.model, "claude-3-7-sonnet-20250219");
        assert!(matches!(response.message.role, Role::Assistant));
        assert_eq!(response.stop_reason, "end_turn");
        assert_eq!(response.stop_sequence, None);
        assert_eq!(response.usage.input_tokens, 2095);
        assert_eq!(response.usage.output_tokens, 503);
        assert_eq!(response.message.content.len(), 1);

        let Content::Text { text } = &response.message.content[0] else {
            panic!("should be text");
        };
        assert_eq!(text, "Hi! My name is Claude.");
    }

    #[test]
    fn test_messages_request_builder_with_system_prompt() {
        let api = super::Api::new("test-api-key");

        let http_request = super::MessagesRequestBuilder::new()
            .system("You are a helpful assistant.")
            .push_message(super::anthropic::Role::User, "Hello!")
            .build(&api);

        assert_eq!(http_request.method, "POST");
        assert_eq!(http_request.path, "/v1/messages");
        assert_eq!(http_request.host, "api.anthropic.com");

        // Verify the body contains the system prompt
        assert!(
            http_request
                .body
                .contains("\"system\":\"You are a helpful assistant.\"")
        );
        assert!(http_request.body.contains("\"messages\":["));
        assert!(http_request.body.contains("\"role\":\"user\""));
        assert!(http_request.body.contains("\"text\":\"Hello!\""));
    }
}

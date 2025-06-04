#![doc = include_str!("../README.md")]

/// Make it easier for users to hold shares message histories, if necessary.
pub use im;

pub mod anthropic;
pub mod conversation;
pub mod http_request;

use std::sync::Arc;

use crate::{anthropic::ApiResponse, http_request::HttpRequest};

/// A client for the Anthropic API.
///
/// The [`Api`] struct holds configuration necessary to make API requests. Create one using
/// [`Api::new`] with your API key, then use it to build requests with types like
/// [`MessagesRequestBuilder`].
#[derive(Clone, Debug)]
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
    /// Creates a new [`Api`] client with the given API key.
    ///
    /// Requires a valid Anthropic API key. If you do not have one, you can get one from
    /// [Anthropic's console](https://console.anthropic.com/settings/keys).
    pub fn new<S: Into<Arc<str>>>(api_key: S) -> Self {
        Self {
            api_key: api_key.into(),
            default_model: Arc::from(anthropic::DEFAULT_MODEL),
            default_max_tokens: 1024,
            endpoint_host: Arc::from(anthropic::DEFAULT_ENDPOINT_HOST),
        }
    }

    /// Sets the default model to use for requests.
    ///
    /// If not set, [`anthropic::DEFAULT_MODEL`] will be used.
    pub fn default_model<S: Into<Arc<str>>>(mut self, model: S) -> Self {
        self.default_model = model.into();
        self
    }

    /// Sets the default maximum tokens for responses.
    ///
    /// Responses will be cut off at this number of tokens, but may end earlier if the model
    /// decides to do so.
    ///
    /// If not set, the default is 1024.
    pub fn default_max_tokens(mut self, max_tokens: u32) -> Self {
        self.default_max_tokens = max_tokens;
        self
    }

    /// Sets the API endpoint host.
    ///
    /// This can only be a hostname, not a full URL.
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

/// Request builder for the `messages` endpoint.
///
/// This builder is used to construct [`HttpRequest`]s for the `messages` endpoint. Once sent,
/// you should expect to receive a [`crate::anthropic::MessagesResponse`] from the API, see
/// [`crate::anthropic::deserialize_response`] for details.
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
    system: Option<Arc<str>>,
    /// The messages to send.
    messages: im::Vector<anthropic::Message>,
    /// Tools available for the model to use.
    tools: Option<im::Vector<anthropic::Tool>>,
    // Note: Missing: container, mcp_servers, metadata, service_tier,
    //                stop_sequences, stream, temperature, thinking,
    //                tool_choice, top_k, top_p
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
            messages: im::Vector::new(),
            tools: None,
        }
    }

    /// Sets the model for the request.
    ///
    /// If not set, uses the default model set by [`Api`].
    pub fn model<S: Into<String>>(mut self, model: S) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Sets the maximum tokens for the request.
    ///
    /// If not set, uses the default max tokens set by [`Api`].
    pub fn max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }

    /// Sets the system prompt for the request.
    ///
    /// A system prompt is a message that will always be sent to the model at the beginning of the
    /// conversation. See [Anthropic's documentation](https://docs.anthropic.com/en/api/system-prompts) for
    /// more details.
    ///
    /// If not set, no system prompt is included in the request.
    pub fn system<S: Into<Arc<str>>>(mut self, system: S) -> Self {
        self.system = Some(system.into());
        self
    }

    /// Appends a message to the request.
    pub fn push(mut self, message: anthropic::Message) -> Self {
        self.messages.push_back(message);
        self
    }

    /// Constructs and appends a message to the request.
    ///
    /// This is a convenience method to construct a [`Message`] with a single text [`Content`].
    pub fn push_message<S: Into<String>>(self, role: anthropic::Role, text: S) -> Self {
        let message = anthropic::Message::from_text(role, text);
        self.push(message)
    }

    /// Replace all messages in the request with given messages.
    pub fn set_messages(mut self, messages: im::Vector<anthropic::Message>) -> Self {
        self.messages = messages;
        self
    }

    /// Sets the tools available for the model to use.
    pub fn set_tools<T: Into<im::Vector<anthropic::Tool>>>(mut self, tools: T) -> Self {
        self.tools = Some(tools.into());
        self
    }

    /// Builds the HTTP request.
    ///
    /// The resulting [`HttpRequest`] can be sent to the API using a suitable HTTP client.
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

            let body = anthropic::MessagesBody {
                model,
                max_tokens: self.max_tokens.unwrap_or(api.default_max_tokens),
                system,
                messages: &self.messages,
                tools: self.tools.as_ref(),
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

/// A unified error for responses from the API.
#[derive(Debug, thiserror::Error)]
pub enum ResponseError {
    /// The given JSON could not be parsed.
    #[error("Deserialization error: {0}")]
    Serde(#[from] serde_json::Error),
    /// The API returned an explicit error (see [`anthropic::ApiError`]).
    #[error("API error: {0}")]
    Api(#[from] anthropic::ApiError),
    /// The API returned a response, but it was not the expected type.
    #[error("Unexpected response type")]
    UnexpectedResponseType {
        expected: &'static str,
        actual: &'static str,
    },
}

/// Deserializes an Anthropic API response from JSON.
///
/// This is the central low-level entry point for parsing responses from the API.
pub fn deserialize_response<T>(json: &str) -> Result<T, ResponseError>
where
    T: TryFrom<ApiResponse, Error = ()>,
{
    let api_response: ApiResponse = serde_json::from_str(json)?;

    match api_response {
        ApiResponse::Error { error } => Err(ResponseError::Api(error)),
        other => {
            let kind = other.kind();
            other
                .try_into()
                .map_err(|()| ResponseError::UnexpectedResponseType {
                    expected: std::any::type_name::<T>(),
                    actual: kind,
                })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        anthropic::{ApiError, Content, MessagesResponse, Role, StopReason},
        deserialize_response,
    };

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
        if let Err(super::ResponseError::Api(api_error)) = result {
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
        if let Err(super::ResponseError::Api(api_error)) = result {
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
        assert_eq!(response.stop_reason, StopReason::EndTurn);
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

    #[test]
    fn test_messages_request_builder_with_tools() {
        use schemars::JsonSchema;

        #[derive(JsonSchema)]
        #[allow(dead_code)]
        struct WeatherInput {
            /// The city and state, e.g. San Francisco, CA
            location: String,
            /// Unit for the output - one of (celsius, fahrenheit)
            unit: Option<String>,
        }

        let api = super::Api::new("test-api-key");

        // Create a weather tool
        let weather_tool = super::anthropic::Tool::new::<WeatherInput, _, _>(
            "get_weather",
            "Get the current weather in a given location",
        );

        let tools = im::vector![weather_tool];

        let http_request = super::MessagesRequestBuilder::new()
            .push_message(
                super::anthropic::Role::User,
                "What's the weather in San Francisco?",
            )
            .set_tools(tools)
            .build(&api);

        assert_eq!(http_request.method, "POST");
        assert_eq!(http_request.path, "/v1/messages");
        assert_eq!(http_request.host, "api.anthropic.com");

        // Verify the body contains the tools
        assert!(http_request.body.contains("\"tools\":["));
        assert!(http_request.body.contains("\"name\":\"get_weather\""));
        assert!(
            http_request
                .body
                .contains("\"description\":\"Get the current weather in a given location\"")
        );
        assert!(http_request.body.contains("\"input_schema\""));
        assert!(http_request.body.contains("\"properties\""));
        assert!(http_request.body.contains("\"location\""));
        assert!(http_request.body.contains("\"unit\""));
        assert!(http_request.body.contains("\"required\":[\"location\"]"));

        // Verify the message is also present
        assert!(http_request.body.contains("\"messages\":["));
        assert!(http_request.body.contains("\"role\":\"user\""));
        assert!(
            http_request
                .body
                .contains("\"What's the weather in San Francisco?\"")
        );
    }
}

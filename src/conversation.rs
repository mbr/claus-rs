//! Conversation management for ongoing chats with the API.
//!
//! To interact, a caller creates a conversation, then calls [`Conversation::user_message`] to
//! obtain a message it is expected to send to the API.
//!
//! Once a response to this message is received, the caller should call
//! [`Conversation::handle_response`] to handle the response from the API.
//!
//! Calls to [`Conversation::handle_response`] will return an [`Action`], which the caller must
//! process (see [`Action`] for more details).
//!
//! ## State management
//!
//! Every conversation holds a series of messages and some configuration, since the Anthropic API
//! does not persist any state remotely. For one, this means that any number of conversations can
//! be created and managed in parallel, using the same [`Api`] instance.
//!
//! To persist the state of a conversation the [`Conversation`] itself can be serialized and
//! deserialized using [`serde`]. Additionally the convenience [`Conversation::to_json`] and
//! [`Conversation::from_json`] methods can be used.
//!
//! ## Example
//!
//! ```no_run
//! use klaus::{Api, conversation::Conversation};
//!
//! let api = Api::new("sk-ant-api03-...");
//! let mut conversation = Conversation::new();
//!
//! // Set a system prompt
//! conversation.set_system("You are a helpful assistant.");
//!
//! // Send a user message
//! let http_request = conversation.user_message(&api, "Hello!");
//!
//! // ... send http_request and get response_json ...
//! # let response_json = r#"{"type":"message","id":"msg_123","model":"claude-sonnet-4-20250514","stop_reason":"end_turn","stop_sequence":null,"usage":{"input_tokens":10,"output_tokens":5},"role":"assistant","content":[{"type":"text","text":"Hello!"}]}"#;
//!
//! // Handle the response
//! match conversation.handle_response(response_json) {
//!     Ok(action) => match action {
//!         klaus::conversation::Action::HandleAgentMessage(content) => {
//!             for item in content {
//!                 println!("Assistant: {}", item);
//!             }
//!         }
//!     },
//!     Err(e) => eprintln!("Error: {}", e),
//! }
//!
//! // Save conversation state
//! let mut buffer = Vec::new();
//! conversation.to_json(&mut buffer).unwrap();
//!
//! // Later, restore conversation state
//! let restored_conversation = Conversation::from_json(&buffer[..]).unwrap();
//! ```
//!

use std::{io, sync::Arc};

use serde::{Deserialize, Serialize};

use crate::{Api, ResponseError, anthropic, anthropic::Message, http_request::HttpRequest};

/// Actions that the caller needs to take based on the API response.
#[derive(Debug)]
pub enum Action {
    /// Handle a message from the agent/assistant.
    HandleAgentMessage(Vec<anthropic::Content>),
}

/// A conversation that manages message history and generates HTTP requests.
#[derive(Debug, Serialize, Deserialize)]
pub struct Conversation {
    /// The system prompt for the conversation.
    system: Option<Arc<str>>,
    /// The conversation's message history.
    messages: im::Vector<anthropic::Message>,
    /// Tools available for the model to use.
    tools: im::Vector<anthropic::Tool>,
}

impl Conversation {
    /// Creates a new conversation.
    pub fn new() -> Self {
        Self {
            system: None,
            messages: im::Vector::new(),
            tools: im::Vector::new(),
        }
    }

    /// Sets the system prompt for the conversation.
    ///
    /// By default, the system prompt is not set.
    pub fn set_system<S: Into<Arc<str>>>(&mut self, system: S) -> &mut Self {
        self.system = Some(system.into());
        self
    }

    /// Adds a user message and returns an HTTP request to send.
    ///
    /// The message will automatically be added to the conversation history.
    pub fn user_message<S: Into<String>>(&mut self, api: &Api, user_message: S) -> HttpRequest {
        let message = anthropic::Message::from_text(anthropic::Role::User, user_message);
        self.build_message(api, message)
    }

    /// Adds tool results to the conversation and returns an HTTP request to send.
    ///
    /// The tool results will be added as a user message to the conversation history.
    pub fn tool_result(&mut self, api: &Api, tool_result: anthropic::ToolResult) -> HttpRequest {
        let content = vec![anthropic::Content::ToolResult(tool_result)];

        let message = anthropic::Message {
            role: anthropic::Role::User,
            content,
        };
        self.build_message(api, message)
    }

    /// Common logic for building and sending messages.
    fn build_message(&mut self, api: &Api, message: anthropic::Message) -> HttpRequest {
        self.messages.push_back(message);

        let mut builder = crate::MessagesRequestBuilder::new().set_messages(self.messages.clone());

        if let Some(ref system) = self.system {
            builder = builder.system(system.clone());
        }

        if !self.tools.is_empty() {
            builder = builder.set_tools(self.tools.clone());
        }

        builder.build(api)
    }

    /// Handles the response from the API and returns the action to take.
    ///
    /// This method parses the response, adds the assistant's message to the conversation
    /// history, and returns the appropriate action for the caller to take.
    pub fn handle_response(&mut self, response_json: &str) -> Result<Action, ResponseError> {
        let response: anthropic::MessagesResponse = crate::deserialize_response(response_json)?;

        // Add assistant's message to history
        self.messages.push_back(response.message.clone());

        Ok(Action::HandleAgentMessage(response.message.content))
    }

    /// Serializes the conversation to JSON using the provided writer.
    pub fn to_json<W: io::Write>(&self, writer: W) -> Result<(), serde_json::Error> {
        serde_json::to_writer(writer, self)
    }

    /// Deserializes a conversation from JSON using the provided reader.
    pub fn from_json<R: io::Read>(reader: R) -> Result<Self, serde_json::Error> {
        serde_json::from_reader(reader)
    }

    /// Clears the conversation history.
    pub fn clear(&mut self) {
        self.messages = im::Vector::new();
    }

    /// Returns the message history.
    pub fn history(&self) -> &im::Vector<Message> {
        &self.messages
    }

    /// Adds a tool to the conversation.
    ///
    /// Tools are available to the model and will be included in all subsequent requests.
    pub fn add_tool(&mut self, tool: anthropic::Tool) -> &mut Self {
        self.tools.push_back(tool);
        self
    }

    /// Sets the tools for the conversation.
    ///
    /// This replaces any existing tools with the provided ones.
    pub fn set_tools<T: Into<im::Vector<anthropic::Tool>>>(&mut self, tools: T) -> &mut Self {
        self.tools = tools.into();
        self
    }
}

impl Default for Conversation {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use schemars::JsonSchema;

    use crate::conversation::Conversation;

    #[derive(JsonSchema)]
    #[allow(dead_code)]
    struct TestToolInput {
        /// A test parameter
        param: String,
    }

    #[test]
    fn test_conversation_with_tools() {
        let api = crate::Api::new("test-api-key");
        let mut conversation = Conversation::new();

        // Create a test tool
        let test_tool = crate::anthropic::Tool::new::<TestToolInput, _, _>(
            "test_tool",
            "A test tool for testing",
        );

        // Add the tool to the conversation
        conversation.add_tool(test_tool);

        // Create a user message request
        let http_request = conversation.user_message(&api, "Hello, use the tool!");

        // Verify the request includes tools
        assert!(http_request.body.contains("\"tools\":["));
        assert!(http_request.body.contains("\"name\":\"test_tool\""));
        assert!(
            http_request
                .body
                .contains("\"description\":\"A test tool for testing\"")
        );

        // Verify the message is also present
        assert!(http_request.body.contains("\"messages\":["));
        assert!(http_request.body.contains("\"Hello, use the tool!\""));
    }
}

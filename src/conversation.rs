//! Conversation management for ongoing chats with the API.

use std::sync::Arc;

use crate::{Api, Error, MessagesRequestBuilder, anthropic, http_request::HttpRequest};

/// A conversation that manages message history and generates HTTP requests.
#[derive(Debug)]
pub struct Conversation {
    api: Api,
    system: Option<String>,
    messages: Vec<Arc<anthropic::Message>>,
}

impl Conversation {
    /// Creates a new conversation with the given API configuration.
    pub fn new(api: Api) -> Self {
        Self {
            api,
            system: None,
            messages: Vec::new(),
        }
    }

    /// Sets the system prompt for the conversation.
    pub fn set_system<S: Into<String>>(&mut self, system: S) -> &mut Self {
        self.system = Some(system.into());
        self
    }

    /// Adds a user message and returns an HTTP request to send.
    pub fn chat_message<S: Into<String>>(&mut self, user_message: S) -> HttpRequest {
        // Add user message to history
        let message = anthropic::Message::from_text(anthropic::Role::User, user_message);
        self.messages.push(Arc::new(message));

        // Build and return HTTP request with full conversation history
        let mut builder = MessagesRequestBuilder::new().set_messages(self.messages.clone());

        if let Some(ref system) = self.system {
            builder = builder.system(system.clone());
        }

        builder.build(&self.api)
    }

    /// Handles the response from the API and returns the assistant's message content.
    ///
    /// This method parses the response, adds the assistant's message to the conversation
    /// history, and returns the text content of the response.
    pub fn handle_response(&mut self, response_json: &str) -> Result<String, Error> {
        let response: anthropic::MessagesResponse = anthropic::deserialize_response(response_json)?;

        // Add assistant's message to history
        self.messages.push(Arc::new(response.message.clone()));

        // Extract and return text content
        let mut result = String::new();
        for (i, content) in response.message.content.iter().enumerate() {
            if i > 0 {
                result.push('\n');
            }
            result.push_str(&content.to_string());
        }

        Ok(result)
    }

    /// Returns the current number of messages in the conversation.
    pub fn message_count(&self) -> usize {
        self.messages.len()
    }

    /// Clears the conversation history.
    pub fn clear(&mut self) {
        self.messages.clear();
    }
}

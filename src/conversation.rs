//! Conversation management for ongoing chats with the API.

use std::sync::Arc;

use crate::{Api, ResponseError, anthropic, http_request::HttpRequest};

/// Actions that the caller needs to take based on the API response.
#[derive(Debug)]
pub enum Action {
    /// Handle a message from the agent/assistant.
    HandleAgentMessage(Vec<anthropic::Content>),
}

/// A conversation that manages message history and generates HTTP requests.
#[derive(Debug)]
pub struct Conversation {
    system: Option<String>,
    messages: Vec<Arc<anthropic::Message>>,
}

impl Conversation {
    /// Creates a new conversation.
    pub fn new() -> Self {
        Self {
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
    pub fn user_message<S: Into<String>>(&mut self, api: &Api, user_message: S) -> HttpRequest {
        // Add user message to history
        let message = anthropic::Message::from_text(anthropic::Role::User, user_message);
        self.messages.push(Arc::new(message));

        // Build and return HTTP request with full conversation history
        let mut builder = crate::MessagesRequestBuilder::new().set_messages(self.messages.clone());

        if let Some(ref system) = self.system {
            builder = builder.system(system.clone());
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
        self.messages.push(Arc::new(response.message.clone()));

        Ok(Action::HandleAgentMessage(response.message.content))
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

impl Default for Conversation {
    fn default() -> Self {
        Self::new()
    }
}

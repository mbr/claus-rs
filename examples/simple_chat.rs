//! Simple chat example using the low-level `MessagesRequestBuilder` API.
//!
//! This example demonstrates the lower-level approach to building chat conversations
//! by manually managing message history and using the `MessagesRequestBuilder`.
//!
//! ## How to run
//!
//! ```shell
//! $ cargo run --example simple_chat --features reqwest-blocking -- config.toml
//! ```
//!
//! Requires a TOML configuration file with your Anthropic API key.
//!
//! Enter messages line by line. Press Ctrl+D (EOF) to exit.

use std::{
    env, fs,
    io::{self, Write},
};

use klaus::{
    anthropic::{Message, MessagesResponse, Role},
    deserialize_response,
};
use serde::Deserialize;

/// Configuration structure for simple chat.
/// Only requires the Anthropic API key - other fields are ignored.
#[derive(Debug, Deserialize)]
struct Config {
    /// Anthropic API key for Claude access
    anthropic_api_key: String,
}

fn main() {
    // Read config from first command line argument, panic if not provided.
    let config_file = env::args()
        .nth(1)
        .expect("requires argument: path to TOML config file");

    let client = reqwest::blocking::Client::new();

    // Load configuration from TOML file
    let config_content = fs::read_to_string(&config_file).expect("failed to read config file");
    let config: Config = toml::from_str(&config_content).expect("failed to parse config TOML");

    let api = klaus::Api::new(config.anthropic_api_key);

    // Messages will hold our conversation, it will include both user messages and model responses.
    let mut messages = im::Vector::new();
    while let Some(input) = read_next_line() {
        messages.push_back(Message::from_text(Role::User, input));

        // Build the request, then send it.
        let http_req = klaus::MessagesRequestBuilder::new()
            .set_messages(messages.clone())
            .build(&api);
        let raw = client
            .execute(http_req.into())
            .expect("failed to execute request")
            .text()
            .expect("failed to fetch response text");

        // Parse the response, then display and store it.
        let response: MessagesResponse =
            deserialize_response(&raw).expect("failed to parse response");

        for content in &response.message {
            println!("Claude: {}", content);
        }

        messages.push_back(response.message);
    }
}

/// Helper function that shows a prompt and reads a line from stdin.
fn read_next_line() -> Option<String> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let mut input = String::new();

    stdout.write_all(b"You: ").expect("stdout failed to write");
    stdout.flush().expect("stdout failed to flush");
    if stdin.read_line(&mut input).expect("stdin failed to read") == 0 {
        None
    } else {
        Some(input)
    }
}

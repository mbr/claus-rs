//! Streaming chat example, similar to `simple_chat.rs` but using the streaming API.
//!
//! ## How to run
//!
//! ```shell
//! $ cargo run --example streaming --features reqwest -- config.toml
//! ```
//!
//! Requires a TOML configuration file with your Anthropic API key.
//!
//! Enter messages line by line. Press Ctrl+D (EOF) to exit.

use std::{
    env, fs,
    io::{self, Write},
};

use futures::stream::StreamExt;
use klaus::anthropic::{Message, Role};
use reqwest_eventsource::{Event, EventSource};
use serde::Deserialize;

/// Configuration structure for simple chat.
/// Only requires the Anthropic API key - other fields are ignored.
#[derive(Debug, Deserialize)]
struct Config {
    /// Anthropic API key for Claude access
    anthropic_api_key: String,
}

#[tokio::main]
async fn main() {
    // Read config from first command line argument, panic if not provided.
    let config_file = env::args()
        .skip(1)
        .next()
        .expect("requires argument: path to TOML config file");

    let client = reqwest::Client::new();

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
            .stream(true)
            .build(&api);

        println!("Sending request...");

        // Create RequestBuilder directly from HttpRequest data
        let url = format!("https://{}{}", http_req.host, http_req.path);
        let method = reqwest::Method::from_bytes(http_req.method.as_bytes()).unwrap();

        let mut request_builder = client.request(method, &url).body(http_req.body.clone());

        // Add headers
        for (key, value) in &http_req.headers {
            request_builder = request_builder.header(*key, value.as_ref());
        }

        let mut es = EventSource::new(request_builder).expect("failed to create event source");

        println!("Receiving events:");
        while let Some(event) = es.next().await {
            match event {
                Ok(Event::Open) => {
                    println!("Connection opened");
                }
                Ok(Event::Message(message)) => {
                    println!("Event: {}", message.event);
                    println!("Data: {}", message.data);
                    // TODO: Parse the SSE message data and handle different event types
                    // TODO: For message_delta events, extract text content and accumulate
                    // TODO: For message_stop events, finalize the response
                }
                Err(err) => {
                    println!("Error: {}", err);
                    break;
                }
            }
        }

        println!("Stream ended\n");

        // TODO: After processing all events, add the complete assistant message to messages
        // messages.push_back(assistant_message);
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

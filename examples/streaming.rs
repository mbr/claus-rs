//! Streaming chat example, similar to `simple_chat.rs` but using the streaming API.
//!
//! Response fragments will be flushed to stdout as they are received.
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
use klaus::anthropic::{Content, Delta, Message, Role, StreamEvent};
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

    let mut messages = im::Vector::new();
    while let Some(input) = read_next_line() {
        messages.push_back(Message::from_text(Role::User, input));

        // Build the request, then send it.
        let http_req = klaus::MessagesRequestBuilder::new()
            .set_messages(messages.clone())
            .stream(true)
            .build(&api);

        let request_builder = http_req
            .try_into_reqwest_builder(&client)
            .expect("failed to create request builder");

        let mut es = EventSource::new(request_builder).expect("failed to create event source");

        let mut assistant_content = Vec::new();
        let mut current_text = String::new();

        while let Some(event) = es.next().await {
            match event {
                Ok(Event::Open) => {
                    eprintln!("Connection opened");
                }
                Ok(Event::Message(message)) => {
                    // Parse the SSE message data using our deserialize_event function
                    match klaus::deserialize_event(message.data.as_bytes()) {
                        Ok(stream_event) => match stream_event {
                            StreamEvent::MessageStart(_) => {
                                print!("Assistant: ");
                                io::stdout().flush().expect("failed to flush stdout");
                            }
                            StreamEvent::ContentBlockStart {
                                index: _,
                                content_block,
                            } => {
                                match content_block {
                                    Content::Text { text } => {
                                        // Display the start of a text message immediately.
                                        print!("{}", text);
                                        io::stdout().flush().expect("failed to flush stdout");
                                        current_text.push_str(&text);
                                    }
                                    _ => {
                                        eprintln!("Other content block: {:?}", content_block);
                                    }
                                }
                            }
                            StreamEvent::ContentBlockDelta { index, delta } => {
                                match delta {
                                    Delta::TextDelta { text } => {
                                        // Display text immediately as it comes in
                                        print!("{}", text);
                                        io::stdout().flush().expect("failed to flush stdout");
                                        current_text.push_str(&text);
                                    }
                                    other_delta => {
                                        eprintln!(
                                            "Other delta for block {}: {:?}",
                                            index, other_delta
                                        );
                                    }
                                }
                            }
                            StreamEvent::ContentBlockStop { index: _ } => {
                                if !current_text.is_empty() {
                                    // We are relying on the API sending content in order.
                                    assistant_content
                                        .push(Content::from_text(current_text.clone()));
                                    current_text.clear();
                                }
                            }
                            StreamEvent::MessageDelta { delta, usage } => {
                                // We currently don't handle message deltas.
                                eprintln!("Message delta: {:?}, usage: {:?}", delta, usage);
                            }
                            StreamEvent::MessageStop => {
                                // Finalize the response and break out of the event loop
                                println!();
                                break;
                            }
                            StreamEvent::Ping => {
                                // We quietly accept pings.
                            }
                            StreamEvent::Error { error } => {
                                eprintln!("Error event: {:?}", error);
                                break;
                            }
                            StreamEvent::Unknown {
                                event_type,
                                contents,
                            } => {
                                eprintln!(
                                    "Unknown event type: {:?}, contents: {:?}",
                                    String::from_utf8_lossy(&event_type),
                                    contents
                                );
                            }
                        },
                        Err(parse_err) => {
                            eprintln!("Failed to parse event data: {}", parse_err);
                            eprintln!("Raw data: {}", message.data);
                        }
                    }
                }
                Err(err) => {
                    eprintln!("Error: {}", err);
                    break;
                }
            }
        }

        // Add the complete assistant message to our conversation history
        if !assistant_content.is_empty() {
            let assistant_message = Message {
                role: Role::Assistant,
                content: assistant_content,
            };
            messages.push_back(assistant_message);
        }
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

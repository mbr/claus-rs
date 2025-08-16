//! A full-featured AI assistant example demonstrating web search, page fetching, and datetime tools.
//!
//! This example showcases a complete CLI application that uses the claus library to create
//! an interactive chat interface with Claude, equipped with multiple tools for enhanced
//! functionality. Both API keys are configured via a TOML configuration file.
//!
//! ## Usage
//!
//! ```bash
//! cargo run --example assistant --features="reqwest-blocking" -- config.toml
//! ```
//!
//! ## Configuration File
//!
//! See `config.example.toml` for the required TOML structure with both API keys.
//!
//! ## Key Bindings
//!
//! - Enter: Send message
//! - Alt+Enter: Insert newline (multiline input)
//! - Ctrl+C: Quit application

mod tools;
mod ui;

use std::{env, fs, io};

use claus::anthropic::{Content, Tool, ToolResult, ToolUse};
use reqwest::blocking::{Client, Request};
use serde::Deserialize;
use tools::{
    DateTimeInput, FetchPageInput, WebSearchInput, tool_fetch_page, tool_get_datetime,
    tool_web_search,
};
use ui::{create_editor, get_user_input};

/// Configuration structure containing API keys needed for the assistant.
#[derive(Debug, Deserialize)]
struct Config {
    /// Anthropic API key for Claude access
    anthropic_api_key: String,
    /// Brave Search API key for web search functionality
    brave_api_key: String,
}

/// Main entry point for the AI assistant application.
fn main() -> io::Result<()> {
    let config_file = env::args()
        .nth(1)
        .expect("requires argument: path to TOML config file with API keys");

    // Load configuration from TOML file
    let config_content = fs::read_to_string(&config_file).expect("failed to read config file");
    let config: Config = toml::from_str(&config_content).expect("failed to parse config TOML");

    // Setup API.
    let api = claus::Api::new(config.anthropic_api_key);

    // Setup HTTP client.
    let client = Client::new();

    // Create the conversation instance.
    let mut conversation = claus::conversation::Conversation::new();
    conversation.set_system("You are a helpful personal assistant. You are able to answer questions, search the web, and help with tasks.");
    conversation.add_tool(Tool::new::<WebSearchInput, _, _>(
        "web_search",
        "Searches the web for information. Use this tool to search the web for information. When results are returned, you should use the `fetch_page` tool to fetch the page content, unless the description of the result is enough to answer the user's question.",
    ));
    conversation.add_tool(Tool::new::<DateTimeInput, _, _>(
        "get_datetime",
        "Gets the current date and time in ISO 8601 format. Use this tool to get the current date and time. Do not use this tool to get the date and time of a specific event. Use this especially when the user asks for information about the latest of anything, in case you need to make a web search.",
    ));
    conversation.add_tool(Tool::new::<FetchPageInput, _, _>(
        "fetch_page",
        "Fetches the content of a web page. Use this tool to fetch the content of a web page. This is useful when the description of the result is not enough to answer the user's question. The page returned will be in Markdown, with all HTML removed, potentially truncated if it was too long. Sometimes the page may not have the information you need, in which case you should discard this result and continue with the next one.",
    ));

    // Set up reedline with custom keybindings
    let mut line_editor = create_editor();

    println!("Chat with Claude! Send messages with enter, Alt+Enter for multiline, Ctrl+C to quit");

    let mut pending_request = None;
    loop {
        let Some(http_req) = pending_request.take() else {
            let Some(line) = get_user_input(&conversation, &mut line_editor) else {
                // User requested to quit.
                break;
            };
            pending_request = Some(conversation.user_message(&api, &line));
            continue;
        };

        let raw = send_request(&client, http_req.into()).expect("failed to send request");

        for (idx, item) in conversation
            .handle_response(&raw)
            .expect("failed to handle response")
            .contents
            .into_iter()
            .enumerate()
        {
            let mut tool_results = Vec::new();
            let offset = conversation.history().len() - 1;

            println!("[{}.{}] Claude> {}", offset, idx, item);

            // Once everything has been printed, handle actual tool use.
            if let Content::ToolUse(ToolUse { id, name, input }) = item {
                match name.as_str() {
                    "web_search" => {
                        let input: WebSearchInput = serde_json::from_value(input).unwrap();

                        match tool_web_search(&client, Some(&config.brave_api_key), &input.query) {
                            Ok(results) => {
                                eprintln!("web_search:Web search results:");

                                for result in &results {
                                    eprintln!("web_search:  * {}", result.title);
                                }

                                let results_json =
                                    serde_json::to_string(&results).unwrap_or_else(|_| {
                                        "Failed to serialize search results".to_string()
                                    });
                                tool_results.push(ToolResult::success(id, results_json));
                            }
                            Err(error) => {
                                eprintln!("web_search: {}", error);
                                tool_results.push(ToolResult::error(id, error));
                            }
                        }
                    }
                    "get_datetime" => {
                        tool_results.push(ToolResult::success(id, tool_get_datetime()));
                    }
                    "fetch_page" => {
                        let input: FetchPageInput = serde_json::from_value(input).unwrap();
                        match tool_fetch_page(&client, &input.url) {
                            Ok(content) => {
                                tool_results.push(ToolResult::success(id, content));
                            }
                            Err(error) => {
                                eprintln!("fetch_page: error: {}", error);
                                tool_results.push(ToolResult::error(id, error));
                            }
                        }
                    }
                    _ => {
                        tool_results.push(ToolResult::unknown_tool(id, &name));
                    }
                }
            }

            if !tool_results.is_empty() {
                pending_request = Some(conversation.tool_results(&api, tool_results));
            }
        }
    }

    Ok(())
}

/// Sends an HTTP request with automatic retry logic for rate limiting.
pub fn send_request(client: &Client, req: Request) -> Result<String, String> {
    let mut retries_left = 3;
    while retries_left > 0 {
        let response = client
            .execute(req.try_clone().expect("Failed to clone request"))
            .map_err(|e| format!("Failed to send request: {}", e))?;

        if response.status().as_u16() == 429 || response.status().as_u16() == 420 {
            let retry_after = response
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(1);

            retries_left -= 1;

            eprintln!("Rate limit exceeded. Retrying in {} seconds.", retry_after);

            std::thread::sleep(std::time::Duration::from_secs(retry_after));
        } else {
            if !response.status().is_success() {
                let status = response.status();
                let text = response
                    .text()
                    .unwrap_or_else(|err| format!("(failed to fetch response text: {})", err));
                return Err(format!("Request failed with HTTP {}: {}", status, text));
            }
            let body = response
                .text()
                .map_err(|e| format!("Failed to read response body: {}", e))?;
            return Ok(body);
        }
    }
    Err("Rate limit exceeded.".to_string())
}

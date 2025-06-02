use std::{env, fs, io};

use chrono::{DateTime, Utc};
use klaus::anthropic::{Content, Tool, ToolResult, ToolUse};
use reedline::{
    DefaultPrompt, DefaultPromptSegment, DefaultValidator, EditCommand, Emacs, KeyCode,
    KeyModifiers, Reedline, ReedlineEvent, Signal, default_emacs_keybindings,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Brave Search API endpoint
const BRAVE_SEARCH_ENDPOINT: &str = "https://api.search.brave.com/res/v1/web/search";

/// Input to the web search tool.
#[derive(Debug, JsonSchema, Serialize, Deserialize)]
struct WebSearchInput {
    /// The query to search for.
    query: String,
}

/// Input to the datetime tool (empty).
#[derive(Debug, JsonSchema, Serialize, Deserialize)]
struct DateTimeInput {}
// TODO: Make this easier?

/// A search result from the web search API.
#[derive(Debug, Serialize, Deserialize)]
struct SearchResult {
    title: String,
    description: String,
    url: String,
}

impl std::fmt::Display for SearchResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let json = serde_json::to_string(self).map_err(|_| std::fmt::Error)?;
        write!(f, "{}", json)
    }
}

/// Tool that returns the current date and time in ISO 8601 format.
fn tool_get_datetime() -> String {
    let now: DateTime<Utc> = Utc::now();
    now.to_rfc3339()
}

/// Performs a web search using the Brave Search API.
fn tool_web_search(api_key: Option<&str>, term: &str) -> Result<Vec<SearchResult>, String> {
    /// Internal response structure for Brave Search API.
    #[derive(Debug, Deserialize)]
    struct BraveSearchResponse {
        web: Option<Vec<BraveWebResult>>,
    }

    #[derive(Debug, Deserialize)]
    struct BraveWebResult {
        title: String,
        description: String,
        url: String,
    }

    let api_key = api_key.ok_or("API key is required for web search")?;

    let client = reqwest::blocking::Client::new();

    let search_response: BraveSearchResponse = client
        .get(BRAVE_SEARCH_ENDPOINT)
        .query(&[("q", term)])
        .header("Accept", "application/json")
        .header("X-Subscription-Token", api_key)
        .send()
        .map_err(|e| format!("Failed to send request: {}", e))?
        .error_for_status()
        .map_err(|e| format!("Search API error: {}", e))?
        .json()
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    let web_results = search_response
        .web
        .ok_or("No web results found in response")?;

    let results = web_results
        .into_iter()
        .map(|result| SearchResult {
            title: result.title,
            description: result.description,
            url: result.url,
        })
        .collect();

    Ok(results)
}

fn main() -> io::Result<()> {
    let key_file = env::args()
        .skip(1)
        .next()
        .expect("requires argument: anthropic api key file");

    // Setup API.
    let api_key = fs::read_to_string(key_file).expect("failed to read key");
    let api = klaus::Api::new(api_key);

    // Setup HTTP client.
    let client = reqwest::blocking::Client::new();

    // Read Brave Search API key from environment variable
    let brave_api_key = env::var("BRAVE_API_KEY").ok();
    if brave_api_key.is_none() {
        eprintln!("Warning: BRAVE_API_KEY environment variable not set. Web search will not work.");
    }

    // Create the conversation instance.
    let mut conversation = klaus::conversation::Conversation::new();
    conversation.set_system("You are a helpful personal assistant. You are able to answer questions, search the web, and help with tasks.");
    conversation.add_tool(Tool::new::<WebSearchInput, _, _>(
        "web_search",
        "Search the web for information",
    ));
    conversation.add_tool(Tool::new::<DateTimeInput, _, _>(
        "get_datetime",
        "Gets the current date and time in ISO 8601 format. Use this tool to get the current date and time. Do not use this tool to get the date and time of a specific event. Use this especially when the user asks for information about the latest of anything, in case you need to make a web search.",
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

        // Send the request.
        let raw = client
            .execute(http_req.into())
            .expect("failed to execute request")
            .text()
            .expect("failed to fetch contents");

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

                        match tool_web_search(brave_api_key.as_deref(), &input.query) {
                            Ok(results) => {
                                let results_json =
                                    serde_json::to_string(&results).unwrap_or_else(|_| {
                                        "Failed to serialize search results".to_string()
                                    });
                                tool_results.push(ToolResult::success(id, results_json));
                            }
                            Err(error) => {
                                tool_results.push(ToolResult::error(id, error));
                            }
                        }
                    }
                    "get_datetime" => {
                        tool_results.push(ToolResult::success(id, tool_get_datetime()));
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

/// Creates a new configured [`Reedline`] instance.
fn create_editor() -> Reedline {
    let mut keybindings = default_emacs_keybindings();

    keybindings.add_binding(KeyModifiers::NONE, KeyCode::Enter, ReedlineEvent::Enter);

    // Add Alt+Enter for manual newlines (helps with multiline input)
    keybindings.add_binding(
        KeyModifiers::ALT,
        KeyCode::Enter,
        ReedlineEvent::Edit(vec![EditCommand::InsertNewline]),
    );

    let edit_mode = Box::new(Emacs::new(keybindings));

    Reedline::create()
        .with_edit_mode(edit_mode)
        // Note: We need a validator to support multiline input.
        .with_validator(Box::new(DefaultValidator))
}

/// Returns the next user input, returning `None` if the program should exit.
fn get_user_input(
    conversation: &klaus::conversation::Conversation,
    line_editor: &mut Reedline,
) -> Option<String> {
    let prompt = DefaultPrompt::new(
        DefaultPromptSegment::Basic(format!("[{}] You", conversation.history().len())),
        DefaultPromptSegment::CurrentDateTime,
    );

    loop {
        let sig = line_editor.read_line(&prompt);
        match sig {
            Ok(Signal::Success(buffer)) => {
                let user_message = buffer.trim();

                if user_message.is_empty() {
                    continue;
                }

                return Some(user_message.to_owned());
            }
            Ok(Signal::CtrlC) | Ok(Signal::CtrlD) => {
                return None;
            }
            Err(err) => {
                eprintln!("Error: {}", err);
                return None;
            }
        }
    }
}

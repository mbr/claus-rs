use std::{env, fs, io};

use chrono::{DateTime, Utc};
use klaus::anthropic::{Content, Tool, ToolResult, ToolUse};
use reedline::{
    DefaultPrompt, DefaultPromptSegment, DefaultValidator, EditCommand, Emacs, KeyCode,
    KeyModifiers, Reedline, ReedlineEvent, Signal, default_emacs_keybindings,
};
use reqwest::{blocking::{Client, Request}, Method, Body};
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

/// Input to the fetch page tool.
#[derive(Debug, JsonSchema, Serialize, Deserialize)]
struct FetchPageInput {
    /// The URL of the page to fetch.
    url: String,
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
    let client = Client::new();

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
        "Searches the web for information. Use this tool to search the web for information. When results are returned, you should use the `fetch_page` tool to fetch the page content, unless the description of the result is enough to answer the user's question.",
    ));
    conversation.add_tool(Tool::new::<DateTimeInput, _, _>(
        "get_datetime",
        "Gets the current date and time in ISO 8601 format. Use this tool to get the current date and time. Do not use this tool to get the date and time of a specific event. Use this especially when the user asks for information about the latest of anything, in case you need to make a web search.",
    ));
    conversation.add_tool(Tool::new::<FetchPageInput, _, _>(
        "fetch_page",
        "Fetches the content of a web page. Use this tool to fetch the content of a web page. This is useful when the description of the result is not enough to answer the user's question. The page returned will be in HTML format. Sometimes the page may not have the information you need, in which case you should discard this result and continue with the next one.",
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

                        match tool_web_search(&client,  brave_api_key.as_deref(), &input.query) {
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

fn send_request(client: &Client, req: Request) -> Result<String, String> {
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
            let response = response.error_for_status().map_err(|e| format!("Request failed: {}", e))?;
            let body = response
                .text()
                .map_err(|e| format!("Failed to read response body: {}", e))?;
            return Ok(body);
        }
    }
    Err("Rate limit exceeded.".to_string())
}

/// Tool that returns the current date and time in ISO 8601 format.
fn tool_get_datetime() -> String {
    let now: DateTime<Utc> = Utc::now();
    now.to_rfc3339()
}

/// Performs a web search using the Brave Search API.
fn tool_web_search(client: &Client, api_key: Option<&str>, term: &str) -> Result<Vec<SearchResult>, String> {
    #[derive(Debug, Deserialize)]
    struct BraveWebSearchApiResponse {
        web: Option<BraveSearch>,
    }

    #[derive(Debug, Deserialize, Default)]
    struct BraveSearch {
        results: Vec<BraveResult>,
    }

    #[derive(Debug, Deserialize)]
    struct BraveResult {
        title: String,
        description: Option<String>,
        url: String,
    }

    let api_key = api_key.ok_or("API key is required for web search")?;

    let request = client.get(BRAVE_SEARCH_ENDPOINT)
        .query(&[("q", term)])
        .header("Accept", "application/json")
        .header("X-Subscription-Token", api_key)
        .build()
        .expect("Failed to build request");

    let response = send_request(client, request)?;
    let search_response: BraveWebSearchApiResponse = serde_json::from_str(&response).map_err(|e| format!("Failed to parse response: {}", e))?;

    let results = search_response
        .web
        .unwrap_or_default()
        .results
        .into_iter()
        .map(|result| SearchResult {
            title: result.title,
            description: result.description.unwrap_or_default(),
            url: result.url,
        })
        .collect();

    Ok(results)
}

fn tool_fetch_page(client: &Client, url: &str) -> Result<String, String> {
    let request = Request::new( Method::GET, url.parse().expect("Failed to parse URL"));
    send_request(client, request)
}

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

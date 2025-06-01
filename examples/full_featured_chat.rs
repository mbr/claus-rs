use std::{collections::VecDeque, env, fs, io};

use chrono::{DateTime, Utc};
use klaus::anthropic::{Content, Tool, ToolResult, ToolUse};
use reedline::{
    DefaultPrompt, DefaultPromptSegment, DefaultValidator, EditCommand, Emacs, KeyCode,
    KeyModifiers, Reedline, ReedlineEvent, Signal, default_emacs_keybindings,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

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

    // Create the conversation instance.
    let mut conversation = klaus::conversation::Conversation::new();
    conversation.set_system("You are a helpful personal assistant. You are able to answer questions, search the web, and help with tasks.");
    conversation.add_tool(Tool::new::<WebSearchInput, _, _>(
        "web_search",
        "Search the web for information",
    ));
    conversation.add_tool(Tool::new::<DateTimeInput, _, _>(
        "get_datetime",
        "Get the current date and time in ISO 8601 format",
    ));

    // Set up reedline with custom keybindings
    let mut line_editor = create_editor();

    println!("Chat with Claude! Send messages with enter, Alt+Enter for multiline, Ctrl+C to quit");

    let mut request_stack = VecDeque::new();
    loop {
        let http_req = if let Some(req) = request_stack.pop_front() {
            req
        } else {
            // If we have no request that we need to process buffered, get a new user message.
            let Some(line) = get_user_input(&conversation, &mut line_editor) else {
                // User requested to quit.
                break;
            };

            conversation.user_message(&api, &line)
        };

        // println!("Sending request: {}", http_req);

        let raw = client
            .execute(http_req.into())
            .expect("failed to execute request")
            .text()
            .expect("failed to fetch contents");

        match conversation.handle_response(&raw) {
            Ok(action) => {
                match action {
                    klaus::conversation::Action::HandleAgentMessage(contents) => {
                        let offset = conversation.history().len() - 1;
                        for (idx, item) in contents.into_iter().enumerate() {
                            match item {
                                Content::Text { .. } | Content::Image | Content::ToolResult(_) => {
                                    println!("[{}.{}] Claude> {}", offset, idx, item);
                                }
                                Content::ToolUse(ToolUse { id, name, input }) => {
                                    println!("[{}.{}] Tool use: {}", offset, idx, name);
                                    let response = match name.as_str() {
                                        "web_search" => {
                                            let input: WebSearchInput =
                                                serde_json::from_value(input).unwrap();
                                            "todo".to_string()
                                        }
                                        "get_datetime" => tool_get_datetime(),
                                        _ => {
                                            // TODO: Return error instead of panicking
                                            panic!("Unknown tool: {}", name);
                                        }
                                    };
                                    println!("RESPONSE: {}", response);
                                    request_stack.push_back(
                                        conversation
                                            .tool_result(&api, ToolResult::success(id, response)),
                                    );
                                }
                            }
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("Error: {:?}", e);
                break;
            }
        }
    }

    Ok(())
}

/// Tool that returns the current date and time in ISO 8601 format.
fn tool_get_datetime() -> String {
    let now: DateTime<Utc> = Utc::now();
    now.to_rfc3339()
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

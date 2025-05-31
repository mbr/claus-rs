use std::{env, fs, io};

use reedline::{
    DefaultCompleter, DefaultHinter, DefaultValidator, EditCommand, Emacs, KeyCode, KeyModifiers,
    Prompt, PromptEditMode, PromptHistorySearch, Reedline, ReedlineEvent, Signal,
    default_emacs_keybindings,
};

// Custom prompt that shows "You: " for single line and "   | " for continuation
struct ChatPrompt;

impl Prompt for ChatPrompt {
    fn render_prompt_left(&self) -> std::borrow::Cow<str> {
        "You: ".into()
    }

    fn render_prompt_right(&self) -> std::borrow::Cow<str> {
        "".into()
    }

    fn render_prompt_indicator(&self, _edit_mode: PromptEditMode) -> std::borrow::Cow<str> {
        "".into()
    }

    fn render_prompt_multiline_indicator(&self) -> std::borrow::Cow<str> {
        "   | ".into()
    }

    fn render_prompt_history_search_indicator(
        &self,
        _history_search: PromptHistorySearch,
    ) -> std::borrow::Cow<str> {
        "".into()
    }
}

fn main() -> io::Result<()> {
    let key_file = env::args()
        .skip(1)
        .next()
        .expect("requires argument: anthropic api key file");

    let client = reqwest::blocking::Client::new();

    let api_key = fs::read_to_string(key_file).expect("failed to read key");
    let api = klaus::Api::new(api_key);

    // Create a conversation instance
    let mut conversation = klaus::Conversation::new(api);

    // Set up reedline with custom keybindings for Shift+Enter
    let mut keybindings = default_emacs_keybindings();

    // Shift+Enter should insert a newline (multiline mode)
    keybindings.add_binding(
        KeyModifiers::SHIFT,
        KeyCode::Enter,
        ReedlineEvent::Edit(vec![EditCommand::InsertNewline]),
    );

    // Regular Enter should submit
    keybindings.add_binding(KeyModifiers::NONE, KeyCode::Enter, ReedlineEvent::Enter);

    let edit_mode = Box::new(Emacs::new(keybindings));

    let mut line_editor = Reedline::create()
        .with_edit_mode(edit_mode)
        .with_validator(Box::new(DefaultValidator))
        .with_completer(Box::new(DefaultCompleter::default()))
        .with_hinter(Box::new(DefaultHinter::default()));

    let prompt = ChatPrompt;

    println!("Chat with Claude! Features:");
    println!("- Type your message and press Enter to send");
    println!("- Press Shift+Enter to add newlines within your message");
    println!("- Use standard editing commands (Ctrl+A, Ctrl+E, etc.)");
    println!("- Use Ctrl+C to exit");
    println!();

    loop {
        let sig = line_editor.read_line(&prompt);

        match sig {
            Ok(Signal::Success(buffer)) => {
                let user_message = buffer.trim();

                // Skip empty messages
                if user_message.is_empty() {
                    continue;
                }

                // Generate HTTP request with the conversation abstraction
                let http_req = conversation.chat_message(user_message);

                // Send the request
                let reqwest_req = http_req
                    .try_into_reqwest_blocking()
                    .expect("failed to convert to reqwest request");

                println!("Sending request...");

                let raw = client
                    .execute(reqwest_req)
                    .expect("failed to execute request")
                    .text()
                    .expect("failed to fetch contents");

                // Handle the response and get the assistant's message
                match conversation.handle_response(&raw) {
                    Ok(assistant_message) => {
                        println!("\nClaude: {}\n", assistant_message);
                    }
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        break;
                    }
                }

                println!("Messages in conversation: {}", conversation.message_count());
                println!();
            }
            Ok(Signal::CtrlC) => {
                println!("Goodbye!");
                break;
            }
            Ok(Signal::CtrlD) => {
                println!("Goodbye!");
                break;
            }
            Err(err) => {
                eprintln!("Error: {}", err);
                break;
            }
        }
    }

    Ok(())
}

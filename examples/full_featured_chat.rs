use std::{env, fs, io};

use reedline::{
    DefaultCompleter, DefaultHinter, DefaultValidator, EditCommand, Emacs, KeyCode, KeyModifiers,
    Prompt, PromptEditMode, PromptHistorySearch, Reedline, ReedlineEvent, Signal,
    default_emacs_keybindings,
};

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

    // Create a conversation instance
    let mut conversation = klaus::conversation::Conversation::new();

    // Set up reedline with custom keybindings

    let mut line_editor = create_editor();

    println!("Chat with Claude! Send messages with enter, Alt+Enter for multiline, Ctrl+C to quit");

    while let Some(line) = get_user_input(&conversation, &mut line_editor) {
        let http_req = conversation.user_message(&api, &line);

        let raw = client
            .execute(http_req.into())
            .expect("failed to execute request")
            .text()
            .expect("failed to fetch contents");

        match conversation.handle_response(&raw) {
            Ok(action) => match action {
                klaus::conversation::Action::HandleAgentMessage(content) => {
                    for item in content {
                        println!("Claude: {}", item);
                    }
                }
            },
            Err(e) => {
                eprintln!("Error: {}", e);
                break;
            }
        }
    }

    Ok(())
}

/// Custom prompt that shows "You: " for single line and "   | " for continuation
struct ChatPrompt(usize);

impl Prompt for ChatPrompt {
    fn render_prompt_left(&self) -> std::borrow::Cow<str> {
        format!("You ({}): ", self.0).into()
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

/// Creates a new configured [`Reedline`] instance.
fn create_editor() -> Reedline {
    let mut keybindings = default_emacs_keybindings();

    // Regular Enter sends the message (standard chat behavior)
    keybindings.add_binding(KeyModifiers::NONE, KeyCode::Enter, ReedlineEvent::Enter);

    // Alt+Enter adds newlines when you specifically want them
    keybindings.add_binding(
        KeyModifiers::ALT,
        KeyCode::Enter,
        ReedlineEvent::Edit(vec![EditCommand::InsertNewline]),
    );

    let edit_mode = Box::new(Emacs::new(keybindings));

    Reedline::create()
        .with_edit_mode(edit_mode)
        .with_validator(Box::new(DefaultValidator))
        .with_completer(Box::new(DefaultCompleter::default()))
        .with_hinter(Box::new(DefaultHinter::default()))
}

/// Returns the next user input, returning `None` if the program should exit.
fn get_user_input(
    conversation: &klaus::conversation::Conversation,
    line_editor: &mut Reedline,
) -> Option<String> {
    let prompt = ChatPrompt(conversation.history().len());

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

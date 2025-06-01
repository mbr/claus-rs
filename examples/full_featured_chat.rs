use std::{env, fs, io};

use reedline::{
    DefaultCompleter, DefaultHinter, DefaultPrompt, DefaultPromptSegment, DefaultValidator,
    EditCommand, Emacs, KeyCode, KeyModifiers, Reedline, ReedlineEvent, Signal,
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
                    let offset = conversation.history().len() - content.len();
                    for (idx, item) in content.iter().enumerate() {
                        println!("[{}] Claude> {}", idx + offset, item);
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

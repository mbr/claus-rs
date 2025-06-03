use reedline::{
    DefaultPrompt, DefaultPromptSegment, DefaultValidator, EditCommand, Emacs, KeyCode,
    KeyModifiers, Reedline, ReedlineEvent, Signal, default_emacs_keybindings,
};

/// Creates a new configured [`Reedline`] instance.
pub fn create_editor() -> Reedline {
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
pub fn get_user_input(
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

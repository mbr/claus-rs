use reedline::{DefaultPrompt, DefaultPromptSegment, Reedline, Signal};

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

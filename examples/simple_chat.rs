use std::{env, fs};

use rustyline::{
    Config, EditMode, Editor,
    error::ReadlineError,
    history::DefaultHistory,
    validate::{ValidationContext, ValidationResult, Validator},
};

// Helper struct for multi-line editing that detects when input seems incomplete
#[derive(Default)]
struct MultilineValidator;

impl Validator for MultilineValidator {
    fn validate(&self, ctx: &mut ValidationContext) -> rustyline::Result<ValidationResult> {
        let input = ctx.input().trim();

        // If input ends with a backslash, continue to next line
        if input.ends_with('\\') {
            return Ok(ValidationResult::Incomplete);
        }

        // If input has unmatched opening brackets/quotes, continue to next line
        let mut paren_count = 0;
        let mut bracket_count = 0;
        let mut brace_count = 0;
        let mut in_single_quote = false;
        let mut in_double_quote = false;
        let mut escape_next = false;

        for ch in input.chars() {
            if escape_next {
                escape_next = false;
                continue;
            }

            match ch {
                '\\' => escape_next = true,
                '\'' if !in_double_quote => in_single_quote = !in_single_quote,
                '"' if !in_single_quote => in_double_quote = !in_double_quote,
                '(' if !in_single_quote && !in_double_quote => paren_count += 1,
                ')' if !in_single_quote && !in_double_quote => paren_count -= 1,
                '[' if !in_single_quote && !in_double_quote => bracket_count += 1,
                ']' if !in_single_quote && !in_double_quote => bracket_count -= 1,
                '{' if !in_single_quote && !in_double_quote => brace_count += 1,
                '}' if !in_single_quote && !in_double_quote => brace_count -= 1,
                _ => {}
            }
        }

        // Continue input if quotes are unmatched or brackets are unmatched
        if in_single_quote
            || in_double_quote
            || paren_count > 0
            || bracket_count > 0
            || brace_count > 0
        {
            return Ok(ValidationResult::Incomplete);
        }

        Ok(ValidationResult::Valid(None))
    }
}

// Custom helper that combines all the traits we need
struct ChatHelper {
    validator: MultilineValidator,
}

impl Default for ChatHelper {
    fn default() -> Self {
        ChatHelper {
            validator: MultilineValidator::default(),
        }
    }
}

impl rustyline::Helper for ChatHelper {}
impl rustyline::completion::Completer for ChatHelper {
    type Candidate = String;
}
impl rustyline::hint::Hinter for ChatHelper {
    type Hint = String;
}
impl rustyline::highlight::Highlighter for ChatHelper {}

impl Validator for ChatHelper {
    fn validate(&self, ctx: &mut ValidationContext) -> rustyline::Result<ValidationResult> {
        self.validator.validate(ctx)
    }
}

fn main() -> rustyline::Result<()> {
    let key_file = env::args()
        .skip(1)
        .next()
        .expect("requires argument: anthropic api key file");

    let client = reqwest::blocking::Client::new();

    let api_key = fs::read_to_string(key_file).expect("failed to read key");
    let api = klaus::Api::new(api_key);

    // Create a conversation instance
    let mut conversation = klaus::Conversation::new(api);

    // Configure rustyline for multiline editing
    let config = Config::builder().edit_mode(EditMode::Emacs).build();

    let mut rl: Editor<ChatHelper, DefaultHistory> = Editor::with_config(config)?;
    rl.set_helper(Some(ChatHelper::default()));

    println!("Chat with Claude! Features:");
    println!("- Type your message and press Enter to send");
    println!("- For multiline input:");
    println!("  * End lines with \\ to continue on next line");
    println!("  * Unmatched quotes or brackets will continue automatically");
    println!("  * Use standard editing commands like Ctrl+A, Ctrl+E, etc.");
    println!("- Use Ctrl+C to exit");
    println!();

    loop {
        // Use readline for multiline support
        let readline = rl.readline("You: ");

        match readline {
            Ok(mut line) => {
                // Remove trailing backslashes used for line continuation
                while line.ends_with('\\') {
                    line.pop();
                    if line.ends_with(' ') {
                        line.push(' ');
                    }
                }

                let user_message = line.trim();

                // Skip empty messages
                if user_message.is_empty() {
                    continue;
                }

                // Add to history for up/down arrow navigation
                let _ = rl.add_history_entry(&line);

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
            Err(ReadlineError::Interrupted) => {
                println!("Goodbye!");
                break;
            }
            Err(ReadlineError::Eof) => {
                println!("Goodbye!");
                break;
            }
            Err(err) => {
                println!("Error: {:?}", err);
                break;
            }
        }
    }

    Ok(())
}

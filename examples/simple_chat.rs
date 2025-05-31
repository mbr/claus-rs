use std::{env, fs, io};

fn main() {
    let key_file = env::args()
        .skip(1)
        .next()
        .expect("requires argument: anthropic api key file");

    let client = reqwest::blocking::Client::new();

    let api_key = fs::read_to_string(key_file).expect("failed to read key");
    let api = klaus::Api::new(api_key);

    // Create a conversation instance
    let mut conversation = klaus::Conversation::new(api);

    let mut input = String::new();
    let stdin = io::stdin();

    println!("Chat with Claude! Type your messages (Ctrl+C to exit):");

    while stdin.read_line(&mut input).expect("stdin failed to read") != 0 {
        let user_message = input.trim();
        if user_message.is_empty() {
            input.clear();
            continue;
        }

        // Generate HTTP request with the conversation abstraction
        let http_req = conversation.chat_message(user_message);
        input.clear();

        // Send the request (use try_into for feature-gated conversion)
        let reqwest_req = http_req
            .try_into_reqwest_blocking()
            .expect("failed to convert to reqwest request");

        let raw = client
            .execute(reqwest_req)
            .expect("failed to execute request")
            .text()
            .expect("failed to fetch contents");

        // Handle the response and get the assistant's message
        match conversation.handle_response(&raw) {
            Ok(assistant_message) => {
                println!("Claude: {}", assistant_message);
            }
            Err(e) => {
                eprintln!("Error: {}", e);
                break;
            }
        }

        println!("Messages in conversation: {}", conversation.message_count());
    }
}

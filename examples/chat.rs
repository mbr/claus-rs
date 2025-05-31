use std::{env, fs, io, mem, sync::Arc};

use klaus::anthropic::{Message, MessagesResponse, Role, deserialize_response};

fn main() {
    let key_file = env::args()
        .skip(1)
        .next()
        .expect("requires argument: anthropic api key file");

    let client = reqwest::blocking::Client::new();

    let api_key = fs::read_to_string(key_file).expect("failed to read key");
    let api = klaus::Api::new(api_key);

    let mut input = String::new();
    let stdin = io::stdin();

    let mut messages = Vec::new();

    while stdin.read_line(&mut input).expect("stdin failed to read") != 0 {
        messages.push(Arc::new(Message::from_text(
            Role::User,
            mem::take(&mut input),
        )));
        let http_req = klaus::MessagesRequestBuilder::new()
            .set_messages(messages.clone())
            .build(&api);

        let raw = client
            .execute(http_req.into())
            .expect("failed to execute request")
            .text()
            .expect("failed to fetch contents");

        let response: MessagesResponse =
            deserialize_response(&raw).expect("failed to parse response");

        for content in &response.message {
            println!("Claude: {}", content);
        }
        messages.push(Arc::new(response.message));
    }
}

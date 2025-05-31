use std::{env, fs};

use klaus::{Role, deserialize_response};

fn main() {
    let key_file = env::args()
        .skip(1)
        .next()
        .expect("requires argument: anthropic api key file");

    let client = reqwest::blocking::Client::new();

    let api_key = fs::read_to_string(key_file).expect("failed to read key");
    let api = klaus::Api::new(api_key);

    let messages = klaus::MessagesRequestBuilder::new()
        .push_message(Role::User, "Hello, how are you?")
        .build(&api);

    println!("{}", messages);

    let raw = client
        .execute(messages.into())
        .expect("failed to execute request")
        .text()
        .expect("failed to fetch contents");

    let response = deserialize_response(&raw).expect("failed to parse response");
    println!("{:?}", response);
}

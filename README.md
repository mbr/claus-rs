# Klaus

Klaus is a client crate for [Anthropic's API](https://www.anthropic.com/api), which is often known only as "the Claude API". It allows having "conversations" with a hosted version of the latest Claude [large language models](https://en.wikipedia.org/wiki/Large_language_model).

Klaus is set apart by a few features from many other implementations:

* **Layered**: Direct access to API "primitives" is possible; all functionality is built on top of a set of data types covering a large portion of the API.
* **I/O-less**: Klaus itself does not perform any I/O, i.e., it does not make any HTTP requests and all of its methods are pure functions. This makes it HTTP client framework agnostic by default, although it contains convenience functions for some.

## Basic Usage

On the lowest layer sits an [`Api`] struct, which represents the configuration for making requests. You will need [an API key](https://console.anthropic.com/settings/keys) to utilize it. Once it is set up, you can create calls to the API through the [`MessagesRequestBuilder`]:

```rust
use klaus::{Api, HttpRequest, MessagesRequestBuilder, Role};

let api = Api::new("sk-ant-api03-...");

let http_request: HttpRequest = MessagesRequestBuilder::new()
    .push_message(Role::User, "Hello, world!")
    .build(&api);

assert_eq!(http_request.host, "api.anthropic.com");
assert_eq!(http_request.path, "/v1/messages");
assert_eq!(http_request.method, "POST");

assert_eq!(
    http_request.render_headers(),
    "content-type: application/json\n\
     anthropic-version: 2023-06-01\n\
     x-api-key: sk-ant-api03-...\n\
     anthropic-model: claude-sonnet-4-20250514\n\
     max-tokens: 1024"
);

assert_eq!(
    &http_request.body,
    r#"{"model":"claude-sonnet-4-20250514","max_tokens":1024,"messages":[{"role":"user","content":[{"type":"text","text":"Hello, world!"}]}]}"#
);

// now the request can be sent with any HTTP client
```

Calling the Anthropic API means sending the entire conversation every time a request is made, i.e., you are responsible for attaching all responses to the set of messages (that includes the user's) every time a request is made. See [`examples/simple_chat.rs`](examples/simple_chat.rs) for a complete example.

## Higher-level: Conversations

For conversation management, you can use the [`Conversation`] type:

```rust
use klaus::{Api, Conversation};

let api = Api::new("sk-ant-api03-...");
let mut conversation = Conversation::new(api);

// Generate request for user message
let http_request = conversation.chat_message("Hello!");

// After sending the request and receiving response JSON:
// let assistant_message = conversation.handle_response(&response_json)?;
``` 
# Klaus

Klaus. The AI client, Claude's German second-degree cousin.

The crate separates IO from the protocol, thus it can be run with a variety of backends.

At its core sits the [`Api`] struct, which holds common information for all requests. A
typical interaction is through the [`MessageRequestBuilder`]:

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
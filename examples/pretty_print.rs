//! Pretty prints the JSON stream output from `claude -p`.
//!
//! Reads newline-delimited JSON from stdin and displays formatted output with colors.
//!
//! ## Usage
//!
//! ```shell
//! $ claude -p --output-format stream-json "your prompt" | cargo run --example pretty_print
//! ```
//!
//! To see streaming deltas as they arrive, add `--include-partial-messages`:
//!
//! ```shell
//! $ claude -p --output-format stream-json --include-partial-messages "your prompt" \
//!     | cargo run --example pretty_print
//! ```

use std::io::{self, BufRead, Write};

use claus::{
    anthropic::{Content, Delta, StreamEvent},
    claudio::protocol::{OutputMessage, parse_line},
};
use crossterm::style::{Attribute, Color, SetAttribute, SetForegroundColor};

fn main() {
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    for line in stdin.lock().lines() {
        let line = line.expect("failed to read line");

        let Some(result) = parse_line(&line) else {
            continue;
        };

        match result {
            Ok(msg) => print_message(&mut stdout, &msg),
            Err(err) => {
                eprintln!("parse error: {}", err);
                eprintln!("  line: {}", line);
            }
        }
    }
}

/// Prints a formatted output message.
fn print_message(w: &mut impl Write, msg: &OutputMessage) {
    match msg {
        OutputMessage::System(sys) => {
            write_colored(w, Color::Cyan, &format!("[system] {}\n", sys.subtype));
            writeln!(w, "  model: {}", sys.model).expect("write failed");
            writeln!(w, "  cwd: {}", sys.cwd).expect("write failed");
            if !sys.tools.is_empty() {
                writeln!(w, "  tools: {}", sys.tools.join(", ")).expect("write failed");
            }
            if !sys.mcp_servers.is_empty() {
                let names: Vec<_> = sys.mcp_servers.iter().map(|s| s.name.as_str()).collect();
                writeln!(w, "  mcp: {}", names.join(", ")).expect("write failed");
            }
            writeln!(w).expect("write failed");
        }

        OutputMessage::StreamEvent(ev) => {
            print_stream_event(w, &ev.event);
        }

        OutputMessage::Assistant(asst) => {
            write_colored(w, Color::Green, "[assistant]\n");
            for content in &asst.message.content {
                print_content(w, content, "  ");
            }
            if let Some(reason) = &asst.message.stop_reason {
                writeln!(w, "  stop_reason: {:?}", reason).expect("write failed");
            }
            writeln!(w).expect("write failed");
        }

        OutputMessage::User(user) => {
            write_colored(w, Color::Yellow, "[user]\n");
            for content in &user.message.content {
                print_content(w, content, "  ");
            }
            writeln!(w).expect("write failed");
        }

        OutputMessage::Result(res) => {
            let color = if res.is_error {
                Color::Red
            } else {
                Color::Magenta
            };
            write_colored(w, color, &format!("[result] {}\n", res.subtype));

            if let Some(text) = &res.result {
                writeln!(w, "  result: {}", truncate(text, 200)).expect("write failed");
            }

            writeln!(w, "  duration: {}ms", res.duration_ms).expect("write failed");
            writeln!(w, "  turns: {}", res.num_turns).expect("write failed");
            writeln!(w, "  cost: ${:.6}", res.total_cost_usd).expect("write failed");

            let u = &res.usage;
            writeln!(
                w,
                "  tokens: {} in, {} out, {} cache_create, {} cache_read",
                u.input_tokens,
                u.output_tokens,
                u.cache_creation_input_tokens,
                u.cache_read_input_tokens
            )
            .expect("write failed");

            writeln!(w).expect("write failed");
        }
    }

    w.flush().expect("flush failed");
}

/// Prints a streaming event (from --include-partial-messages).
fn print_stream_event(w: &mut impl Write, event: &StreamEvent) {
    match event {
        StreamEvent::MessageStart { .. } => {
            write_colored(w, Color::DarkGreen, "[stream] message_start\n");
        }

        StreamEvent::ContentBlockStart {
            index,
            content_block,
        } => {
            write_colored(
                w,
                Color::DarkGreen,
                &format!("[stream] block {} start: ", index),
            );
            match content_block {
                Content::Text { text } => {
                    write!(w, "{}", text).expect("write failed");
                }
                Content::ToolUse(tu) => {
                    writeln!(w, "tool_use {}({})", tu.name, tu.id).expect("write failed");
                }
                other => {
                    writeln!(w, "{:?}", other).expect("write failed");
                }
            }
        }

        StreamEvent::ContentBlockDelta { index: _, delta } => match delta {
            Delta::TextDelta { text } => {
                write!(w, "{}", text).expect("write failed");
            }
            Delta::InputJsonDelta { partial_json } => {
                write_colored(w, Color::DarkGrey, partial_json);
            }
            Delta::ThinkingDelta { thinking } => {
                write_colored(w, Color::DarkBlue, thinking);
            }
            Delta::SignatureDelta { .. } => {}
        },

        StreamEvent::ContentBlockStop { index } => {
            write_colored(
                w,
                Color::DarkGreen,
                &format!("\n[stream] block {} stop\n", index),
            );
        }

        StreamEvent::MessageDelta { delta, .. } => {
            if let Some(reason) = &delta.stop_reason {
                write_colored(
                    w,
                    Color::DarkGreen,
                    &format!("[stream] stop: {:?}\n", reason),
                );
            }
        }

        StreamEvent::MessageStop => {
            write_colored(w, Color::DarkGreen, "[stream] message_stop\n");
        }

        StreamEvent::Ping => {}

        StreamEvent::Error { error } => {
            write_colored(w, Color::Red, &format!("[stream] error: {:?}\n", error));
        }

        StreamEvent::Unknown { event_type, .. } => {
            write_colored(
                w,
                Color::DarkGrey,
                &format!(
                    "[stream] unknown: {}\n",
                    String::from_utf8_lossy(event_type)
                ),
            );
        }
    }

    w.flush().expect("flush failed");
}

/// Prints a content block with the given prefix.
fn print_content(w: &mut impl Write, content: &Content, prefix: &str) {
    match content {
        Content::Text { text } => {
            for line in text.lines() {
                writeln!(w, "{}{}", prefix, line).expect("write failed");
            }
        }

        Content::ToolUse(tu) => {
            write!(w, "{}", prefix).expect("write failed");
            write_colored(
                w,
                Color::Blue,
                &format!("tool_use: {} ({})\n", tu.name, tu.id),
            );
            let input =
                serde_json::to_string_pretty(&tu.input).unwrap_or_else(|_| tu.input.to_string());
            for line in input.lines() {
                writeln!(w, "{}  {}", prefix, line).expect("write failed");
            }
        }

        Content::ToolResult(tr) => {
            write!(w, "{}", prefix).expect("write failed");
            let status = if tr.is_error == Some(true) {
                "error"
            } else {
                "ok"
            };
            write_colored(
                w,
                Color::Blue,
                &format!("tool_result: {} ({})\n", tr.tool_use_id, status),
            );
            let content_str = format!("{}", tr.content);
            let truncated = truncate(&content_str, 500);
            for line in truncated.lines() {
                writeln!(w, "{}  {}", prefix, line).expect("write failed");
            }
        }

        Content::ServerToolUse { id, name, input } => {
            write!(w, "{}", prefix).expect("write failed");
            write_colored(w, Color::Cyan, &format!("server_tool: {} ({})\n", name, id));
            writeln!(w, "{}  input: {}", prefix, input).expect("write failed");
        }

        Content::WebSearchToolResult {
            tool_use_id,
            content,
        } => {
            write!(w, "{}", prefix).expect("write failed");
            write_colored(
                w,
                Color::Cyan,
                &format!("web_search_result: {}\n", tool_use_id),
            );
            for result in content {
                writeln!(w, "{}  - {} ({})", prefix, result.title, result.url)
                    .expect("write failed");
            }
        }

        Content::Image => {
            writeln!(w, "{}<image>", prefix).expect("write failed");
        }

        Content::Unknown => {
            writeln!(w, "{}<unknown content>", prefix).expect("write failed");
        }
    }
}

/// Writes colored text to the output.
fn write_colored(w: &mut impl Write, color: Color, text: &str) {
    write!(
        w,
        "{}{}{}{}",
        SetForegroundColor(color),
        SetAttribute(Attribute::Bold),
        text,
        SetAttribute(Attribute::Reset)
    )
    .expect("write failed");
}

/// Truncates a string to the given length, adding "..." if truncated.
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len])
    }
}

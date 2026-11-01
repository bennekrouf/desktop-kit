//! `<app> mcp`: the part of an MCP server over stdio that every app wrote the
//! same way — reading JSON-RPC lines, the handshake, `ping`, the tool list,
//! and the shape of a tool's answer. What is the app's own is its tools and
//! what calling them does.
//!
//! An app with nothing to do between messages hands its tool calls to
//! [`Server::serve`], or to [`Server::serve_async`] when they are async. One
//! with its own loop (a timer, progress notifications) reads each line with
//! [`Server::read`], answers the calls it gets back with [`Server::answer`],
//! and [`write`]s every reply.
//!
//! An app that serves the media library ([`Server::library`]) gets its tools
//! and instructions added after its own, and its calls answered first.

use serde_json::{json, Value};
use std::future::Future;
use std::io::{BufRead, Write};
use tokio::io::AsyncBufReadExt;

/// The newest protocol revision these servers speak. A client asking for an
/// older one gets its own back; everything used here exists in all of them.
pub const PROTOCOL: &str = "2025-06-18";

/// What the client is told about the app.
pub struct Server {
    /// The server's name, as `serverInfo.name`: usually the app's id.
    pub name: &'static str,
    /// The app's version: `env!("CARGO_PKG_VERSION")` written in the app.
    pub version: &'static str,
    /// What the client reads before calling anything.
    pub instructions: &'static str,
    /// The app's tools, as `tools/list` gives them.
    pub tools: fn() -> Value,
    /// Whether the shared media library's tools are served too.
    pub library: bool,
}

/// A line that asks the app for something.
#[derive(Clone, Debug, PartialEq)]
pub struct Call {
    pub id: Value,
    pub name: String,
    pub args: Value,
    /// `_meta.progressToken`, or `null`: where progress on a long call goes.
    pub progress: Value,
}

/// What a line from the client comes to.
#[derive(Clone, Debug, PartialEq)]
pub enum Incoming {
    /// Answered here: write it back.
    Reply(Value),
    /// A tool call for the app.
    Call(Call),
}

impl Server {
    /// Reads one line. `None` for a blank line or a notification (no id),
    /// which ask for nothing back: `initialized`, cancellations, and whatever
    /// later revisions add.
    pub fn read(&self, line: &str) -> Option<Incoming> {
        if line.trim().is_empty() {
            return None;
        }
        let Ok(message) = serde_json::from_str::<Value>(line) else {
            return Some(Incoming::Reply(error(Value::Null, -32700, "not JSON")));
        };
        let id = message.get("id").cloned()?;
        let method = message["method"].as_str().unwrap_or("");
        let params = &message["params"];
        let reply = match method {
            "initialize" => {
                let asked = params["protocolVersion"].as_str().unwrap_or(PROTOCOL);
                let version = if asked <= PROTOCOL { asked } else { PROTOCOL };
                result(
                    id,
                    json!({
                        "protocolVersion": version,
                        "capabilities": { "tools": {} },
                        "serverInfo": { "name": self.name, "version": self.version },
                        "instructions": self.instructions(),
                    }),
                )
            }
            "ping" => result(id, json!({})),
            "tools/list" => result(id, json!({ "tools": self.tools() })),
            "tools/call" => {
                let call = Call {
                    id,
                    name: params["name"].as_str().unwrap_or("").to_string(),
                    args: params["arguments"].clone(),
                    progress: params["_meta"]["progressToken"].clone(),
                };
                return Some(match self.library_call(&call) {
                    Some(outcome) => Incoming::Reply(self.answer(&call, outcome)),
                    None => Incoming::Call(call),
                });
            }
            _ => error(id, -32601, &format!("no method {method}")),
        };
        Some(Incoming::Reply(reply))
    }

    /// The reply to `call`, from what the tool came to: a value for the
    /// client to read, or why it failed.
    pub fn answer(&self, call: &Call, outcome: Result<Value, String>) -> Value {
        let (text, is_error) = match outcome {
            Ok(value) => (pretty(&value), false),
            Err(why) => (why, true),
        };
        result(
            call.id.clone(),
            json!({ "content": [{ "type": "text", "text": text }], "isError": is_error }),
        )
    }

    /// Serves stdin until it closes, handing every tool call to `call`.
    pub fn serve(&self, mut call: impl FnMut(&str, &Value) -> Result<Value, String>) {
        for line in std::io::stdin().lock().lines() {
            let Ok(line) = line else { break };
            match self.read(&line) {
                Some(Incoming::Reply(reply)) => write(&reply),
                Some(Incoming::Call(c)) => write(&self.answer(&c, call(&c.name, &c.args))),
                None => {}
            }
        }
    }

    /// [`Server::serve`] for async tools, inside the app's runtime. Calls are
    /// answered one at a time, in the order they came.
    pub async fn serve_async<F, Fut>(&self, mut call: F)
    where
        F: FnMut(Call) -> Fut,
        Fut: Future<Output = Result<Value, String>>,
    {
        let mut lines = tokio::io::BufReader::new(tokio::io::stdin()).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            match self.read(&line) {
                Some(Incoming::Reply(reply)) => write(&reply),
                Some(Incoming::Call(c)) => {
                    let reply = self.answer(&c, call(c.clone()).await);
                    write(&reply);
                }
                None => {}
            }
        }
    }

    fn instructions(&self) -> String {
        if self.library {
            format!(
                "{} {}",
                self.instructions,
                crate::library::mcp::INSTRUCTIONS
            )
        } else {
            self.instructions.to_string()
        }
    }

    /// The app's tools, then the media library's.
    fn tools(&self) -> Value {
        let mut tools = (self.tools)();
        if self.library {
            if let Some(list) = tools.as_array_mut() {
                list.extend(crate::library::mcp::tools());
            }
        }
        tools
    }

    fn library_call(&self, call: &Call) -> Option<Result<Value, String>> {
        if self.library {
            crate::library::mcp::call(&call.name, &call.args)
        } else {
            None
        }
    }
}

/// Writes one message. One per line, and nothing else ever on stdout: the
/// client reads it as the protocol, so a stray print is a corrupt message.
pub fn write(message: &Value) {
    let mut bytes = serde_json::to_vec(message).unwrap_or_default();
    bytes.push(b'\n');
    let mut stdout = std::io::stdout().lock();
    let _ = stdout.write_all(&bytes);
    let _ = stdout.flush();
}

/// A notification, e.g. `notifications/progress`.
pub fn notification(method: &str, params: Value) -> Value {
    json!({ "jsonrpc": "2.0", "method": method, "params": params })
}

fn result(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn error(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

/// A tool result as the client reads it: compact, with no null fields. It
/// stays in the client's context for the rest of the conversation, so every
/// byte here is paid for again on each later turn.
pub fn pretty(value: &Value) -> String {
    serde_json::to_string(&without_nulls(value.clone())).unwrap_or_default()
}

fn without_nulls(value: Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.into_iter()
                .filter(|(_, v)| !v.is_null())
                .map(|(k, v)| (k, without_nulls(v)))
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.into_iter().map(without_nulls).collect()),
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tools() -> Value {
        json!([{ "name": "hello" }])
    }

    const SERVER: Server = Server {
        name: "test",
        version: "1.2.3",
        instructions: "Says hello.",
        tools,
        library: false,
    };

    fn reply(line: &str) -> Value {
        match SERVER.read(line) {
            Some(Incoming::Reply(r)) => r,
            other => panic!("expected a reply, got {other:?}"),
        }
    }

    #[test]
    fn the_handshake_offers_the_older_of_the_two_revisions() {
        let r = reply(
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05"}}"#,
        );
        assert_eq!(r["result"]["protocolVersion"], "2024-11-05");
        assert_eq!(r["result"]["serverInfo"]["version"], "1.2.3");
        assert_eq!(r["result"]["instructions"], "Says hello.");
        let r =
            reply(r#"{"id":1,"method":"initialize","params":{"protocolVersion":"2099-01-01"}}"#);
        assert_eq!(r["result"]["protocolVersion"], PROTOCOL);
    }

    #[test]
    fn notifications_and_blank_lines_get_nothing_back() {
        assert_eq!(SERVER.read("   "), None);
        assert_eq!(
            SERVER.read(r#"{"method":"notifications/initialized"}"#),
            None
        );
    }

    #[test]
    fn bad_lines_and_unknown_methods_are_errors() {
        assert_eq!(reply("{nope")["error"]["code"], -32700);
        assert_eq!(
            reply(r#"{"id":"a","method":"resources/list"}"#)["error"]["code"],
            -32601
        );
        assert_eq!(reply(r#"{"id":2,"method":"ping"}"#)["result"], json!({}));
        assert_eq!(
            reply(r#"{"id":3,"method":"tools/list"}"#)["result"]["tools"],
            tools()
        );
    }

    #[test]
    fn a_tool_call_goes_to_the_app_and_comes_back_without_nulls() {
        let line = r#"{"id":7,"method":"tools/call","params":{"name":"hello","arguments":{"to":"you"},"_meta":{"progressToken":"p"}}}"#;
        let Some(Incoming::Call(call)) = SERVER.read(line) else {
            panic!("expected a call");
        };
        assert_eq!(call.name, "hello");
        assert_eq!(call.args["to"], "you");
        assert_eq!(call.progress, "p");
        let ok = SERVER.answer(
            &call,
            Ok(json!({ "said": "hi", "nothing": null, "deep": [{ "gone": null }] })),
        );
        assert_eq!(ok["id"], 7);
        assert_eq!(
            ok["result"]["content"][0]["text"],
            r#"{"deep":[{}],"said":"hi"}"#
        );
        assert_eq!(ok["result"]["isError"], false);
        let failed = SERVER.answer(&call, Err("no".into()));
        assert_eq!(failed["result"]["content"][0]["text"], "no");
        assert_eq!(failed["result"]["isError"], true);
    }
}

//! One resident authoring session plus the guard bookkeeping every caller shares.
//!
//! The MCP server and the local HTTP API both mutate the same Session type, so
//! revision tracking and error mapping live here rather than in each door.
use crate::session_protocol::{PROTOCOL, Session};
use serde_json::{Value, json};
use std::io;

/// A request the authoring session rejected. Codes mirror the session protocol.
#[derive(Debug)]
pub struct SessionError {
    pub code: String,
    pub detail: String,
}
impl SessionError {
    pub fn new(code: &str, detail: impl Into<String>) -> Self {
        Self {
            code: code.to_owned(),
            detail: detail.into(),
        }
    }
}

fn request_line(id: u64, payload: &Value) -> String {
    json!({
        "protocol": PROTOCOL,
        "protocolVersion": 1,
        "requestId": format!("call-{id}"),
        "payload": payload,
    })
    .to_string()
}

pub struct Client {
    session: Session,
    identity: String,
    revision: u64,
    counter: u64,
}

impl Client {
    pub fn new() -> io::Result<Self> {
        let mut session = Session::new()?;
        let hello = session.handle_line(&request_line(1, &json!({"op": "hello"})));
        let mut client = Self {
            session,
            identity: String::new(),
            revision: 0,
            counter: 1,
        };
        client.absorb(&hello);
        Ok(client)
    }

    /// Send one authoring request. Every rejected request leaves the document,
    /// pending change and history exactly as they were.
    pub fn call(&mut self, payload: Value) -> Result<Value, SessionError> {
        self.counter += 1;
        let response = self
            .session
            .handle_line(&request_line(self.counter, &payload));
        self.absorb(&response);
        if response.get("ok").and_then(Value::as_bool) == Some(true) {
            Ok(response.get("data").cloned().unwrap_or(Value::Null))
        } else {
            let error = response.get("error");
            Err(SessionError::new(
                error
                    .and_then(|e| e.get("code"))
                    .and_then(Value::as_str)
                    .unwrap_or("session_error"),
                error
                    .and_then(|e| e.get("detail"))
                    .and_then(Value::as_str)
                    .unwrap_or("The session rejected the request"),
            ))
        }
    }

    fn absorb(&mut self, response: &Value) {
        let identity = [
            "/data/documentSession",
            "/data/session/documentSession",
            "/documentSession",
        ]
        .iter()
        .find_map(|pointer| response.pointer(pointer).and_then(Value::as_str));
        if let Some(identity) = identity {
            self.identity = identity.to_owned();
        }
        let revision = ["/data/revision", "/data/session/revision", "/revision"]
            .iter()
            .find_map(|pointer| response.pointer(pointer).and_then(Value::as_u64));
        if let Some(revision) = revision {
            self.revision = revision;
        }
    }

    pub fn identity(&self) -> &str {
        &self.identity
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn guard(&self) -> Value {
        json!({
            "documentSession": self.identity,
            "expectedRevision": self.revision,
        })
    }

    pub fn snapshot(&mut self) -> Result<Value, SessionError> {
        self.call(json!({"op": "snapshot"}))
    }

    pub fn template(&mut self) -> Result<Value, SessionError> {
        let data = self.snapshot()?;
        data.get("template")
            .cloned()
            .ok_or_else(|| SessionError::new("no_document", "Open a document first"))
    }

    pub fn open(&mut self, template: Value) -> Result<Value, SessionError> {
        let guard = self.guard();
        self.call(json!({"op": "open", "guard": guard, "template": template}))
    }

    pub fn edit(&mut self, command: Value) -> Result<Value, SessionError> {
        let guard = self.guard();
        self.call(json!({"op": "edit", "guard": guard, "command": command}))
    }

    pub fn prepare(&mut self, command: Value) -> Result<Value, SessionError> {
        let guard = self.guard();
        self.call(json!({"op": "prepare_edit", "guard": guard, "command": command}))
    }

    pub fn commit(&mut self, change: &str) -> Result<Value, SessionError> {
        let guard = self.guard();
        self.call(json!({"op": "commit", "guard": guard, "change": change}))
    }

    pub fn cancel(&mut self, change: &str) -> Result<Value, SessionError> {
        let guard = self.guard();
        self.call(json!({"op": "cancel", "guard": guard, "change": change}))
    }

    pub fn undo(&mut self) -> Result<Value, SessionError> {
        let guard = self.guard();
        self.call(json!({"op": "undo", "guard": guard}))
    }

    pub fn redo(&mut self) -> Result<Value, SessionError> {
        let guard = self.guard();
        self.call(json!({"op": "redo", "guard": guard}))
    }
}

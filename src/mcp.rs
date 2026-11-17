//! Model Context Protocol server over the CPU authoring session.
//!
//! One JSON-RPC 2.0 message per line on stdin/stdout. No solver, renderer or
//! network listener; the only filesystem access is document_open (read) and
//! document_save (write), which the caller names explicitly.
//!
//! Every tool call is translated into the documented clothcraft.authoring
//! requests, so validation, revision guards and history stay in one place.
use crate::inspect::{summarize_document, summarize_template};
use crate::session_client::{Client, SessionError};
use crate::skills;
use crate::template_catalog::Selection;
use serde_json::{Value, json};
use std::io::{self, BufRead, Write};

pub const SERVER_NAME: &str = "clothcraft-mcp";
pub const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");
/// MCP revision announced when a client sends nothing usable.
pub const MCP_PROTOCOL_VERSION: &str = "2025-06-18";
const MAX_TEMPLATE_BYTES: usize = 8 * 1024 * 1024;

/// A rejected tool call. Codes mirror the authoring session where possible.
pub struct ToolError {
    pub code: String,
    pub detail: String,
}
impl ToolError {
    pub fn new(code: &str, detail: impl Into<String>) -> Self {
        Self {
            code: code.to_owned(),
            detail: detail.into(),
        }
    }
}
pub type ToolResult = Result<Value, ToolError>;

/// One MCP server holding one resident authoring session.
pub struct Server {
    client: Client,
}

fn tool_failure(error: SessionError) -> ToolError {
    ToolError::new(&error.code, error.detail)
}

impl Server {
    pub fn new() -> io::Result<Self> {
        Ok(Self {
            client: Client::new()?,
        })
    }

    fn call(&mut self, payload: Value) -> ToolResult {
        self.client.call(payload).map_err(tool_failure)
    }

    fn snapshot(&mut self) -> ToolResult {
        self.client.snapshot().map_err(tool_failure)
    }

    fn template(&mut self) -> ToolResult {
        self.client.template().map_err(tool_failure)
    }

    fn edit(&mut self, command: Value) -> ToolResult {
        Ok(summarize_document(&self.client.edit(command).map_err(tool_failure)?))
    }

    fn execute(&mut self, name: &str, args: &Value) -> ToolResult {
        match name {
            "document_catalog" => Ok(json!({
                "templates": crate::template_catalog::ENTRIES
                    .iter()
                    .map(|entry| json!({
                        "id": entry.id,
                        "title": entry.title,
                        "description": entry.description,
                        "license": entry.license,
                    }))
                    .collect::<Vec<_>>(),
            })),
            "document_open" => self.open(args),
            "document_state" => {
                let data = self.snapshot()?;
                if args.get("summary_only").and_then(Value::as_bool).unwrap_or(false) {
                    Ok(summarize_document(&data))
                } else {
                    Ok(data)
                }
            }
            "document_inspect" => {
                let data = self.snapshot()?;
                Ok(summarize_document(&data))
            }
            "document_save" => {
                let path = require_string(args, "path")?;
                let template = self.template()?;
                let text = serde_json::to_string_pretty(&template)
                    .map_err(|e| ToolError::new("io_failure", e.to_string()))?;
                std::fs::write(&path, text.as_bytes())
                    .map_err(|e| ToolError::new("io_failure", format!("{path}: {e}")))?;
                Ok(json!({"saved": path, "bytes": text.len(), "revision": self.client.revision()}))
            }
            "garment_add_panel" => {
                let points = args.get("points").cloned().ok_or_else(|| {
                    ToolError::new("invalid_arguments", "points [[u,v],...] is required")
                })?;
                if points.as_array().map(Vec::len).unwrap_or(0) < 3 {
                    return Err(ToolError::new(
                        "invalid_arguments",
                        "points needs at least three [u,v] pairs",
                    ));
                }
                let fabric = require_string(args, "fabric")?;
                let tangents = args
                    .get("tangents")
                    .cloned()
                    .unwrap_or_else(|| points.clone());
                let meshing = json!({
                    "target_edge_m": number(args, "target_edge_m").unwrap_or(0.025),
                    "anisotropy": number(args, "anisotropy").unwrap_or(1.0),
                    "axis_degrees": number(args, "axis_degrees").unwrap_or(0.0),
                    "min_angle_degrees": number(args, "min_angle_degrees").unwrap_or(25.0),
                    "max_vertices": args.get("max_vertices").and_then(Value::as_u64).unwrap_or(20_000),
                });
                let command = json!({
                    "kind": "polygon",
                    "points": points,
                    "tangents": tangents,
                    "meshing": meshing,
                    "fabric": fabric,
                });
                self.edit(command)
            }
            "garment_sew" => {
                let a = boundary_ref(args, "a")?;
                let b = boundary_ref(args, "b")?;
                let command = json!({
                    "kind": "sew",
                    "a": a,
                    "b": b,
                    "reverse": args.get("reverse").and_then(Value::as_bool).unwrap_or(false),
                    "ease": args.get("ease").and_then(Value::as_bool).unwrap_or(false),
                });
                self.edit(command)
            }
            "garment_remove_seam" => {
                let seam = require_string(args, "seam")?;
                self.edit(json!({"kind": "remove_seam", "seam": seam}))
            }
            "garment_place" => {
                let panel = require_string(args, "panel")?;
                let translation = args.get("translation").cloned().ok_or_else(|| {
                    ToolError::new("invalid_arguments", "translation [x,y,z] is required")
                })?;
                let rotation = args
                    .get("rotation_degrees")
                    .cloned()
                    .unwrap_or(json!([0.0, 0.0, 0.0]));
                self.edit(json!({
                    "kind": "place",
                    "panel": panel,
                    "placement": {"translation": translation, "rotation_degrees": rotation},
                }))
            }
            "garment_mirror" => {
                let panel = require_string(args, "panel")?;
                let axis = require_string(args, "axis")?;
                let copy_side = args.get("copy_side").cloned().unwrap_or(Value::Null);
                self.edit(
                    json!({"kind": "mirror", "panel": panel, "axis": axis, "copy_side": copy_side}),
                )
            }
            "garment_layout" => {
                let panel = require_string(args, "panel")?;
                let origin = args.get("origin").cloned().ok_or_else(|| {
                    ToolError::new("invalid_arguments", "origin [u,v] is required")
                })?;
                self.edit(json!({"kind": "layout", "panel": panel, "origin": origin}))
            }
            "garment_fabric" => {
                let panel = require_string(args, "panel")?;
                let name = require_string(args, "name")?;
                let properties = match args.get("properties") {
                    Some(properties) => properties.clone(),
                    None => {
                        let template = self.template()?;
                        template
                            .get("fabrics")
                            .and_then(Value::as_array)
                            .and_then(|fabrics| {
                                fabrics.iter().find(|fabric| {
                                    fabric.get("name").and_then(Value::as_str)
                                        == Some(name.as_str())
                                })
                            })
                            .and_then(|fabric| fabric.get("properties"))
                            .cloned()
                            .ok_or_else(|| {
                                ToolError::new(
                                    "unknown_fabric",
                                    format!("No fabric named {name}; pass properties explicitly"),
                                )
                            })?
                    }
                };
                let all_panels = args.get("all_panels").and_then(Value::as_bool).unwrap_or(false);
                self.edit(json!({
                    "kind": "fabric",
                    "panel": panel,
                    "name": name,
                    "properties": properties,
                    "all_panels": all_panels,
                }))
            }
            "garment_duplicate" => {
                let panel = require_string(args, "panel")?;
                self.edit(json!({"kind": "duplicate", "panel": panel}))
            }
            "garment_remove_panel" => {
                let panel = require_string(args, "panel")?;
                self.edit(json!({"kind": "remove_panel", "panel": panel}))
            }
            "garment_undo" => {
                let guard = self.client.guard();
                let data = self.call(json!({"op": "undo", "guard": guard}))?;
                Ok(summarize_document(&data))
            }
            "garment_redo" => {
                let guard = self.client.guard();
                let data = self.call(json!({"op": "redo", "guard": guard}))?;
                Ok(summarize_document(&data))
            }
            "garment_edit" => {
                let command = args.get("command").cloned().ok_or_else(|| {
                    ToolError::new("invalid_arguments", "command object is required")
                })?;
                self.edit(command)
            }
            "change_prepare" => {
                let command = args.get("command").cloned().ok_or_else(|| {
                    ToolError::new("invalid_arguments", "command object is required")
                })?;
                let guard = self.client.guard();
                let data = self.call(json!({"op": "prepare_edit", "guard": guard, "command": command}))?;
                Ok(json!({
                    "change": data.get("change"),
                    "documentSession": data.get("documentSession"),
                    "revision": data.get("revision"),
                    "resetsPhysicalCaches": data.get("resetsPhysicalCaches"),
                    "preview": summarize_template(data.get("template").unwrap_or(&Value::Null)),
                }))
            }
            "change_commit" => {
                let change = require_string(args, "change")?;
                let guard = self.client.guard();
                let data = self.call(json!({"op": "commit", "guard": guard, "change": change}))?;
                Ok(summarize_document(&data))
            }
            "change_cancel" => {
                let change = require_string(args, "change")?;
                let guard = self.client.guard();
                let data = self.call(json!({"op": "cancel", "guard": guard, "change": change}))?;
                Ok(summarize_document(&data))
            }
            "skills_list" => Ok(json!({
                "skills": skills::SKILLS
                    .iter()
                    .map(|skill| json!({"name": skill.name, "description": skill.description}))
                    .collect::<Vec<_>>(),
            })),
            "skills_read" => {
                let name = require_string(args, "name")?;
                let skill = skills::find(&name).ok_or_else(|| {
                    ToolError::new("unknown_skill", format!("No skill named {name}"))
                })?;
                Ok(json!({
                    "name": skill.name,
                    "description": skill.description,
                    "body": skill.body,
                }))
            }
            other => Err(ToolError::new("unknown_tool", format!("No tool named {other}"))),
        }
    }

    fn open(&mut self, args: &Value) -> ToolResult {
        let template = if let Some(id) = string(args, "catalog") {
            let selection =
                Selection::bundled(&id).map_err(|e| ToolError::new("invalid_template", e))?;
            serde_json::to_value(selection.template())
                .map_err(|e| ToolError::new("invalid_template", e.to_string()))?
        } else if let Some(path) = string(args, "path") {
            let bytes = std::fs::read(&path)
                .map_err(|e| ToolError::new("io_failure", format!("{path}: {e}")))?;
            if bytes.len() > MAX_TEMPLATE_BYTES {
                return Err(ToolError::new("invalid_template", "Template exceeds 8 MiB"));
            }
            serde_json::from_slice::<Value>(&bytes)
                .map_err(|e| ToolError::new("invalid_template", e.to_string()))?
        } else if let Some(template) = args.get("template") {
            template.clone()
        } else {
            return Err(ToolError::new(
                "invalid_arguments",
                "Pass catalog, path or template",
            ));
        };
        let data = self.client.open(template).map_err(tool_failure)?;
        Ok(summarize_document(&data))
    }

    fn initialize(&self, params: &Value) -> Value {
        let requested = params
            .get("protocolVersion")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .unwrap_or(MCP_PROTOCOL_VERSION);
        json!({
            "protocolVersion": requested,
            "capabilities": {
                "tools": {"listChanged": false},
                "resources": {"subscribe": false, "listChanged": false},
            },
            "serverInfo": {"name": SERVER_NAME, "version": SERVER_VERSION},
            "instructions": "Drive a ClothCraft garment document over the local CPU authoring session. Open a bundled template (document_open with catalog) or a native template file, read panels and their named boundaries with document_inspect, then mutate with garment_* tools. Every mutation bumps the revision; the server tracks the guard itself. Use change_prepare plus change_commit when a person should confirm a change before it lands.",
        })
    }

    fn tools_call(&mut self, params: &Value) -> ToolResult {
        let name = params
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| ToolError::new("invalid_arguments", "tools/call needs a name"))?;
        let args = params.get("arguments").cloned().unwrap_or(json!({}));
        match self.execute(name, &args) {
            Ok(value) => Ok(tool_value(value)),
            Err(error) => Ok(json!({
                "content": [{"type": "text", "text": format!("{}: {}", error.code, error.detail)}],
                "isError": true,
            })),
        }
    }

    fn resources_read(&mut self, params: &Value) -> ToolResult {
        let uri = params
            .get("uri")
            .and_then(Value::as_str)
            .ok_or_else(|| ToolError::new("invalid_arguments", "resources/read needs a uri"))?;
        let text = match uri {
            "clothcraft://document" => {
                let data = self.snapshot()?;
                serde_json::to_string_pretty(&data)
                    .map_err(|e| ToolError::new("io_failure", e.to_string()))?
            }
            "clothcraft://catalog" => serde_json::to_string_pretty(&json!({
                "templates": crate::template_catalog::ENTRIES
                    .iter()
                    .map(|entry| json!({"id": entry.id, "title": entry.title}))
                    .collect::<Vec<_>>(),
            }))
            .map_err(|e| ToolError::new("io_failure", e.to_string()))?,
            "clothcraft://skills" => serde_json::to_string_pretty(&json!({
                "skills": skills::SKILLS
                    .iter()
                    .map(|skill| json!({"name": skill.name, "description": skill.description}))
                    .collect::<Vec<_>>(),
            }))
            .map_err(|e| ToolError::new("io_failure", e.to_string()))?,
            other if other.starts_with("clothcraft://skills/") => {
                let name = other.trim_start_matches("clothcraft://skills/");
                let skill = skills::find(name).ok_or_else(|| {
                    ToolError::new("unknown_skill", format!("No skill named {name}"))
                })?;
                skill.body.to_owned()
            }
            other => {
                return Err(ToolError::new(
                    "unknown_resource",
                    format!("No resource at {other}"),
                ))
            }
        };
        Ok(json!({"contents": [{"uri": uri, "mimeType": "application/json", "text": text}]}))
    }

    /// One MCP request line in, one response line out. Notifications return None.
    pub fn handle_line(&mut self, line: &str) -> Option<String> {
        let message: Value = match serde_json::from_str(line) {
            Ok(value) => value,
            Err(error) => {
                return Some(
                    json!({
                        "jsonrpc": "2.0",
                        "id": Value::Null,
                        "error": {"code": -32700, "message": format!("Parse error: {error}")},
                    })
                    .to_string(),
                )
            }
        };
        self.handle_message(&message)
    }

    fn handle_message(&mut self, message: &Value) -> Option<String> {
        let method = message.get("method").and_then(Value::as_str)?;
        let id = message.get("id").cloned()?;
        let params = message.get("params").cloned().unwrap_or(Value::Null);
        let outcome: Result<Value, (i64, String)> = match method {
            "initialize" => Ok(self.initialize(&params)),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({"tools": tool_definitions()})),
            "tools/call" => self.tools_call(&params).map_err(|error| (-32602, error.detail)),
            "resources/list" => Ok(json!({"resources": resource_definitions()})),
            "resources/read" => self
                .resources_read(&params)
                .map_err(|error| (-32602, error.detail)),
            "prompts/list" => Ok(json!({"prompts": []})),
            "logging/setLevel" => Ok(json!({})),
            other => Err((-32601, format!("Unknown method: {other}"))),
        };
        Some(match outcome {
            Ok(result) => json!({"jsonrpc": "2.0", "id": id, "result": result}).to_string(),
            Err((code, message)) => json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": {"code": code, "message": message},
            })
            .to_string(),
        })
    }
}

fn string(args: &Value, key: &str) -> Option<String> {
    args.get(key).and_then(Value::as_str).map(str::to_owned)
}
fn require_string(args: &Value, key: &str) -> Result<String, ToolError> {
    string(args, key)
        .ok_or_else(|| ToolError::new("invalid_arguments", format!("{key} must be a string")))
}
fn number(args: &Value, key: &str) -> Option<f64> {
    args.get(key).and_then(Value::as_f64)
}

/// A sewing boundary reference: panel plus either a named polygon edge or a
/// legacy rectangle side. The edge wins when both are present.
fn boundary_ref(args: &Value, key: &str) -> Result<Value, ToolError> {
    let value = args
        .get(key)
        .ok_or_else(|| ToolError::new("invalid_arguments", format!("{key} object is required")))?;
    let panel = value
        .get("panel")
        .and_then(Value::as_str)
        .ok_or_else(|| ToolError::new("invalid_arguments", format!("{key}.panel is required")))?;
    let edge = value.get("edge").and_then(Value::as_str);
    let side = value
        .get("side")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .or_else(|| edge.map(|_| "top".to_owned()))
        .ok_or_else(|| {
            ToolError::new(
                "invalid_arguments",
                format!("{key} needs a side or a polygon edge"),
            )
        })?;
    let mut reference = json!({
        "panel": panel,
        "side": side,
        "reversed": value.get("reversed").and_then(Value::as_bool).unwrap_or(false),
        "range": value.get("range").cloned().unwrap_or(json!([0.0, 1.0])),
    });
    if let Some(edge) = edge {
        reference["edge"] = json!(edge);
    }
    Ok(reference)
}

fn tool_value(value: Value) -> Value {
    let text = serde_json::to_string_pretty(&value).unwrap_or_else(|_| "{}".to_owned());
    json!({
        "content": [{"type": "text", "text": text}],
        "structuredContent": value,
        "isError": false,
    })
}

fn tool_definitions() -> Value {
    let boundary = json!({
        "type": "object",
        "description": "Sewing boundary: a panel plus a named polygon edge (preferred) or a rectangle side.",
        "properties": {
            "panel": {"type": "string"},
            "edge": {"type": "string", "description": "Named polygon boundary, from document_inspect."},
            "side": {"type": "string", "enum": ["top", "right", "bottom", "left"]},
            "range": {"type": "array", "items": {"type": "number"}, "description": "Normalized arclength interval; defaults to the whole boundary."},
            "reversed": {"type": "boolean"}
        },
        "required": ["panel"]
    });
    json!([
        {
            "name": "document_catalog",
            "description": "List the bundled engineering templates that can be opened by id.",
            "inputSchema": {"type": "object", "properties": {}}
        },
        {
            "name": "document_open",
            "description": "Open a garment document. Pass exactly one of catalog (bundled id), path (native template JSON on disk) or template (inline object). Replaces the resident document and resets history.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "catalog": {"type": "string"},
                    "path": {"type": "string"},
                    "template": {"type": "object"}
                }
            }
        },
        {
            "name": "document_state",
            "description": "Read the resident document: identity, revision, undo state and the full native template.",
            "inputSchema": {
                "type": "object",
                "properties": {"summary_only": {"type": "boolean", "default": false}}
            }
        },
        {
            "name": "document_inspect",
            "description": "Read the resident document as a compact summary: panels, their named boundaries, seams, fabrics, measurements and placement. Use it to find targets before editing.",
            "inputSchema": {"type": "object", "properties": {}}
        },
        {
            "name": "document_save",
            "description": "Write the resident template to a path as native template JSON. Save preserves the editable document only; it writes no simulated state.",
            "inputSchema": {
                "type": "object",
                "properties": {"path": {"type": "string"}},
                "required": ["path"]
            }
        },
        {
            "name": "garment_add_panel",
            "description": "Add a named-edge polygon panel. Points are [u,v] in metres in panel material space; tangents are cubic control points per vertex (default: straight edges).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "points": {"type": "array", "items": {"type": "array", "items": {"type": "number"}}},
                    "tangents": {"type": "array", "items": {"type": "array", "items": {"type": "number"}}},
                    "fabric": {"type": "string"},
                    "target_edge_m": {"type": "number", "default": 0.025},
                    "min_angle_degrees": {"type": "number", "default": 25},
                    "max_vertices": {"type": "integer", "default": 20000}
                },
                "required": ["points", "fabric"]
            }
        },
        {
            "name": "garment_sew",
            "description": "Sew two boundaries together. Set ease to permit unequal lengths (gathering); otherwise unequal lengths need explicit easing later.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "a": boundary.clone(),
                    "b": boundary.clone(),
                    "reverse": {"type": "boolean", "default": false},
                    "ease": {"type": "boolean", "default": false}
                },
                "required": ["a", "b"]
            }
        },
        {
            "name": "garment_remove_seam",
            "description": "Remove one seam by id.",
            "inputSchema": {
                "type": "object",
                "properties": {"seam": {"type": "string"}},
                "required": ["seam"]
            }
        },
        {
            "name": "garment_place",
            "description": "Move or rotate a panel in 3D. Translation is metres in world space; rotation is XYZ Euler degrees.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "panel": {"type": "string"},
                    "translation": {"type": "array", "items": {"type": "number"}},
                    "rotation_degrees": {"type": "array", "items": {"type": "number"}}
                },
                "required": ["panel", "translation"]
            }
        },
        {
            "name": "garment_mirror",
            "description": "Mirror a panel across the x or y drafting axis, optionally producing a linked copy.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "panel": {"type": "string"},
                    "axis": {"type": "string", "enum": ["x", "y"]},
                    "copy_side": {"type": "integer"}
                },
                "required": ["panel", "axis"]
            }
        },
        {
            "name": "garment_layout",
            "description": "Place a panel on the shared 2D drafting board. Presentation only: material coordinates and 3D placement are unchanged.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "panel": {"type": "string"},
                    "origin": {"type": "array", "items": {"type": "number"}}
                },
                "required": ["panel", "origin"]
            }
        },
        {
            "name": "garment_fabric",
            "description": "Assign physical fabric properties to a panel (or all panels). Omit properties to reuse a fabric already in the document by name.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "panel": {"type": "string"},
                    "name": {"type": "string"},
                    "properties": {"type": "object"},
                    "all_panels": {"type": "boolean", "default": false}
                },
                "required": ["panel", "name"]
            }
        },
        {
            "name": "garment_duplicate",
            "description": "Duplicate a panel.",
            "inputSchema": {
                "type": "object",
                "properties": {"panel": {"type": "string"}},
                "required": ["panel"]
            }
        },
        {
            "name": "garment_remove_panel",
            "description": "Remove a panel and its seams.",
            "inputSchema": {
                "type": "object",
                "properties": {"panel": {"type": "string"}},
                "required": ["panel"]
            }
        },
        {
            "name": "garment_undo",
            "description": "Undo the last document change.",
            "inputSchema": {"type": "object", "properties": {}}
        },
        {
            "name": "garment_redo",
            "description": "Redo the last undone change.",
            "inputSchema": {"type": "object", "properties": {}}
        },
        {
            "name": "garment_edit",
            "description": "Send one raw authoring command unchanged. Escape hatch for command fields the typed tools do not expose.",
            "inputSchema": {
                "type": "object",
                "properties": {"command": {"type": "object"}},
                "required": ["command"]
            }
        },
        {
            "name": "change_prepare",
            "description": "Compute a change without applying it. Returns a change token and a preview summary; the document and its revision stay untouched.",
            "inputSchema": {
                "type": "object",
                "properties": {"command": {"type": "object"}},
                "required": ["command"]
            }
        },
        {
            "name": "change_commit",
            "description": "Apply a prepared change and bump the revision.",
            "inputSchema": {
                "type": "object",
                "properties": {"change": {"type": "string"}},
                "required": ["change"]
            }
        },
        {
            "name": "change_cancel",
            "description": "Discard a prepared change.",
            "inputSchema": {
                "type": "object",
                "properties": {"change": {"type": "string"}},
                "required": ["change"]
            }
        },
        {
            "name": "skills_list",
            "description": "List the agent skills shipped with this engine. Each skill is a short card: when to use it, exact requests, refusals and what to verify.",
            "inputSchema": {"type": "object", "properties": {}}
        },
        {
            "name": "skills_read",
            "description": "Read one skill card by name, including the request bodies and the verification checklist.",
            "inputSchema": {
                "type": "object",
                "properties": {"name": {"type": "string"}},
                "required": ["name"]
            }
        }
    ])
}

fn resource_definitions() -> Value {
    json!([
        {
            "uri": "clothcraft://document",
            "name": "Resident garment document",
            "description": "Current native template with identity, revision and undo state.",
            "mimeType": "application/json"
        },
        {
            "uri": "clothcraft://catalog",
            "name": "Bundled templates",
            "description": "Engineering templates that document_open accepts by id.",
            "mimeType": "application/json"
        },
        {
            "uri": "clothcraft://skills",
            "name": "Agent skills",
            "description": "Skill cards shipped with the engine, also readable one at a time at clothcraft://skills/<name>.",
            "mimeType": "application/json"
        }
    ])
}

/// Serve MCP on stdin/stdout until EOF.
pub fn serve(mut input: impl BufRead, mut output: impl Write) -> io::Result<()> {
    let mut server = Server::new()?;
    let mut line = String::new();
    loop {
        line.clear();
        if input.read_line(&mut line)? == 0 {
            break;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(response) = server.handle_line(trimmed) {
            output.write_all(response.as_bytes())?;
            output.write_all(b"\n")?;
            output.flush()?;
        }
    }
    Ok(())
}

pub fn run_stdio() -> io::Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    serve(stdin.lock(), stdout.lock())
}
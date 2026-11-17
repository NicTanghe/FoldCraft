//! Local HTTP API for the React studio.
//!
//! Loopback only, one resident document, no authentication: this is the local
//! editor's own socket, not a service. Every mutation goes through the same
//! Session the MCP server uses, so guards and validation cannot drift apart.
use crate::inspect::{preview_paths, summarize_document};
use crate::session_client::{Client, SessionError};
use crate::template_catalog::Selection;
use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    routing::{get, post},
};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex, MutexGuard};

type Shared = Arc<Mutex<Client>>;
type ApiResult = Result<Json<Value>, (StatusCode, Json<Value>)>;

const MAX_TEMPLATE_BYTES: usize = 8 * 1024 * 1024;

pub fn router() -> std::io::Result<Router> {
    let state: Shared = Arc::new(Mutex::new(Client::new()?));
    Ok(Router::new()
        .route("/api/health", get(health))
        .route("/api/catalog", get(catalog))
        .route("/api/document", get(document))
        .route("/api/preview", get(preview))
        .route("/api/open", post(open))
        .route("/api/command", post(command))
        .route("/api/prepare", post(prepare))
        .route("/api/commit", post(commit))
        .route("/api/cancel", post(cancel))
        .route("/api/undo", post(undo))
        .route("/api/redo", post(redo))
        .with_state(state))
}

fn lock(state: &Shared) -> Result<MutexGuard<'_, Client>, (StatusCode, Json<Value>)> {
    state.lock().map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": {"code": "poisoned", "detail": "The document session did not recover"}})),
        )
    })
}

fn rejected(error: SessionError) -> (StatusCode, Json<Value>) {
    (
        StatusCode::BAD_REQUEST,
        Json(json!({"error": {"code": error.code, "detail": error.detail}})),
    )
}

fn bad_request(detail: impl Into<String>) -> (StatusCode, Json<Value>) {
    (
        StatusCode::BAD_REQUEST,
        Json(json!({"error": {"code": "invalid_arguments", "detail": detail.into()}})),
    )
}

async fn health() -> Json<Value> {
    Json(json!({"ok": true, "service": "cloth-server", "version": env!("CARGO_PKG_VERSION")}))
}

async fn catalog() -> Json<Value> {
    Json(json!({
        "templates": crate::template_catalog::ENTRIES
            .iter()
            .map(|entry| json!({
                "id": entry.id,
                "title": entry.title,
                "description": entry.description,
                "license": entry.license,
            }))
            .collect::<Vec<_>>(),
    }))
}

async fn document(State(state): State<Shared>) -> ApiResult {
    let snapshot = lock(&state)?.snapshot().map_err(rejected)?;
    Ok(Json(summarize_document(&snapshot)))
}

async fn preview(State(state): State<Shared>) -> ApiResult {
    let template = lock(&state)?.template().map_err(rejected)?;
    Ok(Json(preview_paths(&template).map_err(rejected)?))
}

/// Body: {"catalog": id} | {"path": path} | {"template": {...}}
async fn open(State(state): State<Shared>, Json(body): Json<Value>) -> ApiResult {
    let template = if let Some(id) = body.get("catalog").and_then(Value::as_str) {
        let selection =
            Selection::bundled(id).map_err(|error| bad_request(error))?;
        serde_json::to_value(selection.template())
            .map_err(|error| bad_request(error.to_string()))?
    } else if let Some(path) = body.get("path").and_then(Value::as_str) {
        let bytes = std::fs::read(path).map_err(|error| bad_request(format!("{path}: {error}")))?;
        if bytes.len() > MAX_TEMPLATE_BYTES {
            return Err(bad_request("Template exceeds 8 MiB"));
        }
        serde_json::from_slice::<Value>(&bytes).map_err(|error| bad_request(error.to_string()))?
    } else if let Some(template) = body.get("template") {
        template.clone()
    } else {
        return Err(bad_request("Pass catalog, path or template"));
    };
    let snapshot = lock(&state)?.open(template).map_err(rejected)?;
    Ok(Json(summarize_document(&snapshot)))
}

async fn command(State(state): State<Shared>, Json(body): Json<Value>) -> ApiResult {
    let command = body
        .get("command")
        .cloned()
        .ok_or_else(|| bad_request("command object is required"))?;
    let snapshot = lock(&state)?.edit(command).map_err(rejected)?;
    Ok(Json(summarize_document(&snapshot)))
}

async fn prepare(State(state): State<Shared>, Json(body): Json<Value>) -> ApiResult {
    let command = body
        .get("command")
        .cloned()
        .ok_or_else(|| bad_request("command object is required"))?;
    let data = lock(&state)?.prepare(command).map_err(rejected)?;
    Ok(Json(json!({
        "change": data.get("change"),
        "documentSession": data.get("documentSession"),
        "revision": data.get("revision"),
        "resetsPhysicalCaches": data.get("resetsPhysicalCaches"),
        "preview": crate::inspect::summarize_template(data.get("template").unwrap_or(&Value::Null)),
    })))
}

async fn commit(State(state): State<Shared>, Json(body): Json<Value>) -> ApiResult {
    let change = body
        .get("change")
        .and_then(Value::as_str)
        .ok_or_else(|| bad_request("change token is required"))?;
    let snapshot = lock(&state)?.commit(change).map_err(rejected)?;
    Ok(Json(summarize_document(&snapshot)))
}

async fn cancel(State(state): State<Shared>, Json(body): Json<Value>) -> ApiResult {
    let change = body
        .get("change")
        .and_then(Value::as_str)
        .ok_or_else(|| bad_request("change token is required"))?;
    let snapshot = lock(&state)?.cancel(change).map_err(rejected)?;
    Ok(Json(summarize_document(&snapshot)))
}

async fn undo(State(state): State<Shared>) -> ApiResult {
    let snapshot = lock(&state)?.undo().map_err(rejected)?;
    Ok(Json(summarize_document(&snapshot)))
}

async fn redo(State(state): State<Shared>) -> ApiResult {
    let snapshot = lock(&state)?.redo().map_err(rejected)?;
    Ok(Json(summarize_document(&snapshot)))
}

//! MCP surface: handshake, tool listing and a document round trip.
use marvelous_cloth::mcp::Server;
use serde_json::{Value, json};

fn rpc(server: &mut Server, method: &str, params: Value) -> Value {
    let request = json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": params});
    let response = server
        .handle_line(&request.to_string())
        .unwrap_or_else(|| panic!("{method} returned no response"));
    serde_json::from_str(&response).expect("response is JSON")
}

fn call(server: &mut Server, name: &str, arguments: Value) -> Value {
    let response = rpc(server, "tools/call", json!({"name": name, "arguments": arguments}));
    response["result"].clone()
}

fn structured(server: &mut Server, name: &str, arguments: Value) -> Value {
    let result = call(server, name, arguments);
    assert_eq!(result["isError"], json!(false), "tool {name} failed: {result}");
    result["structuredContent"].clone()
}

#[test]
fn initialize_advertises_tools_and_resources() {
    let mut server = Server::new().expect("server");
    let response = rpc(
        &mut server,
        "initialize",
        json!({"protocolVersion": "2025-06-18", "capabilities": {}}),
    );
    let result = &response["result"];
    assert_eq!(result["protocolVersion"], json!("2025-06-18"));
    assert_eq!(result["serverInfo"]["name"], json!("clothcraft-mcp"));
    assert!(result["capabilities"]["tools"].is_object());
}

#[test]
fn tools_list_covers_document_and_garment_work() {
    let mut server = Server::new().expect("server");
    let response = rpc(&mut server, "tools/list", json!({}));
    let names: Vec<String> = response["result"]["tools"]
        .as_array()
        .expect("tools array")
        .iter()
        .map(|tool| tool["name"].as_str().expect("name").to_owned())
        .collect();
    for expected in [
        "document_open",
        "document_inspect",
        "garment_add_panel",
        "garment_sew",
        "garment_place",
        "garment_undo",
        "change_prepare",
        "change_commit",
    ] {
        assert!(names.contains(&expected.to_owned()), "missing {expected} in {names:?}");
    }
    assert!(names.contains(&"skills_list".to_owned()));
    assert!(names.contains(&"skills_read".to_owned()));
    assert_eq!(names.len(), 22, "unexpected tool count: {names:?}");
}

#[test]
fn catalog_open_reports_panels_and_named_boundaries() {
    let mut server = Server::new().expect("server");
    let opened = structured(&mut server, "document_open", json!({"catalog": "tshirt"}));
    assert_eq!(opened["revision"], json!(0));
    let document = &opened["document"];
    let panels = document["panels"].as_array().expect("panels");
    assert_eq!(panels.len(), 2);
    let boundaries = panels[0]["boundaries"].as_array().expect("boundaries");
    assert!(boundaries.iter().any(|name| name == "side-right"));
    assert!(document["seams"].as_array().expect("seams").len() >= 4);
}

#[test]
fn unknown_tool_is_a_tool_error_not_a_transport_error() {
    let mut server = Server::new().expect("server");
    let result = call(&mut server, "no_such_tool", json!({}));
    assert_eq!(result["isError"], json!(true));
    assert!(result["content"][0]["text"].as_str().unwrap_or("").contains("unknown_tool"));
}

#[test]
fn prepared_change_does_not_touch_the_document_until_commit() {
    let mut server = Server::new().expect("server");
    structured(&mut server, "document_open", json!({"catalog": "tshirt"}));
    let prepared = structured(
        &mut server,
        "change_prepare",
        json!({"command": {"kind": "layout", "panel": "front", "origin": [0.4, 0.3]}}),
    );
    assert_eq!(prepared["revision"], json!(0));
    let change = prepared["change"].as_str().expect("change token").to_owned();

    let before = structured(&mut server, "document_inspect", json!({}));
    assert_eq!(before["revision"], json!(0), "prepare must not bump the revision");

    let committed = structured(&mut server, "change_commit", json!({"change": change}));
    assert_eq!(committed["revision"], json!(1));
    assert_eq!(committed["canUndo"], json!(true));
}

#[test]
fn undo_returns_the_document_to_the_previous_revision() {
    let mut server = Server::new().expect("server");
    structured(&mut server, "document_open", json!({"catalog": "tshirt"}));
    let edited = structured(
        &mut server,
        "garment_layout",
        json!({"panel": "front", "origin": [0.2, 0.1]}),
    );
    assert_eq!(edited["revision"], json!(1));
    let undone = structured(&mut server, "garment_undo", json!({}));
    assert_eq!(undone["revision"], json!(2));
    assert_eq!(undone["canRedo"], json!(true));
}
#[test]
fn skill_cards_are_listed_and_readable() {
    let mut server = Server::new().expect("server");
    let listed = structured(&mut server, "skills_list", json!({}));
    let names: Vec<&str> = listed["skills"]
        .as_array()
        .expect("skills")
        .iter()
        .map(|skill| skill["name"].as_str().expect("name"))
        .collect();
    assert_eq!(names.len(), 4, "unexpected skills: {names:?}");
    for expected in ["inspect-document", "draft-panel", "sew-seams", "gate-changes"] {
        assert!(names.contains(&expected), "missing skill {expected}");
    }
    let card = structured(&mut server, "skills_read", json!({"name": "sew-seams"}));
    assert_eq!(card["name"], json!("sew-seams"));
    let body = card["body"].as_str().expect("body");
    assert!(body.contains("garment_sew"), "skill body should name the tools");
    let missing = call(&mut server, "skills_read", json!({"name": "no-such-skill"}));
    assert_eq!(missing["isError"], json!(true));
}

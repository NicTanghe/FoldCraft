//! Integration acceptance tests for Ticket FC-0011:
//! Top menu bar, simulation transport, 3D/2D tool rails, radial menu, and command palette in cssimpler.
#![cfg(feature = "desktop")]

use marvelous_cloth::commands::CapabilityContext;
use marvelous_cloth::css_ui::{
    ALL_COMMAND_IDS, CommandPaletteState, CornerMode, PaletteKey, PinMode, RadialMenuAction,
    RadialMenuState, RadialSector, Rail2dAction, Rail2dState, Rail3dAction, Rail3dState, Tool3d,
    TopBarAction, TopBarState, render_2d_rail, render_3d_rail, render_command_palette,
    render_radial_menu, render_top_bar,
};
use cssimpler::core::{ElementNode, Node};

fn ready_ctx() -> CapabilityContext {
    CapabilityContext {
        document_loaded: true,
        selected_panel: true,
        worker_ready: true,
        filesystem_ready: true,
        playing: false,
        pending: false,
    }
}

fn find_element_by_id<'a>(node: &'a Node, id: &str) -> Option<&'a ElementNode> {
    if let Node::Element(elem) = node {
        if elem.id.as_deref() == Some(id) {
            return Some(elem);
        }
        for child in &elem.children {
            if let Some(found) = find_element_by_id(child, id) {
                return Some(found);
            }
        }
    }
    None
}

fn find_elements_by_class<'a>(
    node: &'a Node,
    class_name: &str,
    acc: &mut Vec<&'a ElementNode>,
) {
    if let Node::Element(elem) = node {
        if elem.classes.iter().any(|c| c == class_name) {
            acc.push(elem);
        }
        for child in &elem.children {
            find_elements_by_class(child, class_name, acc);
        }
    }
}

fn find_elements_by_attr<'a>(node: &'a Node, attr: &str, acc: &mut Vec<&'a ElementNode>) {
    if let Node::Element(elem) = node {
        if elem.attributes.contains_key(attr) {
            acc.push(elem);
        }
        for child in &elem.children {
            find_elements_by_attr(child, attr, acc);
        }
    }
}

fn find_elements_by_class_in_element<'a>(
    elem: &'a ElementNode,
    class_name: &str,
    acc: &mut Vec<&'a ElementNode>,
) {
    if elem.classes.iter().any(|c| c == class_name) {
        acc.push(elem);
    }
    for child in &elem.children {
        find_elements_by_class(child, class_name, acc);
    }
}

fn text_content(node: &Node) -> String {
    match node {
        Node::Text(t) => t.clone(),
        Node::Element(e) => element_text(e),
    }
}

fn element_text(elem: &ElementNode) -> String {
    elem.children.iter().map(text_content).collect::<Vec<_>>().join("")
}

#[test]
fn all_12_command_ids_render_in_cssimpler_command_palette_with_shortcuts_and_availability() {
    let mut state = CommandPaletteState::new(ready_ctx());
    state.open = true;

    let palette_node = render_command_palette(&state).expect("palette renders when open");
    let mut items = Vec::new();
    find_elements_by_class(&palette_node, "command-palette-item", &mut items);

    assert_eq!(
        items.len(),
        12,
        "All 12 typed CommandId items must be rendered in the cssimpler palette"
    );

    // Verify each CommandId item is present
    for id in ALL_COMMAND_IDS {
        let desc = id.descriptor();
        let item = items
            .iter()
            .find(|e| e.attribute("data-command-id") == Some(desc.key))
            .unwrap_or_else(|| panic!("command {} missing from rendered palette", desc.key));

        assert_eq!(item.attribute("data-enabled"), Some("true"));

        // Check shortcut badge
        if let Some(shortcut) = desc.shortcut {
            let mut shortcuts = Vec::new();
            find_elements_by_class_in_element(item, "command-shortcut", &mut shortcuts);
            assert_eq!(shortcuts.len(), 1);
            assert_eq!(element_text(shortcuts[0]), shortcut);
        }
    }
}

#[test]
fn pending_trim_draft_disables_reset_drape_and_destructive_commands_in_transport_and_palette() {
    let trim_reason = "Apply or cancel the trim draft before resetting the simulation";
    let example_reason = "Apply or cancel the trim draft before loading an example";

    // 1. Simulation Transport Bar
    let mut top_bar_state = TopBarState {
        has_pending_trim_draft: true,
        ..Default::default()
    };
    let top_bar_node = render_top_bar(&top_bar_state);
    let reset_btn = find_element_by_id(&top_bar_node, "transport-reset").expect("reset button exists");

    assert_eq!(reset_btn.attribute("data-enabled"), Some("false"));
    assert!(reset_btn.classes.contains(&"disabled".to_string()));
    assert_eq!(reset_btn.attribute("data-disabled-reason"), Some(trim_reason));
    assert_eq!(reset_btn.attribute("title"), Some(trim_reason));
    assert_eq!(top_bar_state.dispatch(TopBarAction::SimulationReset), None);

    // 2. Command Palette
    let mut palette_state = CommandPaletteState::new(ready_ctx());
    palette_state.open = true;
    palette_state.has_pending_trim_draft = true;

    let palette_node = render_command_palette(&palette_state).expect("palette renders when open");
    let mut palette_items = Vec::new();
    find_elements_by_class(&palette_node, "command-palette-item", &mut palette_items);

    let reset_cmd = palette_items
        .iter()
        .find(|e| e.attribute("data-command-id") == Some("simulation.reset"))
        .expect("simulation.reset item exists");
    assert_eq!(reset_cmd.attribute("data-enabled"), Some("false"));
    assert_eq!(reset_cmd.attribute("data-disabled-reason"), Some(trim_reason));

    let tshirt_cmd = palette_items
        .iter()
        .find(|e| e.attribute("data-command-id") == Some("example.tshirt"))
        .expect("example.tshirt item exists");
    assert_eq!(tshirt_cmd.attribute("data-enabled"), Some("false"));
    assert_eq!(tshirt_cmd.attribute("data-disabled-reason"), Some(example_reason));

    let dress_cmd = palette_items
        .iter()
        .find(|e| e.attribute("data-command-id") == Some("example.dress"))
        .expect("example.dress item exists");
    assert_eq!(dress_cmd.attribute("data-enabled"), Some("false"));
    assert_eq!(dress_cmd.attribute("data-disabled-reason"), Some(example_reason));

    // Keyboard enter on blocked reset returns None
    palette_state.selected_index = 2; // ResetSimulation
    assert_eq!(palette_state.handle_key(PaletteKey::Enter), None);
    assert!(palette_state.open);
}

#[test]
fn tool_rails_and_corner_tool_radius_shape_modes_render_and_dispatch() {
    // 3D Rail
    let mut rail_3d = Rail3dState::default();
    let node_3d = render_3d_rail(&rail_3d);

    let mut btns_3d = Vec::new();
    find_elements_by_attr(&node_3d, "data-tool", &mut btns_3d);
    assert_eq!(btns_3d.len(), 6);

    let mut grab_readouts = Vec::new();
    find_elements_by_class(&node_3d, "readout", &mut grab_readouts);
    assert!(element_text(grab_readouts[0]).contains("50mm"));
    assert!(element_text(grab_readouts[1]).contains("1.0"));

    rail_3d.dispatch(Rail3dAction::SelectTool(Tool3d::Sew));
    assert_eq!(rail_3d.active_tool, Tool3d::Sew);

    rail_3d.dispatch(Rail3dAction::TogglePinMode);
    assert_eq!(rail_3d.pin_mode, PinMode::Interior);

    rail_3d.dispatch(Rail3dAction::TriggerAvatarDiagnostics);
    assert_eq!(rail_3d.active_tool, Tool3d::AvatarDiagnostics);

    // 2D Rail & Corner Tool
    let mut rail_2d = Rail2dState::default();
    let node_2d = render_2d_rail(&rail_2d);

    let mut btns_2d = Vec::new();
    find_elements_by_attr(&node_2d, "data-tool", &mut btns_2d);
    assert_eq!(btns_2d.len(), 10);

    let mut mode_btns = Vec::new();
    find_elements_by_attr(&node_2d, "data-mode", &mut mode_btns);
    assert_eq!(mode_btns.len(), 2);
    let radius_btn = mode_btns.iter().find(|b| b.attribute("data-mode") == Some("radius")).unwrap();
    let shape_btn = mode_btns.iter().find(|b| b.attribute("data-mode") == Some("shape")).unwrap();
    assert!(radius_btn.classes.contains(&"active".to_string()));
    assert!(!shape_btn.classes.contains(&"active".to_string()));

    let mut corner_readouts = Vec::new();
    find_elements_by_class(&node_2d, "readout", &mut corner_readouts);
    assert!(element_text(corner_readouts[0]).contains("10.0 mm"));
    assert!(element_text(corner_readouts[1]).contains("0.55"));

    // Switch to Shape mode and adjust parameters
    rail_2d.dispatch(Rail2dAction::SetCornerMode(CornerMode::Shape));
    rail_2d.dispatch(Rail2dAction::SetCornerRadius(20.0));
    rail_2d.dispatch(Rail2dAction::SetCornerTension(0.65));

    let node_2d_updated = render_2d_rail(&rail_2d);
    let mut mode_btns_updated = Vec::new();
    find_elements_by_attr(&node_2d_updated, "data-mode", &mut mode_btns_updated);
    let shape_btn_updated = mode_btns_updated.iter().find(|b| b.attribute("data-mode") == Some("shape")).unwrap();
    assert!(shape_btn_updated.classes.contains(&"active".to_string()));

    let mut readouts_updated = Vec::new();
    find_elements_by_class(&node_2d_updated, "readout", &mut readouts_updated);
    assert!(element_text(readouts_updated[0]).contains("20.0 mm"));
    assert!(element_text(readouts_updated[1]).contains("0.65"));
}

#[test]
fn radial_menu_renders_at_pointer_coordinates_when_open() {
    let mut state = RadialMenuState::default();
    assert!(render_radial_menu(&state).is_none());

    state.dispatch(RadialMenuAction::Open { x: 280.0, y: 410.5 });
    assert!(state.open);

    let node = render_radial_menu(&state).expect("radial menu renders when open");
    let elem = find_element_by_id(&node, "radial-menu").unwrap();
    assert_eq!(elem.attribute("data-x"), Some("280.0"));
    assert_eq!(elem.attribute("data-y"), Some("410.5"));

    let style = elem.attribute("style").unwrap();
    assert!(style.contains("left: 280.0px"));
    assert!(style.contains("top: 410.5px"));

    let mut sectors = Vec::new();
    find_elements_by_attr(&node, "data-sector", &mut sectors);
    assert_eq!(sectors.len(), 8);

    for (i, sec) in RadialSector::ALL.iter().enumerate() {
        let i_str = i.to_string();
        let sector_elem = sectors.iter().find(|s| s.attribute("data-sector") == Some(i_str.as_str())).unwrap();
        assert_eq!(sector_elem.attribute("data-action"), Some(sec.key()));
        assert!(element_text(sector_elem).contains(sec.label()));
    }
}

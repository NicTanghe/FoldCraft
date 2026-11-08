//! Searchable command palette overlay authored in cssimpler.
//!
//! Routes all 12 typed `CommandId` actions, provides subsequence filtering,
//! keyboard navigation (`Up`/`Down`/`Enter`/`Escape`), and enforces pending-draft
//! guards (FC-0008) and simulation worker availability states.

use cssimpler::core::Node;
use cssimpler::ui;
use marvelous_cloth::commands::{self, CapabilityContext, CommandDescriptor, CommandId};

pub const ALL_COMMAND_IDS: [CommandId; 12] = [
    CommandId::ToggleSimulation,
    CommandId::StepSimulation,
    CommandId::ResetSimulation,
    CommandId::ExportSession,
    CommandId::SplitView,
    CommandId::GarmentView,
    CommandId::PatternView,
    CommandId::ToggleInspector,
    CommandId::ResetLayout,
    CommandId::OpenPreferences,
    CommandId::LoadTshirt,
    CommandId::LoadDress,
];

pub fn command_aliases(id: CommandId) -> &'static str {
    match id {
        CommandId::ToggleSimulation => "play pause run simulate start stop space",
        CommandId::StepSimulation => "step frame advance forward single",
        CommandId::ResetSimulation => "reset flat arrange restart drape resetdrape",
        CommandId::ExportSession => "export write artifacts save files disk",
        CommandId::SplitView => "split both 2d 3d view dual viewport",
        CommandId::GarmentView => "garment cloth 3d viewport only",
        CommandId::PatternView => "pattern flat 2d viewport only board",
        CommandId::ToggleInspector => "properties inspector sidebar toggle dock pane",
        CommandId::ResetLayout => "layout workspace reset restore default factory",
        CommandId::OpenPreferences => "settings preferences options appearance config",
        CommandId::LoadTshirt => "tshirt t-shirt tee example preset sample",
        CommandId::LoadDress => "dress example preset sample",
        _ => "",
    }
}

pub fn evaluate_availability(
    id: CommandId,
    ctx: CapabilityContext,
    has_pending_trim_draft: bool,
    has_pending_measurement_draft: bool,
) -> Result<(), &'static str> {
    if has_pending_trim_draft {
        match id {
            CommandId::LoadTshirt | CommandId::LoadDress => {
                return Err("Apply or cancel the trim draft before loading an example");
            }
            CommandId::ResetSimulation => {
                return Err("Apply or cancel the trim draft before resetting the simulation");
            }
            _ => {}
        }
    }
    if has_pending_measurement_draft {
        match id {
            CommandId::LoadTshirt | CommandId::LoadDress => {
                return Err("Save or discard the measurement draft before loading an example");
            }
            CommandId::ResetSimulation => {
                return Err("Save or discard the measurement draft before resetting the simulation");
            }
            _ => {}
        }
    }
    commands::availability(id, ctx).map_err(|reason| reason.message())
}

pub fn is_subsequence(needle: &str, haystack: &str) -> bool {
    let mut chars = haystack.chars();
    needle
        .chars()
        .all(|c| chars.any(|candidate| candidate.eq_ignore_ascii_case(&c)))
}

pub fn matches_query(query: &str, id: CommandId) -> bool {
    let term = query.trim().to_lowercase();
    if term.is_empty() {
        return true;
    }
    let desc = id.descriptor();
    let label = desc.label.to_lowercase();
    let key = desc.key.to_lowercase();
    let shortcut = desc.shortcut.unwrap_or("").to_lowercase();
    let aliases = command_aliases(id);
    let full_text = format!("{label} {key} {shortcut} {aliases}");

    for word in term.split_whitespace() {
        if full_text.contains(word) {
            continue;
        }
        if word.chars().count() >= 3
            && full_text
                .split(|c: char| !c.is_alphanumeric())
                .any(|candidate| is_subsequence(word, candidate))
        {
            continue;
        }
        return false;
    }
    true
}

#[derive(Clone, Debug, PartialEq)]
pub struct PaletteItem {
    pub id: CommandId,
    pub key: &'static str,
    pub label: &'static str,
    pub shortcut: Option<&'static str>,
    pub availability: Result<(), &'static str>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaletteKey {
    Up,
    Down,
    Enter,
    Escape,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PaletteAction {
    Open,
    Close,
    SetQuery(String),
    SelectIndex(usize),
    Execute(CommandId),
}

#[derive(Clone, Debug, PartialEq)]
pub struct CommandPaletteState {
    pub open: bool,
    pub query: String,
    pub selected_index: usize,
    pub capability: CapabilityContext,
    pub has_pending_trim_draft: bool,
    pub has_pending_measurement_draft: bool,
}

impl CommandPaletteState {
    pub fn new(capability: CapabilityContext) -> Self {
        Self {
            open: false,
            query: String::new(),
            selected_index: 0,
            capability,
            has_pending_trim_draft: false,
            has_pending_measurement_draft: false,
        }
    }

    pub fn filtered_items(&self) -> Vec<PaletteItem> {
        ALL_COMMAND_IDS
            .iter()
            .copied()
            .filter(|&id| matches_query(&self.query, id))
            .map(|id| {
                let desc: &'static CommandDescriptor = id.descriptor();
                let availability = evaluate_availability(
                    id,
                    self.capability,
                    self.has_pending_trim_draft,
                    self.has_pending_measurement_draft,
                );
                PaletteItem {
                    id,
                    key: desc.key,
                    label: desc.label,
                    shortcut: desc.shortcut,
                    availability,
                }
            })
            .collect()
    }

    pub fn handle_key(&mut self, key: PaletteKey) -> Option<CommandId> {
        let items = self.filtered_items();
        if items.is_empty() {
            if key == PaletteKey::Escape {
                self.open = false;
            }
            return None;
        }

        match key {
            PaletteKey::Escape => {
                self.open = false;
                None
            }
            PaletteKey::Up => {
                if self.selected_index == 0 {
                    self.selected_index = items.len().saturating_sub(1);
                } else {
                    self.selected_index -= 1;
                }
                None
            }
            PaletteKey::Down => {
                self.selected_index = (self.selected_index + 1) % items.len();
                None
            }
            PaletteKey::Enter => {
                let item = items.get(self.selected_index)?;
                if item.availability.is_ok() {
                    self.open = false;
                    Some(item.id)
                } else {
                    None
                }
            }
        }
    }

    pub fn dispatch(&mut self, action: PaletteAction) -> Option<CommandId> {
        match action {
            PaletteAction::Open => {
                self.open = true;
                self.selected_index = 0;
                None
            }
            PaletteAction::Close => {
                self.open = false;
                None
            }
            PaletteAction::SetQuery(q) => {
                self.query = q;
                self.selected_index = 0;
                None
            }
            PaletteAction::SelectIndex(idx) => {
                self.selected_index = idx;
                None
            }
            PaletteAction::Execute(id) => {
                let is_available = evaluate_availability(
                    id,
                    self.capability,
                    self.has_pending_trim_draft,
                    self.has_pending_measurement_draft,
                )
                .is_ok();
                if is_available {
                    self.open = false;
                    Some(id)
                } else {
                    None
                }
            }
        }
    }
}

pub fn render_command_palette(state: &CommandPaletteState) -> Option<Node> {
    if !state.open {
        return None;
    }

    let items = state.filtered_items();
    let rendered_items: Vec<Node> = items
        .iter()
        .enumerate()
        .map(|(idx, item)| {
            let is_selected = idx == state.selected_index;
            let is_enabled = item.availability.is_ok();
            let mut classes = vec!["command-palette-item".to_string()];
            if is_selected {
                classes.push("selected".to_string());
            }
            if is_enabled {
                classes.push("available".to_string());
            } else {
                classes.push("disabled".to_string());
            }
            let class_str = classes.join(" ");
            let enabled_str = if is_enabled { "true" } else { "false" };
            let reason_str = item.availability.err().unwrap_or("");
            let key_str = item.key;
            let label_str = item.label;

            let shortcut_node = item.shortcut.map(|sc| {
                ui! {
                    <span class="command-shortcut">{sc}</span>
                }
            });

            let reason_node = item.availability.err().map(|reason| {
                ui! {
                    <span class="command-disabled-reason">{reason}</span>
                }
            });

            ui! {
                <div
                    class={class_str}
                    data-command-id={key_str}
                    data-enabled={enabled_str}
                    data-disabled-reason={reason_str}
                    title={if is_enabled { label_str } else { reason_str }}
                >
                    <span class="command-label">{label_str}</span>
                    {shortcut_node}
                    {reason_node}
                </div>
            }
        })
        .collect();

    let query_val = state.query.as_str();

    Some(ui! {
        <div id="command-palette-overlay" class="command-palette-overlay">
            <div class="command-palette-modal">
                <input
                    type="text"
                    id="command-palette-input"
                    class="command-palette-input"
                    value={query_val}
                    placeholder="Search commands… (Ctrl/Cmd+Shift+P)"
                />
                <div class="command-palette-list">
                    {rendered_items}
                </div>
            </div>
        </div>
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use cssimpler::core::ElementNode;

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

    #[allow(dead_code)]
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

    #[allow(dead_code)]
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

    fn text_content(node: &Node) -> String {
        match node {
            Node::Text(t) => t.clone(),
            Node::Element(e) => element_text(e),
        }
    }

    fn element_text(elem: &ElementNode) -> String {
        elem.children.iter().map(text_content).collect::<Vec<_>>().join("")
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

    #[test]
    fn all_12_command_ids_render_with_shortcuts_and_availability() {
        let mut state = CommandPaletteState::new(ready_ctx());
        state.open = true;

        let node = render_command_palette(&state).expect("palette renders when open");
        let mut items = Vec::new();
        find_elements_by_class(&node, "command-palette-item", &mut items);

        assert_eq!(items.len(), 12, "exactly 12 typed CommandId items must render");

        // Verify all 12 command keys are present and enabled in ready context
        let expected_keys = [
            "simulation.toggle",
            "simulation.step",
            "simulation.reset",
            "file.export",
            "workspace.view_split",
            "workspace.view_garment",
            "workspace.view_patterns",
            "workspace.inspector",
            "workspace.reset_layout",
            "workspace.preferences",
            "example.tshirt",
            "example.dress",
        ];

        for key in expected_keys {
            let item = items
                .iter()
                .find(|elem| elem.attribute("data-command-id") == Some(key))
                .unwrap_or_else(|| panic!("command item with key {key} must exist"));

            assert_eq!(
                item.attribute("data-enabled"),
                Some("true"),
                "command {key} should be enabled in ready context"
            );
            assert!(item.classes.contains(&"available".to_string()));
        }

        // ToggleSimulation has shortcut "Space"
        let toggle_item = items
            .iter()
            .find(|elem| elem.attribute("data-command-id") == Some("simulation.toggle"))
            .unwrap();
        let mut shortcuts = Vec::new();
        find_elements_by_class_in_element(toggle_item, "command-shortcut", &mut shortcuts);
        assert_eq!(shortcuts.len(), 1);
        assert_eq!(element_text(shortcuts[0]), "Space");

        // When document_loaded is false, simulation and file commands are disabled
        let mut unavail_ctx = ready_ctx();
        unavail_ctx.document_loaded = false;
        let unavail_state = CommandPaletteState::new(unavail_ctx);
        let unavail_state = CommandPaletteState { open: true, ..unavail_state };
        let unavail_node = render_command_palette(&unavail_state).unwrap();
        let mut unavail_items = Vec::new();
        find_elements_by_class(&unavail_node, "command-palette-item", &mut unavail_items);

        for key in ["simulation.toggle", "simulation.step", "simulation.reset", "file.export"] {
            let item = unavail_items
                .iter()
                .find(|elem| elem.attribute("data-command-id") == Some(key))
                .unwrap();
            assert_eq!(item.attribute("data-enabled"), Some("false"));
            assert_eq!(
                item.attribute("data-disabled-reason"),
                Some("No garment document is loaded")
            );
        }

        // When worker_ready is false, simulation commands are disabled
        let mut no_worker_ctx = ready_ctx();
        no_worker_ctx.worker_ready = false;
        let no_worker_state = CommandPaletteState { open: true, ..CommandPaletteState::new(no_worker_ctx) };
        let no_worker_node = render_command_palette(&no_worker_state).unwrap();
        let mut no_worker_items = Vec::new();
        find_elements_by_class(&no_worker_node, "command-palette-item", &mut no_worker_items);

        let toggle_no_worker = no_worker_items
            .iter()
            .find(|elem| elem.attribute("data-command-id") == Some("simulation.toggle"))
            .unwrap();
        assert_eq!(
            toggle_no_worker.attribute("data-disabled-reason"),
            Some("The simulation worker is unavailable")
        );

        // When playing is true, StepSimulation is disabled
        let mut playing_ctx = ready_ctx();
        playing_ctx.playing = true;
        let playing_state = CommandPaletteState { open: true, ..CommandPaletteState::new(playing_ctx) };
        let playing_node = render_command_palette(&playing_state).unwrap();
        let mut playing_items = Vec::new();
        find_elements_by_class(&playing_node, "command-palette-item", &mut playing_items);

        let step_playing = playing_items
            .iter()
            .find(|elem| elem.attribute("data-command-id") == Some("simulation.step"))
            .unwrap();
        assert_eq!(
            step_playing.attribute("data-disabled-reason"),
            Some("Pause simulation before stepping")
        );

        // When pending is true, StepSimulation is disabled
        let mut busy_ctx = ready_ctx();
        busy_ctx.pending = true;
        let busy_state = CommandPaletteState { open: true, ..CommandPaletteState::new(busy_ctx) };
        let busy_node = render_command_palette(&busy_state).unwrap();
        let mut busy_items = Vec::new();
        find_elements_by_class(&busy_node, "command-palette-item", &mut busy_items);

        let step_busy = busy_items
            .iter()
            .find(|elem| elem.attribute("data-command-id") == Some("simulation.step"))
            .unwrap();
        assert_eq!(
            step_busy.attribute("data-disabled-reason"),
            Some("The previous simulation step is still pending")
        );
    }

    #[test]
    fn pending_trim_draft_disables_reset_and_destructive_commands_with_explicit_explanation() {
        let mut state = CommandPaletteState::new(ready_ctx());
        state.open = true;
        state.has_pending_trim_draft = true;

        let node = render_command_palette(&state).unwrap();
        let mut items = Vec::new();
        find_elements_by_class(&node, "command-palette-item", &mut items);

        // simulation.reset is disabled with explicit FC-0008 reason
        let reset_item = items
            .iter()
            .find(|elem| elem.attribute("data-command-id") == Some("simulation.reset"))
            .unwrap();
        assert_eq!(reset_item.attribute("data-enabled"), Some("false"));
        assert_eq!(
            reset_item.attribute("data-disabled-reason"),
            Some("Apply or cancel the trim draft before resetting the simulation")
        );

        // example.tshirt and example.dress are disabled with explicit explanation
        let tshirt_item = items
            .iter()
            .find(|elem| elem.attribute("data-command-id") == Some("example.tshirt"))
            .unwrap();
        assert_eq!(tshirt_item.attribute("data-enabled"), Some("false"));
        assert_eq!(
            tshirt_item.attribute("data-disabled-reason"),
            Some("Apply or cancel the trim draft before loading an example")
        );

        let dress_item = items
            .iter()
            .find(|elem| elem.attribute("data-command-id") == Some("example.dress"))
            .unwrap();
        assert_eq!(dress_item.attribute("data-enabled"), Some("false"));
        assert_eq!(
            dress_item.attribute("data-disabled-reason"),
            Some("Apply or cancel the trim draft before loading an example")
        );

        // Non-destructive commands like ToggleSimulation and SplitView remain enabled
        let toggle_item = items
            .iter()
            .find(|elem| elem.attribute("data-command-id") == Some("simulation.toggle"))
            .unwrap();
        assert_eq!(toggle_item.attribute("data-enabled"), Some("true"));

        let split_item = items
            .iter()
            .find(|elem| elem.attribute("data-command-id") == Some("workspace.view_split"))
            .unwrap();
        assert_eq!(split_item.attribute("data-enabled"), Some("true"));

        // Measurement draft guard
        state.has_pending_trim_draft = false;
        state.has_pending_measurement_draft = true;
        let meas_node = render_command_palette(&state).unwrap();
        let mut meas_items = Vec::new();
        find_elements_by_class(&meas_node, "command-palette-item", &mut meas_items);

        let reset_meas = meas_items
            .iter()
            .find(|elem| elem.attribute("data-command-id") == Some("simulation.reset"))
            .unwrap();
        assert_eq!(reset_meas.attribute("data-enabled"), Some("false"));
        assert_eq!(
            reset_meas.attribute("data-disabled-reason"),
            Some("Save or discard the measurement draft before resetting the simulation")
        );
    }

    #[test]
    fn subsequence_and_fuzzy_search_filters_matching_commands() {
        let mut state = CommandPaletteState::new(ready_ctx());
        state.open = true;

        // "reset" or "drape" or "resetdrape" matches ResetSimulation
        state.query = "resetdrape".into();
        let items = state.filtered_items();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, CommandId::ResetSimulation);

        state.query = "drape".into();
        let items_drape = state.filtered_items();
        assert!(items_drape.iter().any(|i| i.id == CommandId::ResetSimulation));

        // "tee" matches LoadTshirt
        state.query = "tee".into();
        let items_tee = state.filtered_items();
        assert_eq!(items_tee.len(), 1);
        assert_eq!(items_tee[0].id, CommandId::LoadTshirt);

        // Subsequence filtering
        state.query = "spc".into();
        let items_spc = state.filtered_items();
        assert!(items_spc.iter().any(|i| i.id == CommandId::ToggleSimulation));

        // Substring
        state.query = "split".into();
        let items_split = state.filtered_items();
        assert_eq!(items_split.len(), 1);
        assert_eq!(items_split[0].id, CommandId::SplitView);
    }

    #[test]
    fn keyboard_navigation_and_dispatch_actions() {
        let mut state = CommandPaletteState::new(ready_ctx());
        state.open = true;

        assert_eq!(state.selected_index, 0);

        // Down advances selection
        state.handle_key(PaletteKey::Down);
        assert_eq!(state.selected_index, 1);

        state.handle_key(PaletteKey::Down);
        assert_eq!(state.selected_index, 2);

        // Up retreats selection
        state.handle_key(PaletteKey::Up);
        assert_eq!(state.selected_index, 1);

        // Up at index 0 wraps to end
        state.selected_index = 0;
        state.handle_key(PaletteKey::Up);
        assert_eq!(state.selected_index, 11);

        // Enter on available command dispatches and closes palette
        state.selected_index = 0; // ToggleSimulation
        let executed = state.handle_key(PaletteKey::Enter);
        assert_eq!(executed, Some(CommandId::ToggleSimulation));
        assert!(!state.open);

        // Enter on disabled command does not dispatch
        state.open = true;
        state.has_pending_trim_draft = true;
        state.selected_index = 2; // ResetSimulation
        let blocked = state.handle_key(PaletteKey::Enter);
        assert_eq!(blocked, None);
        assert!(state.open, "palette should remain open when blocked");

        // Escape closes palette
        state.handle_key(PaletteKey::Escape);
        assert!(!state.open);
    }
}


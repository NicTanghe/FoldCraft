//! Modular cssimpler UI components for the top menu bar, simulation transport,
//! 3D/2D tool rails, and radial context menu.

use cssimpler::core::Node;
use cssimpler::ui;
pub use marvelous_cloth::authoring::CornerRule;
use marvelous_cloth::commands::CommandId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackendBadge {
    Cpu,
    Gpu,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TopBarAction {
    Command(CommandId),
    ToggleCleanView,
    FileNew,
    FileOpen,
    FileSave,
    FileExport,
    EditUndo,
    EditRedo,
    ViewSplit,
    View3D,
    View2D,
    WorkspaceInspector,
    WorkspaceResetLayout,
    WorkspacePreferences,
    SimulationPlay,
    SimulationPause,
    SimulationStep,
    SimulationReset,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TopBarState {
    pub document_loaded: bool,
    pub playing: bool,
    pub worker_pending: bool,
    pub worker_ready: bool,
    pub has_pending_trim_draft: bool,
    pub has_pending_measurement_draft: bool,
    pub clean_view: bool,
    pub backend: BackendBadge,
}

impl Default for TopBarState {
    fn default() -> Self {
        Self {
            document_loaded: true,
            playing: false,
            worker_pending: false,
            worker_ready: true,
            has_pending_trim_draft: false,
            has_pending_measurement_draft: false,
            clean_view: false,
            backend: BackendBadge::Cpu,
        }
    }
}

impl TopBarState {
    pub fn dispatch(&mut self, action: TopBarAction) -> Option<CommandId> {
        match action {
            TopBarAction::Command(cmd) => Some(cmd),
            TopBarAction::ToggleCleanView => {
                self.clean_view = !self.clean_view;
                None
            }
            TopBarAction::FileExport => Some(CommandId::ExportSession),
            TopBarAction::ViewSplit => Some(CommandId::SplitView),
            TopBarAction::View3D => Some(CommandId::GarmentView),
            TopBarAction::View2D => Some(CommandId::PatternView),
            TopBarAction::WorkspaceInspector => Some(CommandId::ToggleInspector),
            TopBarAction::WorkspaceResetLayout => Some(CommandId::ResetLayout),
            TopBarAction::WorkspacePreferences => Some(CommandId::OpenPreferences),
            TopBarAction::SimulationPlay => {
                if !self.playing && play_simulation_disabled_reason(self).is_none() {
                    self.playing = true;
                    Some(CommandId::ToggleSimulation)
                } else {
                    None
                }
            }
            TopBarAction::SimulationPause => {
                if self.playing && play_simulation_disabled_reason(self).is_none() {
                    self.playing = false;
                    Some(CommandId::ToggleSimulation)
                } else {
                    None
                }
            }
            TopBarAction::SimulationStep => {
                if step_simulation_disabled_reason(self).is_none() {
                    Some(CommandId::StepSimulation)
                } else {
                    None
                }
            }
            TopBarAction::SimulationReset => {
                if reset_simulation_disabled_reason(self).is_none() {
                    self.playing = false;
                    Some(CommandId::ResetSimulation)
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}

pub fn reset_simulation_disabled_reason(state: &TopBarState) -> Option<&'static str> {
    if state.has_pending_trim_draft {
        Some("Apply or cancel the trim draft before resetting the simulation")
    } else if state.has_pending_measurement_draft {
        Some("Save or discard the measurement draft before resetting the simulation")
    } else if !state.document_loaded {
        Some("No garment document is loaded")
    } else {
        None
    }
}

pub fn play_simulation_disabled_reason(state: &TopBarState) -> Option<&'static str> {
    if !state.document_loaded {
        Some("No garment document is loaded")
    } else if !state.worker_ready {
        Some("The simulation worker is unavailable")
    } else {
        None
    }
}

pub fn step_simulation_disabled_reason(state: &TopBarState) -> Option<&'static str> {
    if !state.document_loaded {
        Some("No garment document is loaded")
    } else if !state.worker_ready {
        Some("The simulation worker is unavailable")
    } else if state.playing {
        Some("Pause simulation before stepping")
    } else if state.worker_pending {
        Some("The previous simulation step is still pending")
    } else {
        None
    }
}

pub fn render_top_bar(state: &TopBarState) -> Node {
    let play_reason = play_simulation_disabled_reason(state);
    let step_reason = step_simulation_disabled_reason(state);
    let reset_reason = reset_simulation_disabled_reason(state);

    let play_class = format!(
        "transport-btn play {}",
        if play_reason.is_none() {
            "enabled"
        } else {
            "disabled"
        }
    );
    let step_class = format!(
        "transport-btn step {}",
        if step_reason.is_none() {
            "enabled"
        } else {
            "disabled"
        }
    );
    let reset_class = format!(
        "transport-btn reset {}",
        if reset_reason.is_none() {
            "enabled"
        } else {
            "disabled"
        }
    );

    let clean_view_class = format!(
        "clean-view-toggle {}",
        if state.clean_view { "active" } else { "" }
    );

    let (badge_text, badge_class) = match state.backend {
        BackendBadge::Cpu => ("CPU", "backend-badge cpu"),
        BackendBadge::Gpu => ("GPU", "backend-badge gpu"),
    };

    let play_label = if state.playing { "Pause" } else { "Play" };
    let play_title = play_reason.unwrap_or(if state.playing {
        "Pause simulation (Space)"
    } else {
        "Start simulation (Space)"
    });
    let step_title = step_reason.unwrap_or("Advance one simulation step");
    let reset_title = reset_reason.unwrap_or("Reset flat arrangement");

    let play_enabled_str = if play_reason.is_none() {
        "true"
    } else {
        "false"
    };
    let step_enabled_str = if step_reason.is_none() {
        "true"
    } else {
        "false"
    };
    let reset_enabled_str = if reset_reason.is_none() {
        "true"
    } else {
        "false"
    };

    ui! {
        <header id="top-bar" class="top-bar">
            <nav class="top-bar-menus">
                <div class="menu-item" data-menu="file">File</div>
                <div class="menu-item" data-menu="edit">Edit</div>
                <div class="menu-item" data-menu="view">View</div>
                <div class="menu-item" data-menu="workspace">Workspace</div>
                <div class="menu-item" data-menu="simulation">Simulation</div>
            </nav>
            <div class="transport-controls">
                <button
                    id="transport-play"
                    class={play_class}
                    data-action="simulation.toggle"
                    data-enabled={play_enabled_str}
                    data-disabled-reason={play_reason.unwrap_or("")}
                    title={play_title}
                >
                    {play_label}
                </button>
                <button
                    id="transport-step"
                    class={step_class}
                    data-action="simulation.step"
                    data-enabled={step_enabled_str}
                    data-disabled-reason={step_reason.unwrap_or("")}
                    title={step_title}
                >
                    Step
                </button>
                <button
                    id="transport-reset"
                    class={reset_class}
                    data-action="simulation.reset"
                    data-enabled={reset_enabled_str}
                    data-disabled-reason={reset_reason.unwrap_or("")}
                    title={reset_title}
                >
                    Reset
                </button>
            </div>
            <div class="top-bar-trailing">
                <button id="clean-view-toggle" class={clean_view_class} data-action="workspace.clean_view">
                    Clean View
                </button>
                <span id="backend-badge" class={badge_class}>
                    {badge_text}
                </span>
            </div>
        </header>
    }
}

// ---------------------------------------------------------------------------
// 3D Tool Rail
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PinMode {
    Contour,
    Interior,
}

impl PinMode {
    pub fn toggle(self) -> Self {
        match self {
            Self::Contour => Self::Interior,
            Self::Interior => Self::Contour,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Contour => "Contour",
            Self::Interior => "Interior",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Tool3d {
    Gizmo,
    Grab { radius: f32, strength: f32 },
    Sew,
    Pin { mode: PinMode },
    Measure,
    AvatarDiagnostics,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Rail3dState {
    pub active_tool: Tool3d,
    pub grab_radius: f32,
    pub grab_strength: f32,
    pub pin_mode: PinMode,
}

impl Default for Rail3dState {
    fn default() -> Self {
        Self {
            active_tool: Tool3d::Gizmo,
            grab_radius: 0.05,
            grab_strength: 1.0,
            pin_mode: PinMode::Contour,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Rail3dAction {
    SelectTool(Tool3d),
    SetGrabRadius(f32),
    SetGrabStrength(f32),
    TogglePinMode,
    TriggerAvatarDiagnostics,
}

impl Rail3dState {
    pub fn dispatch(&mut self, action: Rail3dAction) {
        match action {
            Rail3dAction::SelectTool(tool) => {
                self.active_tool = tool;
            }
            Rail3dAction::SetGrabRadius(r) => {
                self.grab_radius = r;
                if let Tool3d::Grab { ref mut radius, .. } = self.active_tool {
                    *radius = r;
                }
            }
            Rail3dAction::SetGrabStrength(s) => {
                self.grab_strength = s;
                if let Tool3d::Grab {
                    ref mut strength, ..
                } = self.active_tool
                {
                    *strength = s;
                }
            }
            Rail3dAction::TogglePinMode => {
                self.pin_mode = self.pin_mode.toggle();
                if let Tool3d::Pin { ref mut mode } = self.active_tool {
                    *mode = self.pin_mode;
                }
            }
            Rail3dAction::TriggerAvatarDiagnostics => {
                self.active_tool = Tool3d::AvatarDiagnostics;
            }
        }
    }
}

pub fn render_3d_rail(state: &Rail3dState) -> Node {
    let gizmo_active = matches!(state.active_tool, Tool3d::Gizmo);
    let grab_active = matches!(state.active_tool, Tool3d::Grab { .. });
    let sew_active = matches!(state.active_tool, Tool3d::Sew);
    let pin_active = matches!(state.active_tool, Tool3d::Pin { .. });
    let measure_active = matches!(state.active_tool, Tool3d::Measure);
    let avatar_active = matches!(state.active_tool, Tool3d::AvatarDiagnostics);

    let gizmo_class = format!("tool-btn {}", if gizmo_active { "active" } else { "" });
    let grab_class = format!("tool-btn {}", if grab_active { "active" } else { "" });
    let sew_class = format!("tool-btn {}", if sew_active { "active" } else { "" });
    let pin_class = format!("tool-btn {}", if pin_active { "active" } else { "" });
    let measure_class = format!("tool-btn {}", if measure_active { "active" } else { "" });
    let avatar_class = format!("tool-btn {}", if avatar_active { "active" } else { "" });

    let grab_radius_mm = format!("{:.0}mm", state.grab_radius * 1000.0);
    let grab_strength_str = format!("{:.1}", state.grab_strength);
    let pin_mode_label = state.pin_mode.label();

    ui! {
        <aside id="rail-3d" class="tool-rail tool-rail-3d">
            <button
                class={gizmo_class}
                data-tool="gizmo"
                title="Select & Arrange (Gizmo)"
            >
                Gizmo
            </button>
            <div class="tool-composite grab-composite">
                <button
                    class={grab_class}
                    data-tool="grab"
                    title="Grab Cloth (Pull & Drag)"
                >
                    Grab
                </button>
                <div class="grab-readouts">
                    <span class="readout grab-radius" title="Grab radius">{grab_radius_mm}</span>
                    <span class="readout grab-strength" title="Grab strength">{grab_strength_str}</span>
                </div>
            </div>
            <button
                class={sew_class}
                data-tool="sew"
                title="3D Sew (Connect boundaries)"
            >
                3D Sew
            </button>
            <div class="tool-composite pin-composite">
                <button
                    class={pin_class}
                    data-tool="pin"
                    title="Pin Cloth (Click vertices)"
                >
                    Pin
                </button>
                <span class="pin-mode-badge" title="Pin mode (Contour / Interior)">
                    {pin_mode_label}
                </span>
            </div>
            <button
                class={measure_class}
                data-tool="measure"
                title="3D Measure"
            >
                Measure
            </button>
            <button
                id="avatar-diagnostics-btn"
                class={avatar_class}
                data-tool="avatar-diagnostics"
                title="Avatar Diagnostics"
            >
                Avatar Diagnostics
            </button>
        </aside>
    }
}

// ---------------------------------------------------------------------------
// 2D Pattern Tool Rail
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CornerMode {
    Radius,
    Shape,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CornerToolParams {
    pub mode: CornerMode,
    pub radius_mm: f64,
    pub tension: f64,
    pub rule: CornerRule,
}

impl Default for CornerToolParams {
    fn default() -> Self {
        Self {
            mode: CornerMode::Radius,
            radius_mm: 10.0,
            tension: 0.55,
            rule: CornerRule::Smooth,
        }
    }
}

impl CornerToolParams {
    pub fn to_command(&self, panel: &str, edge: &str) -> marvelous_cloth::authoring::Command {
        marvelous_cloth::authoring::Command::RoundCornerShape {
            panel: panel.to_string(),
            edge: edge.to_string(),
            radius_m: marvelous_cloth::authoring::mm_to_m(self.radius_mm),
            tension: match self.mode {
                CornerMode::Radius => 0.55,
                CornerMode::Shape => self.tension,
            },
            rule: self.rule,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Tool2d {
    Edit,
    Rect,
    Pen,
    Freehand,
    Sew,
    SplitPoint,
    InternalLine,
    Measure,
    Trim,
    Corner(CornerToolParams),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Rail2dState {
    pub active_tool: Tool2d,
    pub corner_params: CornerToolParams,
}

impl Default for Rail2dState {
    fn default() -> Self {
        Self {
            active_tool: Tool2d::Edit,
            corner_params: CornerToolParams::default(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Rail2dAction {
    SelectTool(Tool2d),
    SetCornerMode(CornerMode),
    SetCornerRadius(f64),
    SetCornerTension(f64),
    SetCornerRule(CornerRule),
}

impl Rail2dState {
    pub fn dispatch(&mut self, action: Rail2dAction) {
        match action {
            Rail2dAction::SelectTool(tool) => {
                self.active_tool = tool;
                if let Tool2d::Corner(ref params) = self.active_tool {
                    self.corner_params = params.clone();
                }
            }
            Rail2dAction::SetCornerMode(mode) => {
                self.corner_params.mode = mode;
                self.active_tool = Tool2d::Corner(self.corner_params.clone());
            }
            Rail2dAction::SetCornerRadius(r) => {
                self.corner_params.radius_mm = r;
                self.active_tool = Tool2d::Corner(self.corner_params.clone());
            }
            Rail2dAction::SetCornerTension(t) => {
                self.corner_params.tension = t;
                self.active_tool = Tool2d::Corner(self.corner_params.clone());
            }
            Rail2dAction::SetCornerRule(rule) => {
                self.corner_params.rule = rule;
                self.active_tool = Tool2d::Corner(self.corner_params.clone());
            }
        }
    }
}

pub fn render_2d_rail(state: &Rail2dState) -> Node {
    let edit_active = matches!(state.active_tool, Tool2d::Edit);
    let rect_active = matches!(state.active_tool, Tool2d::Rect);
    let pen_active = matches!(state.active_tool, Tool2d::Pen);
    let freehand_active = matches!(state.active_tool, Tool2d::Freehand);
    let sew_active = matches!(state.active_tool, Tool2d::Sew);
    let split_active = matches!(state.active_tool, Tool2d::SplitPoint);
    let internal_active = matches!(state.active_tool, Tool2d::InternalLine);
    let measure_active = matches!(state.active_tool, Tool2d::Measure);
    let trim_active = matches!(state.active_tool, Tool2d::Trim);
    let corner_active = matches!(state.active_tool, Tool2d::Corner(_));

    let edit_class = format!("tool-btn {}", if edit_active { "active" } else { "" });
    let rect_class = format!("tool-btn {}", if rect_active { "active" } else { "" });
    let pen_class = format!("tool-btn {}", if pen_active { "active" } else { "" });
    let freehand_class = format!("tool-btn {}", if freehand_active { "active" } else { "" });
    let sew_class = format!("tool-btn {}", if sew_active { "active" } else { "" });
    let split_class = format!("tool-btn {}", if split_active { "active" } else { "" });
    let internal_class = format!("tool-btn {}", if internal_active { "active" } else { "" });
    let measure_class = format!("tool-btn {}", if measure_active { "active" } else { "" });
    let trim_class = format!("tool-btn {}", if trim_active { "active" } else { "" });
    let corner_class = format!("tool-btn {}", if corner_active { "active" } else { "" });

    let corner_radius_mode_active = state.corner_params.mode == CornerMode::Radius;
    let corner_shape_mode_active = state.corner_params.mode == CornerMode::Shape;

    let radius_mode_class = format!(
        "corner-mode-btn {}",
        if corner_radius_mode_active {
            "active"
        } else {
            ""
        }
    );
    let shape_mode_class = format!(
        "corner-mode-btn {}",
        if corner_shape_mode_active {
            "active"
        } else {
            ""
        }
    );

    let radius_str = format!("{:.1} mm", state.corner_params.radius_mm);
    let tension_str = format!("{:.2}", state.corner_params.tension);

    ui! {
        <aside id="rail-2d" class="tool-rail tool-rail-2d">
            <button class={edit_class} data-tool="edit" title="Edit Point / Curve">
                Edit
            </button>
            <button class={rect_class} data-tool="rect" title="Draw Rectangle">
                Rect
            </button>
            <button class={pen_class} data-tool="pen" title="Draw Polygon / Pen">
                Pen
            </button>
            <button class={freehand_class} data-tool="freehand" title="Freehand Pattern">
                Freehand
            </button>
            <button class={sew_class} data-tool="sew" title="Sew Seams">
                Sew
            </button>
            <button class={split_class} data-tool="split-point" title="Split / Add Point">
                Split
            </button>
            <button class={internal_class} data-tool="internal-line" title="Internal Line / Cutout">
                Internal
            </button>
            <button class={measure_class} data-tool="measure" title="2D Measure">
                Measure
            </button>
            <button class={trim_class} data-tool="trim" title="Trim">
                Trim
            </button>
            <div class="tool-composite corner-composite">
                <button class={corner_class} data-tool="corner" title="Corner Tool (Rounding & Shaping)">
                    Corner
                </button>
                <div class="corner-subcontrols">
                    <button class={radius_mode_class} data-mode="radius" title="Fillet Radius mode">
                        Radius
                    </button>
                    <button class={shape_mode_class} data-mode="shape" title="Bézier Shape mode">
                        Shape
                    </button>
                    <span class="readout corner-radius" title="Numeric fillet radius in mm">
                        {radius_str}
                    </span>
                    <span class="readout corner-tension" title="Numeric handle tension / curvature">
                        {tension_str}
                    </span>
                </div>
            </div>
        </aside>
    }
}

// ---------------------------------------------------------------------------
// Radial Context Menu
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadialSector {
    Shape = 0,
    Mirror = 1,
    Boolean = 2,
    Sew = 3,
    Reset = 4,
    Edit = 5,
    Pen = 6,
    Grab = 7,
}

impl RadialSector {
    pub const ALL: [RadialSector; 8] = [
        RadialSector::Shape,
        RadialSector::Mirror,
        RadialSector::Boolean,
        RadialSector::Sew,
        RadialSector::Reset,
        RadialSector::Edit,
        RadialSector::Pen,
        RadialSector::Grab,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Shape => "Shape",
            Self::Mirror => "Mirror",
            Self::Boolean => "Boolean",
            Self::Sew => "Sew",
            Self::Reset => "Reset",
            Self::Edit => "Edit",
            Self::Pen => "Pen",
            Self::Grab => "Grab",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::Shape => "radial.shape",
            Self::Mirror => "radial.mirror",
            Self::Boolean => "radial.boolean",
            Self::Sew => "radial.sew",
            Self::Reset => "radial.reset",
            Self::Edit => "radial.edit",
            Self::Pen => "radial.pen",
            Self::Grab => "radial.grab",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RadialMenuState {
    pub open: bool,
    pub position: (f32, f32),
    pub hovered_sector: Option<usize>,
}

impl Default for RadialMenuState {
    fn default() -> Self {
        Self {
            open: false,
            position: (0.0, 0.0),
            hovered_sector: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum RadialMenuAction {
    Open { x: f32, y: f32 },
    Close,
    HoverSector(Option<usize>),
    SelectSector(RadialSector),
}

impl RadialMenuState {
    pub fn dispatch(&mut self, action: RadialMenuAction) -> Option<RadialSector> {
        match action {
            RadialMenuAction::Open { x, y } => {
                self.open = true;
                self.position = (x, y);
                self.hovered_sector = None;
                None
            }
            RadialMenuAction::Close => {
                self.open = false;
                self.hovered_sector = None;
                None
            }
            RadialMenuAction::HoverSector(sector) => {
                self.hovered_sector = sector;
                None
            }
            RadialMenuAction::SelectSector(sector) => {
                self.open = false;
                Some(sector)
            }
        }
    }
}

pub fn render_radial_menu(state: &RadialMenuState) -> Option<Node> {
    if !state.open {
        return None;
    }

    let pos_x = format!("{:.1}px", state.position.0);
    let pos_y = format!("{:.1}px", state.position.1);
    let pos_style = format!("left: {}; top: {};", pos_x, pos_y);
    let x_data = format!("{:.1}", state.position.0);
    let y_data = format!("{:.1}", state.position.1);

    let sectors: Vec<Node> = RadialSector::ALL
        .iter()
        .copied()
        .map(|sec| {
            let idx = sec as usize;
            let is_hovered = state.hovered_sector == Some(idx);
            let class_str = format!(
                "radial-sector sector-{} {}",
                idx,
                if is_hovered { "active" } else { "" }
            );
            let sec_str = format!("{idx}");
            let key = sec.key();
            let label = sec.label();

            ui! {
                <div
                    class={class_str}
                    data-sector={sec_str}
                    data-action={key}
                >
                    <span class="sector-label">{label}</span>
                </div>
            }
        })
        .collect();

    Some(ui! {
        <div
            id="radial-menu"
            class="radial-menu open"
            style={pos_style}
            data-x={x_data}
            data-y={y_data}
        >
            <div class="radial-center">
                <span class="radial-center-dot"></span>
            </div>
            <div class="radial-sectors">
                {sectors}
            </div>
        </div>
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use cssimpler::core::ElementNode;

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

    fn text_content(node: &Node) -> String {
        match node {
            Node::Text(t) => t.clone(),
            Node::Element(e) => element_text(e),
        }
    }

    fn element_text(elem: &ElementNode) -> String {
        elem.children
            .iter()
            .map(text_content)
            .collect::<Vec<_>>()
            .join("")
    }

    #[test]
    fn top_bar_renders_transport_controls_clean_view_and_backend_badge() {
        let mut state = TopBarState::default();
        let node = render_top_bar(&state);

        let play_btn = find_element_by_id(&node, "transport-play").expect("play button exists");
        assert_eq!(play_btn.attribute("data-enabled"), Some("true"));
        assert!(element_text(play_btn).contains("Play"));

        let step_btn = find_element_by_id(&node, "transport-step").expect("step button exists");
        assert_eq!(step_btn.attribute("data-enabled"), Some("true"));

        let reset_btn = find_element_by_id(&node, "transport-reset").expect("reset button exists");
        assert_eq!(reset_btn.attribute("data-enabled"), Some("true"));

        let clean_view_btn =
            find_element_by_id(&node, "clean-view-toggle").expect("clean view toggle exists");
        assert!(!clean_view_btn.classes.contains(&"active".to_string()));

        let badge = find_element_by_id(&node, "backend-badge").expect("backend badge exists");
        assert_eq!(badge.attribute("class"), Some("backend-badge cpu"));
        assert!(element_text(badge).contains("CPU"));

        // Toggle Clean View
        state.dispatch(TopBarAction::ToggleCleanView);
        assert!(state.clean_view);
        let updated_node = render_top_bar(&state);
        let updated_clean_view = find_element_by_id(&updated_node, "clean-view-toggle").unwrap();
        assert!(updated_clean_view.classes.contains(&"active".to_string()));

        // GPU backend
        state.backend = BackendBadge::Gpu;
        let gpu_node = render_top_bar(&state);
        let gpu_badge = find_element_by_id(&gpu_node, "backend-badge").unwrap();
        assert_eq!(gpu_badge.attribute("class"), Some("backend-badge gpu"));
        assert!(element_text(gpu_badge).contains("GPU"));
    }

    #[test]
    fn top_bar_pending_trim_draft_disables_reset_with_explicit_explanation() {
        let mut state = TopBarState {
            has_pending_trim_draft: true,
            ..Default::default()
        };

        let node = render_top_bar(&state);
        let reset_btn = find_element_by_id(&node, "transport-reset").expect("reset button exists");

        assert_eq!(reset_btn.attribute("data-enabled"), Some("false"));
        assert!(reset_btn.classes.contains(&"disabled".to_string()));
        assert_eq!(
            reset_btn.attribute("data-disabled-reason"),
            Some("Apply or cancel the trim draft before resetting the simulation")
        );
        assert_eq!(
            reset_btn.attribute("title"),
            Some("Apply or cancel the trim draft before resetting the simulation")
        );

        // Dispatching Reset returns None when blocked by pending trim draft
        assert_eq!(state.dispatch(TopBarAction::SimulationReset), None);

        // Measurement draft guard
        state.has_pending_trim_draft = false;
        state.has_pending_measurement_draft = true;
        let node_meas = render_top_bar(&state);
        let reset_meas = find_element_by_id(&node_meas, "transport-reset").unwrap();
        assert_eq!(reset_meas.attribute("data-enabled"), Some("false"));
        assert_eq!(
            reset_meas.attribute("data-disabled-reason"),
            Some("Save or discard the measurement draft before resetting the simulation")
        );
        assert_eq!(state.dispatch(TopBarAction::SimulationReset), None);
    }

    #[test]
    fn rail_3d_renders_buttons_and_dispatches_tool_selection() {
        let mut state = Rail3dState::default();
        let node = render_3d_rail(&state);

        let mut tool_btns = Vec::new();
        find_elements_by_attr(&node, "data-tool", &mut tool_btns);
        assert_eq!(tool_btns.len(), 6);

        // Gizmo is active by default
        let gizmo = tool_btns
            .iter()
            .find(|b| b.attribute("data-tool") == Some("gizmo"))
            .unwrap();
        assert!(gizmo.classes.contains(&"active".to_string()));

        // Grab tool shows readouts
        let mut readouts = Vec::new();
        find_elements_by_class(&node, "readout", &mut readouts);
        assert_eq!(readouts.len(), 2);
        assert!(element_text(readouts[0]).contains("50mm"));
        assert!(element_text(readouts[1]).contains("1.0"));

        // Switch to Grab with custom params
        state.dispatch(Rail3dAction::SelectTool(Tool3d::Grab {
            radius: 0.08,
            strength: 2.5,
        }));
        state.dispatch(Rail3dAction::SetGrabRadius(0.08));
        state.dispatch(Rail3dAction::SetGrabStrength(2.5));
        let node_grab = render_3d_rail(&state);
        let mut readouts_grab = Vec::new();
        find_elements_by_class(&node_grab, "readout", &mut readouts_grab);
        assert!(element_text(readouts_grab[0]).contains("80mm"));
        assert!(element_text(readouts_grab[1]).contains("2.5"));

        // Pin mode toggling
        assert_eq!(state.pin_mode, PinMode::Contour);
        state.dispatch(Rail3dAction::TogglePinMode);
        assert_eq!(state.pin_mode, PinMode::Interior);
        let node_pin = render_3d_rail(&state);
        let mut pin_badges = Vec::new();
        find_elements_by_class(&node_pin, "pin-mode-badge", &mut pin_badges);
        assert_eq!(pin_badges.len(), 1);
        assert!(element_text(pin_badges[0]).contains("Interior"));

        // Avatar Diagnostics trigger
        state.dispatch(Rail3dAction::TriggerAvatarDiagnostics);
        assert_eq!(state.active_tool, Tool3d::AvatarDiagnostics);
        let node_diag = render_3d_rail(&state);
        let diag_btn = find_element_by_id(&node_diag, "avatar-diagnostics-btn").unwrap();
        assert!(diag_btn.classes.contains(&"active".to_string()));
    }

    #[test]
    fn rail_2d_renders_buttons_and_corner_tool_radius_shape_modes() {
        let mut state = Rail2dState::default();
        let node = render_2d_rail(&state);

        let mut tool_btns = Vec::new();
        find_elements_by_attr(&node, "data-tool", &mut tool_btns);
        assert_eq!(tool_btns.len(), 10);

        // Edit active by default
        let edit_btn = tool_btns
            .iter()
            .find(|b| b.attribute("data-tool") == Some("edit"))
            .unwrap();
        assert!(edit_btn.classes.contains(&"active".to_string()));

        // Corner Tool subcontrols
        let mut mode_btns = Vec::new();
        find_elements_by_attr(&node, "data-mode", &mut mode_btns);
        assert_eq!(mode_btns.len(), 2);
        let radius_btn = mode_btns
            .iter()
            .find(|b| b.attribute("data-mode") == Some("radius"))
            .unwrap();
        let shape_btn = mode_btns
            .iter()
            .find(|b| b.attribute("data-mode") == Some("shape"))
            .unwrap();
        assert!(radius_btn.classes.contains(&"active".to_string()));
        assert!(!shape_btn.classes.contains(&"active".to_string()));

        // Numeric readouts
        let mut readouts = Vec::new();
        find_elements_by_class(&node, "readout", &mut readouts);
        assert!(element_text(readouts[0]).contains("10.0 mm"));
        assert!(element_text(readouts[1]).contains("0.55"));

        // Switch to Corner Tool in Shape mode with custom radius/tension
        state.dispatch(Rail2dAction::SelectTool(Tool2d::Corner(CornerToolParams {
            mode: CornerMode::Shape,
            radius_mm: 15.0,
            tension: 0.8,
            rule: CornerRule::Smooth,
        })));
        assert_eq!(state.corner_params.mode, CornerMode::Shape);
        assert_eq!(state.corner_params.radius_mm, 15.0);
        assert_eq!(state.corner_params.tension, 0.8);
        assert_eq!(state.corner_params.rule, CornerRule::Smooth);

        let node_corner = render_2d_rail(&state);
        let mut mode_btns_shape = Vec::new();
        find_elements_by_attr(&node_corner, "data-mode", &mut mode_btns_shape);
        let shape_btn_active = mode_btns_shape
            .iter()
            .find(|b| b.attribute("data-mode") == Some("shape"))
            .unwrap();
        let radius_btn_inactive = mode_btns_shape
            .iter()
            .find(|b| b.attribute("data-mode") == Some("radius"))
            .unwrap();
        assert!(shape_btn_active.classes.contains(&"active".to_string()));
        assert!(!radius_btn_inactive.classes.contains(&"active".to_string()));

        let mut readouts_shape = Vec::new();
        find_elements_by_class(&node_corner, "readout", &mut readouts_shape);
        assert!(element_text(readouts_shape[0]).contains("15.0 mm"));
        assert!(element_text(readouts_shape[1]).contains("0.80"));
    }

    #[test]
    fn radial_menu_renders_at_pointer_coordinates_when_open() {
        let mut state = RadialMenuState::default();
        assert!(render_radial_menu(&state).is_none());

        // Open radial menu at pointer coordinates (175.5, 290.0)
        state.dispatch(RadialMenuAction::Open { x: 175.5, y: 290.0 });
        assert!(state.open);
        assert_eq!(state.position, (175.5, 290.0));

        let node = render_radial_menu(&state).expect("radial menu renders when open");
        let root = find_element_by_id(&node, "radial-menu").expect("root radial element exists");
        assert_eq!(root.attribute("data-x"), Some("175.5"));
        assert_eq!(root.attribute("data-y"), Some("290.0"));
        let style = root.attribute("style").unwrap();
        assert!(style.contains("left: 175.5px"));
        assert!(style.contains("top: 290.0px"));

        // All 8 sectors are rendered
        let mut sectors = Vec::new();
        find_elements_by_attr(&node, "data-sector", &mut sectors);
        assert_eq!(sectors.len(), 8);

        for (idx, sec) in RadialSector::ALL.iter().enumerate() {
            let idx_str = idx.to_string();
            let elem = sectors
                .iter()
                .find(|e| e.attribute("data-sector") == Some(idx_str.as_str()))
                .unwrap();
            assert_eq!(elem.attribute("data-action"), Some(sec.key()));
            assert!(element_text(elem).contains(sec.label()));
        }

        // Hover sector 4 (Reset)
        state.dispatch(RadialMenuAction::HoverSector(Some(4)));
        let node_hover = render_radial_menu(&state).unwrap();
        let mut sectors_hover = Vec::new();
        find_elements_by_attr(&node_hover, "data-sector", &mut sectors_hover);
        let sector_4 = sectors_hover
            .iter()
            .find(|e| e.attribute("data-sector") == Some("4"))
            .unwrap();
        assert!(sector_4.classes.contains(&"active".to_string()));

        // Select sector 4
        let action = state.dispatch(RadialMenuAction::SelectSector(RadialSector::Reset));
        assert_eq!(action, Some(RadialSector::Reset));
        assert!(!state.open);
        assert!(render_radial_menu(&state).is_none());
    }
}

//! CSSimpler Studio V2 App Shell, Dark/Light CSS Tokens & Viewport Compositor.
//!
//! Provides:
//! - 5-row / 4-column Studio V2 behavioral layout using `cssimpler::ui!` and `studio_v2.css`.
//! - Dark & Light CSS tokens (`--bg`, `--surface`, `--surface-raised`, `--border`,
//!   `--text`, `--text-muted`, `--accent`, `--warn`, `--danger`).
//! - Exact logical and physical rectangle extraction for `#viewport-3d`, `#viewport-2d`,
//!   `#object-browser`, `#property-editor`, and chrome rails/dock.
//! - Input routing separating UI chrome from 3D viewport canvas hits and 2D pattern board hits.
//! - Viewport cutout punching (alpha=0) preserving underlying WGPU 3D cloth render and 2D board.
//! - Pure-SVG 2D pattern board outlines fallback / preview in `#viewport-2d`.
//!
//! 100% original MIT-licensed implementation for FoldCraft.

#![allow(dead_code)]

#[cfg(feature = "desktop")]
pub mod browsers;
pub mod compositor;
pub mod palette;
pub mod properties_part1;
pub mod rails;

#[cfg(feature = "desktop")]
pub mod properties_part2;

#[allow(unused_imports)]
pub use compositor::{
    InputTarget, PatternPanelPreview, PhysicalRect, ViewportLayoutMap, composite_ui_over_frame,
    extract_viewport_layout, generate_pattern_board_svg, pattern_board_svg_node,
    punch_viewport_cutouts, route_pointer_event, studio_window_config,
};
#[allow(unused_imports)]
pub use palette::{
    ALL_COMMAND_IDS, CommandPaletteState, PaletteAction, PaletteItem, PaletteKey, command_aliases,
    evaluate_availability, is_subsequence, matches_query, render_command_palette,
};
#[allow(unused_imports)]
pub use rails::{
    BackendBadge, CornerMode, CornerToolParams, PinMode, RadialMenuAction, RadialMenuState,
    RadialSector, Rail2dAction, Rail2dState, Rail3dAction, Rail3dState, Tool2d, Tool3d,
    TopBarAction, TopBarState, play_simulation_disabled_reason, render_2d_rail, render_3d_rail,
    render_radial_menu, render_top_bar, reset_simulation_disabled_reason,
    step_simulation_disabled_reason,
};

use cssimpler::core::{Node, RenderNode};
use cssimpler::renderer::{RendererError, WindowConfig};
use cssimpler::style::{StyleError, Stylesheet, build_render_tree_in_viewport, parse_stylesheet};
use cssimpler::ui;

/// Embedded stylesheet source for Studio V2.
pub const STUDIO_V2_CSS: &str = include_str!("studio_v2.css");

/// Loads and parses the Studio V2 stylesheet.
pub fn load_studio_stylesheet() -> Result<Stylesheet, StyleError> {
    parse_stylesheet(STUDIO_V2_CSS)
}

/// Visual theme mode for Studio V2.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Theme {
    #[default]
    Dark,
    Light,
}

impl Theme {
    pub fn class_name(&self) -> &'static str {
        match self {
            Theme::Dark => "theme-dark",
            Theme::Light => "theme-light",
        }
    }

    pub fn toggled(&self) -> Self {
        match self {
            Theme::Dark => Theme::Light,
            Theme::Light => Theme::Dark,
        }
    }
}

/// Viewport layout mode for Studio V2 dual viewport stage.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ViewLayoutMode {
    #[default]
    Split,
    ThreeDOnly,
    TwoDOnly,
}

impl ViewLayoutMode {
    pub fn stage_class(&self) -> &'static str {
        match self {
            ViewLayoutMode::Split => "stage-split",
            ViewLayoutMode::ThreeDOnly => "stage-3d-only",
            ViewLayoutMode::TwoDOnly => "stage-2d-only",
        }
    }
}

/// Simulation transport and execution diagnostics.
#[derive(Clone, Debug, PartialEq)]
pub struct SimTransportState {
    pub is_running: bool,
    pub step_count: u64,
    pub step_ms: f32,
    pub is_gpu: bool,
    pub fps: f32,
    pub vertex_count: usize,
    pub triangle_count: usize,
}

impl Default for SimTransportState {
    fn default() -> Self {
        Self {
            is_running: false,
            step_count: 0,
            step_ms: 1.4,
            is_gpu: false,
            fps: 60.0,
            vertex_count: 1024,
            triangle_count: 1984,
        }
    }
}

/// Full state for rendering the Studio V2 app shell.
#[derive(Clone, Debug, PartialEq)]
pub struct StudioV2State {
    pub theme: Theme,
    pub layout_mode: ViewLayoutMode,
    pub transport: SimTransportState,
    pub left_drawer_open: bool,
    pub command_palette_open: bool,
    pub active_3d_tool: String,
    pub active_2d_tool: String,
    pub active_object_category: String,
    pub status_message: String,
    pub pointer_coords: (f32, f32),
    pub pattern_previews: Vec<PatternPanelPreview>,
}

impl Default for StudioV2State {
    fn default() -> Self {
        Self {
            theme: Theme::Dark,
            layout_mode: ViewLayoutMode::Split,
            transport: SimTransportState::default(),
            left_drawer_open: true,
            command_palette_open: false,
            active_3d_tool: "select".to_string(),
            active_2d_tool: "select".to_string(),
            active_object_category: "panels".to_string(),
            status_message: "Ready".to_string(),
            pointer_coords: (0.0, 0.0),
            pattern_previews: Vec::new(),
        }
    }
}

/// Render the Studio V2 app shell scene graph using `cssimpler::ui!`.
pub fn render_studio_v2_shell(state: &StudioV2State) -> Node {
    let theme_class = state.theme.class_name();
    let shell_class = format!("studio-shell {theme_class}");

    // Transport buttons state
    let play_label = if state.transport.is_running {
        "Pause"
    } else {
        "Play"
    };
    let backend_label = if state.transport.is_gpu {
        "GPU (VBD)"
    } else {
        "CPU (OGC)"
    };
    let backend_class = if state.transport.is_gpu {
        "sim-indicator gpu"
    } else {
        "sim-indicator cpu"
    };

    // Layout switcher active classes
    let split_class = if state.layout_mode == ViewLayoutMode::Split {
        "layout-btn active"
    } else {
        "layout-btn"
    };
    let threed_class = if state.layout_mode == ViewLayoutMode::ThreeDOnly {
        "layout-btn active"
    } else {
        "layout-btn"
    };
    let twod_class = if state.layout_mode == ViewLayoutMode::TwoDOnly {
        "layout-btn active"
    } else {
        "layout-btn"
    };

    // Theme toggle label
    let theme_btn_label = match state.theme {
        Theme::Dark => "Theme: Dark",
        Theme::Light => "Theme: Light",
    };

    // Drawer collapse class
    let drawer_class = if state.left_drawer_open {
        "left-drawer"
    } else {
        "left-drawer collapsed"
    };

    // Viewport stage class
    let stage_class = format!("stage {}", state.layout_mode.stage_class());

    // Status bar strings
    let status_text = state.status_message.clone();
    let mesh_info = format!(
        "Verts: {} | Tris: {}",
        state.transport.vertex_count, state.transport.triangle_count
    );
    let perf_info = format!(
        "{:.1} fps | {:.2} ms/step",
        state.transport.fps, state.transport.step_ms
    );
    let coords_info = format!(
        "X: {:.1} Y: {:.1}",
        state.pointer_coords.0, state.pointer_coords.1
    );

    // Optional 2D pattern SVG preview node
    let pattern_svg_child: Option<Node> = if !state.pattern_previews.is_empty() {
        Some(pattern_board_svg_node(
            &state.pattern_previews,
            (0.0, 0.0, 600.0, 600.0),
        ))
    } else {
        None
    };

    // Optional command palette modal
    let modal_child: Option<Node> = if state.command_palette_open {
        Some(ui! {
            <div class="modal-scrim">
                <div id="command-palette-modal">
                    <div class="palette-input">
                        "Type a command or search tools (Esc to close)..."
                    </div>
                    <div class="palette-results">
                        <div class="tree-item">"Play / Pause Simulation"</div>
                        <div class="tree-item">"Step Solver (1 Frame)"</div>
                        <div class="tree-item">"Reset Garment to Rest Geometry"</div>
                        <div class="tree-item">"Toggle 2D/3D Split View"</div>
                        <div class="tree-item">"Toggle Dark / Light Theme"</div>
                    </div>
                </div>
            </div>
        })
    } else {
        None
    };

    ui! {
        <div class={shell_class}>
            // 1. Top Menu & Transport Bar
            <div id="top-bar" class="top-bar">
                <div class="top-bar-left">
                    <div class="app-title">"FoldCraft Studio V2"</div>
                    <div class="menu-bar">
                        <div class="menu-item">"File"</div>
                        <div class="menu-item">"Edit"</div>
                        <div class="menu-item">"Workspace"</div>
                        <div class="menu-item">"View"</div>
                        <div class="menu-item">"Simulation"</div>
                    </div>
                    <div class="sim-transport">
                        <div id="transport-play" class="transport-btn">{play_label}</div>
                        <div id="transport-step" class="transport-btn">"Step"</div>
                        <div id="transport-reset" class="transport-btn">"Reset"</div>
                        <div id="sim-backend-indicator" class={backend_class}>{backend_label}</div>
                    </div>
                </div>

                <div class="top-bar-right">
                    <div class="layout-switcher">
                        <div id="layout-split" class={split_class}>"Split 2D/3D"</div>
                        <div id="layout-3d" class={threed_class}>"3D Only"</div>
                        <div id="layout-2d" class={twod_class}>"2D Only"</div>
                    </div>
                    <div id="theme-toggle">{theme_btn_label}</div>
                    <div id="command-palette-trigger">"Ctrl+K"</div>
                </div>
            </div>

            // 2. Main Studio Body
            <div class="studio-body">
                // Left 3D Tool Rail
                <div id="rail-3d" class="rail-3d">
                    <div class="rail-tool-btn active">"Select"</div>
                    <div class="rail-tool-btn">"Move"</div>
                    <div class="rail-tool-btn">"Rotate"</div>
                    <div class="rail-tool-btn">"Pin"</div>
                    <div class="rail-tool-btn">"Measure"</div>
                </div>

                // Left Asset & Piece Browser Drawer
                <div id="left-drawer" class={drawer_class}>
                    <div class="drawer-header">"ASSETS & PIECES"</div>
                    <div class="drawer-content">
                        <div class="tree-item">"Bodice Front"</div>
                        <div class="tree-item">"Bodice Back"</div>
                        <div class="tree-item">"Sleeve Left"</div>
                        <div class="tree-item">"Sleeve Right"</div>
                        <div class="tree-item">"Collar"</div>
                    </div>
                </div>

                // Center Dual Viewport Stage (Transparent backgrounds for hybrid 3D WGPU and 2D Board)
                <div id="viewport-stage" class={stage_class}>
                    <div id="viewport-3d">
                        <div class="viewport-hud">"3D Viewport - VBD Sim"</div>
                    </div>
                    <div id="viewport-2d">
                        <div class="viewport-hud">"2D Pattern Board"</div>
                        {pattern_svg_child}
                    </div>
                </div>

                // Center/Right 2D Tool Rail
                <div id="rail-2d" class="rail-2d">
                    <div class="rail-tool-btn active">"Select"</div>
                    <div class="rail-tool-btn">"Draft"</div>
                    <div class="rail-tool-btn">"Seam"</div>
                    <div class="rail-tool-btn">"Cut"</div>
                    <div class="rail-tool-btn">"Dart"</div>
                    <div class="rail-tool-btn">"Notch"</div>
                    <div class="rail-tool-btn">"Measure"</div>
                </div>

                // Right Dock (Object Browser on top, Property Editor on bottom)
                <div id="right-dock" class="right-dock">
                    <div id="object-browser">
                        <div class="dock-section-title">"OBJECT BROWSER"</div>
                        <div class="browser-categories">
                            <div class="category-tab active">"Panels"</div>
                            <div class="category-tab">"Seams"</div>
                            <div class="category-tab">"Pins"</div>
                            <div class="category-tab">"Trims"</div>
                            <div class="category-tab">"Fabrics"</div>
                        </div>
                        <div class="browser-tree">
                            <div class="tree-item">"Panel #1 (Front)"</div>
                            <div class="tree-item">"Panel #2 (Back)"</div>
                            <div class="tree-item">"Panel #3 (Sleeve L)"</div>
                            <div class="tree-item">"Panel #4 (Sleeve R)"</div>
                        </div>
                    </div>

                    <div id="property-editor">
                        <div class="dock-section-title">"PROPERTY EDITOR"</div>
                        <div class="property-list">
                            <div class="property-row">
                                <div class="property-label">"Width"</div>
                                <div class="property-value">"0.48 m"</div>
                            </div>
                            <div class="property-row">
                                <div class="property-label">"Height"</div>
                                <div class="property-value">"0.62 m"</div>
                            </div>
                            <div class="property-row">
                                <div class="property-label">"Fabric"</div>
                                <div class="property-value">"Cotton Poplin"</div>
                            </div>
                            <div class="property-row">
                                <div class="property-label">"Density"</div>
                                <div class="property-value">"0.18 kg/m²"</div>
                            </div>
                            <div class="property-row">
                                <div class="property-label">"Thickness"</div>
                                <div class="property-value">"0.6 mm"</div>
                            </div>
                        </div>
                    </div>
                </div>
            </div>

            // 3. Bottom Status Bar
            <div id="status-bar" class="status-bar">
                <div class="status-bar-left">
                    <div>{status_text}</div>
                    <div>{mesh_info}</div>
                </div>
                <div class="status-bar-right">
                    <div>{perf_info}</div>
                    <div>{coords_info}</div>
                </div>
            </div>

            // 4. Modal Overlays (if active)
            {modal_child}
        </div>
    }
}

/// Resolves and lays out the Studio V2 UI scene in the specified viewport dimensions.
pub fn layout_studio_shell(
    state: &StudioV2State,
    stylesheet: &Stylesheet,
    viewport_width: usize,
    viewport_height: usize,
) -> RenderNode {
    let root = render_studio_v2_shell(state);
    build_render_tree_in_viewport(&root, stylesheet, viewport_width, viewport_height)
}

pub type StudioV2UpdateFn =
    fn(&mut StudioV2State, cssimpler::renderer::FrameInfo) -> cssimpler::app::Invalidation;
pub type StudioV2ViewFn = fn(&StudioV2State) -> Node;
pub type StudioV2App<'a> = cssimpler::app::App<'a, StudioV2State, StudioV2UpdateFn, StudioV2ViewFn>;

fn studio_v2_default_update(
    _state: &mut StudioV2State,
    _frame: cssimpler::renderer::FrameInfo,
) -> cssimpler::app::Invalidation {
    cssimpler::app::Invalidation::Structure
}

fn studio_v2_default_view(state: &StudioV2State) -> Node {
    render_studio_v2_shell(state)
}

/// Create a fully wired CSSimpler `App` running the Studio V2 native interface.
pub fn create_studio_v2_app<'a>(
    initial_state: StudioV2State,
    stylesheet: &'a Stylesheet,
) -> StudioV2App<'a> {
    cssimpler::app::App::new(
        initial_state,
        stylesheet,
        studio_v2_default_update as StudioV2UpdateFn,
        studio_v2_default_view as StudioV2ViewFn,
    )
    .with_event_handler(|state, event| {
        if let cssimpler::renderer::EngineEvent::Key(k) = event
            && k.state == cssimpler::renderer::ButtonState::Pressed
        {
            if let cssimpler::renderer::KeyIdentity::Character(ref ch) = k.logical_key
                && (ch == "k" || ch == "K")
                && k.modifiers.control
            {
                state.command_palette_open = !state.command_palette_open;
                return true;
            }
            if let cssimpler::renderer::KeyIdentity::Named(ref name) = k.logical_key
                && name == "Escape"
                && state.command_palette_open
            {
                state.command_palette_open = false;
                return true;
            }
        }
        false
    })
}

/// Standalone native UI entry point for Studio V2.
pub fn run_studio_v2(
    initial_state: StudioV2State,
    config: WindowConfig,
) -> cssimpler::renderer::Result<()> {
    let stylesheet = load_studio_stylesheet().map_err(|e| RendererError::Window(e.to_string()))?;
    let app = create_studio_v2_app(initial_state, &stylesheet);
    app.run(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cssimpler::core::LayoutBox;

    #[test]
    fn test_studio_v2_css_parses_cleanly() {
        let stylesheet = load_studio_stylesheet().expect("studio_v2.css should parse cleanly");
        assert!(
            !stylesheet.rules.is_empty(),
            "parsed stylesheet should have rules"
        );
    }

    #[test]
    fn test_studio_v2_shell_renders_all_modes_and_themes() {
        let stylesheet = load_studio_stylesheet().expect("stylesheet should parse");

        let modes = [
            ViewLayoutMode::Split,
            ViewLayoutMode::ThreeDOnly,
            ViewLayoutMode::TwoDOnly,
        ];
        let themes = [Theme::Dark, Theme::Light];

        for &mode in &modes {
            for &theme in &themes {
                let state = StudioV2State {
                    theme,
                    layout_mode: mode,
                    ..StudioV2State::default()
                };

                let scene = layout_studio_shell(&state, &stylesheet, 1920, 1080);
                assert_eq!(scene.layout.width, 1920.0);
                assert_eq!(scene.layout.height, 1080.0);

                let map = extract_viewport_layout(&scene);
                assert!(map.top_bar.is_some(), "top_bar should exist");
                assert!(map.status_bar.is_some(), "status_bar should exist");
                assert!(map.rail_3d.is_some(), "rail_3d should exist");
                assert!(map.rail_2d.is_some(), "rail_2d should exist");
                assert!(map.right_dock.is_some(), "right_dock should exist");
                assert!(map.object_browser.is_some(), "object_browser should exist");
                assert!(
                    map.property_editor.is_some(),
                    "property_editor should exist"
                );
            }
        }
    }

    #[test]
    fn test_layout_rect_extraction_finds_core_regions_with_non_zero_bounds() {
        let stylesheet = load_studio_stylesheet().expect("stylesheet should parse");
        let state = StudioV2State {
            layout_mode: ViewLayoutMode::Split,
            ..StudioV2State::default()
        };

        let scene = layout_studio_shell(&state, &stylesheet, 1600, 900);
        let map = extract_viewport_layout(&scene);

        // Core viewports must have positive width and height
        let v3d = map
            .viewport_3d
            .expect("#viewport-3d must be present in Split mode");
        let v2d = map
            .viewport_2d
            .expect("#viewport-2d must be present in Split mode");
        assert!(
            v3d.width > 100.0,
            "3D viewport width must be > 100px: {}",
            v3d.width
        );
        assert!(
            v3d.height > 100.0,
            "3D viewport height must be > 100px: {}",
            v3d.height
        );
        assert!(
            v2d.width > 100.0,
            "2D viewport width must be > 100px: {}",
            v2d.width
        );
        assert!(
            v2d.height > 100.0,
            "2D viewport height must be > 100px: {}",
            v2d.height
        );

        // Dock components must have positive width and height
        let ob = map.object_browser.expect("#object-browser must be present");
        let pe = map
            .property_editor
            .expect("#property-editor must be present");
        assert!(ob.width > 50.0, "object browser width > 50px: {}", ob.width);
        assert!(
            ob.height > 50.0,
            "object browser height > 50px: {}",
            ob.height
        );
        assert!(
            pe.width > 50.0,
            "property editor width > 50px: {}",
            pe.width
        );
        assert!(
            pe.height > 50.0,
            "property editor height > 50px: {}",
            pe.height
        );

        // Physical rect conversions
        let phys_3d = map.viewport_3d_physical(1.5).expect("physical 3d rect");
        assert_eq!(phys_3d.width, (v3d.width * 1.5).round() as u32);
        assert_eq!(phys_3d.height, (v3d.height * 1.5).round() as u32);
    }

    #[test]
    fn test_layout_modes_activate_and_deactivate_viewports() {
        let stylesheet = load_studio_stylesheet().expect("stylesheet should parse");

        // 3D Only mode
        let state_3d = StudioV2State {
            layout_mode: ViewLayoutMode::ThreeDOnly,
            ..StudioV2State::default()
        };
        let scene_3d = layout_studio_shell(&state_3d, &stylesheet, 1600, 900);
        let map_3d = extract_viewport_layout(&scene_3d);
        let v3d = map_3d.viewport_3d.expect("3D viewport exists in 3D-only");
        assert!(v3d.width > 0.0);

        // 2D Only mode
        let state_2d = StudioV2State {
            layout_mode: ViewLayoutMode::TwoDOnly,
            ..StudioV2State::default()
        };
        let scene_2d = layout_studio_shell(&state_2d, &stylesheet, 1600, 900);
        let map_2d = extract_viewport_layout(&scene_2d);
        let v2d = map_2d.viewport_2d.expect("2D viewport exists in 2D-only");
        assert!(v2d.width > 0.0);
    }

    #[test]
    fn test_input_routing_distinguishes_chrome_and_canvas_hits() {
        let stylesheet = load_studio_stylesheet().expect("stylesheet should parse");
        let state = StudioV2State {
            layout_mode: ViewLayoutMode::Split,
            command_palette_open: false,
            ..StudioV2State::default()
        };

        let scene = layout_studio_shell(&state, &stylesheet, 1600, 900);
        let map = extract_viewport_layout(&scene);

        // Top bar hit (y = 15.0)
        let top_target = route_pointer_event(200.0, 15.0, &map, &state);
        assert_eq!(top_target, InputTarget::UiChrome);

        // Left rail hit (x = 20.0, y = 200.0)
        let rail_target = route_pointer_event(20.0, 200.0, &map, &state);
        assert_eq!(rail_target, InputTarget::UiChrome);

        // Right dock hit (x = 1500.0, y = 300.0)
        let dock_target = route_pointer_event(1500.0, 300.0, &map, &state);
        assert_eq!(dock_target, InputTarget::UiChrome);

        // Bottom status bar hit (y = 890.0)
        let status_target = route_pointer_event(500.0, 890.0, &map, &state);
        assert_eq!(status_target, InputTarget::UiChrome);

        // 3D Viewport canvas hit
        let v3d = map.viewport_3d.unwrap();
        let target_3d = route_pointer_event(v3d.x + 20.0, v3d.y + 20.0, &map, &state);
        assert_eq!(target_3d, InputTarget::Viewport3D);

        // 2D Viewport canvas hit
        let v2d = map.viewport_2d.unwrap();
        let target_2d = route_pointer_event(v2d.x + 20.0, v2d.y + 20.0, &map, &state);
        assert_eq!(target_2d, InputTarget::Viewport2D);

        // Modal occlusion: when command palette is open, input routes to modal
        let state_modal = StudioV2State {
            command_palette_open: true,
            ..state
        };
        let scene_modal = layout_studio_shell(&state_modal, &stylesheet, 1600, 900);
        let map_modal = extract_viewport_layout(&scene_modal);
        let modal_target =
            route_pointer_event(v3d.x + 20.0, v3d.y + 20.0, &map_modal, &state_modal);
        assert_eq!(modal_target, InputTarget::ModalOrOverlay);
    }

    #[test]
    fn test_transparent_viewport_cutouts_and_composition() {
        let width = 200;
        let height = 150;
        let mut ui_buffer = vec![255_u8; width * height * 4]; // Opaque white UI

        let active_viewports = [
            LayoutBox::new(20.0, 20.0, 60.0, 80.0), // 3D viewport
            LayoutBox::new(90.0, 20.0, 60.0, 80.0), // 2D viewport
        ];

        punch_viewport_cutouts(&mut ui_buffer, width, height, &active_viewports, 1.0);

        // Test alpha inside cutouts
        for &(cx, cy) in &[(30, 30), (50, 60), (100, 30), (120, 60)] {
            let idx = (cy * width + cx) * 4;
            assert_eq!(
                ui_buffer[idx + 3],
                0,
                "Pixel ({}, {}) inside viewport cutout must have alpha = 0",
                cx,
                cy
            );
        }

        // Test alpha outside cutouts (top bar, rails, dock)
        for &(cx, cy) in &[(5, 5), (10, 100), (170, 50), (100, 140)] {
            let idx = (cy * width + cx) * 4;
            assert_eq!(
                ui_buffer[idx + 3],
                255,
                "Pixel ({}, {}) outside viewport cutout must remain opaque",
                cx,
                cy
            );
        }

        // Test composite over frame
        let mut frame_buffer = vec![128_u8; width * height * 4];
        composite_ui_over_frame(&ui_buffer, &mut frame_buffer, width, height);

        // Frame pixel inside cutout was untouched (remains 128)
        let cutout_idx = (30 * width + 30) * 4;
        assert_eq!(frame_buffer[cutout_idx], 128);

        // Frame pixel outside cutout was overwritten by UI (now 255)
        let chrome_idx = (5 * width + 5) * 4;
        assert_eq!(frame_buffer[chrome_idx], 255);
    }

    #[test]
    fn test_pure_svg_pattern_board_generation() {
        let panels = vec![
            PatternPanelPreview {
                id: "front".to_string(),
                label: "Bodice Front".to_string(),
                vertices: vec![(10.0, 10.0), (100.0, 10.0), (100.0, 150.0), (10.0, 150.0)],
                seam_lines: vec![((10.0, 10.0), (100.0, 10.0))],
            },
            PatternPanelPreview {
                id: "back".to_string(),
                label: "Bodice Back".to_string(),
                vertices: vec![(120.0, 10.0), (210.0, 10.0), (210.0, 150.0), (120.0, 150.0)],
                seam_lines: vec![((120.0, 10.0), (210.0, 10.0))],
            },
        ];

        let svg_str = generate_pattern_board_svg(&panels, (0.0, 0.0, 300.0, 200.0));
        assert!(svg_str.contains("<svg"));
        assert!(svg_str.contains("panel-front"));
        assert!(svg_str.contains("panel-back"));
        assert!(svg_str.contains("stroke-dasharray"));

        let _node = pattern_board_svg_node(&panels, (0.0, 0.0, 300.0, 200.0));
    }

    #[test]
    fn test_studio_v2_app_headless_embed_and_events() {
        let stylesheet = load_studio_stylesheet().expect("stylesheet should parse");
        let initial_state = StudioV2State {
            layout_mode: ViewLayoutMode::Split,
            ..StudioV2State::default()
        };

        let mut app = create_studio_v2_app(initial_state, &stylesheet);

        // Test state inspection and mutation
        assert_eq!(app.state().layout_mode, ViewLayoutMode::Split);
        app.state_mut().layout_mode = ViewLayoutMode::ThreeDOnly;
        assert_eq!(app.state().layout_mode, ViewLayoutMode::ThreeDOnly);

        // Test engine event handling (Ctrl+K toggles command palette)
        let ctrl_k = cssimpler::renderer::EngineEvent::Key(cssimpler::renderer::KeyboardEvent {
            logical_key: cssimpler::renderer::KeyIdentity::Character("k".to_string()),
            physical_key: None,
            location: Default::default(),
            state: cssimpler::renderer::ButtonState::Pressed,
            repeat: false,
            modifiers: cssimpler::renderer::KeyboardModifiers {
                control: true,
                ..Default::default()
            },
        });
        assert!(!app.state().command_palette_open);
        let handled = app.handle_engine_event(&ctrl_k);
        assert!(
            handled,
            "Ctrl+K should be handled by Studio V2 event handler"
        );
        assert!(
            app.state().command_palette_open,
            "command palette should be open"
        );

        // Test headless RGBA8 rendering
        let width = 800;
        let height = 600;
        let mut rgba = vec![0_u8; width * height * 4];
        app.render_rgba8(
            cssimpler::renderer::FrameInfo {
                delta: std::time::Duration::from_millis(16),
                frame_index: 0,
            },
            &mut rgba,
            width,
            height,
            1.0,
            cssimpler::core::Color::rgba(0, 0, 0, 0),
        );

        // Check that viewports were located by ID
        let vp_3d = app.find_element_bounds_by_id("#viewport-3d");
        assert!(vp_3d.is_some(), "#viewport-3d should be located on app");
    }
}

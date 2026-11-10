//! Hybrid WGPU Viewport Compositor and Input Router for CSSimpler Studio V2.
//!
//! Provides:
//! - Exact logical and physical bounding rect extraction for `#viewport-3d`, `#viewport-2d`,
//!   `#object-browser`, `#property-editor`, and other chrome regions from the CSSimpler `RenderNode` scene tree.
//! - Input routing separating UI chrome from 3D viewport canvas hits and 2D pattern board hits.
//! - RGBA8 buffer composition punching transparent viewport cutouts (alpha=0) to allow underlying
//!   hybrid 3D WGPU cloth renders and 2D pattern surfaces to show through without occlusion.
//! - Pure-SVG 2D pattern board outline rendering for fallback / fast preview in `#viewport-2d`.
//!
//! 100% original MIT-licensed implementation for FoldCraft.

#![allow(dead_code)]

use cssimpler::core::{LayoutBox, Node, RenderNode};
use cssimpler::renderer::WindowConfig;

use super::{StudioV2State, ViewLayoutMode};

/// Physical integer rectangle in pixel units.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PhysicalRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl PhysicalRect {
    pub const fn new(x: u32, y: u32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn contains(&self, px: u32, py: u32) -> bool {
        px >= self.x
            && px < self.x.saturating_add(self.width)
            && py >= self.y
            && py < self.y.saturating_add(self.height)
    }

    pub fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0
    }
}

/// Extracted layout bounds of all major Studio V2 functional areas.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ViewportLayoutMap {
    pub viewport_stage: Option<LayoutBox>,
    pub viewport_3d: Option<LayoutBox>,
    pub viewport_2d: Option<LayoutBox>,
    pub object_browser: Option<LayoutBox>,
    pub property_editor: Option<LayoutBox>,
    pub left_drawer: Option<LayoutBox>,
    pub right_dock: Option<LayoutBox>,
    pub rail_3d: Option<LayoutBox>,
    pub rail_2d: Option<LayoutBox>,
    pub top_bar: Option<LayoutBox>,
    pub status_bar: Option<LayoutBox>,
    pub command_palette_modal: Option<LayoutBox>,
}

impl ViewportLayoutMap {
    /// Convert a logical LayoutBox to a physical pixel rectangle.
    pub fn to_physical(
        &self,
        layout: Option<LayoutBox>,
        scale_factor: f32,
    ) -> Option<PhysicalRect> {
        let b = layout?;
        if b.width <= 0.0 || b.height <= 0.0 {
            return None;
        }
        let scale = scale_factor.max(0.1);
        let x = (b.x * scale).max(0.0).round() as u32;
        let y = (b.y * scale).max(0.0).round() as u32;
        let width = (b.width * scale).max(0.0).round() as u32;
        let height = (b.height * scale).max(0.0).round() as u32;
        Some(PhysicalRect::new(x, y, width, height))
    }

    /// Physical rectangle for the 3D viewport canvas.
    pub fn viewport_3d_physical(&self, scale_factor: f32) -> Option<PhysicalRect> {
        self.to_physical(self.viewport_3d, scale_factor)
    }

    /// Physical rectangle for the 2D pattern board viewport canvas.
    pub fn viewport_2d_physical(&self, scale_factor: f32) -> Option<PhysicalRect> {
        self.to_physical(self.viewport_2d, scale_factor)
    }
}

/// Target destination for pointer and keyboard inputs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputTarget {
    /// Top menu bar, tool rails, dock drawers, or status bar.
    UiChrome,
    /// 3D Viewport canvas: routes to camera controller, picking, and pin grabbing.
    Viewport3D,
    /// 2D Metric pattern board: routes to pattern selection, drafting, and seam wiring.
    Viewport2D,
    /// Modal dialog or command palette overlay.
    ModalOrOverlay,
}

/// Helper to test if a logical point is within a LayoutBox.
pub fn layout_box_contains(box_layout: &LayoutBox, x: f32, y: f32) -> bool {
    x >= box_layout.x
        && x <= box_layout.x + box_layout.width
        && y >= box_layout.y
        && y <= box_layout.y + box_layout.height
}

/// Traverse the CSSimpler `RenderNode` scene tree and extract layout rectangles by element ID.
pub fn extract_viewport_layout(scene_root: &RenderNode) -> ViewportLayoutMap {
    let roots = [scene_root.clone()];
    ViewportLayoutMap {
        viewport_stage: cssimpler::renderer::find_element_bounds_by_id(&roots, "viewport-stage"),
        viewport_3d: cssimpler::renderer::find_element_bounds_by_id(&roots, "viewport-3d"),
        viewport_2d: cssimpler::renderer::find_element_bounds_by_id(&roots, "viewport-2d"),
        object_browser: cssimpler::renderer::find_element_bounds_by_id(&roots, "object-browser"),
        property_editor: cssimpler::renderer::find_element_bounds_by_id(&roots, "property-editor"),
        left_drawer: cssimpler::renderer::find_element_bounds_by_id(&roots, "left-drawer"),
        right_dock: cssimpler::renderer::find_element_bounds_by_id(&roots, "right-dock"),
        rail_3d: cssimpler::renderer::find_element_bounds_by_id(&roots, "rail-3d"),
        rail_2d: cssimpler::renderer::find_element_bounds_by_id(&roots, "rail-2d"),
        top_bar: cssimpler::renderer::find_element_bounds_by_id(&roots, "top-bar"),
        status_bar: cssimpler::renderer::find_element_bounds_by_id(&roots, "status-bar"),
        command_palette_modal: cssimpler::renderer::find_element_bounds_by_id(
            &roots,
            "command-palette-modal",
        ),
    }
}

/// Routes pointer events to UI chrome, 3D viewport canvas, 2D pattern board, or active modals.
pub fn route_pointer_event(
    x: f32,
    y: f32,
    layout: &ViewportLayoutMap,
    state: &StudioV2State,
) -> InputTarget {
    // 1. If command palette or a modal is open, modal captures input
    if state.command_palette_open {
        if let Some(modal_box) = &layout.command_palette_modal
            && layout_box_contains(modal_box, x, y)
        {
            return InputTarget::ModalOrOverlay;
        }
        return InputTarget::ModalOrOverlay;
    }

    // 2. Check viewports based on current layout mode
    match state.layout_mode {
        ViewLayoutMode::Split => {
            if let Some(v3d) = &layout.viewport_3d
                && layout_box_contains(v3d, x, y)
            {
                return InputTarget::Viewport3D;
            }
            if let Some(v2d) = &layout.viewport_2d
                && layout_box_contains(v2d, x, y)
            {
                return InputTarget::Viewport2D;
            }
        }
        ViewLayoutMode::ThreeDOnly => {
            if let Some(v3d) = &layout.viewport_3d
                && layout_box_contains(v3d, x, y)
            {
                return InputTarget::Viewport3D;
            }
        }
        ViewLayoutMode::TwoDOnly => {
            if let Some(v2d) = &layout.viewport_2d
                && layout_box_contains(v2d, x, y)
            {
                return InputTarget::Viewport2D;
            }
        }
    }

    // 3. Otherwise hit belongs to UI chrome (top bar, rails, dock, drawer, status bar)
    InputTarget::UiChrome
}

/// Punch transparent cutouts (alpha=0) in an RGBA8 buffer over active viewports.
///
/// This ensures the underlying WGPU 3D cloth simulation frame and 2D metric board surface
/// show through clearly without any UI chrome occlusion or opaque fill.
pub fn punch_viewport_cutouts(
    rgba_buffer: &mut [u8],
    buffer_width: usize,
    buffer_height: usize,
    active_viewports: &[LayoutBox],
    scale_factor: f32,
) {
    let scale = scale_factor.max(0.1);
    for vp in active_viewports {
        if vp.width <= 0.0 || vp.height <= 0.0 {
            continue;
        }

        let x0 = (vp.x * scale).max(0.0).round() as usize;
        let y0 = (vp.y * scale).max(0.0).round() as usize;
        let x1 = ((vp.x + vp.width) * scale).max(0.0).round() as usize;
        let y1 = ((vp.y + vp.height) * scale).max(0.0).round() as usize;

        let x0 = x0.min(buffer_width);
        let x1 = x1.min(buffer_width);
        let y0 = y0.min(buffer_height);
        let y1 = y1.min(buffer_height);

        for y in y0..y1 {
            let row_offset = y * buffer_width * 4;
            for x in x0..x1 {
                let idx = row_offset + x * 4;
                rgba_buffer[idx] = 0;
                rgba_buffer[idx + 1] = 0;
                rgba_buffer[idx + 2] = 0;
                rgba_buffer[idx + 3] = 0; // Transparent cutout!
            }
        }
    }
}

/// Composite the rendered UI RGBA8 buffer over an underlying frame (such as the WGPU framebuffer).
///
/// Pixels with alpha = 0 (punched viewport cutouts) preserve the destination frame untouched.
/// Pixels with alpha = 255 (opaque UI chrome) completely overwrite the destination.
/// Intermediate alpha blends smoothly using standard linear blending.
pub fn composite_ui_over_frame(ui_rgba: &[u8], frame_rgba: &mut [u8], width: usize, height: usize) {
    let pixel_count = width * height;
    let byte_count = pixel_count * 4;
    assert_eq!(ui_rgba.len(), byte_count);
    assert_eq!(frame_rgba.len(), byte_count);

    let (ui_chunks, _) = ui_rgba.as_chunks::<4>();
    let (frame_chunks, _) = frame_rgba.as_chunks_mut::<4>();
    for (ui_chunk, frame_chunk) in ui_chunks.iter().zip(frame_chunks.iter_mut()) {
        let ui_a = ui_chunk[3];
        if ui_a == 0 {
            // Cutout: underlying frame passes through untouched
            continue;
        } else if ui_a == 255 {
            // Opaque chrome: fully overwrite frame
            frame_chunk.copy_from_slice(ui_chunk);
        } else {
            // Translucent overlay: blend
            let a = ui_a as u32;
            let inv_a = 255 - a;
            frame_chunk[0] = ((ui_chunk[0] as u32 * a + frame_chunk[0] as u32 * inv_a) / 255) as u8;
            frame_chunk[1] = ((ui_chunk[1] as u32 * a + frame_chunk[1] as u32 * inv_a) / 255) as u8;
            frame_chunk[2] = ((ui_chunk[2] as u32 * a + frame_chunk[2] as u32 * inv_a) / 255) as u8;
            frame_chunk[3] = (a + (frame_chunk[3] as u32 * inv_a) / 255).min(255) as u8;
        }
    }
}

/// Creates a frameless `WindowConfig` configured for the Studio V2 app shell.
pub fn studio_window_config(title: &str, width: usize, height: usize) -> WindowConfig {
    WindowConfig::new(title, width, height)
        .with_decorations(false)
        .with_drag_region_height(38.0) // .top-bar draggable
        .with_resize_border_width(6.0)
}

/// Panel outline data for pure-SVG 2D pattern board preview fallback.
#[derive(Clone, Debug, PartialEq)]
pub struct PatternPanelPreview {
    pub id: String,
    pub label: String,
    pub vertices: Vec<(f32, f32)>,
    pub seam_lines: Vec<((f32, f32), (f32, f32))>,
}

/// Generates a pure-SVG string rendering 2D pattern board outlines.
pub fn generate_pattern_board_svg(
    panels: &[PatternPanelPreview],
    view_box: (f32, f32, f32, f32),
) -> String {
    let (min_x, min_y, width, height) = view_box;
    let mut out = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"{min_x} {min_y} {width} {height}\" class=\"pattern-preview-svg\">\n"
    );
    out.push_str(
        "  <g stroke=\"#3b82f6\" stroke-width=\"1.5\" fill=\"rgba(59, 130, 246, 0.1)\">\n",
    );

    for panel in panels {
        if panel.vertices.is_empty() {
            continue;
        }
        let mut d = String::new();
        for (i, &(x, y)) in panel.vertices.iter().enumerate() {
            if i == 0 {
                d.push_str(&format!("M {x:.2} {y:.2} "));
            } else {
                d.push_str(&format!("L {x:.2} {y:.2} "));
            }
        }
        d.push('Z');
        out.push_str(&format!(
            "    <path d=\"{d}\" id=\"panel-{}\" />\n",
            panel.id
        ));

        for ((x0, y0), (x1, y1)) in &panel.seam_lines {
            out.push_str(&format!(
                "    <line x1=\"{x0:.2}\" y1=\"{y0:.2}\" x2=\"{x1:.2}\" y2=\"{y1:.2}\" stroke=\"#10b981\" stroke-dasharray=\"4,3\" stroke-width=\"1.2\" />\n"
            ));
        }

        if let Some(&(lx, ly)) = panel.vertices.first() {
            out.push_str(&format!(
                "    <text x=\"{lx:.2}\" y=\"{:.2}\" fill=\"#848d9a\" font-size=\"12\" font-family=\"sans-serif\">{}</text>\n",
                ly - 4.0,
                panel.label
            ));
        }
    }

    out.push_str("  </g>\n</svg>");
    out
}

/// Generates a CSSimpler `Node` containing SVG elements for pattern board outline preview.
pub fn pattern_board_svg_node(
    panels: &[PatternPanelPreview],
    view_box: (f32, f32, f32, f32),
) -> Node {
    let (min_x, min_y, width, height) = view_box;
    let mut svg_el = Node::element("svg")
        .with_class("pattern-preview-svg")
        .with_attribute("viewBox", format!("{min_x} {min_y} {width} {height}"));

    for panel in panels {
        if panel.vertices.is_empty() {
            continue;
        }
        let mut d = String::new();
        for (i, &(x, y)) in panel.vertices.iter().enumerate() {
            if i == 0 {
                d.push_str(&format!("M {x:.2} {y:.2} "));
            } else {
                d.push_str(&format!("L {x:.2} {y:.2} "));
            }
        }
        d.push('Z');

        let path = Node::element("path")
            .with_attribute("d", d)
            .with_attribute("stroke", "#3b82f6")
            .with_attribute("fill", "rgba(59, 130, 246, 0.1)");
        svg_el = svg_el.with_child(Node::from(path));

        for ((x0, y0), (x1, y1)) in &panel.seam_lines {
            let line = Node::element("line")
                .with_attribute("x1", x0.to_string())
                .with_attribute("y1", y0.to_string())
                .with_attribute("x2", x1.to_string())
                .with_attribute("y2", y1.to_string())
                .with_attribute("stroke", "#10b981");
            svg_el = svg_el.with_child(Node::from(line));
        }
    }

    Node::from(svg_el)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cssimpler::core::LayoutBox;

    #[test]
    fn test_physical_rect_operations() {
        let rect = PhysicalRect::new(100, 50, 400, 300);
        assert!(!rect.is_empty());
        assert!(rect.contains(100, 50));
        assert!(rect.contains(499, 349));
        assert!(!rect.contains(500, 350));
        assert!(!rect.contains(99, 49));

        let empty = PhysicalRect::default();
        assert!(empty.is_empty());
    }

    #[test]
    fn test_viewport_layout_map_conversion() {
        let mut map = ViewportLayoutMap::default();
        map.viewport_3d = Some(LayoutBox::new(200.0, 100.0, 800.0, 600.0));
        map.viewport_2d = Some(LayoutBox::new(1000.0, 100.0, 600.0, 600.0));

        let phys_3d = map.viewport_3d_physical(1.5).unwrap();
        assert_eq!(phys_3d, PhysicalRect::new(300, 150, 1200, 900));

        let phys_2d = map.viewport_2d_physical(1.0).unwrap();
        assert_eq!(phys_2d, PhysicalRect::new(1000, 100, 600, 600));

        assert!(map.to_physical(None, 1.0).is_none());
    }

    #[test]
    fn test_route_pointer_event() {
        let mut layout = ViewportLayoutMap::default();
        layout.viewport_3d = Some(LayoutBox::new(0.0, 40.0, 600.0, 500.0));
        layout.viewport_2d = Some(LayoutBox::new(600.0, 40.0, 600.0, 500.0));
        layout.top_bar = Some(LayoutBox::new(0.0, 0.0, 1200.0, 40.0));
        layout.command_palette_modal = Some(LayoutBox::new(300.0, 150.0, 600.0, 300.0));

        let mut state = StudioV2State::default();
        state.layout_mode = ViewLayoutMode::Split;

        // Chrome hit (top bar)
        assert_eq!(
            route_pointer_event(100.0, 20.0, &layout, &state),
            InputTarget::UiChrome
        );

        // 3D Viewport hit
        assert_eq!(
            route_pointer_event(300.0, 200.0, &layout, &state),
            InputTarget::Viewport3D
        );

        // 2D Viewport hit
        assert_eq!(
            route_pointer_event(800.0, 200.0, &layout, &state),
            InputTarget::Viewport2D
        );

        // Modal captures when command palette is open
        state.command_palette_open = true;
        assert_eq!(
            route_pointer_event(300.0, 200.0, &layout, &state),
            InputTarget::ModalOrOverlay
        );

        // 3D-only mode routes only to 3D
        state.command_palette_open = false;
        state.layout_mode = ViewLayoutMode::ThreeDOnly;
        assert_eq!(
            route_pointer_event(800.0, 200.0, &layout, &state),
            InputTarget::UiChrome
        );
        assert_eq!(
            route_pointer_event(300.0, 200.0, &layout, &state),
            InputTarget::Viewport3D
        );
    }

    #[test]
    fn test_punch_viewport_cutouts_and_composition() {
        let width = 20;
        let height = 20;
        let mut ui_buffer = vec![255_u8; width * height * 4]; // Opaque white UI

        // Punch a 10x10 cutout in center
        let cutouts = [LayoutBox::new(5.0, 5.0, 10.0, 10.0)];
        punch_viewport_cutouts(&mut ui_buffer, width, height, &cutouts, 1.0);

        // Verify cutout has alpha=0
        for y in 5..15 {
            for x in 5..15 {
                let idx = (y * width + x) * 4;
                assert_eq!(ui_buffer[idx + 3], 0, "cutout pixel should have alpha=0");
            }
        }
        // Verify border has alpha=255
        assert_eq!(ui_buffer[3], 255);

        // Test composite over underlying frame
        let mut underlying_frame = vec![128_u8; width * height * 4]; // Underlying frame gray
        composite_ui_over_frame(&ui_buffer, &mut underlying_frame, width, height);

        // Cutout area preserved untouched
        for y in 5..15 {
            for x in 5..15 {
                let idx = (y * width + x) * 4;
                assert_eq!(
                    underlying_frame[idx], 128,
                    "underlying frame must pass through cutout untouched"
                );
            }
        }

        // Chrome area overwritten with UI
        assert_eq!(underlying_frame[0], 255);
    }

    #[test]
    fn test_pattern_board_svg_generation() {
        let panels = vec![PatternPanelPreview {
            id: "front".to_string(),
            label: "Front Panel".to_string(),
            vertices: vec![(0.0, 0.0), (100.0, 0.0), (100.0, 150.0), (0.0, 150.0)],
            seam_lines: vec![((0.0, 0.0), (100.0, 0.0))],
        }];

        let svg_str = generate_pattern_board_svg(&panels, (0.0, 0.0, 200.0, 200.0));
        assert!(svg_str.contains("Front Panel"));
        assert!(svg_str.contains("panel-front"));
        assert!(svg_str.contains("<line"));

        let svg_node = pattern_board_svg_node(&panels, (0.0, 0.0, 200.0, 200.0));
        assert!(matches!(svg_node, Node::Element(_)));
    }
}

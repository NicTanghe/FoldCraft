//! UI-independent navigation for a metric 2D pattern board.
//!
//! This is the navigation prerequisite for native viewport adapters, not a tool
//! controller: hit testing, authoring drafts and 3D gestures are not implemented.
//! Coordinates and wheel deltas are logical pixels; apply the host's DPI
//! conversion once before routing events. Board coordinates are metres with Y
//! increasing downward, matching the existing drafting board. View transforms
//! never change panel-local rest geometry or the physical scene.
use glam::DVec2;

/// Logical pixels per metre, matching the existing drafting board limits.
pub const MIN_BOARD_SCALE: f64 = 40.;
pub const MAX_BOARD_SCALE: f64 = 2400.;

/// A finite, positive-area viewport in the host's logical coordinate system.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LogicalViewport {
    origin: DVec2,
    size: DVec2,
}

impl LogicalViewport {
    pub fn new(origin: DVec2, size: DVec2) -> Option<Self> {
        (origin.is_finite()
            && size.is_finite()
            && size.min_element() > 0.
            && (origin + size).is_finite()
            && (origin + size).cmpgt(origin).all())
        .then_some(Self { origin, size })
    }

    pub fn origin(self) -> DVec2 {
        self.origin
    }

    pub fn size(self) -> DVec2 {
        self.size
    }

    pub fn center(self) -> DVec2 {
        self.origin + self.size * 0.5
    }

    /// Half-open bounds ensure adjacent viewports cannot both start a gesture.
    pub fn contains(self, point: DVec2) -> bool {
        point.is_finite()
            && point.cmpge(self.origin).all()
            && point.cmplt(self.origin + self.size).all()
    }
}

/// Presentation-only board transform. Rotation is in radians.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoardView {
    center_metres: DVec2,
    scale: f64,
    rotation: f64,
}

impl Default for BoardView {
    fn default() -> Self {
        Self {
            center_metres: DVec2::ZERO,
            scale: 300.,
            rotation: 0.,
        }
    }
}

impl BoardView {
    /// Reject non-finite values and non-positive scales; clamp valid scales to
    /// the board limits. This does not reconstruct geometry from a drape.
    pub fn new(center_metres: DVec2, scale: f64, rotation: f64) -> Option<Self> {
        (center_metres.is_finite() && scale.is_finite() && scale > 0. && rotation.is_finite())
            .then_some(Self {
                center_metres,
                scale: scale.clamp(MIN_BOARD_SCALE, MAX_BOARD_SCALE),
                rotation,
            })
    }

    pub fn center_metres(self) -> DVec2 {
        self.center_metres
    }

    pub fn scale(self) -> f64 {
        self.scale
    }

    pub fn rotation(self) -> f64 {
        self.rotation
    }

    fn rotate(self, point: DVec2, inverse: bool) -> DVec2 {
        let (sin, cos) = if inverse {
            -self.rotation
        } else {
            self.rotation
        }
        .sin_cos();
        DVec2::new(cos * point.x - sin * point.y, sin * point.x + cos * point.y)
    }

    /// Mapping remains available outside the viewport for captured gestures.
    pub fn board_to_logical(self, viewport: LogicalViewport, metres: DVec2) -> Option<DVec2> {
        let result =
            viewport.center() + self.rotate((metres - self.center_metres) * self.scale, false);
        (metres.is_finite() && result.is_finite()).then_some(result)
    }

    pub fn logical_to_board(self, viewport: LogicalViewport, point: DVec2) -> Option<DVec2> {
        let result =
            self.center_metres + self.rotate((point - viewport.center()) / self.scale, true);
        (point.is_finite() && result.is_finite()).then_some(result)
    }

    /// Follow a logical pointer delta, respecting the view rotation.
    /// Returns whether the view changed; invalid input leaves the view intact.
    pub fn pan_by(&mut self, delta: DVec2) -> bool {
        let center = self.center_metres - self.rotate(delta / self.scale, true);
        if !delta.is_finite() || !center.is_finite() || center == self.center_metres {
            return false;
        }
        self.center_metres = center;
        true
    }

    /// Keep the board point under the cursor fixed, including at scale limits.
    pub fn zoom_at(&mut self, viewport: LogicalViewport, cursor: DVec2, factor: f64) -> bool {
        if !factor.is_finite() || factor <= 0. {
            return false;
        }
        let Some(before) = self.logical_to_board(viewport, cursor) else {
            return false;
        };
        let mut next = *self;
        next.scale = (self.scale * factor).clamp(MIN_BOARD_SCALE, MAX_BOARD_SCALE);
        let Some(after) = next.logical_to_board(viewport, cursor) else {
            return false;
        };
        next.center_metres += before - after;
        if !next.center_metres.is_finite() || next == *self {
            return false;
        }
        *self = next;
        true
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Modifiers {
    pub shift: bool,
    pub control: bool,
    pub alt: bool,
    pub command: bool,
}

impl Modifiers {
    fn any(self) -> bool {
        self.shift || self.control || self.alt || self.command
    }
}

/// Host-owned routing gates. Set pointer availability false for overlays or
/// other tools that own the canvas, and keyboard availability false in text
/// fields. Deliver focus/ownership changes even without pointer motion.
#[derive(Clone, Copy, Debug)]
pub struct InputOwnership {
    pub window_focused: bool,
    pub pointer_available: bool,
    pub keyboard_available: bool,
}

impl Default for InputOwnership {
    fn default() -> Self {
        Self {
            window_focused: true,
            pointer_available: true,
            keyboard_available: true,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointerButton {
    Primary,
    Middle,
    Secondary,
}

/// Pointer IDs are host identities, not garment or mesh identifiers. Hosts must
/// forward captured moves/releases outside the viewport and issue Cancel when
/// native capture is lost. Secondary clicks are left to the context menu.
#[derive(Clone, Copy, Debug)]
pub enum NavigationEvent {
    PointerPressed {
        id: u64,
        position: DVec2,
        button: PointerButton,
        modifiers: Modifiers,
    },
    PointerMoved {
        id: u64,
        position: DVec2,
    },
    PointerReleased {
        id: u64,
        position: DVec2,
        button: PointerButton,
    },
    /// Positive logical-pixel deltas zoom in. Convert line-wheel units in the
    /// adapter; they must not be mistaken for physical or logical pixels.
    Wheel {
        position: DVec2,
        delta_y: f64,
        modifiers: Modifiers,
    },
    Escape,
    Cancel,
    /// Reconcile focus/viewport/ownership changes without synthesizing motion.
    OwnershipChanged,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NavigationResult {
    pub consumed: bool,
    pub view_changed: bool,
    pub capture_changed: bool,
}

#[derive(Clone, Copy, Debug)]
struct PanCapture {
    id: u64,
    last_position: DVec2,
    viewport: LogicalViewport,
}

/// Captured middle-button pan and unmodified cursor-anchored wheel zoom.
/// Modifier changes during an existing pan do not transfer pointer ownership.
/// Cancel, Escape, focus loss, pointer ownership loss and viewport changes end
/// capture, retaining the latest view. None creates document/history entries.
#[derive(Debug, Default)]
pub struct BoardNavigation {
    pan: Option<PanCapture>,
}

impl BoardNavigation {
    pub fn captured_pointer(&self) -> Option<u64> {
        self.pan.map(|pan| pan.id)
    }

    pub fn handle(
        &mut self,
        view: &mut BoardView,
        viewport: Option<LogicalViewport>,
        ownership: InputOwnership,
        event: NavigationEvent,
    ) -> NavigationResult {
        let mut result = NavigationResult::default();
        if !ownership.window_focused
            || !ownership.pointer_available
            || viewport.is_none()
            || self.pan.is_some_and(|pan| Some(pan.viewport) != viewport)
        {
            // Releasing the old capture does not own this event: it may be
            // the first click delivered to an overlay or a resized viewport.
            result.capture_changed = self.pan.take().is_some();
            return result;
        }
        let viewport = viewport.expect("validated viewport");
        match event {
            NavigationEvent::Cancel => {
                result.capture_changed = self.pan.take().is_some();
                result.consumed = result.capture_changed;
            }
            NavigationEvent::Escape if ownership.keyboard_available => {
                result.capture_changed = self.pan.take().is_some();
                result.consumed = result.capture_changed;
            }
            NavigationEvent::PointerPressed {
                id,
                position,
                button: PointerButton::Middle,
                modifiers,
            } if self.pan.is_none() && !modifiers.any() && viewport.contains(position) => {
                self.pan = Some(PanCapture {
                    id,
                    last_position: position,
                    viewport,
                });
                result.consumed = true;
                result.capture_changed = true;
            }
            NavigationEvent::PointerMoved { id, position } => {
                if let Some(pan) = &mut self.pan
                    && pan.id == id
                {
                    result.consumed = true;
                    let delta = position - pan.last_position;
                    if position.is_finite() && delta.is_finite() {
                        result.view_changed = view.pan_by(delta);
                        pan.last_position = position;
                    }
                }
            }
            NavigationEvent::PointerReleased {
                id,
                position,
                button: PointerButton::Middle,
            } if self.pan.is_some_and(|pan| pan.id == id) => {
                let pan = self.pan.take().expect("matching pan capture");
                result.view_changed = view.pan_by(position - pan.last_position);
                result.consumed = true;
                result.capture_changed = true;
            }
            NavigationEvent::Wheel {
                position,
                delta_y,
                modifiers,
            } if self.pan.is_none()
                && !modifiers.any()
                && viewport.contains(position)
                && delta_y.is_finite()
                && delta_y != 0. =>
            {
                // Saturate large finite wheel deltas before exp, so they reach
                // the scale limit without overflowing or losing the anchor.
                let factor = (delta_y * 0.002).clamp(-32., 32.).exp();
                result.view_changed = view.zoom_at(viewport, position, factor);
                result.consumed = true;
            }
            _ => {}
        }
        result
    }
}

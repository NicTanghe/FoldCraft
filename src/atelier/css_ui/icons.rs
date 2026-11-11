//! Scalable vector graphics (SVG) icons for Studio V2 tool rails, transport controls,
//! view modes, and chrome elements.
//!
//! Provides crisp, lightweight inline SVG icons that match modern CAD / EXO Studio V2
//! styling without external image asset dependencies.

use cssimpler::core::Node;
use cssimpler::ui;

// ---------------------------------------------------------------------------
// Top Bar Transport Icons (14x14 viewport, 24x24 viewBox)
// ---------------------------------------------------------------------------

pub fn icon_play() -> Node {
    ui! {
        <svg viewBox="0 0 24 24" class="icon transport-icon" fill="currentColor">
            <polygon points="6,4 20,12 6,20" />
        </svg>
    }
}

pub fn icon_pause() -> Node {
    ui! {
        <svg viewBox="0 0 24 24" class="icon transport-icon" fill="currentColor">
            <rect x="6" y="4" width="4" height="16" rx="1" />
            <rect x="14" y="4" width="4" height="16" rx="1" />
        </svg>
    }
}

pub fn icon_step() -> Node {
    ui! {
        <svg viewBox="0 0 24 24" class="icon transport-icon" fill="currentColor">
            <polygon points="5,4 15,12 5,20" />
            <rect x="16" y="4" width="3" height="16" rx="1" />
        </svg>
    }
}

pub fn icon_reset() -> Node {
    ui! {
        <svg viewBox="0 0 24 24" class="icon transport-icon" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8" />
            <path d="M3 3v5h5" />
        </svg>
    }
}

pub fn icon_clean_view() -> Node {
    ui! {
        <svg viewBox="0 0 24 24" class="icon" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M2 12s3-7 10-7 10 7 10 7-3 7-10 7-10-7-10-7Z" />
            <circle cx="12" cy="12" r="3" />
        </svg>
    }
}

// ---------------------------------------------------------------------------
// View Mode Switcher Icons
// ---------------------------------------------------------------------------

pub fn icon_view_split() -> Node {
    ui! {
        <svg viewBox="0 0 24 24" class="icon" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <rect x="3" y="3" width="18" height="18" rx="2" />
            <line x1="12" y1="3" x2="12" y2="21" />
        </svg>
    }
}

pub fn icon_view_3d() -> Node {
    ui! {
        <svg viewBox="0 0 24 24" class="icon" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <polygon points="12,2 21,7.5 21,16.5 12,22 3,16.5 3,7.5" />
            <line x1="12" y1="22" x2="12" y2="12" />
            <line x1="12" y1="12" x2="21" y2="7.5" />
            <line x1="12" y1="12" x2="3" y2="7.5" />
        </svg>
    }
}

pub fn icon_view_2d() -> Node {
    ui! {
        <svg viewBox="0 0 24 24" class="icon" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <rect x="3" y="3" width="18" height="18" rx="2" />
            <line x1="3" y1="9" x2="21" y2="9" stroke-dasharray="2,2" stroke-width="1.5" />
            <line x1="3" y1="15" x2="21" y2="15" stroke-dasharray="2,2" stroke-width="1.5" />
            <line x1="9" y1="3" x2="9" y2="21" stroke-dasharray="2,2" stroke-width="1.5" />
            <line x1="15" y1="3" x2="15" y2="21" stroke-dasharray="2,2" stroke-width="1.5" />
        </svg>
    }
}

pub fn icon_palette() -> Node {
    ui! {
        <svg viewBox="0 0 24 24" class="icon" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <circle cx="11" cy="11" r="8" />
            <line x1="21" y1="21" x2="16.65" y2="16.65" />
        </svg>
    }
}

pub fn icon_sun() -> Node {
    ui! {
        <svg viewBox="0 0 24 24" class="icon" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <circle cx="12" cy="12" r="5" />
            <line x1="12" y1="1" x2="12" y2="3" />
            <line x1="12" y1="21" x2="12" y2="23" />
            <line x1="4.22" y1="4.22" x2="5.64" y2="5.64" />
            <line x1="18.36" y1="18.36" x2="19.78" y2="19.78" />
            <line x1="1" y1="12" x2="3" y2="12" />
            <line x1="21" y1="12" x2="23" y2="12" />
            <line x1="4.22" y1="19.78" x2="5.64" y2="18.36" />
            <line x1="18.36" y1="5.64" x2="19.78" y2="4.22" />
        </svg>
    }
}

pub fn icon_moon() -> Node {
    ui! {
        <svg viewBox="0 0 24 24" class="icon" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z" />
        </svg>
    }
}

// ---------------------------------------------------------------------------
// 3D Tool Rail Icons (18x18 viewport, 24x24 viewBox)
// ---------------------------------------------------------------------------

pub fn icon_gizmo() -> Node {
    ui! {
        <svg viewBox="0 0 24 24" class="icon tool-icon" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <polygon points="3,3 10,21 13,14 20,11" />
        </svg>
    }
}

pub fn icon_grab() -> Node {
    ui! {
        <svg viewBox="0 0 24 24" class="icon tool-icon" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <circle cx="12" cy="12" r="4" />
            <line x1="12" y1="2" x2="12" y2="6" />
            <line x1="12" y1="18" x2="12" y2="22" />
            <line x1="2" y1="12" x2="6" y2="12" />
            <line x1="18" y1="12" x2="22" y2="12" />
        </svg>
    }
}

pub fn icon_sew_3d() -> Node {
    ui! {
        <svg viewBox="0 0 24 24" class="icon tool-icon" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <line x1="4" y1="6" x2="20" y2="6" />
            <line x1="4" y1="18" x2="20" y2="18" />
            <line x1="7" y1="6" x2="7" y2="18" stroke-dasharray="2,2" stroke="#10b981" />
            <line x1="12" y1="6" x2="12" y2="18" stroke-dasharray="2,2" stroke="#10b981" />
            <line x1="17" y1="6" x2="17" y2="18" stroke-dasharray="2,2" stroke="#10b981" />
        </svg>
    }
}

pub fn icon_pin() -> Node {
    ui! {
        <svg viewBox="0 0 24 24" class="icon tool-icon" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <circle cx="12" cy="7" r="4" fill="currentColor" />
            <line x1="12" y1="11" x2="12" y2="21" stroke-width="2.5" />
            <line x1="8" y1="11" x2="16" y2="11" stroke-width="2" />
        </svg>
    }
}

pub fn icon_measure_3d() -> Node {
    ui! {
        <svg viewBox="0 0 24 24" class="icon tool-icon" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <rect x="3" y="7" width="18" height="10" rx="2" />
            <line x1="7" y1="7" x2="7" y2="11" />
            <line x1="11" y1="7" x2="11" y2="13" />
            <line x1="15" y1="7" x2="15" y2="11" />
            <line x1="19" y1="7" x2="19" y2="13" />
        </svg>
    }
}

pub fn icon_avatar_diag() -> Node {
    ui! {
        <svg viewBox="0 0 24 24" class="icon tool-icon" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <circle cx="12" cy="7" r="4" />
            <path d="M5.5 21a6.5 6.5 0 0 1 13 0" />
        </svg>
    }
}

// ---------------------------------------------------------------------------
// 2D Tool Rail Icons (18x18 viewport, 24x24 viewBox)
// ---------------------------------------------------------------------------

pub fn icon_edit_2d() -> Node {
    ui! {
        <svg viewBox="0 0 24 24" class="icon tool-icon" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M12 20h9" />
            <path d="M16.5 3.5a2.121 2.121 0 0 1 3 3L7 19l-4 1 1-4L16.5 3.5z" />
        </svg>
    }
}

pub fn icon_rect_2d() -> Node {
    ui! {
        <svg viewBox="0 0 24 24" class="icon tool-icon" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <rect x="4" y="5" width="16" height="14" rx="2" />
        </svg>
    }
}

pub fn icon_pen_2d() -> Node {
    ui! {
        <svg viewBox="0 0 24 24" class="icon tool-icon" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M12 19l7-7 3 3-7 7-3-3z" />
            <path d="M18 13l-1.5-7.5L2 2l3.5 14.5L13 18l5-5z" />
            <path d="M2 2l7.586 7.586" />
            <circle cx="11" cy="11" r="2" />
        </svg>
    }
}

pub fn icon_freehand_2d() -> Node {
    ui! {
        <svg viewBox="0 0 24 24" class="icon tool-icon" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M3 17c3.5-6 7.5-8 11-4s5 8 7 1" />
        </svg>
    }
}

pub fn icon_sew_2d() -> Node {
    ui! {
        <svg viewBox="0 0 24 24" class="icon tool-icon" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <line x1="4" y1="4" x2="20" y2="4" />
            <line x1="4" y1="20" x2="20" y2="20" />
            <line x1="8" y1="4" x2="8" y2="20" stroke-dasharray="3,3" stroke="#10b981" />
            <line x1="16" y1="4" x2="16" y2="20" stroke-dasharray="3,3" stroke="#10b981" />
        </svg>
    }
}

pub fn icon_split_2d() -> Node {
    ui! {
        <svg viewBox="0 0 24 24" class="icon tool-icon" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <line x1="3" y1="12" x2="9" y2="12" />
            <line x1="15" y1="12" x2="21" y2="12" />
            <circle cx="12" cy="12" r="3" fill="var(--accent)" stroke="currentColor" />
        </svg>
    }
}

pub fn icon_internal_2d() -> Node {
    ui! {
        <svg viewBox="0 0 24 24" class="icon tool-icon" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <line x1="5" y1="19" x2="19" y2="5" stroke-dasharray="3,3" stroke="var(--accent)" />
            <circle cx="5" cy="19" r="2" fill="currentColor" />
            <circle cx="19" cy="5" r="2" fill="currentColor" />
        </svg>
    }
}

pub fn icon_measure_2d() -> Node {
    ui! {
        <svg viewBox="0 0 24 24" class="icon tool-icon" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <line x1="4" y1="20" x2="20" y2="4" />
            <line x1="3" y1="15" x2="7" y2="19" />
            <line x1="7" y1="11" x2="11" y2="15" />
            <line x1="11" y1="7" x2="15" y2="11" />
            <line x1="17" y1="5" x2="21" y2="9" />
        </svg>
    }
}

pub fn icon_trim_2d() -> Node {
    ui! {
        <svg viewBox="0 0 24 24" class="icon tool-icon" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <circle cx="6" cy="6" r="3" />
            <circle cx="6" cy="18" r="3" />
            <line x1="20" y1="4" x2="8.12" y2="15.88" />
            <line x1="14.47" y1="14.48" x2="20" y2="20" />
            <line x1="8.12" y1="8.12" x2="12" y2="12" />
        </svg>
    }
}

pub fn icon_corner_2d() -> Node {
    ui! {
        <svg viewBox="0 0 24 24" class="icon tool-icon" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M4 4v9a7 7 0 0 0 7 7h9" />
            <line x1="4" y1="4" x2="4" y2="20" stroke-dasharray="2,2" stroke-opacity="0.4" />
            <line x1="4" y1="20" x2="20" y2="20" stroke-dasharray="2,2" stroke-opacity="0.4" />
        </svg>
    }
}

// ---------------------------------------------------------------------------
// Accordion & Disclosure Icons
// ---------------------------------------------------------------------------

pub fn icon_chevron_down() -> Node {
    ui! {
        <svg viewBox="0 0 24 24" class="icon accordion-chevron" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
            <polyline points="6 9 12 15 18 9" />
        </svg>
    }
}

pub fn icon_chevron_right() -> Node {
    ui! {
        <svg viewBox="0 0 24 24" class="icon accordion-chevron" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
            <polyline points="9 18 15 12 9 6" />
        </svg>
    }
}

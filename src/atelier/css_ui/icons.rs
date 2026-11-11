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
            <line x1="3" y1="10" x2="21" y2="10" stroke-dasharray="2,2" stroke-width="1.5" />
            <line x1="10" y1="3" x2="10" y2="21" stroke-dasharray="2,2" stroke-width="1.5" />
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
            <circle cx="12" cy="12" r="4" />
            <line x1="12" y1="2" x2="12" y2="4" />
            <line x1="12" y1="20" x2="12" y2="22" />
            <line x1="4.93" y1="4.93" x2="6.34" y2="6.34" />
            <line x1="17.66" y1="17.66" x2="19.07" y2="19.07" />
            <line x1="2" y1="12" x2="4" y2="12" />
            <line x1="20" y1="12" x2="22" y2="12" />
            <line x1="4.93" y1="19.07" x2="6.34" y2="17.66" />
            <line x1="17.66" y1="6.34" x2="19.07" y2="4.93" />
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

pub fn icon_save() -> Node {
    ui! {
        <svg viewBox="0 0 24 24" class="icon" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M19 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11l5 5v11a2 2 0 0 1-2 2z" />
            <polyline points="17 21 17 13 7 13 7 21" />
            <polyline points="7 3 7 8 15 8" />
        </svg>
    }
}

pub fn icon_export() -> Node {
    ui! {
        <svg viewBox="0 0 24 24" class="icon" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
            <polyline points="7 10 12 15 17 10" />
            <line x1="12" y1="15" x2="12" y2="3" />
        </svg>
    }
}

pub fn icon_day_night_switch(is_dark: bool) -> Node {
    let mode_class = if is_dark {
        "day-night-switch dark"
    } else {
        "day-night-switch light"
    };

    ui! {
        <div class={mode_class}>
            <div class="toggle__handler switch-thumb">
                <div class="crater crater--1"></div>
                <div class="crater crater--2"></div>
                <div class="crater crater--3"></div>
            </div>
            <div class="star star--1"></div>
            <div class="star star--2"></div>
            <div class="star star--3"></div>
            <div class="star star--4"></div>
            <div class="star star--5"></div>
            <div class="star star--6"></div>
        </div>
    }
}

// ---------------------------------------------------------------------------
// 3D Tool Rail Icons (18x18 viewport, 24x24 viewBox)
// ---------------------------------------------------------------------------

pub fn icon_gizmo() -> Node {
    ui! {
        <svg viewBox="0 0 24 24" class="icon tool-icon" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <polygon points="4,4 10,20 13,13 20,10" />
            <line x1="13" y1="13" x2="20" y2="20" />
        </svg>
    }
}

pub fn icon_grab() -> Node {
    ui! {
        <svg viewBox="0 0 24 24" class="icon tool-icon" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M18 11V6a2 2 0 0 0-4 0v5M14 10V4a2 2 0 0 0-4 0v7M10 10.5V6a2 2 0 0 0-4 0v8a7 7 0 0 0 14 0v-3a2 2 0 0 0-4 0" />
        </svg>
    }
}

pub fn icon_sew_3d() -> Node {
    ui! {
        <svg viewBox="0 0 24 24" class="icon tool-icon" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <line x1="5" y1="4" x2="5" y2="20" stroke-width="2.5" />
            <line x1="19" y1="4" x2="19" y2="20" stroke-width="2.5" />
            <path d="M5 8l14 3M5 12l14 3M5 16l14 3" stroke-width="1.8" />
        </svg>
    }
}

pub fn icon_pin() -> Node {
    ui! {
        <svg viewBox="0 0 24 24" class="icon tool-icon" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <circle cx="12" cy="6" r="3.5" fill="currentColor" />
            <line x1="12" y1="9.5" x2="12" y2="21" stroke-width="2.5" />
            <line x1="8" y1="10" x2="16" y2="10" stroke-width="2" />
        </svg>
    }
}

pub fn icon_measure_3d() -> Node {
    ui! {
        <svg viewBox="0 0 24 24" class="icon tool-icon" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <rect x="3" y="6" width="18" height="12" rx="2" />
            <line x1="7" y1="6" x2="7" y2="11" />
            <line x1="11" y1="6" x2="11" y2="14" />
            <line x1="15" y1="6" x2="15" y2="11" />
            <line x1="19" y1="6" x2="19" y2="14" />
        </svg>
    }
}

pub fn icon_avatar_diag() -> Node {
    ui! {
        <svg viewBox="0 0 24 24" class="icon tool-icon" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M8 3h8l2 6-3 4v8H9v-8L6 9l2-6z" />
            <line x1="12" y1="3" x2="12" y2="21" stroke-width="1.5" />
            <line x1="8" y1="8" x2="16" y2="8" stroke-width="1.5" />
        </svg>
    }
}

pub fn icon_cam_reset() -> Node {
    ui! {
        <svg viewBox="0 0 24 24" class="icon tool-icon" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <circle cx="12" cy="12" r="7" />
            <line x1="12" y1="2" x2="12" y2="6" />
            <line x1="12" y1="18" x2="12" y2="22" />
            <line x1="2" y1="12" x2="6" y2="12" />
            <line x1="18" y1="12" x2="22" y2="12" />
        </svg>
    }
}

pub fn icon_sidebar_toggle() -> Node {
    ui! {
        <svg viewBox="0 0 24 24" class="icon tool-icon" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <rect x="3" y="3" width="18" height="18" rx="2" />
            <line x1="9" y1="3" x2="9" y2="21" />
        </svg>
    }
}

// ---------------------------------------------------------------------------
// 2D Tool Rail Icons (18x18 viewport, 24x24 viewBox)
// ---------------------------------------------------------------------------

pub fn icon_edit_2d() -> Node {
    ui! {
        <svg viewBox="0 0 24 24" class="icon tool-icon" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M4 4l7 16 2.5-6.5L20 11 4 4z" />
            <circle cx="4" cy="4" r="2" fill="currentColor" />
        </svg>
    }
}

pub fn icon_rect_2d() -> Node {
    ui! {
        <svg viewBox="0 0 24 24" class="icon tool-icon" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <rect x="4" y="6" width="16" height="12" rx="1" />
            <circle cx="4" cy="6" r="1.5" fill="currentColor" />
            <circle cx="20" cy="6" r="1.5" fill="currentColor" />
            <circle cx="20" cy="18" r="1.5" fill="currentColor" />
            <circle cx="4" cy="18" r="1.5" fill="currentColor" />
        </svg>
    }
}

pub fn icon_pen_2d() -> Node {
    ui! {
        <svg viewBox="0 0 24 24" class="icon tool-icon" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M12 19l7-7 3 3-7 7-3-3z" />
            <path d="M18 13l-1.5-7.5L2 2l3.5 14.5L13 18l5-5z" />
            <circle cx="11" cy="11" r="2" fill="currentColor" />
        </svg>
    }
}

pub fn icon_freehand_2d() -> Node {
    ui! {
        <svg viewBox="0 0 24 24" class="icon tool-icon" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M4 18C8 18 8 6 12 6s4 12 8 12" />
            <circle cx="4" cy="18" r="2" fill="currentColor" />
            <circle cx="20" cy="18" r="2" fill="currentColor" />
        </svg>
    }
}

pub fn icon_sew_2d() -> Node {
    ui! {
        <svg viewBox="0 0 24 24" class="icon tool-icon" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <line x1="5" y1="4" x2="5" y2="20" stroke-width="2.5" />
            <line x1="19" y1="4" x2="19" y2="20" stroke-width="2.5" />
            <line x1="5" y1="8" x2="19" y2="8" stroke-width="1.8" stroke-dasharray="3,2" />
            <line x1="5" y1="12" x2="19" y2="12" stroke-width="1.8" stroke-dasharray="3,2" />
            <line x1="5" y1="16" x2="19" y2="16" stroke-width="1.8" stroke-dasharray="3,2" />
        </svg>
    }
}

pub fn icon_split_2d() -> Node {
    ui! {
        <svg viewBox="0 0 24 24" class="icon tool-icon" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <line x1="3" y1="12" x2="9" y2="12" stroke-width="2.5" />
            <line x1="15" y1="12" x2="21" y2="12" stroke-width="2.5" />
            <circle cx="12" cy="12" r="3" fill="currentColor" />
            <line x1="12" y1="6" x2="12" y2="18" stroke-width="1.5" />
        </svg>
    }
}

pub fn icon_internal_2d() -> Node {
    ui! {
        <svg viewBox="0 0 24 24" class="icon tool-icon" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <rect x="4" y="4" width="16" height="16" rx="2" stroke-width="1.5" />
            <line x1="7" y1="17" x2="17" y2="7" stroke-width="2" stroke-dasharray="3,3" />
            <circle cx="7" cy="17" r="1.5" fill="currentColor" />
            <circle cx="17" cy="7" r="1.5" fill="currentColor" />
        </svg>
    }
}

pub fn icon_measure_2d() -> Node {
    ui! {
        <svg viewBox="0 0 24 24" class="icon tool-icon" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <line x1="4" y1="12" x2="20" y2="12" stroke-width="2" />
            <line x1="4" y1="6" x2="4" y2="18" stroke-width="2" />
            <line x1="20" y1="6" x2="20" y2="18" stroke-width="2" />
            <path d="M7 9l-3 3 3 3M17 9l3 3-3 3" />
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
            <path d="M4 4v7a9 9 0 0 0 9 9h7" />
            <circle cx="13" cy="13" r="2" fill="currentColor" />
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

// ---------------------------------------------------------------------------
// Advanced CAD Vector Icons (Matching Clo3D / EXO Studio V2)
// ---------------------------------------------------------------------------

pub fn icon_select_2d() -> Node {
    ui! {
        <svg viewBox="0 0 24 24" class="icon tool-icon" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M3 3l7 18 2.5-7 7-2.5L3 3z" />
            <rect x="13" y="13" width="8" height="8" stroke-dasharray="2,2" stroke-width="1.5" />
        </svg>
    }
}

pub fn icon_curve_point_2d() -> Node {
    ui! {
        <svg viewBox="0 0 24 24" class="icon tool-icon" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M4 18C4 10 14 6 20 6" />
            <circle cx="12" cy="10" r="3" fill="#f59e0b" stroke="currentColor" />
            <line x1="8" y1="13" x2="16" y2="7" stroke-dasharray="2,2" stroke-width="1.5" />
            <circle cx="8" cy="13" r="1.5" fill="currentColor" />
            <circle cx="16" cy="7" r="1.5" fill="currentColor" />
        </svg>
    }
}

pub fn icon_polygon_2d() -> Node {
    ui! {
        <svg viewBox="0 0 24 24" class="icon tool-icon" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <polygon points="12,3 21,9 18,20 6,20 3,9" />
            <circle cx="12" cy="3" r="2" fill="currentColor" />
            <circle cx="21" cy="9" r="2" fill="currentColor" />
            <circle cx="18" cy="20" r="2" fill="currentColor" />
            <circle cx="6" cy="20" r="2" fill="currentColor" />
            <circle cx="3" cy="9" r="2" fill="currentColor" />
        </svg>
    }
}

pub fn icon_circle_2d() -> Node {
    ui! {
        <svg viewBox="0 0 24 24" class="icon tool-icon" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <ellipse cx="12" cy="12" rx="9" ry="8" />
            <circle cx="12" cy="4" r="1.5" fill="currentColor" />
            <circle cx="21" cy="12" r="1.5" fill="currentColor" />
            <circle cx="12" cy="20" r="1.5" fill="currentColor" />
            <circle cx="3" cy="12" r="1.5" fill="currentColor" />
        </svg>
    }
}

pub fn icon_dart_2d() -> Node {
    ui! {
        <svg viewBox="0 0 24 24" class="icon tool-icon" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <line x1="3" y1="5" x2="21" y2="5" stroke-width="2" />
            <polyline points="8,5 12,19 16,5" stroke-width="2" />
            <line x1="12" y1="5" x2="12" y2="19" stroke-dasharray="2,2" stroke-width="1.5" />
            <circle cx="12" cy="19" r="2" fill="#f59e0b" />
        </svg>
    }
}

pub fn icon_notch_2d() -> Node {
    ui! {
        <svg viewBox="0 0 24 24" class="icon tool-icon" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <line x1="3" y1="12" x2="21" y2="12" stroke-width="2.5" />
            <line x1="12" y1="6" x2="12" y2="12" stroke-width="2.5" stroke="#f59e0b" />
            <polyline points="9,6 12,6 15,6" stroke-width="2" stroke="#f59e0b" />
            <circle cx="12" cy="12" r="2" fill="#f59e0b" />
        </svg>
    }
}

pub fn icon_seam_allowance_2d() -> Node {
    ui! {
        <svg viewBox="0 0 24 24" class="icon tool-icon" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <rect x="3" y="3" width="18" height="18" rx="2" stroke-width="2" />
            <rect x="7" y="7" width="10" height="10" rx="1" stroke-dasharray="2,2" stroke-width="1.5" />
            <line x1="17" y1="7" x2="21" y2="3" stroke-width="1.5" stroke="#f59e0b" />
        </svg>
    }
}

pub fn icon_pleat_2d() -> Node {
    ui! {
        <svg viewBox="0 0 24 24" class="icon tool-icon" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <line x1="3" y1="4" x2="21" y2="4" stroke-width="2" />
            <line x1="3" y1="20" x2="21" y2="20" stroke-width="2" />
            <polyline points="6,4 8,20 12,4 14,20 18,4" stroke-width="1.8" />
            <line x1="8" y1="4" x2="8" y2="20" stroke-dasharray="2,2" stroke-width="1.2" stroke="#f59e0b" />
            <line x1="14" y1="4" x2="14" y2="20" stroke-dasharray="2,2" stroke-width="1.2" stroke="#f59e0b" />
        </svg>
    }
}

pub fn icon_free_sew_3d() -> Node {
    ui! {
        <svg viewBox="0 0 24 24" class="icon tool-icon" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M4 19c4-6 6-6 10-12l2-3 2 1-1 3.5c-2 6-4 6-9 12.5" />
            <line x1="17" y1="5" x2="21" y2="2" stroke="#f59e0b" stroke-width="1.5" />
            <circle cx="19" cy="4" r="1" fill="#f59e0b" />
            <path d="M6 16c2 1 4 0 6 2s3 2 5 1" stroke-dasharray="2,2" stroke-width="1.5" />
        </svg>
    }
}

pub fn icon_cloth_pull() -> Node {
    ui! {
        <svg viewBox="0 0 24 24" class="icon tool-icon" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M12 2v6m0 0l-3-3m3 3l3-3" stroke="#f59e0b" stroke-width="2" />
            <path d="M4 12c4-2 8 4 12 0s4-2 4-2v6c0 0-3 2-6 1s-6-3-10 0v-5z" stroke-width="1.8" />
        </svg>
    }
}

pub fn icon_dress_form() -> Node {
    ui! {
        <svg viewBox="0 0 24 24" class="icon tool-icon" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M8 2h8l2 5-3 5v8H9v-8L6 7l2-5z" />
            <line x1="12" y1="20" x2="12" y2="23" stroke-width="2.5" />
            <line x1="8" y1="23" x2="16" y2="23" stroke-width="2.5" />
            <line x1="8" y1="7" x2="16" y2="7" stroke-width="1.5" stroke="#f59e0b" />
        </svg>
    }
}

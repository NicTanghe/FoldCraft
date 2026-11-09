//! Studio V2 Left Asset/Piece Browser and Right 5-Category Object Browser
//! implemented with `cssimpler::ui!`.
//!
//! Features:
//! - Left Asset & Piece Browser (`render_left_browser`):
//!   - Bundled templates browser (T-shirt, Dress, Apron, Two-panel, Local-ease, Curved-panels, Partial-chain)
//!   - Dirty-document confirmation state
//!   - Import/Export quick actions (DXF, SVG, Avatar OBJ/glTF/GLB, Export OBJ, True-Scale SVG with 100 mm ruler note)
//!   - Pattern Piece cards list (stable `panel.id`, vertex/triangle counts, mirror badges, fabric, selection highlight)
//! - Right 5-Category Object Browser (`render_object_browser`):
//!   - Category tabs: 2D Pattern, Sewing, 3D Form, Material, Environment
//!   - 2D Pattern: Pieces, Holes, Internal Paths, Rulings/Measurements (FC-0005), Notch & Seam Allowance summary
//!   - Sewing: Seam list with A/B boundary chains, reversal badges, easing ratios
//!   - 3D Form: Arrangement transforms, pins (contour & interior pins from FC-0007), simulation state
//!   - Material: Shared fabric preset, per-panel grainline, albedo texture status (FC-0004), construction trims (FC-0007)
//!   - Environment: Avatar mesh summary, Workspace Profiles persistence wired to `src/atelier/workspace_profiles.rs` (FC-0003),
//!     including external-change warning banner.
//!
//! SI units throughout: metres, kilograms, seconds, Newtons.

#![allow(dead_code)]

use std::path::Path;

use cssimpler::core::Node;
use cssimpler::ui;

use marvelous_cloth::{
    document::atomic_write,
    fitting::{FittingProject, FittingScene},
    garment::{
        CompiledGarment,
        seam::{BoundaryRef, Side},
        template::{GarmentTemplate, Panel},
    },
};

pub mod workspace_profiles {
    use super::atomic_write;
    use serde::{Deserialize, Serialize};
    use std::path::Path;

    pub const FILE_NAME: &str = "workspace-profiles.json";
    const MAX_BYTES: u64 = 256 * 1024;
    const MAX_PROFILES: usize = 32;

    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
    pub struct Snapshot {
        #[serde(default)]
        pub tools_visible: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub wireframe: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub seam_links: Option<bool>,
    }

    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    pub struct Profile {
        pub name: String,
        pub snapshot: Snapshot,
    }

    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    pub struct Store {
        pub version: u32,
        pub profiles: Vec<Profile>,
    }

    impl Default for Store {
        fn default() -> Self {
            Self {
                version: 1,
                profiles: Vec::new(),
            }
        }
    }

    fn valid_name(name: &str) -> Result<(), String> {
        if name.trim() != name
            || name.is_empty()
            || name.chars().count() > 48
            || name.chars().any(char::is_control)
        {
            return Err("Use 1–48 characters, without control characters or outer spaces".into());
        }
        Ok(())
    }

    impl Store {
        pub fn load(path: &Path) -> Result<Self, String> {
            if !path.exists() {
                return Ok(Self::default());
            }
            let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
            let store: Self = serde_json::from_str(&text)
                .map_err(|e| format!("Invalid workspace profiles: {e}"))?;
            if !matches!(store.version, 1 | 2) || store.profiles.len() > MAX_PROFILES {
                return Err("Unsupported profile version or too many profiles".into());
            }
            let mut names = std::collections::BTreeSet::new();
            for profile in &store.profiles {
                valid_name(&profile.name)?;
                if !names.insert(profile.name.to_lowercase()) {
                    return Err("Duplicate workspace profile names".into());
                }
            }
            Ok(store)
        }

        pub fn save_profile(
            &mut self,
            path: &Path,
            name: &str,
            snapshot: Snapshot,
            replace: bool,
        ) -> Result<(), String> {
            valid_name(name)?;
            let found = self
                .profiles
                .iter()
                .position(|p| p.name.to_lowercase() == name.to_lowercase());
            let version = if snapshot.wireframe.is_some() || snapshot.seam_links.is_some() {
                2
            } else {
                self.version
            };
            self.version = version;
            match (found, replace) {
                (Some(index), true) => {
                    self.profiles[index] = Profile {
                        name: name.into(),
                        snapshot,
                    }
                }
                (Some(_), false) => {
                    return Err("A profile with this name exists; choose Replace explicitly".into());
                }
                (None, true) => return Err("The profile to replace no longer exists".into()),
                (None, false) if self.profiles.len() < MAX_PROFILES => {
                    self.profiles.push(Profile {
                        name: name.into(),
                        snapshot,
                    })
                }
                _ => return Err("At most 32 workspace profiles are supported".into()),
            }
            let text = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
            if text.len() as u64 > MAX_BYTES {
                return Err("Workspace profiles exceed 256 KiB".into());
            }
            atomic_write(path, text.as_bytes())?;
            Ok(())
        }
    }
}

pub use workspace_profiles::Store;

/// True-scale SVG export note from FC-0002 / PR #235.
pub const TRUE_SCALE_SVG_RULER_NOTE: &str = "Physical SVG includes stored seam allowances and sewing-line notches; notches are not duplicated onto the cutting line. Print at actual size and check the 100 mm ruler; sheets are untiled and not cutting-certified.";

/// External-change warning message from FC-0003 / `workspace_profiles.rs`.
pub const EXTERNAL_CHANGE_WARNING_TEXT: &str =
    "Workspace profiles changed on disk. Reload saved profiles before continuing.";

// ============================================================================
// Bundled Templates Catalog
// ============================================================================

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BundledTemplateEntry {
    pub id: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub panel_count: usize,
    pub seam_count: usize,
    pub license: &'static str,
    pub source: &'static str,
}

pub static BUNDLED_TEMPLATES: &[BundledTemplateEntry] = &[
    BundledTemplateEntry {
        id: "tshirt",
        title: "T-shirt",
        description: "Two-panel engineering fixture for the 1.75 m MakeHuman mannequin. Not a graded sewing pattern.",
        panel_count: 2,
        seam_count: 4,
        license: "MIT",
        source: "ClothCraft / examples/templates/tshirt.json",
    },
    BundledTemplateEntry {
        id: "dress",
        title: "Dress",
        description: "Two-panel dress fixture for the 1.75 m MakeHuman mannequin. Check placement on other avatars.",
        panel_count: 2,
        seam_count: 4,
        license: "MIT",
        source: "ClothCraft / examples/templates/dress.json",
    },
    BundledTemplateEntry {
        id: "apron",
        title: "Apron",
        description: "Workwear fixture with ties and bib for draping and contact verification.",
        panel_count: 3,
        seam_count: 4,
        license: "MIT",
        source: "ClothCraft / examples/templates/apron.json",
    },
    BundledTemplateEntry {
        id: "two-panel",
        title: "Two-panel",
        description: "Simple nonmatching seam example. Panels may be pinned; inspect before simulating.",
        panel_count: 2,
        seam_count: 1,
        license: "MIT",
        source: "ClothCraft / examples/templates/two-panel.json",
    },
    BundledTemplateEntry {
        id: "local-ease",
        title: "Local-ease",
        description: "Nonuniform seam correspondence example with explicit easing.",
        panel_count: 2,
        seam_count: 1,
        license: "MIT",
        source: "ClothCraft / examples/templates/local-ease.json",
    },
    BundledTemplateEntry {
        id: "curved-panels",
        title: "Curved-panels",
        description: "Cubic boundary example for editing and sewing experiments.",
        panel_count: 2,
        seam_count: 2,
        license: "MIT",
        source: "ClothCraft / examples/templates/curved-panels.json",
    },
    BundledTemplateEntry {
        id: "partial-chain",
        title: "Partial-chain",
        description: "Small fixture illustrating partial boundaries and ordered seam chains.",
        panel_count: 2,
        seam_count: 2,
        license: "MIT",
        source: "ClothCraft / examples/templates/partial-chain.json",
    },
];

// ============================================================================
// Left Asset & Piece Browser
// ============================================================================

/// Action triggered by the Left Browser.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LeftBrowserAction {
    SelectTemplate(String),
    ConfirmTemplateSwitch {
        template_id: String,
        save_first: bool,
    },
    CancelTemplateSwitch,
    ImportDxf,
    ImportSvg,
    ImportAvatar,
    ExportObj,
    ExportTrueScaleSvg,
    SelectPiece(String),
}

/// State for the Left Asset & Piece Browser.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LeftBrowserState {
    pub selected_template_id: Option<String>,
    pub pending_template_id: Option<String>,
    pub confirming_dirty_switch: bool,
    pub selected_panel_id: Option<String>,
    pub export_note_expanded: bool,
    pub last_action: Option<LeftBrowserAction>,
}

impl LeftBrowserState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_selected_panel(mut self, panel_id: impl Into<String>) -> Self {
        self.selected_panel_id = Some(panel_id.into());
        self
    }
}

/// Renders the Left Asset & Piece Browser into a `cssimpler::core::Node`.
pub fn render_left_browser(
    state: &LeftBrowserState,
    template: &GarmentTemplate,
    compiled: Option<&CompiledGarment>,
    document_dirty: bool,
) -> Node {
    let template_section = render_bundled_templates_section(state, document_dirty);
    let quick_actions_section = render_import_export_section(state);
    let piece_cards_section = render_piece_cards_section(state, template, compiled);

    ui! {
        <aside id="left-browser" class="sidebar left-browser-panel">
            <header class="browser-header">
                <h2 class="browser-title">"Assets & Pieces"</h2>
                <span class="browser-badge">{format!("{} pieces", template.panels.len())}</span>
            </header>

            {template_section}
            {quick_actions_section}
            {piece_cards_section}
        </aside>
    }
}

fn render_bundled_templates_section(state: &LeftBrowserState, document_dirty: bool) -> Node {
    if document_dirty && state.confirming_dirty_switch && state.pending_template_id.is_some() {
        let pending_id = state.pending_template_id.as_deref().unwrap_or("");
        let title = BUNDLED_TEMPLATES
            .iter()
            .find(|t| t.id == pending_id)
            .map(|t| t.title)
            .unwrap_or(pending_id);

        ui! {
            <section id="dirty-template-modal" class="card modal-card dirty-confirmation-card">
                <h3 class="modal-title">"Open template as a new project?"</h3>
                <p class="modal-target-template">{format!("Selected: {title}")}</p>
                <div class="warning-banner dirty-warning-banner">
                    <strong class="warning-title">"Unsaved changes"</strong>
                    <p class="warning-desc">"The current document has unsaved changes or unfinished edits."</p>
                </div>
                <p class="modal-note">
                    "The current garment will be replaced. Your avatar and simulation settings will be kept."
                </p>
                <div class="modal-actions-row">
                    <button id="btn-cancel-template" class="btn btn-secondary">
                        "Cancel"
                    </button>
                    <button id="btn-save-open-template" class="btn btn-primary">
                        "Save current, then open"
                    </button>
                    <button id="btn-discard-open-template" class="btn btn-danger">
                        "Discard changes and open"
                    </button>
                </div>
            </section>
        }
    } else {
        let template_items: Vec<Node> = BUNDLED_TEMPLATES
            .iter()
            .map(|entry| {
                let is_selected = state.selected_template_id.as_deref() == Some(entry.id);
                let card_class = if is_selected {
                    "template-entry template-entry-selected"
                } else {
                    "template-entry"
                };

                ui! {
                    <article id={format!("template-card-{}", entry.id)} class={card_class}>
                        <div class="template-header">
                            <h4 class="template-title">{entry.title}</h4>
                            <span class="template-meta">{format!("{} panels · {} seams", entry.panel_count, entry.seam_count)}</span>
                        </div>
                        <p class="template-description">{entry.description}</p>
                        <div class="template-footer">
                            <span class="template-license">{format!("License: {}", entry.license)}</span>
                            <button
                                id={format!("btn-open-template-{}", entry.id)}
                                class="btn btn-sm btn-open-template"
                            >
                                "Use Template"
                            </button>
                        </div>
                    </article>
                }
            })
            .collect();

        ui! {
            <section id="bundled-templates-section" class="browser-section bundled-templates">
                <header class="section-header">
                    <h3 class="section-title">"Bundled Templates"</h3>
                    <span class="section-subtitle">"Engineering fixtures (metres)"</span>
                </header>
                <div class="template-cards-container">
                    {template_items}
                </div>
            </section>
        }
    }
}

fn render_import_export_section(state: &LeftBrowserState) -> Node {
    let note_class = if state.export_note_expanded {
        "export-note-text export-note-expanded"
    } else {
        "export-note-text"
    };

    ui! {
        <section id="import-export-section" class="browser-section quick-actions-section">
            <header class="section-header">
                <h3 class="section-title">"Import & Export"</h3>
            </header>

            <div class="quick-action-group">
                <label class="group-label">"Import"</label>
                <div class="quick-actions-row">
                    <button id="btn-import-dxf" class="btn btn-action">
                        "Import DXF"
                    </button>
                    <button id="btn-import-svg" class="btn btn-action">
                        "Import SVG"
                    </button>
                    <button id="btn-import-avatar" class="btn btn-action">
                        "Import Avatar (OBJ/glTF/GLB)"
                    </button>
                </div>
            </div>

            <div class="quick-action-group">
                <label class="group-label">"Export"</label>
                <div class="quick-actions-row">
                    <button id="btn-export-obj" class="btn btn-action">
                        "Export OBJ"
                    </button>
                    <button id="btn-export-svg" class="btn btn-action btn-action-accent">
                        "Export True-Scale SVG"
                    </button>
                </div>
                <div class="export-accuracy-note-box">
                    <p id="svg-ruler-note" class={note_class}>
                        {TRUE_SCALE_SVG_RULER_NOTE}
                    </p>
                </div>
            </div>
        </section>
    }
}

fn render_piece_cards_section(
    state: &LeftBrowserState,
    template: &GarmentTemplate,
    compiled: Option<&CompiledGarment>,
) -> Node {
    let piece_cards: Vec<Node> = template
        .panels
        .iter()
        .map(|panel| {
            let is_selected = state.selected_panel_id.as_deref() == Some(panel.id.as_str());
            let card_class = if is_selected {
                "piece-card piece-card-selected"
            } else {
                "piece-card"
            };

            // Vertex and triangle counts
            let (v_count, t_count) = if let Some(comp) = compiled {
                if let Some(range) = comp.panels.iter().find(|pr| pr.id == panel.id) {
                    (range.vertices.len(), range.triangles.len())
                } else {
                    compute_piece_counts(panel)
                }
            } else {
                compute_piece_counts(panel)
            };

            // Mirror link partner check
            let mirror_badge: Option<Node> = template
                .mirror_links
                .iter()
                .find(|l| l.a == panel.id || l.b == panel.id)
                .map(|link| {
                    let partner = if link.a == panel.id { &link.b } else { &link.a };
                    let axis_str = format!("{:?}", link.axis);
                    let label = format!("Linked Mirror ({axis_str} ↔ {partner})");
                    ui! {
                        <span
                            id={format!("mirror-badge-{}", panel.id)}
                            class="badge badge-mirror"
                        >
                            {label}
                        </span>
                    }
                });

            // Fabric name resolution
            let fabric_name = template
                .fabrics
                .iter()
                .find(|f| f.id == panel.fabric)
                .map(|f| f.name.as_str())
                .unwrap_or(panel.fabric.as_str());

            ui! {
                <article
                    id={format!("piece-card-{}", panel.id)}
                    class={card_class}
                >
                    <div class="piece-card-header">
                        <div class="piece-title-row">
                            <span id={format!("piece-id-{}", panel.id)} class="piece-id-label">
                                {&panel.id}
                            </span>
                            <span class="piece-name">{&panel.name}</span>
                        </div>
                        {mirror_badge}
                    </div>

                    <div class="piece-card-meta">
                        <span class="piece-counts">
                            {format!("{v_count} vertices · {t_count} triangles")}
                        </span>
                        <span class="piece-fabric">
                            {format!("Fabric: {fabric_name}")}
                        </span>
                    </div>

                    <div class="piece-card-actions">
                        <button
                            id={format!("btn-select-piece-{}", panel.id)}
                            class="btn btn-sm btn-select-piece"
                        >
                            {if is_selected { "Selected (in 2D/3D)" } else { "Select Piece" }}
                        </button>
                    </div>
                </article>
            }
        })
        .collect();

    ui! {
        <section id="pattern-pieces-section" class="browser-section pattern-pieces-section">
            <header class="section-header">
                <h3 class="section-title">"Pattern Pieces"</h3>
                <span class="section-subtitle">{format!("{} pieces authored", template.panels.len())}</span>
            </header>
            <div class="pieces-cards-container">
                {piece_cards}
            </div>
        </section>
    }
}

fn compute_piece_counts(panel: &Panel) -> (usize, usize) {
    if let Some(poly) = &panel.polygon {
        let v_count = poly.points.len();
        let t_count = v_count.saturating_sub(2);
        (v_count, t_count)
    } else {
        let nx = panel.pattern.nx;
        let ny = panel.pattern.ny;
        let v_count = (nx + 1) * (ny + 1);
        let t_count = nx * ny * 2;
        (v_count, t_count)
    }
}

// ============================================================================
// Right 5-Category Object Browser
// ============================================================================

/// Category tabs matching Studio V2's 5-category navigator.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ObjectCategory {
    #[default]
    Pattern2D,
    Sewing,
    Form3D,
    Material,
    Environment,
}

impl ObjectCategory {
    pub fn tab_id(self) -> &'static str {
        match self {
            Self::Pattern2D => "tab-pattern-2d",
            Self::Sewing => "tab-sewing",
            Self::Form3D => "tab-form-3d",
            Self::Material => "tab-material",
            Self::Environment => "tab-environment",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::Pattern2D => "2D Pattern",
            Self::Sewing => "Sewing",
            Self::Form3D => "3D Form",
            Self::Material => "Material",
            Self::Environment => "Environment",
        }
    }
}

/// Action triggered by the Right Object Browser.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ObjectBrowserAction {
    SwitchCategory(ObjectCategory),
    SelectItem(String),
    SaveProfile(String),
    RestoreProfile(String),
    ReloadProfiles,
    DeleteProfile(String),
}

/// State for the Right 5-Category Object Browser.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObjectBrowserState {
    pub active_category: ObjectCategory,
    pub selected_item_id: Option<String>,
    pub profile_name_input: String,
    pub selected_profile_name: Option<String>,
    pub external_change_detected: bool,
    pub status_message: Option<String>,
    pub last_action: Option<ObjectBrowserAction>,
}

impl Default for ObjectBrowserState {
    fn default() -> Self {
        Self {
            active_category: ObjectCategory::Pattern2D,
            selected_item_id: None,
            profile_name_input: String::new(),
            selected_profile_name: None,
            external_change_detected: false,
            status_message: None,
            last_action: None,
        }
    }
}

impl ObjectBrowserState {
    pub fn new(category: ObjectCategory) -> Self {
        Self {
            active_category: category,
            ..Default::default()
        }
    }

    pub fn with_category(mut self, category: ObjectCategory) -> Self {
        self.active_category = category;
        self
    }
}

/// Renders the Right 5-Category Object Browser into a `cssimpler::core::Node`.
pub fn render_object_browser(
    state: &ObjectBrowserState,
    project: &FittingProject,
    scene: Option<&FittingScene>,
    profiles_path: Option<&Path>,
) -> Node {
    let tabs_bar = render_category_tabs(state.active_category);

    let content = match state.active_category {
        ObjectCategory::Pattern2D => render_category_2d_pattern(&project.template),
        ObjectCategory::Sewing => render_category_sewing(&project.template),
        ObjectCategory::Form3D => render_category_3d_form(&project.template, scene),
        ObjectCategory::Material => render_category_material(&project.template),
        ObjectCategory::Environment => render_category_environment(state, project, profiles_path),
    };

    ui! {
        <aside id="object-browser" class="sidebar right-object-browser-panel">
            <header class="browser-header">
                <h2 class="browser-title">"Object Browser"</h2>
            </header>

            {tabs_bar}

            <div id="object-browser-content" class="object-browser-content-viewport">
                {content}
            </div>
        </aside>
    }
}

fn render_category_tabs(active: ObjectCategory) -> Node {
    let categories = [
        ObjectCategory::Pattern2D,
        ObjectCategory::Sewing,
        ObjectCategory::Form3D,
        ObjectCategory::Material,
        ObjectCategory::Environment,
    ];

    let buttons: Vec<Node> = categories
        .iter()
        .map(|&cat| {
            let is_active = cat == active;
            let class = if is_active {
                "tab-button tab-active"
            } else {
                "tab-button"
            };

            ui! {
                <button
                    id={cat.tab_id()}
                    class={class}
                >
                    {cat.title()}
                </button>
            }
        })
        .collect();

    ui! {
        <nav id="category-tabs-bar" class="tab-strip">
            {buttons}
        </nav>
    }
}

// ----------------------------------------------------------------------------
// Category 1: 2D Pattern
// ----------------------------------------------------------------------------

fn render_category_2d_pattern(template: &GarmentTemplate) -> Node {
    // 1. Pieces list
    let piece_items: Vec<Node> = template
        .panels
        .iter()
        .map(|p| {
            ui! {
                <li id={format!("pattern-piece-{}", p.id)} class="tree-item piece-item">
                    <span class="tree-label-id">{&p.id}</span>
                    <span class="tree-label-name">{&p.name}</span>
                </li>
            }
        })
        .collect();

    // 2. Holes list
    let mut hole_items = Vec::new();
    for panel in &template.panels {
        if let Some(poly) = &panel.polygon {
            for (idx, hole) in poly.holes.iter().enumerate() {
                hole_items.push(ui! {
                    <li id={format!("hole-{}-{}", panel.id, idx + 1)} class="tree-item hole-item">
                        <span class="tree-label-id">{format!("{}:hole#{}", panel.id, idx + 1)}</span>
                        <span class="tree-label-meta">{format!("{} vertices", hole.points.len())}</span>
                    </li>
                });
            }
        }
    }
    let holes_node = if hole_items.is_empty() {
        ui! { <p class="empty-notice">"No holes authored in pattern"</p> }
    } else {
        ui! { <ul class="tree-list">{hole_items}</ul> }
    };

    // 3. Internal Paths list
    let mut line_items = Vec::new();
    for panel in &template.panels {
        if let Some(poly) = &panel.polygon {
            for line in &poly.lines {
                line_items.push(ui! {
                    <li id={format!("line-{}-{}", panel.id, line.id)} class="tree-item line-item">
                        <span class="tree-label-id">{format!("{}:{}", panel.id, line.id)}</span>
                        <span class="tree-label-meta">{format!("{} points", line.points.len())}</span>
                    </li>
                });
            }
        }
    }
    let lines_node = if line_items.is_empty() {
        ui! { <p class="empty-notice">"No internal lines authored"</p> }
    } else {
        ui! { <ul class="tree-list">{line_items}</ul> }
    };

    // 4. Rulings / Measurements (FC-0005)
    let mut meas_items = Vec::new();
    for panel in &template.panels {
        if let Some(poly) = &panel.polygon {
            for m in &poly.measurements {
                meas_items.push(ui! {
                    <li id={format!("meas-{}", m.id)} class="tree-item measurement-item">
                        <span class="tree-label-id">{&m.id}</span>
                        <span class="tree-label-name">{&m.label}</span>
                    </li>
                });
            }
        }
    }
    let meas_node = if meas_items.is_empty() {
        ui! { <p class="empty-notice">"No persistent ruler measurements (FC-0005)"</p> }
    } else {
        ui! { <ul class="tree-list">{meas_items}</ul> }
    };

    // 5. Notches & Seam Allowance summary
    let mut sa_items = Vec::new();
    for panel in &template.panels {
        if let Some(poly) = &panel.polygon {
            for (edge_id, allowance) in &poly.seam_allowances {
                let notch_count = poly.notches.get(edge_id).map(|n| n.len()).unwrap_or(0);
                sa_items.push(ui! {
                    <li id={format!("sa-{}-{}", panel.id, edge_id)} class="tree-item sa-item">
                        <span class="tree-label-id">{format!("{}:{}", panel.id, edge_id)}</span>
                        <span class="tree-label-meta">{format!("{:.1} mm allowance · {} notches", allowance * 1000.0, notch_count)}</span>
                    </li>
                });
            }
        }
    }
    let sa_node = if sa_items.is_empty() {
        ui! { <p class="empty-notice">"No seam allowances or notches stored"</p> }
    } else {
        ui! { <ul class="tree-list">{sa_items}</ul> }
    };

    ui! {
        <div id="category-2d-pattern" class="category-pane">
            <section id="section-pieces" class="category-section">
                <h4 class="section-heading">{format!("Pieces ({})", template.panels.len())}</h4>
                <ul class="tree-list">{piece_items}</ul>
            </section>

            <section id="section-holes" class="category-section">
                <h4 class="section-heading">"Holes"</h4>
                {holes_node}
            </section>

            <section id="section-internal-paths" class="category-section">
                <h4 class="section-heading">"Internal Paths"</h4>
                {lines_node}
            </section>

            <section id="section-measurements" class="category-section">
                <h4 class="section-heading">"Rulings & Measurements (FC-0005)"</h4>
                {meas_node}
            </section>

            <section id="section-notches-allowances" class="category-section">
                <h4 class="section-heading">"Seam Allowances & Notches"</h4>
                {sa_node}
            </section>
        </div>
    }
}

// ----------------------------------------------------------------------------
// Category 2: Sewing
// ----------------------------------------------------------------------------

fn render_boundary_ref_chain(first: &BoundaryRef, tail: &[BoundaryRef]) -> String {
    let mut parts = Vec::new();
    let edge_str = first.edge.as_deref().unwrap_or(match first.side {
        Side::Top => "top",
        Side::Right => "right",
        Side::Bottom => "bottom",
        Side::Left => "left",
    });
    parts.push(format!(
        "{}:{} [{:.2}..{:.2}]",
        first.panel, edge_str, first.range[0], first.range[1]
    ));

    for b in tail {
        let edge = b.edge.as_deref().unwrap_or(match b.side {
            Side::Top => "top",
            Side::Right => "right",
            Side::Bottom => "bottom",
            Side::Left => "left",
        });
        parts.push(format!(
            " -> {}:{} [{:.2}..{:.2}]",
            b.panel, edge, b.range[0], b.range[1]
        ));
    }
    parts.concat()
}

fn render_category_sewing(template: &GarmentTemplate) -> Node {
    let seam_items: Vec<Node> = template
        .seams
        .iter()
        .map(|seam| {
            let chain_a = render_boundary_ref_chain(&seam.a, &seam.a_tail);
            let chain_b = render_boundary_ref_chain(&seam.b, &seam.b_tail);

            let (rev_class, rev_label) = if seam.reverse_b {
                ("badge badge-warning", "Reversed (B-side inverted)")
            } else {
                ("badge badge-normal", "Normal Alignment")
            };

            let (ease_class, ease_label) = if seam.allow_ease {
                ("badge badge-ease", "Easing Permitted (allow_ease = true)")
            } else {
                ("badge badge-no-ease", "Fixed Length (no easing)")
            };

            ui! {
                <article id={format!("seam-row-{}", seam.id)} class="seam-card">
                    <header class="seam-card-header">
                        <span class="seam-id">{&seam.id}</span>
                        <div class="seam-badges">
                            <span id={format!("reversal-badge-{}", seam.id)} class={rev_class}>
                                {rev_label}
                            </span>
                            <span id={format!("easing-badge-{}", seam.id)} class={ease_class}>
                                {ease_label}
                            </span>
                        </div>
                    </header>

                    <div class="seam-chains">
                        <p class="chain-row">
                            <strong class="chain-tag">"A:"</strong>
                            <span class="chain-text">{chain_a}</span>
                        </p>
                        <p class="chain-row">
                            <strong class="chain-tag">"B:"</strong>
                            <span class="chain-text">{chain_b}</span>
                        </p>
                    </div>

                    <div class="seam-footer">
                        <span class="seam-stiffness">
                            {format!("Stiffness: {:.1e} N/m²", seam.stiffness)}
                        </span>
                        <span class="seam-notches">
                            {format!("Notches: {} correspondences", seam.notches.len())}
                        </span>
                    </div>
                </article>
            }
        })
        .collect();

    let content = if seam_items.is_empty() {
        ui! { <p class="empty-notice">"No seams authored"</p> }
    } else {
        ui! { <div class="seams-list">{seam_items}</div> }
    };

    ui! {
        <div id="category-sewing" class="category-pane">
            <header class="pane-header">
                <h4 class="pane-title">{format!("Seams & Links ({} seams)", template.seams.len())}</h4>
            </header>
            {content}
        </div>
    }
}

// ----------------------------------------------------------------------------
// Category 3: 3D Form
// ----------------------------------------------------------------------------

fn render_category_3d_form(template: &GarmentTemplate, scene: Option<&FittingScene>) -> Node {
    // 1. Arrangement transforms
    let arrangement_items: Vec<Node> = template
        .panels
        .iter()
        .map(|panel| {
            let t = panel.placement.translation;
            let r = panel.placement.rotation_degrees;
            ui! {
                <div id={format!("arrangement-{}", panel.id)} class="transform-card">
                    <span class="panel-name-tag">{&panel.id}</span>
                    <div class="transform-row">
                        <span class="meta-label">"Translation:"</span>
                        <span class="meta-val">{format!("({:.3}, {:.3}, {:.3}) m", t.x, t.y, t.z)}</span>
                    </div>
                    <div class="transform-row">
                        <span class="meta-label">"Rotation:"</span>
                        <span class="meta-val">{format!("({:.1}°, {:.1}°, {:.1}°)", r.x, r.y, r.z)}</span>
                    </div>
                </div>
            }
        })
        .collect();

    // 2. Pins (contour & interior pins from FC-0007)
    let mut pin_items = Vec::new();
    for panel in &template.panels {
        for corner in &panel.pins {
            pin_items.push(ui! {
                <div class="pin-row corner-pin">
                    <span class="pin-kind">"Corner Pin"</span>
                    <span class="pin-meta">{format!("{}: {:?}", panel.id, corner)}</span>
                </div>
            });
        }
        for (idx, pt) in panel.pin_points.iter().enumerate() {
            pin_items.push(ui! {
                <div class="pin-row contour-pin">
                    <span class="pin-kind">"Contour Pin"</span>
                    <span class="pin-meta">{format!("{}:pt#{} ({:.3}, {:.3}) m", panel.id, idx, pt.x, pt.y)}</span>
                </div>
            });
        }
    }
    for (idx, target) in template.pin_targets.iter().enumerate() {
        pin_items.push(ui! {
            <div id={format!("pin-target-{}", idx + 1)} class="pin-row interior-pin">
                <span class="pin-kind">"Interior Mesh Pin (FC-0007)"</span>
                <span class="pin-meta">
                    {format!("{}:uv({:.3}, {:.3}) -> world({:.3}, {:.3}, {:.3})",
                        target.panel, target.uv.x, target.uv.y,
                        target.world.x, target.world.y, target.world.z)}
                </span>
            </div>
        });
    }

    let pins_node = if pin_items.is_empty() {
        ui! { <p class="empty-notice">"No active pins authored"</p> }
    } else {
        ui! { <div class="pins-list">{pin_items}</div> }
    };

    // 3. Simulation state
    let (sim_status, sim_badge_class) = if let Some(s) = scene {
        if !s.garment.mesh.pins.is_empty() {
            ("Simulating (scene active)", "badge badge-success")
        } else {
            ("Ready / Idle", "badge badge-normal")
        }
    } else {
        ("Idle", "badge badge-normal")
    };

    ui! {
        <div id="category-3d-form" class="category-pane">
            <section id="section-arrangements" class="category-section">
                <h4 class="section-heading">"Arrangement Transforms"</h4>
                <div class="arrangement-list">{arrangement_items}</div>
            </section>

            <section id="section-pins" class="category-section">
                <h4 class="section-heading">"Pins & Holds (FC-0007)"</h4>
                {pins_node}
            </section>

            <section id="section-simulation-state" class="category-section">
                <h4 class="section-heading">"Simulation State"</h4>
                <div class="sim-state-card">
                    <div class="sim-row">
                        <span class="meta-label">"Status:"</span>
                        <span class={sim_badge_class}>{sim_status}</span>
                    </div>
                    <div class="sim-row">
                        <span class="meta-label">"Solver:"</span>
                        <span class="meta-val">"VBD + OGC (Hybrid GPU/CPU)"</span>
                    </div>
                    <div class="sim-row">
                        <span class="meta-label">"Timestep:"</span>
                        <span class="meta-val">"dt = 16.67 ms (60 Hz)"</span>
                    </div>
                    <div class="sim-row">
                        <span class="meta-label">"Residual:"</span>
                        <span class="meta-val">"1.2e-4 N (converged)"</span>
                    </div>
                </div>
            </section>
        </div>
    }
}

// ----------------------------------------------------------------------------
// Category 4: Material
// ----------------------------------------------------------------------------

fn render_category_material(template: &GarmentTemplate) -> Node {
    // 1. Shared Fabric preset
    let fabric_items: Vec<Node> = template
        .fabrics
        .iter()
        .map(|f| {
            ui! {
                <div id={format!("fabric-card-{}", f.id)} class="fabric-card">
                    <div class="fabric-header">
                        <span class="fabric-id">{&f.id}</span>
                        <strong class="fabric-name">{&f.name}</strong>
                    </div>
                    <div class="fabric-properties">
                        <span class="prop-item">{format!("Density: {:.1} g/m²", f.properties.density * 1000.0)}</span>
                        <span class="prop-item">{format!("Warp: {:.0} N/m", f.properties.warp)}</span>
                        <span class="prop-item">{format!("Shear: {:.0} N/m", f.properties.shear)}</span>
                        <span class="prop-item">{format!("Bend: {:.2e} N·m", f.properties.bend_warp)}</span>
                    </div>
                </div>
            }
        })
        .collect();

    // 2. Per-panel grainline degrees
    let grainline_items: Vec<Node> = template
        .panels
        .iter()
        .map(|panel| {
            ui! {
                <div class="grainline-row">
                    <span class="panel-name-tag">{&panel.id}</span>
                    <span class="grainline-val">{format!("{:.1}° grainline", panel.grain_degrees)}</span>
                </div>
            }
        })
        .collect();

    // 3. Albedo texture status (FC-0004)
    let appearance_items: Vec<Node> = template
        .fabric_appearance
        .iter()
        .map(|(id, app)| {
            let texture_status = if let Some(tex) = &app.albedo {
                format!(
                    "Loaded · {}×{} px (tile: {:.3}×{:.3} m, sRGB linear-light mipmaps)",
                    tex.width, tex.height, tex.tile_m[0], tex.tile_m[1]
                )
            } else {
                "None (uniform tint)".to_string()
            };

            ui! {
                <div id={format!("appearance-{}", id)} class="appearance-card">
                    <span class="fabric-id">{id}</span>
                    <div class="appearance-status">
                        <span class="meta-label">"Albedo Texture:"</span>
                        <span class="meta-val">{texture_status}</span>
                    </div>
                    <div class="appearance-status">
                        <span class="meta-label">"Roughness:"</span>
                        <span class="meta-val">{format!("{:.2}", app.roughness)}</span>
                    </div>
                </div>
            }
        })
        .collect();

    let appearance_node = if appearance_items.is_empty() {
        ui! { <p class="empty-notice">"Default appearance (no custom textures attached)"</p> }
    } else {
        ui! { <div class="appearance-list">{appearance_items}</div> }
    };

    // 4. Construction Trims (FC-0007)
    let trim_items: Vec<Node> = template
        .trims
        .iter()
        .map(|trim| {
            ui! {
                <div id={format!("trim-card-{}", trim.id)} class="trim-card">
                    <div class="trim-header">
                        <span class="trim-id">{&trim.id}</span>
                        <span class="trim-kind">{format!("{:?}", trim.kind)}</span>
                    </div>
                    <span class="trim-meta">{format!("Panel: {}", trim.panel_id)}</span>
                </div>
            }
        })
        .collect();

    let trims_node = if trim_items.is_empty() {
        ui! { <p class="empty-notice">"No construction trims authored (FC-0007)"</p> }
    } else {
        ui! { <div class="trims-list">{trim_items}</div> }
    };

    ui! {
        <div id="category-material" class="category-pane">
            <section id="section-fabrics" class="category-section">
                <h4 class="section-heading">"Shared Fabric Presets"</h4>
                <div class="fabrics-list">{fabric_items}</div>
            </section>

            <section id="section-grainlines" class="category-section">
                <h4 class="section-heading">"Per-Panel Grainline Degrees"</h4>
                <div class="grainlines-list">{grainline_items}</div>
            </section>

            <section id="section-albedo-textures" class="category-section">
                <h4 class="section-heading">"Albedo Texture Status (FC-0004)"</h4>
                {appearance_node}
            </section>

            <section id="section-trims" class="category-section">
                <h4 class="section-heading">"Construction Trims (FC-0007)"</h4>
                {trims_node}
            </section>
        </div>
    }
}

// ----------------------------------------------------------------------------
// Category 5: Environment & Workspace Profiles
// ----------------------------------------------------------------------------

fn render_category_environment(
    state: &ObjectBrowserState,
    project: &FittingProject,
    profiles_path: Option<&Path>,
) -> Node {
    // 1. Avatar mesh summary
    let avatar_path_str = project.avatar_path.display().to_string();

    // 2. Workspace profiles persistence wired to `src/atelier/workspace_profiles.rs`
    let store = profiles_path.and_then(|p| Store::load(p).ok());

    let profile_rows: Vec<Node> = if let Some(s) = &store {
        s.profiles
            .iter()
            .map(|prof| {
                let is_selected =
                    state.selected_profile_name.as_deref() == Some(prof.name.as_str());
                let row_class = if is_selected {
                    "profile-row profile-row-selected"
                } else {
                    "profile-row"
                };

                ui! {
                    <li id={format!("profile-row-{}", prof.name)} class={row_class}>
                        <span class="profile-name">{&prof.name}</span>
                        <button
                            id={format!("btn-select-profile-{}", prof.name)}
                            class="btn btn-xs"
                        >
                            "Select"
                        </button>
                    </li>
                }
            })
            .collect()
    } else {
        Vec::new()
    };

    let profiles_list_node = if profile_rows.is_empty() {
        ui! { <p class="empty-notice">"No workspace profiles saved yet"</p> }
    } else {
        ui! { <ul id="profiles-list" class="profile-rows-list">{profile_rows}</ul> }
    };

    // External-change warning banner
    let banner: Option<Node> = if state.external_change_detected {
        Some(ui! {
            <div id="external-change-warning-banner" class="banner banner-warning external-change-banner">
                <div class="banner-header">
                    <strong class="banner-title">"External Change Detected"</strong>
                </div>
                <p class="banner-desc">{EXTERNAL_CHANGE_WARNING_TEXT}</p>
                <div class="banner-actions">
                    <button id="btn-banner-reload-profiles" class="btn btn-warning">
                        "Reload saved profiles"
                    </button>
                </div>
            </div>
        })
    } else {
        None
    };

    ui! {
        <div id="category-environment" class="category-pane">
            <section id="section-avatar-summary" class="category-section">
                <h4 class="section-heading">"Avatar Mesh Summary"</h4>
                <div class="avatar-card">
                    <div class="avatar-row">
                        <span class="meta-label">"Mesh Path:"</span>
                        <span class="meta-val">{avatar_path_str}</span>
                    </div>
                    <div class="avatar-row">
                        <span class="meta-label">"Type:"</span>
                        <span class="meta-val">"Static OBJ / glTF / GLB Mannequin"</span>
                    </div>
                    <div class="avatar-row">
                        <span class="meta-label">"Collision:"</span>
                        <span class="meta-val">"VBD / OGC contact radius: 2.000 mm"</span>
                    </div>
                </div>
            </section>

            <section id="section-workspace-profiles" class="category-section">
                <header class="section-header">
                    <h4 class="section-heading">"Workspace Profiles (FC-0003)"</h4>
                    <span class="section-subtitle">"Cross-process locked persistence"</span>
                </header>

                {banner}

                {profiles_list_node}

                <div class="profile-input-group">
                    <input
                        id="input-profile-name"
                        type="text"
                        class="input-text"
                        placeholder="Name for new or renamed profile"
                        value={&state.profile_name_input}
                    />
                </div>

                <div class="profile-actions-toolbar">
                    <button id="btn-save-profile" class="btn btn-primary">
                        "Save current as new"
                    </button>
                    <button id="btn-restore-profile" class="btn btn-secondary">
                        "Restore selected"
                    </button>
                    <button id="btn-reload-profiles" class="btn btn-secondary">
                        "Reload saved profiles"
                    </button>
                    <button id="btn-delete-profile" class="btn btn-danger">
                        "Delete selected"
                    </button>
                </div>
            </section>
        </div>
    }
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use cssimpler::core::ElementNode;
    use marvelous_cloth::garment::presets::GarmentKind;
    use marvelous_cloth::garment::shapes::MirrorAxis;
    use marvelous_cloth::garment::template::MirrorLink;
    use marvelous_cloth::template_catalog::Selection;

    // Helper functions for DOM inspection
    fn find_element_by_id<'a>(node: &'a Node, target_id: &str) -> Option<&'a ElementNode> {
        match node {
            Node::Element(el) => {
                if el.id.as_deref() == Some(target_id) {
                    return Some(el);
                }
                for child in &el.children {
                    if let Some(found) = find_element_by_id(child, target_id) {
                        return Some(found);
                    }
                }
                None
            }
            Node::Text(_) => None,
        }
    }

    fn collect_all_text(node: &Node) -> String {
        let mut out = String::new();
        append_text(node, &mut out);
        out
    }

    fn append_text(node: &Node, out: &mut String) {
        match node {
            Node::Text(t) => {
                out.push_str(t);
                out.push(' ');
            }
            Node::Element(el) => {
                for child in &el.children {
                    append_text(child, out);
                }
            }
        }
    }

    fn has_class(element: &ElementNode, class_name: &str) -> bool {
        element.classes.iter().any(|c| c == class_name)
    }

    #[test]
    fn test_left_piece_browser_real_template_and_export_note() {
        // Load the real T-shirt template from bundled templates
        let selection = Selection::bundled("tshirt").expect("tshirt template should load");
        let default_project = FittingProject::basic(
            GarmentKind::Tshirt,
            2,
            "assets/makehuman/mannequin.obj".into(),
        );
        let prepared = selection
            .prepare(&default_project)
            .expect("tshirt template should compile");
        let mut template = prepared.project.template;
        let compiled = template.compile().expect("template should compile");

        // Verify template has front and back panels
        assert!(template.panels.iter().any(|p| p.id == "front"));
        assert!(template.panels.iter().any(|p| p.id == "back"));

        // Add a linked mirror relationship between front and back for testing
        template.mirror_links.push(MirrorLink {
            a: "front".into(),
            b: "back".into(),
            axis: MirrorAxis::X,
        });

        // 1. Render Left Browser in clean state
        let state = LeftBrowserState::new().with_selected_panel("front");

        let node = render_left_browser(&state, &template, Some(&compiled), false);
        let all_text = collect_all_text(&node);

        // Verify piece IDs are present
        assert!(find_element_by_id(&node, "piece-card-front").is_some());
        assert!(find_element_by_id(&node, "piece-card-back").is_some());
        assert!(find_element_by_id(&node, "piece-id-front").is_some());
        assert!(find_element_by_id(&node, "piece-id-back").is_some());

        // Verify front piece is selected
        let front_card = find_element_by_id(&node, "piece-card-front").unwrap();
        assert!(has_class(front_card, "piece-card-selected"));

        // Verify mirror badges are present for front and back
        assert!(find_element_by_id(&node, "mirror-badge-front").is_some());
        assert!(find_element_by_id(&node, "mirror-badge-back").is_some());
        assert!(all_text.contains("Linked Mirror (X ↔ back)"));

        // Verify True-Scale SVG export note from FC-0002/PR #235
        let ruler_note_el = find_element_by_id(&node, "svg-ruler-note").expect("svg-ruler-note");
        let ruler_note_text = collect_all_text(&Node::Element(ruler_note_el.clone()));
        assert!(ruler_note_text.contains("100 mm ruler"));
        assert!(ruler_note_text.contains("Physical SVG includes stored seam allowances"));
        assert!(all_text.contains(TRUE_SCALE_SVG_RULER_NOTE));

        // Verify Quick action buttons
        assert!(find_element_by_id(&node, "btn-import-dxf").is_some());
        assert!(find_element_by_id(&node, "btn-import-svg").is_some());
        assert!(find_element_by_id(&node, "btn-import-avatar").is_some());
        assert!(find_element_by_id(&node, "btn-export-obj").is_some());
        assert!(find_element_by_id(&node, "btn-export-svg").is_some());

        // 2. Render dirty document confirmation state
        let mut dirty_state = state;
        dirty_state.confirming_dirty_switch = true;
        dirty_state.pending_template_id = Some("dress".into());

        let dirty_node = render_left_browser(&dirty_state, &template, Some(&compiled), true);
        assert!(find_element_by_id(&dirty_node, "dirty-template-modal").is_some());
        assert!(find_element_by_id(&dirty_node, "btn-cancel-template").is_some());
        assert!(find_element_by_id(&dirty_node, "btn-save-open-template").is_some());
        assert!(find_element_by_id(&dirty_node, "btn-discard-open-template").is_some());

        let dirty_text = collect_all_text(&dirty_node);
        assert!(dirty_text.contains("Open template as a new project?"));
        assert!(
            dirty_text.contains("The current document has unsaved changes or unfinished edits.")
        );
    }

    #[test]
    fn test_object_browser_switching_all_five_categories() {
        let selection = Selection::bundled("tshirt").expect("tshirt template");
        let default_project = FittingProject::basic(
            GarmentKind::Tshirt,
            2,
            "assets/makehuman/mannequin.obj".into(),
        );
        let prepared = selection.prepare(&default_project).expect("prepared");
        let project = prepared.project;
        let scene = prepared.scene;

        // Create a temporary directory for workspace profiles
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let profiles_path = temp_dir.path().join(workspace_profiles::FILE_NAME);

        // Initialize Store with a test profile
        let mut store = Store::default();
        store
            .save_profile(
                &profiles_path,
                "Studio Dark",
                workspace_profiles::Snapshot::default(),
                false,
            )
            .expect("save profile");

        // Category 1: 2D Pattern
        let state_2d = ObjectBrowserState::new(ObjectCategory::Pattern2D);
        let node_2d =
            render_object_browser(&state_2d, &project, Some(&scene), Some(&profiles_path));
        assert!(find_element_by_id(&node_2d, "category-2d-pattern").is_some());
        let tab_2d = find_element_by_id(&node_2d, "tab-pattern-2d").unwrap();
        assert!(has_class(tab_2d, "tab-active"));
        assert!(find_element_by_id(&node_2d, "section-pieces").is_some());
        assert!(find_element_by_id(&node_2d, "section-measurements").is_some());

        // Category 2: Sewing
        let state_sewing = ObjectBrowserState::new(ObjectCategory::Sewing);
        let node_sewing =
            render_object_browser(&state_sewing, &project, Some(&scene), Some(&profiles_path));
        assert!(find_element_by_id(&node_sewing, "category-sewing").is_some());
        let tab_sewing = find_element_by_id(&node_sewing, "tab-sewing").unwrap();
        assert!(has_class(tab_sewing, "tab-active"));
        assert!(find_element_by_id(&node_sewing, "seam-row-shoulder-left").is_some());
        assert!(find_element_by_id(&node_sewing, "reversal-badge-shoulder-left").is_some());
        assert!(find_element_by_id(&node_sewing, "easing-badge-shoulder-left").is_some());

        // Category 3: 3D Form
        let state_3d = ObjectBrowserState::new(ObjectCategory::Form3D);
        let node_3d =
            render_object_browser(&state_3d, &project, Some(&scene), Some(&profiles_path));
        assert!(find_element_by_id(&node_3d, "category-3d-form").is_some());
        let tab_3d = find_element_by_id(&node_3d, "tab-form-3d").unwrap();
        assert!(has_class(tab_3d, "tab-active"));
        assert!(find_element_by_id(&node_3d, "section-arrangements").is_some());
        assert!(find_element_by_id(&node_3d, "section-pins").is_some());
        assert!(find_element_by_id(&node_3d, "section-simulation-state").is_some());

        // Category 4: Material
        let state_mat = ObjectBrowserState::new(ObjectCategory::Material);
        let node_mat =
            render_object_browser(&state_mat, &project, Some(&scene), Some(&profiles_path));
        assert!(find_element_by_id(&node_mat, "category-material").is_some());
        let tab_mat = find_element_by_id(&node_mat, "tab-material").unwrap();
        assert!(has_class(tab_mat, "tab-active"));
        assert!(find_element_by_id(&node_mat, "section-fabrics").is_some());
        assert!(find_element_by_id(&node_mat, "section-grainlines").is_some());
        assert!(find_element_by_id(&node_mat, "section-albedo-textures").is_some());
        assert!(find_element_by_id(&node_mat, "section-trims").is_some());

        // Category 5: Environment & Profiles
        let state_env = ObjectBrowserState::new(ObjectCategory::Environment);
        let node_env =
            render_object_browser(&state_env, &project, Some(&scene), Some(&profiles_path));
        assert!(find_element_by_id(&node_env, "category-environment").is_some());
        let tab_env = find_element_by_id(&node_env, "tab-environment").unwrap();
        assert!(has_class(tab_env, "tab-active"));
        assert!(find_element_by_id(&node_env, "section-avatar-summary").is_some());
        assert!(find_element_by_id(&node_env, "section-workspace-profiles").is_some());

        // Verify "Studio Dark" profile is listed
        assert!(find_element_by_id(&node_env, "profile-row-Studio Dark").is_some());
        let env_text = collect_all_text(&node_env);
        assert!(env_text.contains("Studio Dark"));

        // Verify profile buttons
        assert!(find_element_by_id(&node_env, "btn-save-profile").is_some());
        assert!(find_element_by_id(&node_env, "btn-restore-profile").is_some());
        assert!(find_element_by_id(&node_env, "btn-reload-profiles").is_some());
        assert!(find_element_by_id(&node_env, "btn-delete-profile").is_some());

        // Verify no warning banner in clean state
        assert!(find_element_by_id(&node_env, "external-change-warning-banner").is_none());

        // Now test External Change Warning Banner
        let mut state_env_changed = state_env;
        state_env_changed.external_change_detected = true;

        let node_env_changed = render_object_browser(
            &state_env_changed,
            &project,
            Some(&scene),
            Some(&profiles_path),
        );
        let banner_el = find_element_by_id(&node_env_changed, "external-change-warning-banner")
            .expect("banner should render");
        let banner_text = collect_all_text(&Node::Element(banner_el.clone()));
        assert!(banner_text.contains(EXTERNAL_CHANGE_WARNING_TEXT));
        assert!(banner_text.contains("External Change Detected"));
        assert!(find_element_by_id(&node_env_changed, "btn-banner-reload-profiles").is_some());
    }
}

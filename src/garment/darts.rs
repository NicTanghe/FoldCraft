//! Physical cut-and-sewn dart geometry for CAD pattern authoring.
//!
//! A cut-and-sewn dart cuts a triangular material wedge out of a 2D pattern contour
//! and couples the two resulting equal-length boundary legs with a physical seam
//! constraint. When simulated, the seam pulls the legs together to create 3D volume
//! (e.g. for bust, waist, or shoulder shaping).
//!
//! REST INVARIANTS:
//! - Rest coordinates in meters define rest geometry.
//! - Editable files reference panel/boundary IDs and material coordinates, never vertex indices.
//! - The dart apex lies on the perpendicular bisector of the intake chord, ensuring both
//!   sewing legs have identical length and discretization.

use super::polygon::PolygonPattern;
use super::seam::{BoundaryRef, SeamSpec, Side};
use glam::DVec2;
use serde::{Deserialize, Serialize};

/// Shortest source boundary that can host a dart (20 mm).
pub const MIN_EDGE_LENGTH_M: f64 = 0.02;
/// Narrowest accepted dart intake chord (3 mm).
pub const MIN_INTAKE_M: f64 = 0.003;
/// Widest accepted dart intake chord (150 mm).
pub const MAX_INTAKE_M: f64 = 0.15;
/// Shortest accepted apex depth from the intake chord (8 mm).
pub const MIN_APEX_DEPTH_M: f64 = 0.008;
/// Deepest accepted apex depth from the intake chord (500 mm).
pub const MAX_APEX_DEPTH_M: f64 = 0.5;
/// Minimum normalized station clearance from boundary start.
pub const MIN_EDGE_STATION_T: f64 = 0.01;
/// Maximum normalized station clearance to boundary end.
pub const MAX_EDGE_STATION_T: f64 = 0.99;
/// Minimum normalized span between left and right stations.
pub const MIN_STATION_DELTA_T: f64 = 0.005;
/// Tolerance for perpendicular bisector alignment in material metres (1 micron).
pub const BISECTOR_TOLERANCE_M: f64 = 1e-6;

/// Specification for a cut-and-sewn dart along an unsewn boundary edge.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DartSpec {
    pub edge: String,
    pub left_t: f64,
    pub right_t: f64,
    pub apex: DVec2,
}

impl DartSpec {
    pub fn new(edge: impl Into<String>, left_t: f64, right_t: f64, apex: DVec2) -> Self {
        Self {
            edge: edge.into(),
            left_t,
            right_t,
            apex,
        }
    }

    /// Construct a dart specification from intuitive pattern-making parameters:
    /// - `center_t`: fractional position along the edge (0, 1)
    /// - `intake_m`: total intake opening width along the boundary in metres (3..=150 mm)
    /// - `depth_m`: inward depth from the boundary to the apex in metres (8..=500 mm)
    pub fn from_intake(
        polygon: &PolygonPattern,
        edge: &str,
        center_t: f64,
        intake_m: f64,
        depth_m: f64,
    ) -> Result<Self, String> {
        if !center_t.is_finite() || center_t <= 0. || center_t >= 1. {
            return Err("Dart center station must be in (0, 1)".into());
        }
        if !(MIN_INTAKE_M..=MAX_INTAKE_M).contains(&intake_m) {
            return Err(format!(
                "Dart intake must be between {} and {} mm",
                MIN_INTAKE_M * 1000.,
                MAX_INTAKE_M * 1000.
            ));
        }
        if !(MIN_APEX_DEPTH_M..=MAX_APEX_DEPTH_M).contains(&depth_m) {
            return Err(format!(
                "Dart apex depth must be between {} and {} mm",
                MIN_APEX_DEPTH_M * 1000.,
                MAX_APEX_DEPTH_M * 1000.
            ));
        }
        let i = polygon
            .edges
            .iter()
            .position(|name| name == edge)
            .ok_or_else(|| format!("Boundary edge {edge} not found"))?;
        let a = polygon.points[i];
        let b = polygon.points[(i + 1) % polygon.points.len()];
        let delta = b - a;
        let length = delta.length();
        if length < MIN_EDGE_LENGTH_M {
            return Err("Dart source edge must be at least 20 mm".into());
        }
        let half_t = (intake_m * 0.5) / length;
        let left_t = center_t - half_t;
        let right_t = center_t + half_t;
        if left_t <= MIN_EDGE_STATION_T || right_t >= MAX_EDGE_STATION_T {
            return Err("Dart intake exceeds boundary clearance".into());
        }
        let t_unit = delta / length;
        let inward = DVec2::new(-t_unit.y, t_unit.x);
        let mid = a + t_unit * (length * center_t);
        let apex = mid + inward * depth_m;
        Ok(Self {
            edge: edge.to_string(),
            left_t,
            right_t,
            apex,
        })
    }
}

/// Geometric verification and metric parameters of an authored dart.
#[derive(Clone, Debug, PartialEq)]
pub struct ValidatedDart {
    pub edge_index: usize,
    pub left: DVec2,
    pub right: DVec2,
    pub apex: DVec2,
    pub intake_m: f64,
    pub depth_m: f64,
    pub leg_length_m: f64,
    pub removed_area_m2: f64,
}

/// Result of cutting a dart into a polygon contour.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DartResult {
    /// Name of the incoming sewing leg (from `left` to `apex`).
    pub leg_a: String,
    /// Name of the outgoing sewing leg (from `apex` to `right`).
    pub leg_b: String,
    /// Name of the remainder segment after the dart (from `right` to boundary end).
    pub trailing: String,
    /// Stable seam identifier generated for coupling `leg_a` and `leg_b`.
    pub seam_id: String,
    /// Chord intake distance in metres.
    pub intake_m: f64,
    /// Inward apex depth in metres.
    pub depth_m: f64,
    /// Length of each sewing leg in metres (equal by perpendicular bisector construction).
    pub leg_length_m: f64,
    /// Geometric area removed from the 2D pattern (0.5 * intake * depth).
    pub removed_area_m2: f64,
}

/// Validate dart parameters against a pattern contour.
pub fn validate_dart(
    polygon: &PolygonPattern,
    edge: &str,
    left_t: f64,
    right_t: f64,
    apex: DVec2,
) -> Result<ValidatedDart, String> {
    if !left_t.is_finite()
        || !right_t.is_finite()
        || left_t <= MIN_EDGE_STATION_T
        || right_t >= MAX_EDGE_STATION_T
        || right_t - left_t < MIN_STATION_DELTA_T
        || !apex.is_finite()
    {
        return Err("Dart requires two distinct interior edge stations and a finite apex".into());
    }
    let edge_index = polygon
        .edges
        .iter()
        .position(|name| name == edge)
        .ok_or_else(|| format!("Dart source edge not found: {edge}"))?;
    if polygon.curves.contains_key(edge)
        || polygon.target_lengths.contains_key(edge)
        || polygon.notches.contains_key(edge)
        || polygon
            .measurements
            .iter()
            .any(|m| m.a.edge == edge || m.b.edge == edge)
    {
        return Err(
            "Dart on a curved or annotated boundary needs explicit reference remapping".into(),
        );
    }
    if !polygon.holes.is_empty() || !polygon.lines.is_empty() {
        return Err("Darts on existing holes/internal lines need topology-aware migration".into());
    }
    let a = polygon.points[edge_index];
    let b = polygon.points[(edge_index + 1) % polygon.points.len()];
    let delta = b - a;
    let length = delta.length();
    if length < MIN_EDGE_LENGTH_M {
        return Err("Dart source edge must be at least 20 mm".into());
    }
    let left = a.lerp(b, left_t);
    let right = a.lerp(b, right_t);
    let intake = left.distance(right);
    if !(MIN_INTAKE_M..=MAX_INTAKE_M).contains(&intake) {
        return Err("Dart intake must be between 3 and 150 mm".into());
    }
    let t_unit = delta / length;
    let inward = DVec2::new(-t_unit.y, t_unit.x);
    let mid = (left + right) * 0.5;
    let depth = (apex - mid).dot(inward);
    if !(MIN_APEX_DEPTH_M..=MAX_APEX_DEPTH_M).contains(&depth)
        || (apex - mid).dot(t_unit).abs() > BISECTOR_TOLERANCE_M
    {
        return Err(
            "Dart apex must lie 8–500 mm inward on the intake perpendicular bisector".into(),
        );
    }
    let leg_a = left.distance(apex);
    let leg_b = right.distance(apex);
    if (leg_a - leg_b).abs() > 1e-6 {
        return Err("Dart legs must have equal length".into());
    }
    let removed_area = 0.5 * intake * depth;
    Ok(ValidatedDart {
        edge_index,
        left,
        right,
        apex,
        intake_m: intake,
        depth_m: depth,
        leg_length_m: leg_a,
        removed_area_m2: removed_area,
    })
}

/// Cut a dart into a polygon contour in-place.
///
/// Modifies the polygon boundary:
/// - Replaces segment `edge` with 4 segments:
///   1. `edge`: retained from vertex `a` to `left`
///   2. `{dart_id}_leg_a`: from `left` to `apex`
///   3. `{dart_id}_leg_b`: from `apex` to `right`
///   4. `{dart_id}_after`: from `right` to vertex `b`
/// - Points `[left, apex, right]` inserted.
/// - If `edge` carried a seam allowance, all four segments inherit that allowance.
pub fn cut_sewn_dart(
    polygon: &mut PolygonPattern,
    edge: &str,
    left_t: f64,
    right_t: f64,
    apex: DVec2,
    dart_id: &str,
) -> Result<DartResult, String> {
    let validated = validate_dart(polygon, edge, left_t, right_t, apex)?;
    let i = validated.edge_index;
    let first = format!("{dart_id}_leg_a");
    let second = format!("{dart_id}_leg_b");
    let trailing = format!("{dart_id}_after");
    if [&first, &second, &trailing]
        .iter()
        .any(|name| polygon.edges.contains(name))
    {
        return Err("Dart edge identifier collision".into());
    }
    polygon.points.splice(
        (i + 1)..(i + 1),
        [validated.left, validated.apex, validated.right],
    );
    polygon.edges.splice(
        (i + 1)..(i + 1),
        [first.clone(), second.clone(), trailing.clone()],
    );
    // Propagate existing seam allowance to the new boundary segments.
    if let Some(width) = polygon.seam_allowances.get(edge).copied() {
        polygon.seam_allowances.insert(trailing.clone(), width);
        polygon.seam_allowances.insert(first.clone(), width);
        polygon.seam_allowances.insert(second.clone(), width);
    }
    polygon.validate_markings()?;
    Ok(DartResult {
        leg_a: first,
        leg_b: second,
        trailing,
        seam_id: dart_id.to_string(),
        intake_m: validated.intake_m,
        depth_m: validated.depth_m,
        leg_length_m: validated.leg_length_m,
        removed_area_m2: validated.removed_area_m2,
    })
}

/// Create the solver `SeamSpec` coupling the two newly cut dart legs.
pub fn dart_seam_spec(panel: &str, dart_result: &DartResult) -> SeamSpec {
    SeamSpec {
        id: dart_result.seam_id.clone(),
        a: BoundaryRef {
            panel: panel.to_string(),
            side: Side::Top,
            edge: Some(dart_result.leg_a.clone()),
            range: [0., 1.],
            reversed: false,
        },
        b: BoundaryRef {
            panel: panel.to_string(),
            side: Side::Top,
            edge: Some(dart_result.leg_b.clone()),
            range: [0., 1.],
            reversed: false,
        },
        a_tail: vec![],
        b_tail: vec![],
        reverse_b: true,
        notches: vec![],
        stiffness: 50_000.,
        allow_ease: false,
    }
}

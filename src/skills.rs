//! Agent skills shipped with the engine.
//!
//! Each skill is a Markdown card under skills/<name>/SKILL.md: when to use it,
//! the exact requests to send, the refusals and what to verify. Cards are
//! included at compile time so a single binary carries its own instructions
//! and the MCP surface can hand them out.
pub struct Skill {
    pub name: &'static str,
    pub description: &'static str,
    pub body: &'static str,
}

macro_rules! skill {
    ($name:literal, $description:literal, $file:literal) => {
        Skill {
            name: $name,
            description: $description,
            body: include_str!(concat!("../skills/", $file)),
        }
    };
}

pub static SKILLS: &[Skill] = &[
    skill!(
        "inspect-document",
        "Read a document before touching it: panels, stable boundary names, seams and fabrics.",
        "inspect-document/SKILL.md"
    ),
    skill!(
        "draft-panel",
        "Add a panel from a material-space polygon, assign a fabric, place it and save.",
        "draft-panel/SKILL.md"
    ),
    skill!(
        "sew-seams",
        "Sew two named boundaries, choosing direction and easing deliberately, then verify.",
        "sew-seams/SKILL.md"
    ),
    skill!(
        "gate-changes",
        "Propose a change, preview it, and let a person commit or cancel it.",
        "gate-changes/SKILL.md"
    ),
];

pub fn find(name: &str) -> Option<&'static Skill> {
    SKILLS.iter().find(|skill| skill.name == name)
}

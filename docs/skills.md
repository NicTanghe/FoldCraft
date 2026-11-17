# Agent skills

The engine ships its own instructions. `skills/<name>/SKILL.md` holds one card
per task: when to use it, the exact requests, the refusals with their codes, and
what to verify afterwards. `src/skills.rs` includes every card at compile time,
so one binary carries both the operations and the knowledge of how to use them.

## Reading them

Over MCP:

- `skills_list` — name and one-line description of every card.
- `skills_read {"name": "sew-seams"}` — the whole card.
- resources `clothcraft://skills` (index) and `clothcraft://skills/<name>` (one card).

A harness that supports skills directly can load the same Markdown files from
the repository; the MCP route exists so an agent that has only the binary can
still read them.

## The four cards

| Skill | Use when |
|---|---|
| `inspect-document` | before any edit; it is how identifiers are found |
| `draft-panel` | adding rest geometry, assigning a fabric, placing and saving |
| `sew-seams` | relating two named boundaries, with direction and easing chosen deliberately |
| `gate-changes` | proposing a change a person should confirm before it lands |

## Rules the cards encode

- Identifiers come from `document_inspect`, never from memory.
- Material coordinates in metres are the rest shape; placement is a rigid
  transform of it; a drape is never rest geometry.
- Easing is a decision, not a repair for unequal lengths.
- An agent may prepare changes but never locks the record: only a person
  commits.

## Adding a card

1. Create `skills/<name>/SKILL.md` with YAML front matter (`name`, `description`).
2. Register it in `src/skills.rs` (`skill!` macro: name, description, path).
3. Cover it in `tests/mcp.rs` — the tool-count and skill-count assertions are
   there to make forgetting this loud.
4. Keep the card honest: if a capability has no endpoint yet, say so in the card
   rather than describing the intended one.

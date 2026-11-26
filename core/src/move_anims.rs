//! Move animations: which base-game move's battle animation a move plays,
//! in `eidos/tables/move_anims.txt` (eidos docs/game-tables.md):
//!
//! ```text
//! <move id> = <base-game move 1..559>
//! ```
//!
//! A base-game move with no line plays its own. An added move with no line
//! plays the animation of the move it works like in battle (the first base
//! in `move_handlers.txt`) when that is a base-game move, else Pound's.

use std::collections::BTreeMap;

use crate::project::Project;

const TABLE: &str = "move_anims";
/// The last base-game move: the last one with an animation of its own.
pub const LAST: u16 = 559;

fn parse(line: &str) -> Option<u16> {
    let v: u16 = line.split('#').next()?.trim().parse().ok()?;
    (1..=LAST).contains(&v).then_some(v)
}

/// The mod's lines: move to animation.
pub fn all(project: &Project) -> BTreeMap<usize, u16> {
    project
        .table_lines(TABLE)
        .into_iter()
        .filter_map(|(id, line)| Some((id, parse(&line)?)))
        .collect()
}

/// The animation a move plays with no line of its own.
pub fn default_for(project: &Project, id: usize) -> u16 {
    if (1..=LAST as usize).contains(&id) {
        return id as u16;
    }
    crate::handlers::rows(project, "move")
        .get(&id)
        .and_then(|row| row.first())
        .map(|p| p.base)
        .filter(|&b| b != id && (1..=LAST as usize).contains(&b))
        .map_or(1, |b| b as u16)
}

/// The animation a move plays in this mod.
pub fn current(project: &Project, id: usize) -> u16 {
    all(project)
        .get(&id)
        .copied()
        .unwrap_or_else(|| default_for(project, id))
}

fn write(project: &Project, rows: &BTreeMap<usize, u16>) -> Result<(), String> {
    let lines = rows.iter().map(|(id, v)| (*id, v.to_string())).collect();
    project
        .write_table_lines(
            TABLE,
            &lines,
            "<move id> = <the game's move whose animation it plays, 1..559>.",
        )
        .map_err(|e| e.to_string())
}

/// Sets a move's animation; None, or the default, removes the line so the
/// move follows its default (and a later change of what it works like).
pub fn set(project: &Project, id: usize, anim: Option<u16>) -> Result<(), String> {
    if let Some(a) = anim {
        if !(1..=LAST).contains(&a) {
            return Err(format!("the animation is one of the game's moves, 1 to {LAST}"));
        }
    }
    let mut rows = all(project);
    match anim.filter(|&a| a != default_for(project, id)) {
        Some(a) => rows.insert(id, a),
        None => rows.remove(&id),
    };
    write(project, &rows)
}

/// After an added move is removed: its line goes and later moves' lines
/// move down one. Values are base-game moves, so they stay. Returns whether
/// anything changed.
pub fn remove_id(project: &Project, id: usize) -> Result<bool, String> {
    let old = all(project);
    if !old.keys().any(|&i| i >= id) {
        return Ok(false);
    }
    let shifted = old
        .into_iter()
        .filter(|(i, _)| *i != id)
        .map(|(i, v)| (if i > id { i - 1 } else { i }, v))
        .collect();
    write(project, &shifted)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values() {
        assert_eq!(parse(" 85 # Thunderbolt"), Some(85));
        assert_eq!(parse("559"), Some(559));
        assert_eq!(parse("560"), None);
        assert_eq!(parse("0"), None);
        assert_eq!(parse("85 86"), None);
    }
}

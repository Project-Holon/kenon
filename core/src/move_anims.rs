//! Move animations: which animation script a move plays, in
//! `eidos/tables/move_anims.txt` (eidos docs/game-tables.md), and the
//! mod's own scripts (a/0/6/6 member = move id, crate::effscript):
//!
//! ```text
//! <move id> = <move whose script it plays>
//! ```
//!
//! The value is a base-game move (1..559) or a move whose script the mod
//! ships. A move with no line plays its own script when it has one (every
//! base-game move does; an added move when the mod ships one), else the
//! script of the move it works like in battle (the first base in
//! `move_handlers.txt`) when that is a base-game move, else Pound's.

use std::collections::BTreeMap;

use crate::bridge::{Bridge, Error, Member};
use crate::project::Project;

const TABLE: &str = "move_anims";
/// The last base-game move: the last one with an animation of its own.
pub const LAST: u16 = 559;
/// The archive of move scripts.
pub const SCRIPTS: u16 = 66;

fn parse(line: &str) -> Option<u16> {
    let v: u16 = line.split('#').next()?.trim().parse().ok()?;
    (v >= 1).then_some(v)
}

/// Whether the mod ships a script for this move.
pub fn has_own(project: &Project, id: usize) -> bool {
    project.member(SCRIPTS, id).is_some()
}

/// Whether a move has a script to play: a base-game move, or one the mod
/// gives a script.
fn playable(project: &Project, id: usize) -> bool {
    (1..=LAST as usize).contains(&id) || has_own(project, id)
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
    if playable(project, id) {
        return id as u16;
    }
    crate::handlers::rows(project, "move")
        .get(&id)
        .and_then(|row| row.first())
        .map(|p| p.base)
        .filter(|&b| b != id && (1..=LAST as usize).contains(&b))
        .map_or(1, |b| b as u16)
}

/// After a move's battle effect changed: a move with no line that played
/// `before` and now falls back to Pound (its new base is an effect eidos
/// adds by name, or no game move) keeps `before` as its own line.
pub fn keep_after_handler_change(project: &Project, id: usize, before: u16) -> Result<(), String> {
    let mut rows = all(project);
    if rows.contains_key(&id) || playable(project, id) {
        return Ok(());
    }
    if default_for(project, id) == 1 && before != 1 {
        rows.insert(id, before);
        write(project, &rows)?;
    }
    Ok(())
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
            "<move id> = <the move whose animation script it plays>.",
        )
        .map_err(|e| e.to_string())
}

/// Sets a move's animation; None, or the default, removes the line so the
/// move follows its default (and a later change of what it works like).
pub fn set(project: &Project, id: usize, anim: Option<u16>) -> Result<(), String> {
    if let Some(a) = anim {
        if !playable(project, a as usize) {
            return Err(format!(
                "move {a} has no animation: one of the game's moves (1 to {LAST}) or a move with its own"
            ));
        }
    }
    let mut rows = all(project);
    match anim.filter(|&a| a != default_for(project, id)) {
        Some(a) => rows.insert(id, a),
        None => rows.remove(&id),
    };
    write(project, &rows)
}

/// After an added move is removed: its line goes, later moves' lines move
/// down one, and lines that played its script play the default again (later
/// added moves' values move down too). Its script goes and later added
/// moves' scripts move down one id. Returns whether anything changed.
pub fn remove_id(project: &Project, id: usize) -> Result<bool, String> {
    let old = all(project);
    let mut changed = false;
    let shifted: BTreeMap<usize, u16> = old
        .iter()
        .filter(|&(&i, &v)| i != id && v as usize != id)
        .map(|(&i, &v)| {
            let i = if i > id { i - 1 } else { i };
            let v = if v as usize > id && v > LAST {
                v - 1
            } else {
                v
            };
            (i, v)
        })
        .collect();
    if shifted != old {
        write(project, &shifted)?;
        changed = true;
    }
    // The scripts: a/0/6/6 members of later added moves move down one.
    let last = project.last_member(SCRIPTS).unwrap_or(0);
    if last >= id && (id as u16) > LAST {
        let io = |e: std::io::Error| e.to_string();
        for v in id + 1..=last {
            match project.member(SCRIPTS, v) {
                Some(b) => project.write_member(SCRIPTS, v - 1, &b).map_err(io)?,
                None => project.revert_member(SCRIPTS, v - 1).map_err(io)?,
            }
        }
        project.revert_member(SCRIPTS, last).map_err(io)?;
        changed = true;
    }
    Ok(changed)
}

/// A move's script as text: the mod's, else the game's (None for an added
/// move without one).
pub fn script_text(bridge: &Bridge, id: usize) -> Result<Option<String>, Error> {
    let project = bridge.project();
    if !(1..=LAST as usize).contains(&id) && !project.is_some_and(|p| has_own(p, id)) {
        return Ok(None);
    }
    let b = bridge.member(Member {
        arc: SCRIPTS,
        index: id,
    })?;
    crate::effscript::disassemble(&b)
        .map(Some)
        .map_err(Error::Format)
}

/// Writes a move's script from text (the game's own drops the mod's file).
pub fn set_script(bridge: &Bridge, id: usize, text: &str) -> Result<(), Error> {
    let bytes = crate::effscript::assemble(text).map_err(Error::BadPath)?;
    crate::catalog::write_member(
        bridge,
        Member {
            arc: SCRIPTS,
            index: id,
        },
        &bytes,
    )
}

/// Gives a move its own script, a copy of `from`'s script, and removes a
/// line that pointed it elsewhere.
pub fn copy_script(bridge: &Bridge, id: usize, from: usize) -> Result<(), Error> {
    let project = bridge
        .project()
        .ok_or_else(|| Error::BadPath("no project is open".into()))?;
    if !playable(project, from) {
        return Err(Error::BadPath(format!(
            "move {from} has no animation to copy"
        )));
    }
    let bytes = bridge.member_stored(Member {
        arc: SCRIPTS,
        index: from,
    })?;
    project
        .write_member(SCRIPTS, id, &bytes)
        .map_err(|e| Error::Missing(e.to_string()))?;
    let mut rows = all(project);
    rows.remove(&id);
    write(project, &rows).map_err(Error::Missing)?;
    bridge.invalidate();
    Ok(())
}

/// Drops the mod's script for a move (a game move plays the game's again).
pub fn drop_script(bridge: &Bridge, id: usize) -> Result<(), Error> {
    let project = bridge
        .project()
        .ok_or_else(|| Error::BadPath("no project is open".into()))?;
    project
        .revert_member(SCRIPTS, id)
        .map_err(|e| Error::Missing(e.to_string()))?;
    // Lines that played this added move's script play their default again.
    if id > LAST as usize {
        let rows: BTreeMap<usize, u16> = all(project)
            .into_iter()
            .filter(|&(_, v)| v as usize != id)
            .collect();
        write(project, &rows).map_err(Error::Missing)?;
    }
    bridge.invalidate();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values() {
        assert_eq!(parse(" 85 # Thunderbolt"), Some(85));
        assert_eq!(parse("600"), Some(600));
        assert_eq!(parse("0"), None);
        assert_eq!(parse("85 86"), None);
    }
}

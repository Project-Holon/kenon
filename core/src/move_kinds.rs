//! Move kinds the later games test that Black and White's move record has no
//! flag for (eidos `eidos/tables/move_kinds.txt`, 0.2.8): bite, pulse, ball,
//! powder. Abilities such as Strong Jaw or Bulletproof ask for them.
//!
//! ```text
//! <move id> = <kind> [<kind> ...]       # or none
//! ```
//!
//! A line replaces the move's kinds; without one a game move has the later
//! games' kinds (GAME below, as eidos sdk/move_kinds.c gives them).

use std::collections::BTreeMap;

use crate::bridge::Error;
use crate::project::Project;

const TABLE: &str = "move_kinds";
pub const KINDS: [&str; 4] = ["bite", "pulse", "ball", "powder"];

/// The game's moves of each kind (later games' lists).
const GAME: [(&str, &[usize]); 4] = [
    ("bite", &[44, 242, 422, 423, 424, 305, 158]),
    ("pulse", &[352, 399, 406, 396, 505]),
    (
        "ball",
        &[
            491, 396, 140, 331, 121, 486, 412, 411, 360, 301, 443, 296, 426, 190, 439, 545, 402,
            247, 188, 311, 192,
        ],
    ),
    ("powder", &[77, 78, 79, 147, 178, 476]),
];

/// A move's kinds without a line of the mod's.
pub fn game_kinds(id: usize) -> Vec<String> {
    GAME.iter()
        .filter(|(_, ids)| ids.contains(&id))
        .map(|(k, _)| k.to_string())
        .collect()
}

/// A move's kinds in this mod.
pub fn kinds(project: &Project, id: usize) -> Vec<String> {
    match project.table_lines(TABLE).get(&id) {
        Some(line) => line
            .split('#')
            .next()
            .unwrap_or("")
            .split_whitespace()
            .filter(|w| KINDS.contains(w))
            .map(str::to_string)
            .collect(),
        None => game_kinds(id),
    }
}

/// Sets a move's kinds; the game's own removes the line.
pub fn set(project: &Project, id: usize, kinds: &[String]) -> Result<(), Error> {
    if let Some(bad) = kinds.iter().find(|k| !KINDS.contains(&k.as_str())) {
        return Err(Error::BadPath(format!("{bad} is not a move kind")));
    }
    let mut want: Vec<String> = KINDS
        .iter()
        .filter(|k| kinds.iter().any(|x| x == *k))
        .map(|k| k.to_string())
        .collect();
    want.dedup();
    let mut rows: BTreeMap<usize, String> = project.table_lines(TABLE);
    if want == game_kinds(id) {
        rows.remove(&id);
    } else {
        rows.insert(
            id,
            if want.is_empty() {
                "none".into()
            } else {
                want.join(" ")
            },
        );
    }
    project
        .write_table_lines(
            TABLE,
            &rows,
            "<move id> = <bite|pulse|ball|powder ...> or none.",
        )
        .map_err(|e| Error::Missing(e.to_string()))
}

/// After an added move is removed: its line goes, later ones move down.
pub fn removed(project: &Project, id: usize) -> Result<bool, Error> {
    let rows = project.table_lines(TABLE);
    if !rows.keys().any(|&i| i >= id) {
        return Ok(false);
    }
    let shifted = rows
        .into_iter()
        .filter(|(i, _)| *i != id)
        .map(|(i, l)| (if i > id { i - 1 } else { i }, l))
        .collect();
    project
        .write_table_lines(
            TABLE,
            &shifted,
            "<move id> = <bite|pulse|ball|powder ...> or none.",
        )
        .map_err(|e| Error::Missing(e.to_string()))?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_follow_the_games_kinds() {
        let dir = tempfile::tempdir().unwrap();
        let project = Project::create(&dir.path().join("m"), "Test", None).unwrap();
        assert_eq!(kinds(&project, 44), vec!["bite"]);
        assert_eq!(kinds(&project, 396), vec!["pulse", "ball"]);
        set(&project, 44, &[]).unwrap();
        assert!(kinds(&project, 44).is_empty());
        set(&project, 560, &["bite".into()]).unwrap();
        let text =
            std::fs::read_to_string(dir.path().join("m/eidos/tables/move_kinds.txt")).unwrap();
        assert!(text.contains("44 = none\n560 = bite\n"), "{text}");
        set(&project, 44, &["bite".into()]).unwrap();
        assert!(
            !std::fs::read_to_string(dir.path().join("m/eidos/tables/move_kinds.txt"))
                .unwrap()
                .contains("44 =")
        );
        assert!(set(&project, 1, &["slash".into()]).is_err());
    }
}

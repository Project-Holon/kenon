//! Poke Balls (eidos `eidos/tables/balls.txt`, 0.2.8): an item made a ball
//! of its own, or a game ball tuned.
//!
//! ```text
//! <item> = <base ball item> [ball=<26..255>] [key=value ...]
//! ```
//!
//! The base is one of the game's 25 ball items: the ball it works like. An
//! added ball has a ball id of its own (kept on the Pokemon it catches) and
//! its own files: six particle files (a/0/0/6), the shaking ball's character
//! file and palette (a/0/9/5) and the summary icon's palette and character
//! file (a/0/8/3). kenon copies the base's, to recolour.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::bridge::{Bridge, Error, Member};
use crate::project::Project;

const TABLE: &str = "balls";
/// The game's balls (ids 1 to 25).
pub const GAME_BALLS: u16 = 25;
const SPRITES: u16 = 95;
const ICONS: u16 = 83;

/// The keys a line may take, by its base ball (eidos docs/game-tables.md).
pub fn keys_for(base_ball: u16) -> Vec<&'static str> {
    let mut out: Vec<&str> = match base_ball {
        1 => vec![],
        6 => vec!["ratio", "type", "type.2"],
        8 => vec!["ratio", "level"],
        10 => vec!["ratio", "per_turn"],
        15 => vec!["ratio", "turns"],
        _ => vec!["ratio"],
    };
    out.extend(["heal", "friendship", "friendship_bonus"]);
    out
}

/// One line.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Ball {
    /// The game ball item it works like.
    pub base: u16,
    /// Its own ball id (added balls only).
    pub ball: Option<u16>,
    pub keys: BTreeMap<String, String>,
    pub particles: Vec<usize>,
    pub sprite: Vec<usize>,
    pub icon: Vec<usize>,
}

fn list(v: &str) -> Vec<usize> {
    v.split(',').filter_map(|x| x.trim().parse().ok()).collect()
}

fn parse(line: &str) -> Option<Ball> {
    let mut words = line.split('#').next()?.split_whitespace();
    let mut b = Ball {
        base: words.next()?.parse().ok()?,
        ..Default::default()
    };
    for w in words {
        let (k, v) = w.split_once('=')?;
        match k {
            "ball" => b.ball = v.parse().ok(),
            "particles" => b.particles = list(v),
            "sprite" => b.sprite = list(v),
            "icon" => b.icon = list(v),
            _ => {
                b.keys.insert(k.to_string(), v.to_string());
            }
        }
    }
    Some(b)
}

fn format(b: &Ball) -> String {
    let mut s = b.base.to_string();
    if let Some(id) = b.ball {
        s += &format!(" ball={id}");
    }
    for (k, v) in &b.keys {
        s += &format!(" {k}={v}");
    }
    let join = |v: &[usize]| v.iter().map(usize::to_string).collect::<Vec<_>>().join(",");
    if !b.particles.is_empty() {
        s += &format!(" particles={}", join(&b.particles));
    }
    if !b.sprite.is_empty() {
        s += &format!(" sprite={}", join(&b.sprite));
    }
    if !b.icon.is_empty() {
        s += &format!(" icon={}", join(&b.icon));
    }
    s
}

pub fn all(project: &Project) -> BTreeMap<usize, Ball> {
    project
        .table_lines(TABLE)
        .into_iter()
        .filter_map(|(i, l)| Some((i, parse(&l)?)))
        .collect()
}

fn write(project: &Project, rows: &BTreeMap<usize, Ball>) -> Result<(), Error> {
    let lines = rows.iter().map(|(i, b)| (*i, format(b))).collect();
    project
        .write_table_lines(
            TABLE,
            &lines,
            "<item> = <base ball item> [ball=<26..255>] [key=value ...].",
        )
        .map_err(|e| Error::Missing(e.to_string()))
}

/// The game's ball items: item to ball id (the item-to-ball table).
pub fn game_balls(bridge: &Bridge) -> BTreeMap<u16, u16> {
    let Some(table) = bridge.code().and_then(|c| {
        c.table("ball_items", GAME_BALLS as usize * 4)
            .map(<[u8]>::to_vec)
    }) else {
        return BTreeMap::new();
    };
    table
        .as_chunks::<4>()
        .0
        .iter()
        .map(|p| {
            (
                u16::from_le_bytes([p[0], p[1]]),
                u16::from_le_bytes([p[2], p[3]]),
            )
        })
        .filter(|&(_, b)| (1..=GAME_BALLS).contains(&b))
        .collect()
}

/// A game ball's files by its ball id: six particle files, sprite
/// (character, palette), icon (palette, character).
fn game_files(b: u16) -> (Vec<usize>, Vec<usize>, Vec<usize>) {
    let i = (b - 1) as usize;
    let second = if b == 25 {
        0x2D
    } else if b >= 16 {
        0x20
    } else {
        0x1D + i
    };
    let particles = vec![
        3 + i,
        second,
        0x2E + 4 * i,
        0x2F + 4 * i,
        0x30 + 4 * i,
        0x31 + 4 * i,
    ];
    let s = if b == 25 { 17 } else { b as usize };
    (
        particles,
        vec![0x81 + 2 * s, 0x82 + 2 * s],
        vec![0x56 + b as usize, 0x6F + b as usize],
    )
}

/// The next member past an archive's game members and the mod's.
fn next_member(bridge: &Bridge, project: &Project, arc: u16) -> Result<usize, Error> {
    let base = bridge.base_archive(arc)?.len();
    Ok(project.last_member(arc).map_or(base, |l| base.max(l + 1)))
}

fn copy_member(bridge: &Bridge, project: &Project, arc: u16, from: usize) -> Result<usize, Error> {
    let to = next_member(bridge, project, arc)?;
    let bytes = bridge.member_stored(Member { arc, index: from })?;
    project
        .write_member(arc, to, &bytes)
        .map_err(|e| Error::Missing(e.to_string()))?;
    Ok(to)
}

/// After an item copied from a ball (a game ball or an added one) is added:
/// it becomes a ball of its own, working like the source's base, with the
/// next ball id and copies of the source's files. Nothing for other items.
pub fn item_added(bridge: &Bridge, item: usize, from: usize) -> Result<bool, Error> {
    let project = bridge
        .project()
        .ok_or_else(|| Error::BadPath("no project is open".into()))?;
    let game = game_balls(bridge);
    let mut rows = all(project);
    // The source: a game ball, or an added ball (its base and own files).
    let (base, keys, files) = if let Some(&b) = game.get(&(from as u16)) {
        let (p, s, i) = game_files(b);
        (from as u16, BTreeMap::new(), (p, s, i))
    } else if let Some(src) = rows.get(&from).cloned() {
        let b = *game
            .get(&src.base)
            .ok_or_else(|| Error::Missing("the base ball is unknown".into()))?;
        let (gp, gs, gi) = game_files(b);
        let pick =
            |own: &Vec<usize>, game: Vec<usize>| if own.is_empty() { game } else { own.clone() };
        (
            src.base,
            src.keys.clone(),
            (
                pick(&src.particles, gp),
                pick(&src.sprite, gs),
                pick(&src.icon, gi),
            ),
        )
    } else {
        return Ok(false);
    };
    let used: Vec<u16> = rows.values().filter_map(|b| b.ball).collect();
    let ball = (GAME_BALLS + 1..=255)
        .find(|b| !used.contains(b))
        .ok_or_else(|| Error::BadPath("no ball ids left (26 to 255)".into()))?;
    let mut particles = Vec::new();
    for &f in &files.0 {
        particles.push(crate::spa::add(bridge, f)?);
    }
    let mut sprite = Vec::new();
    for &f in &files.1 {
        sprite.push(copy_member(bridge, project, SPRITES, f)?);
    }
    let mut icon = Vec::new();
    for &f in &files.2 {
        icon.push(copy_member(bridge, project, ICONS, f)?);
    }
    rows.insert(
        item,
        Ball {
            base,
            ball: Some(ball),
            keys,
            particles,
            sprite,
            icon,
        },
    );
    write(project, &rows)?;
    bridge.invalidate();
    Ok(true)
}

/// Sets a ball's base and keys (its id and files stay).
pub fn set(
    bridge: &Bridge,
    item: usize,
    base: u16,
    keys: BTreeMap<String, String>,
) -> Result<(), Error> {
    let project = bridge
        .project()
        .ok_or_else(|| Error::BadPath("no project is open".into()))?;
    let game = game_balls(bridge);
    let b = *game
        .get(&base)
        .ok_or_else(|| Error::BadPath(format!("item {base} is not one of the game's balls")))?;
    let allowed = keys_for(b);
    if let Some(k) = keys.keys().find(|k| !allowed.contains(&k.as_str())) {
        return Err(Error::BadPath(format!("this ball takes no {k}")));
    }
    let mut rows = all(project);
    let mut line = rows.get(&item).cloned().unwrap_or_default();
    line.base = base;
    line.keys = keys
        .into_iter()
        .filter(|(_, v)| !v.trim().is_empty())
        .collect();
    let is_game = game.contains_key(&(item as u16));
    if is_game && line.base == item as u16 && line.keys.is_empty() {
        rows.remove(&item);
    } else {
        rows.insert(item, line);
    }
    write(project, &rows)?;
    bridge.invalidate();
    Ok(())
}

/// A ball's line, or a game ball's defaults.
pub fn get(bridge: &Bridge, item: usize) -> Option<Ball> {
    let project = bridge.project()?;
    if let Some(b) = all(project).get(&item) {
        return Some(b.clone());
    }
    game_balls(bridge)
        .contains_key(&(item as u16))
        .then(|| Ball {
            base: item as u16,
            ..Default::default()
        })
}

/// After an added item is removed: its line goes, later items move down.
pub fn item_removed(project: &Project, id: usize) -> Result<bool, Error> {
    let rows = all(project);
    if !rows.keys().any(|&i| i >= id) {
        return Ok(false);
    }
    let shifted = rows
        .into_iter()
        .filter(|(i, _)| *i != id)
        .map(|(i, b)| (if i > id { i - 1 } else { i }, b))
        .collect();
    write(project, &shifted)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_read_back() {
        let line =
            "3 ball=29 ratio=2.5 particles=733,734,735,736,737,738 sprite=165,166 icon=172,173";
        let b = parse(line).unwrap();
        assert_eq!(
            (b.base, b.ball, b.particles.len(), b.keys["ratio"].as_str()),
            (3, Some(29), 6, "2.5")
        );
        assert_eq!(format(&b), line);
        // Ball 4 (Poke Ball), 16 (shares 0x20), 25 (Dream Ball).
        assert_eq!(game_files(4).0, vec![6, 0x20, 0x3A, 0x3B, 0x3C, 0x3D]);
        assert_eq!(game_files(16).0[1], 0x20);
        assert_eq!(
            (game_files(25).0[1], game_files(25).1[0]),
            (0x2D, 0x81 + 34)
        );
    }
}

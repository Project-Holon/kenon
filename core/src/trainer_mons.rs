//! Per-member numbers for trainers' Pokemon, the game's data has no room
//! for: `eidos/tables/trainer_mons.txt`, one line per member (eidos
//! docs/trainer-mons.md):
//!
//! ```text
//! <trainer id>.<slot> = [key=value ...]
//! 164.0 = ivs=31,31,31,31,31,31 evs=252,0,4,0,0,252 nature=jolly
//! ```
//!
//! Every key is optional; a member with no line is the game's own.

use std::collections::BTreeMap;
use std::fs;
use std::io;

use serde::{Deserialize, Serialize};

use crate::project::Project;

const FILE: &str = "eidos/tables/trainer_mons.txt";

/// One member's numbers; None is the game's own.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Extras {
    pub ivs: Option<[u8; 6]>,
    pub evs: Option<[u8; 6]>,
    /// 0 (Hardy) to 24 (Quirky).
    pub nature: Option<u8>,
    pub shiny: Option<bool>,
    pub friendship: Option<u8>,
    /// "1", "2" or "hidden".
    pub ability: Option<String>,
}

impl Extras {
    pub fn is_empty(&self) -> bool {
        *self == Extras::default()
    }
}

fn six(v: &str, max: u32) -> Option<[u8; 6]> {
    let n: Vec<u32> = v
        .split(',')
        .map(|x| x.trim().parse().ok())
        .collect::<Option<_>>()?;
    if n.len() != 6 || n.iter().any(|&x| x > max) {
        return None;
    }
    Some([
        n[0] as u8, n[1] as u8, n[2] as u8, n[3] as u8, n[4] as u8, n[5] as u8,
    ])
}

/// The nature names eidos reads, in number order.
pub const NATURES: [&str; 25] = [
    "hardy", "lonely", "brave", "adamant", "naughty", "bold", "docile", "relaxed", "impish", "lax",
    "timid", "hasty", "serious", "jolly", "naive", "modest", "mild", "quiet", "bashful", "rash",
    "calm", "gentle", "sassy", "careful", "quirky",
];

fn parse(rest: &str) -> Extras {
    let mut e = Extras::default();
    for word in rest.split('#').next().unwrap_or("").split_whitespace() {
        let Some((k, v)) = word.split_once('=') else {
            continue;
        };
        match k {
            "ivs" => e.ivs = six(v, 31),
            "evs" => e.evs = six(v, 255),
            "nature" => {
                e.nature = v.parse::<u8>().ok().filter(|&n| n < 25).or_else(|| {
                    NATURES
                        .iter()
                        .position(|&n| n == v.to_lowercase())
                        .map(|n| n as u8)
                })
            }
            "shiny" => e.shiny = Some(v == "1"),
            "friendship" => e.friendship = v.parse().ok(),
            "ability" if matches!(v, "1" | "2" | "hidden") => e.ability = Some(v.to_string()),
            _ => {}
        }
    }
    e
}

fn format(e: &Extras) -> String {
    let list = |a: &[u8; 6]| a.map(|x| x.to_string()).join(",");
    let mut out = Vec::new();
    if let Some(a) = &e.ivs {
        out.push(format!("ivs={}", list(a)));
    }
    if let Some(a) = &e.evs {
        out.push(format!("evs={}", list(a)));
    }
    if let Some(n) = e.nature {
        out.push(format!("nature={}", NATURES[n as usize % 25]));
    }
    if let Some(s) = e.shiny {
        out.push(format!("shiny={}", u8::from(s)));
    }
    if let Some(f) = e.friendship {
        out.push(format!("friendship={f}"));
    }
    if let Some(a) = &e.ability {
        out.push(format!("ability={a}"));
    }
    out.join(" ")
}

/// Every line of the mod's table: (trainer, slot) to its numbers.
pub fn all(project: &Project) -> BTreeMap<(usize, usize), Extras> {
    let Ok(text) = fs::read_to_string(project.root.join(FILE)) else {
        return BTreeMap::new();
    };
    text.lines()
        .filter_map(|line| {
            let (key, rest) = line.split_once('=')?;
            let (t, s) = key.trim().split_once('.')?;
            Some(((t.parse().ok()?, s.parse().ok()?), parse(rest)))
        })
        .filter(|(_, e)| !e.is_empty())
        .collect()
}

/// One trainer's members, slot by slot (six, None where the game's own).
pub fn of(project: &Project, trainer: usize) -> Vec<Extras> {
    let all = all(project);
    (0..6)
        .map(|s| all.get(&(trainer, s)).cloned().unwrap_or_default())
        .collect()
}

/// Sets one trainer's members (slot by slot; empty ones are dropped). No
/// lines left removes the file.
pub fn set(project: &Project, trainer: usize, members: &[Extras]) -> io::Result<()> {
    // eidos drops a whole line whose EVs pass 510: refuse it here instead.
    for (slot, e) in members.iter().enumerate() {
        let total: u32 = e.evs.map_or(0, |v| v.iter().map(|&x| u32::from(x)).sum());
        if total > 510 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("member {}: EVs add up to {total}, more than 510", slot + 1),
            ));
        }
    }
    let mut rows = all(project);
    rows.retain(|&(t, _), _| t != trainer);
    for (slot, e) in members.iter().enumerate().take(6) {
        if !e.is_empty() {
            rows.insert((trainer, slot), e.clone());
        }
    }
    let path = project.root.join(FILE);
    if rows.is_empty() {
        if path.is_file() {
            fs::remove_file(path)?;
        }
        return Ok(());
    }
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let mut text = String::from("# Written by kenon: <trainer>.<slot> = ivs evs nature shiny friendship ability (eidos docs/trainer-mons.md).\n");
    for ((t, s), e) in &rows {
        text.push_str(&format!("{t}.{s} = {}\n", format(e)));
    }
    eidos_mod::manifest::write_atomic(&path, text.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_round_trip() {
        let e = parse("ivs=31,31,31,31,31,31 evs=252,0,4,0,0,252 nature=jolly shiny=1 friendship=0 ability=hidden");
        assert_eq!(e.ivs, Some([31; 6]));
        assert_eq!(e.nature, Some(13));
        assert_eq!(e.shiny, Some(true));
        assert_eq!(e.ability.as_deref(), Some("hidden"));
        assert_eq!(parse(&format(&e)), e);
        // Out of range or malformed values are left out, as eidos does.
        assert_eq!(
            parse("ivs=32,0,0,0,0,0 nature=25 ability=3"),
            Extras::default()
        );
    }

    #[test]
    fn set_writes_and_refuses_too_many_evs() {
        let dir = tempfile::tempdir().unwrap();
        let project = Project::create(&dir.path().join("m"), "T", None).unwrap();
        let jolly = Extras {
            ivs: Some([31; 6]),
            nature: Some(13),
            ..Extras::default()
        };
        set(
            &project,
            164,
            &[
                jolly.clone(),
                Extras::default(),
                Extras {
                    shiny: Some(true),
                    ..Extras::default()
                },
            ],
        )
        .unwrap();
        let text = fs::read_to_string(project.root.join(FILE)).unwrap();
        assert!(
            text.contains("164.0 = ivs=31,31,31,31,31,31 nature=jolly\n"),
            "{text}"
        );
        assert!(text.contains("164.2 = shiny=1\n"), "{text}");
        assert!(!text.contains("164.1"), "{text}");
        assert_eq!(of(&project, 164)[0], jolly);
        let heavy = Extras {
            evs: Some([252, 252, 252, 0, 0, 0]),
            ..Extras::default()
        };
        assert!(set(&project, 164, &[heavy]).is_err());
        // Clearing every member removes the file.
        set(&project, 164, &[]).unwrap();
        assert!(!project.root.join(FILE).exists());
    }
}

//! How much HP an item heals (eidos `eidos/tables/item_heal.txt`, 0.2.8):
//!
//! ```text
//! <item> = [hp=<1..65535 | 1/2..1/16 | full>] [held_hp=<1..65535>]
//! ```
//!
//! The item record keeps the amount in one byte (hpRestoreAmount: 1 to 252
//! HP, 253 a quarter, 254 half, 255 all) and a held berry's fixed heal in
//! another (holdEffectParam, 1 to 255). A value that fits goes in the
//! record; one that does not goes in the table, with the record's nearest
//! value for an eidos that does not read it.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::bridge::{Bridge, Error, Member};

const TABLE: &str = "item_heal";
const HP_BYTE: usize = 29;
const HELD_BYTE: usize = 3;
const MAX_FIXED_BYTE: u32 = 252;

/// An amount of HP.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "camelCase")]
pub enum Hp {
    /// 1 to 65535 HP.
    Fixed(u32),
    /// 1/n of max HP, n from 2 to 16.
    Fraction(u32),
    Full,
}

impl Hp {
    fn parse(s: &str) -> Option<Hp> {
        if s == "full" {
            return Some(Hp::Full);
        }
        if let Some(n) = s.strip_prefix("1/") {
            return n
                .parse()
                .ok()
                .filter(|n| (2..=16).contains(n))
                .map(Hp::Fraction);
        }
        s.parse()
            .ok()
            .filter(|n| (1..=65535).contains(n))
            .map(Hp::Fixed)
    }

    fn word(self) -> String {
        match self {
            Hp::Fixed(n) => n.to_string(),
            Hp::Fraction(n) => format!("1/{n}"),
            Hp::Full => "full".into(),
        }
    }

    /// The record's byte: exact when it fits, else the nearest it holds.
    fn byte(self) -> (u8, bool) {
        match self {
            Hp::Fixed(n) if n <= MAX_FIXED_BYTE => (n as u8, true),
            Hp::Fixed(_) => (MAX_FIXED_BYTE as u8, false),
            Hp::Full => (255, true),
            Hp::Fraction(2) => (254, true),
            Hp::Fraction(4) => (253, true),
            Hp::Fraction(n) => (if n < 3 { 254 } else { 253 }, false),
        }
    }

    fn of_byte(b: u8) -> Option<Hp> {
        match b {
            0 => None,
            253 => Some(Hp::Fraction(4)),
            254 => Some(Hp::Fraction(2)),
            255 => Some(Hp::Full),
            n => Some(Hp::Fixed(n as u32)),
        }
    }
}

/// An item's heals: from a bag or party use, and held.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Heal {
    pub hp: Option<Hp>,
    pub held: Option<u32>,
}

fn parse_line(line: &str) -> Heal {
    let mut h = Heal::default();
    for w in line.split('#').next().unwrap_or("").split_whitespace() {
        if let Some(v) = w.strip_prefix("hp=") {
            h.hp = Hp::parse(v);
        } else if let Some(v) = w.strip_prefix("held_hp=") {
            h.held = v.parse().ok().filter(|n| (1..=65535).contains(n));
        }
    }
    h
}

fn format_line(h: &Heal) -> String {
    let mut parts = Vec::new();
    if let Some(hp) = h.hp {
        parts.push(format!("hp={}", hp.word()));
    }
    if let Some(n) = h.held {
        parts.push(format!("held_hp={n}"));
    }
    parts.join(" ")
}

fn items_arc() -> Result<u16, Error> {
    Ok(crate::tables::table("items")?.archive)
}

/// An item's heals: the table's values over the record's.
pub fn get(bridge: &Bridge, item: usize) -> Result<Heal, Error> {
    let rec = bridge.member(Member {
        arc: items_arc()?,
        index: item,
    })?;
    let line = bridge
        .project()
        .and_then(|p| p.table_lines(TABLE).get(&item).map(|l| parse_line(l)))
        .unwrap_or_default();
    let held = rec
        .get(HELD_BYTE)
        .copied()
        .filter(|&b| b > 0)
        .map(u32::from);
    Ok(Heal {
        hp: line
            .hp
            .or_else(|| rec.get(HP_BYTE).copied().and_then(Hp::of_byte)),
        held: line.held.or(held),
    })
}

/// Sets an item's heals: the record's bytes, and a table line for what
/// they cannot hold.
pub fn set(bridge: &Bridge, item: usize, heal: Heal) -> Result<(), Error> {
    let project = bridge
        .project()
        .ok_or_else(|| Error::BadPath("no project is open".into()))?;
    if let Some(Hp::Fixed(0) | Hp::Fraction(0..=1 | 17..)) = heal.hp {
        return Err(Error::BadPath("HP is 1 to 65535, or 1/2 to 1/16".into()));
    }
    if heal
        .hp
        .is_some_and(|h| matches!(h, Hp::Fixed(n) if n > 65535))
        || heal.held.is_some_and(|n| n == 0 || n > 65535)
    {
        return Err(Error::BadPath("HP is 1 to 65535".into()));
    }
    let arc = items_arc()?;
    let mut rec = bridge.member(Member { arc, index: item })?;
    let (hp_byte, hp_fits) = heal.hp.map_or((0, true), Hp::byte);
    let held_fits = heal.held.is_none_or(|n| n <= 255);
    rec[HP_BYTE] = hp_byte;
    rec[HELD_BYTE] = heal.held.map_or(0, |n| n.min(255) as u8);
    crate::catalog::write_member(bridge, Member { arc, index: item }, &rec)?;
    let line = Heal {
        hp: heal.hp.filter(|_| !hp_fits),
        held: heal.held.filter(|_| !held_fits),
    };
    let mut rows: BTreeMap<usize, String> = project.table_lines(TABLE);
    if line == Heal::default() {
        rows.remove(&item);
    } else {
        rows.insert(item, format_line(&line));
    }
    project
        .write_table_lines(
            TABLE,
            &rows,
            "<item> = [hp=<1..65535 | 1/2..1/16 | full>] [held_hp=<1..65535>]",
        )
        .map_err(|e| Error::Missing(e.to_string()))?;
    bridge.invalidate();
    Ok(())
}

/// After an added item is removed: its line goes, later ones move down.
pub fn item_removed(project: &crate::project::Project, id: usize) -> Result<bool, Error> {
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
            "<item> = [hp=<1..65535 | 1/2..1/16 | full>] [held_hp=<1..65535>]",
        )
        .map_err(|e| Error::Missing(e.to_string()))?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_and_bytes() {
        assert_eq!(
            parse_line("hp=1/3 held_hp=300"),
            Heal {
                hp: Some(Hp::Fraction(3)),
                held: Some(300)
            }
        );
        assert_eq!(
            format_line(&Heal {
                hp: Some(Hp::Full),
                held: None
            }),
            "hp=full"
        );
        assert_eq!(Hp::Fixed(500).byte(), (252, false));
        assert_eq!(Hp::Fraction(4).byte(), (253, true));
        assert_eq!(Hp::Fraction(3).byte(), (253, false));
        assert_eq!(Hp::of_byte(254), Some(Hp::Fraction(2)));
        assert_eq!(Hp::parse("1/17"), None);
    }
}

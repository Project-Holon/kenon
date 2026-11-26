//! Shops: the game's 26 Poke Mart lists and 3 Battle Subway prize lists in
//! overlay 21, and the mod's lines in `eidos/tables/shops.txt` (eidos
//! docs/game-tables.md):
//!
//! ```text
//! mart.<n> = <item> <item> ...       n = 0..25
//! bp.<n> = <item>:<price> ...        n = 0..2
//! ```
//!
//! A mart item sells at its item-data price. The main clerk picks list
//! 0..5 by badges (0, 1-2, 3-4, 5-6, 7, 8); lists 6..25 are the other
//! clerks. A list with no line is the game's.

use std::collections::BTreeMap;
use std::fs;

use serde::{Deserialize, Serialize};

use crate::bridge::{Bridge, Error};

const FILE: &str = "eidos/tables/shops.txt";
pub const MARTS: usize = 26;
pub const PRIZES: usize = 3;
/// The shop's own buffer (eidos refuses more).
pub const MAX_ITEMS: usize = 40;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Shops {
    pub marts: Vec<Vec<u16>>,
    /// (item, BP price).
    pub prizes: Vec<Vec<(u16, u16)>>,
}

fn word(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

/// The game's lists, read from overlay 21 for this cartridge.
pub fn game(bridge: &Bridge) -> Result<Shops, Error> {
    let code = bridge
        .code()
        .ok_or_else(|| Error::Missing("the game's code is not loaded".into()))?;
    let missing = || Error::Missing("the shop lists are not known for this cartridge".into());
    let (image, counts_at) = code.table_at("shop_counts").ok_or_else(missing)?;
    let counts = code
        .read(image, counts_at, MARTS)
        .ok_or_else(missing)?
        .to_vec();
    let ptrs = code
        .table("shop_lists", MARTS * 4)
        .ok_or_else(missing)?
        .to_vec();
    let mut marts = Vec::new();
    for (n, &count) in counts.iter().enumerate() {
        let at = word(&ptrs, n * 4);
        let raw = code
            .read(image, at, count as usize * 2)
            .ok_or_else(missing)?;
        marts.push(
            raw.chunks(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect(),
        );
    }
    let bp_counts = code
        .table("bp_counts", PRIZES)
        .ok_or_else(missing)?
        .to_vec();
    let bp_ptrs = code
        .table("bp_lists", PRIZES * 4)
        .ok_or_else(missing)?
        .to_vec();
    let mut prizes = Vec::new();
    for (n, &count) in bp_counts.iter().enumerate() {
        let at = word(&bp_ptrs, n * 4);
        let raw = code
            .read(image, at, count as usize * 8)
            .ok_or_else(missing)?;
        prizes.push(
            raw.chunks(8)
                .map(|c| (word(c, 0) as u16, word(c, 4) as u16))
                .collect(),
        );
    }
    Ok(Shops { marts, prizes })
}

enum Line {
    Mart(usize, Vec<u16>),
    Prize(usize, Vec<(u16, u16)>),
}

fn parse(line: &str) -> Option<Line> {
    let line = line.split('#').next()?;
    let (k, v) = line.split_once('=')?;
    let (kind, n) = k.trim().split_once('.')?;
    let n: usize = n.parse().ok()?;
    match kind {
        "mart" if n < MARTS => Some(Line::Mart(
            n,
            v.split_whitespace()
                .map(|x| x.parse().ok())
                .collect::<Option<_>>()?,
        )),
        "bp" if n < PRIZES => Some(Line::Prize(
            n,
            v.split_whitespace()
                .map(|x| {
                    let (i, p) = x.split_once(':')?;
                    Some((i.parse().ok()?, p.parse().ok()?))
                })
                .collect::<Option<_>>()?,
        )),
        _ => None,
    }
}

/// The mod's lines.
/// The mod's mart lines and prize lines, by list number.
type Lines = (BTreeMap<usize, Vec<u16>>, BTreeMap<usize, Vec<(u16, u16)>>);

fn lines(bridge: &Bridge) -> Lines {
    let mut marts = BTreeMap::new();
    let mut prizes = BTreeMap::new();
    if let Some(text) = bridge
        .project()
        .and_then(|p| fs::read_to_string(p.root.join(FILE)).ok())
    {
        for l in text.lines() {
            match parse(l) {
                Some(Line::Mart(n, v)) => {
                    marts.insert(n, v);
                }
                Some(Line::Prize(n, v)) => {
                    prizes.insert(n, v);
                }
                None => {}
            }
        }
    }
    (marts, prizes)
}

/// The lists in this mod: the mod's lines over the game's.
pub fn current(bridge: &Bridge) -> Result<Shops, Error> {
    let mut s = game(bridge)?;
    let (marts, prizes) = lines(bridge);
    for (n, v) in marts {
        s.marts[n] = v;
    }
    for (n, v) in prizes {
        s.prizes[n] = v;
    }
    Ok(s)
}

/// Writes every list: those equal to the game's have no line.
pub fn set(bridge: &Bridge, shops: &Shops) -> Result<(), Error> {
    let project = bridge
        .project()
        .ok_or_else(|| Error::BadPath("no project is open".into()))?;
    let game = game(bridge)?;
    if shops.marts.len() != MARTS || shops.prizes.len() != PRIZES {
        return Err(Error::BadPath("26 mart lists and 3 prize lists".into()));
    }
    let mut text = String::new();
    for (n, list) in shops.marts.iter().enumerate() {
        check(list.len(), list.iter().copied())?;
        if *list != game.marts[n] {
            let items: Vec<String> = list.iter().map(u16::to_string).collect();
            text.push_str(&format!("mart.{n} = {}\n", items.join(" ")));
        }
    }
    for (n, list) in shops.prizes.iter().enumerate() {
        check(list.len(), list.iter().map(|&(i, _)| i))?;
        if *list != game.prizes[n] {
            let items: Vec<String> = list.iter().map(|(i, p)| format!("{i}:{p}")).collect();
            text.push_str(&format!("bp.{n} = {}\n", items.join(" ")));
        }
    }
    let path = project.root.join(FILE);
    if text.is_empty() {
        if path.is_file() {
            fs::remove_file(&path).map_err(|e| Error::Missing(e.to_string()))?;
        }
    } else {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).map_err(|e| Error::Missing(e.to_string()))?;
        }
        let text = format!("# Written by kenon: mart.<n> = items, bp.<n> = item:price (eidos docs/game-tables.md).\n{text}");
        eidos_mod::manifest::write_atomic(&path, text.as_bytes())
            .map_err(|e| Error::Missing(e.to_string()))?;
    }
    bridge.invalidate();
    Ok(())
}

fn check(len: usize, mut items: impl Iterator<Item = u16>) -> Result<(), Error> {
    if len > MAX_ITEMS {
        return Err(Error::BadPath(format!(
            "a shop holds at most {MAX_ITEMS} items"
        )));
    }
    if items.any(|i| i == 0) {
        return Err(Error::BadPath("a shop item cannot be none".into()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_read() {
        assert!(matches!(parse("mart.3 = 4 3 17"), Some(Line::Mart(3, v)) if v == vec![4, 3, 17]));
        assert!(
            matches!(parse("bp.1 = 46:1 49:1 # vitamins"), Some(Line::Prize(1, v)) if v == vec![(46, 1), (49, 1)])
        );
        assert!(parse("mart.26 = 4").is_none());
        assert!(parse("bp.0 = 46").is_none());
    }
}

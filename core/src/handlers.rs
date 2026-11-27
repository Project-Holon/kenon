//! What a move, ability or item does in battle, as eidos lets a mod change
//! it: `eidos/tables/{move,ability,item}_handlers.txt`, one line per id,
//! `<id> = <base id> [key=value ...]` (eidos sdk/battle_handlers.h).
//!
//! The id takes the battle code of `base` (a built-in entry: "works like"),
//! with the numbers named by the keys changed. Which keys a base takes, and
//! their built-in values, come from eidos's `handler_params.json`; a base it
//! does not list takes no keys. eidos refuses a key the base has no place
//! for, so kenon offers and writes only the listed ones.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::project::Project;

/// The copy that came with this kenon (eidos sdk/handler_params.json); the
/// eidos a mod is played on may have a newer one.
const BUNDLED: &str = include_str!("handler_params.json");

/// eidos takes up to four bases on one line.
pub const MAX_BASES: usize = 4;
/// Abilities with no battle code of their own (Illuminate, Early Bird,
/// Gluttony, No Guard, Honey Gather, Multitype): not a base to copy.
pub const NO_ABILITY_CODE: [usize; 6] = [35, 48, 82, 99, 118, 121];

/// The handler kind of a content table.
pub fn kind_of(table: &str) -> Option<&'static str> {
    match table {
        "moves" => Some("move"),
        "items" => Some("item"),
        "abilities" => Some("ability"),
        _ => None,
    }
}

fn table_name(kind: &str) -> String {
    format!("{kind}_handlers")
}

/// The reference for one kind, and where it came from.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Params {
    /// "bundled" or the file read.
    pub source: String,
    /// The eidos version it belongs to, when the file names one.
    pub eidos: Option<String>,
    /// Each key's allowed values: a list of words, or [low, high].
    pub keys: Value,
    /// id to {"keys": {key: built-in value}, "notes": [...]}.
    pub entries: Value,
}

fn parse(text: &str) -> Result<Value, String> {
    let v: Value = serde_json::from_str(text).map_err(|e| format!("handler_params.json: {e}"))?;
    if v.get("format").and_then(Value::as_u64) != Some(1) {
        return Err("handler_params.json: unknown format".into());
    }
    Ok(v)
}

/// The reference: the one at `file` when given and readable, else the
/// bundled copy.
pub fn params(kind: &str, file: Option<&Path>) -> Result<Params, String> {
    let (source, v) = match file.and_then(|f| std::fs::read_to_string(f).ok().map(|t| (f, t))) {
        Some((f, text)) => (f.display().to_string(), parse(&text)?),
        None => ("bundled".to_string(), parse(BUNDLED)?),
    };
    Ok(Params {
        source,
        eidos: v.get("eidos").and_then(Value::as_str).map(str::to_string),
        keys: v.get("keys").cloned().unwrap_or(Value::Null),
        entries: v.get(kind).cloned().unwrap_or(Value::Null),
    })
}

/// One base of a line and the numbers changed in it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Part {
    pub base: usize,
    pub keys: BTreeMap<String, String>,
}

/// One line: the bases it works like, in order (`a [keys] + b [keys]`).
pub type Row = Vec<Part>;

/// Battle code eidos adds that no game entry has, named in the files
/// (eidos research new-move-bases.md). kenon numbers them past every game
/// id so they never meet a real one; the files hold the name.
pub const NAMED_BASES: [(&str, usize); 5] = [
    ("misty_terrain", NAMED_FIRST),
    ("flower_shield", NAMED_FIRST + 1),
    ("crafty_shield", NAMED_FIRST + 2),
    ("fairy_lock", NAMED_FIRST + 3),
    ("geomancy", NAMED_FIRST + 4),
];
/// The first kenon number of a named base: past the u16 ids.
pub const NAMED_FIRST: usize = 0x1_0000;

/// A named base's name, for a kenon number from NAMED_FIRST on.
pub fn base_name(base: usize) -> Option<&'static str> {
    NAMED_BASES.iter().find(|(_, id)| *id == base).map(|(n, _)| *n)
}

/// How a base is written and listed: its name, or its number.
pub fn entry_key(base: usize) -> String {
    base_name(base).map_or_else(|| base.to_string(), str::to_string)
}

fn parse_base(word: &str) -> Option<usize> {
    word.parse()
        .ok()
        .or_else(|| NAMED_BASES.iter().find(|(n, _)| *n == word).map(|(_, id)| *id))
}

fn parse_part(text: &str) -> Option<Part> {
    let mut words = text.split_whitespace();
    let base = parse_base(words.next()?)?;
    let keys = words
        .filter_map(|w| w.split_once('='))
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    Some(Part { base, keys })
}

fn parse_row(line: &str) -> Option<Row> {
    let row: Option<Row> = line.split('#').next()?.split('+').map(parse_part).collect();
    row.filter(|r| !r.is_empty())
}

fn format_row(row: &Row) -> String {
    let parts: Vec<String> = row
        .iter()
        .map(|p| {
            let mut s = entry_key(p.base);
            for (k, v) in &p.keys {
                s.push_str(&format!(" {k}={v}"));
            }
            s
        })
        .collect();
    parts.join(" + ")
}

/// The mod's rows for a kind.
pub fn rows(project: &Project, kind: &str) -> BTreeMap<usize, Row> {
    project
        .table_lines(&table_name(kind))
        .into_iter()
        .filter_map(|(id, line)| Some((id, parse_row(&line)?)))
        .collect()
}

fn write(project: &Project, kind: &str, rows: &BTreeMap<usize, Row>) -> Result<(), String> {
    let lines = rows.iter().map(|(id, r)| (*id, format_row(r))).collect();
    project
        .write_table_lines(
            &table_name(kind),
            &lines,
            "<id> = <works like id> [key=value ...] (eidos battle handlers).",
        )
        .map_err(|e| e.to_string())
}

/// Checks a value against a key's allowed values; returns it in the form
/// eidos reads.
fn check_value(keys: &Value, key: &str, value: &str) -> Result<String, String> {
    let name = key.split('.').next().unwrap_or(key);
    let allowed = keys
        .get(name)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("{key} is not a key eidos knows"))?;
    let bad = || format!("{value} is not allowed for {key}");
    let value = value.trim();
    match name {
        "stages" | "chance" => {
            let n: i64 = value.parse().map_err(|_| bad())?;
            let (lo, hi) = (
                allowed[0].as_i64().unwrap_or(0),
                allowed[1].as_i64().unwrap_or(0),
            );
            if n < lo || n > hi || (name == "stages" && n == 0) {
                return Err(bad());
            }
            Ok(n.to_string())
        }
        "ratio" => {
            let n: f64 = value.parse().map_err(|_| bad())?;
            let (lo, hi) = (
                allowed[0].as_f64().unwrap_or(0.0),
                allowed[1].as_f64().unwrap_or(0.0),
            );
            if !(lo..=hi).contains(&n) {
                return Err(bad());
            }
            Ok(format!("{}", (n * 100.0).round() / 100.0))
        }
        "hp" => {
            let d: u32 = value
                .strip_prefix("1/")
                .and_then(|d| d.parse().ok())
                .ok_or_else(bad)?;
            if !(1..=16).contains(&d) {
                return Err(bad());
            }
            Ok(format!("1/{d}"))
        }
        _ => {
            if allowed.iter().any(|a| a.as_str() == Some(value)) {
                Ok(value.to_string())
            } else {
                Err(bad())
            }
        }
    }
}

/// The factor work slots a base's handlers use ("work" in the reference;
/// none when it does not say).
fn work(params: &Params, base: usize) -> Vec<u64> {
    params
        .entries
        .get(entry_key(base))
        .and_then(|e| e.get("work"))
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_u64).collect())
        .unwrap_or_default()
}

fn same(builtin: &Value, value: &str) -> bool {
    match builtin {
        Value::String(s) => s == value,
        Value::Number(n) => n.as_f64() == value.parse::<f64>().ok(),
        _ => false,
    }
}

/// Sets what `id` does: works like each part's base, in order, with the
/// part's keys changed; None or an empty row is its own built-in behaviour
/// (a new entry: nothing). Keys left at the base's built-in value are not
/// written, and a base may come only once.
pub fn set(
    project: &Project,
    kind: &str,
    params: &Params,
    id: usize,
    row: Option<Row>,
) -> Result<(), String> {
    let mut all = rows(project, kind);
    let mut out: Row = Vec::new();
    let row = row.unwrap_or_default();
    if row.len() > MAX_BASES {
        return Err(format!("at most {MAX_BASES} bases on one line"));
    }
    for part in row {
        // A base whose own line joins several bases cannot be joined again
        // (eidos refuses it); the line's own id is the built-in one.
        if part.base != id && all.get(&part.base).is_some_and(|r| r.len() > 1) {
            return Err(format!("{} already works like several others", part.base));
        }
        if out.iter().any(|p| p.base == part.base) {
            return Err(format!("{} is picked twice", part.base));
        }
        // An ability works only through battle code the game registers for
        // it. These six have none: the game tests their ids elsewhere, so a
        // copy would do nothing (eidos research abilities.md: no registry
        // row, checked against the ROM).
        if kind == "ability" && part.base != id && NO_ABILITY_CODE.contains(&part.base) {
            return Err(format!("ability {} has no battle code of its own to copy", part.base));
        }
        // Bases that keep a counter in the same work slot (Slow Start's
        // turns, Flash Fire's flag) would overwrite each other.
        let slots = work(params, part.base);
        if let Some(p) = out
            .iter()
            .find(|p| work(params, p.base).iter().any(|s| slots.contains(s)))
        {
            return Err(format!("{} and {} cannot be combined", p.base, part.base));
        }
        let builtin = params
            .entries
            .get(entry_key(part.base))
            .and_then(|e| e.get("keys"));
        let mut keys = BTreeMap::new();
        for (k, v) in &part.keys {
            let default = builtin
                .and_then(|b| b.get(k))
                .ok_or_else(|| format!("the base takes no {k}"))?;
            let v = check_value(&params.keys, k, v)?;
            if !same(default, &v) {
                keys.insert(k.clone(), v);
            }
        }
        out.push(Part {
            base: part.base,
            keys,
        });
    }
    let own = out.len() == 1 && out[0].base == id && out[0].keys.is_empty();
    if out.is_empty() || own {
        all.remove(&id);
    } else {
        all.insert(id, out);
    }
    // A move that played its old base's animation keeps it when the new
    // base has none of its own (a named effect).
    let anim_before = (kind == "move").then(|| crate::move_anims::current(project, id));
    write(project, kind, &all)?;
    match anim_before {
        Some(before) => crate::move_anims::keep_after_handler_change(project, id, before),
        None => Ok(()),
    }
}

/// After an added entry is removed: its row goes and the later ids move
/// down one. Returns whether anything changed.
pub fn remove_id(project: &Project, kind: &str, id: usize) -> Result<bool, String> {
    let old = rows(project, kind);
    if !old.keys().any(|&i| i >= id) {
        return Ok(false);
    }
    let shifted = old
        .into_iter()
        .filter(|(i, _)| *i != id)
        .map(|(i, mut r)| {
            r.retain(|p| p.base != id);
            for p in &mut r {
                // Named bases are not entries: they do not move.
                if p.base > id && p.base < NAMED_FIRST {
                    p.base -= 1;
                }
            }
            (if i > id { i - 1 } else { i }, r)
        })
        .filter(|(_, r)| !r.is_empty())
        .collect();
    write(project, kind, &shifted)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_round_trip() {
        let r = parse_row("22 stat=spatk stages=-2").unwrap();
        assert_eq!(r[0].base, 22);
        assert_eq!(r[0].keys["stat"], "spatk");
        assert_eq!(format_row(&r), "22 stages=-2 stat=spatk");
        let two = parse_row("22 stat=spatk + 26").unwrap();
        assert_eq!(two.len(), 2);
        assert_eq!(two[1].base, 26);
        assert_eq!(format_row(&two), "22 stat=spatk + 26");
        assert!(parse_row("22 + x").is_none());
        // Named bases read and write by name.
        let named = parse_row("misty_terrain").unwrap();
        assert_eq!(named[0].base, NAMED_FIRST);
        assert_eq!(format_row(&named), "misty_terrain");
        assert_eq!(format_row(&parse_row("13 + geomancy").unwrap()), "13 + geomancy");
    }

    #[test]
    fn bundled_reference() {
        let p = params("ability", None).unwrap();
        assert_eq!(p.entries["22"]["keys"]["stat"], "atk");
        let keys = &p.keys;
        assert_eq!(check_value(keys, "stages", "-2").unwrap(), "-2");
        assert!(check_value(keys, "stages", "0").is_err());
        assert!(check_value(keys, "stages", "7").is_err());
        assert_eq!(check_value(keys, "hp", "1/2").unwrap(), "1/2");
        assert!(check_value(keys, "hp", "1/17").is_err());
        assert_eq!(check_value(keys, "ratio.2", "1.75").unwrap(), "1.75");
        assert!(check_value(keys, "stat", "hp").is_err());
    }
}

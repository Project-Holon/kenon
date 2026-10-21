//! Schema-driven data tables: Pokemon stats, moves, items, learnsets,
//! evolutions, trainers and their parties, wild encounters.
//!
//! `schemas.json` says where each table lives and how its records are laid
//! out (sourced in `docs/game-data-map.md`). It holds layouts only: every
//! value, and every name shown for an entry or an enum, is read from the
//! user's own ROM at run time.
//!
//! A record is shown as sections: plain fields, and lists (learnset rows,
//! party members, encounter slots). Saving writes the member into the open
//! project, keeping every byte the schema does not describe.

use std::collections::HashMap;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use crate::bridge::{Bridge, Error, Member};
use crate::msg;

const SCHEMAS: &str = include_str!("schemas.json");

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Schemas {
    pub tables: Vec<Table>,
    pub enums: HashMap<String, EnumDef>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BankRef {
    pub archive: u16,
    pub bank: usize,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnumDef {
    pub source: Option<BankRef>,
    pub values: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Repeat {
    pub count: usize,
    pub stride: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct When {
    pub field: String,
    pub equals: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Field {
    pub name: String,
    pub label: String,
    pub offset: usize,
    #[serde(rename = "type")]
    pub kind: String,
    pub size: Option<usize>,
    pub bit: Option<u32>,
    pub width: Option<u32>,
    #[serde(rename = "enum")]
    pub enum_name: Option<String>,
    pub repeat: Option<Repeat>,
    pub when: Option<When>,
    #[serde(default)]
    pub readonly: bool,
    pub confidence: Option<String>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Group {
    pub name: String,
    pub label: String,
    pub offset: usize,
    pub count: usize,
    pub stride: usize,
    pub fields: Vec<Field>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Element {
    pub size: usize,
    pub fields: Vec<Field>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Variant {
    pub size: usize,
    pub label: String,
    pub fields: Vec<Field>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TableField {
    pub table: String,
    pub field: String,
    pub mask: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Seasons {
    #[serde(rename = "blockSize")]
    pub block_size: usize,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Table {
    pub id: String,
    pub label: String,
    pub archive: u16,
    pub member_count: Option<usize>,
    pub record_size: Option<usize>,
    pub records_per_member: Option<usize>,
    pub container: Option<String>,
    pub name_bank: Option<BankRef>,
    pub layout: Option<String>,
    #[serde(default)]
    pub fields: Vec<Field>,
    pub groups: Option<Vec<Group>>,
    pub element: Option<Element>,
    pub header: Option<Vec<Field>>,
    pub variants: Option<HashMap<String, Variant>>,
    pub count_from: Option<TableField>,
    pub variant_by: Option<TableField>,
    pub seasons: Option<Seasons>,
}

pub fn schemas() -> &'static Schemas {
    static S: OnceLock<Schemas> = OnceLock::new();
    S.get_or_init(|| serde_json::from_str(SCHEMAS).expect("schemas.json is valid"))
}

pub fn table(id: &str) -> Result<&'static Table, Error> {
    schemas()
        .tables
        .iter()
        .find(|t| t.id == id)
        .ok_or_else(|| Error::BadPath(format!("no table {id}")))
}

/// The tables the editor offers: those kept in NARC members.
pub fn editable(t: &Table) -> bool {
    t.container.is_none() && t.records_per_member.is_none()
}

// ---------------------------------------------------------------- values

fn width_bytes(f: &Field) -> usize {
    match f.kind.as_str() {
        "u8" | "s8" => 1,
        "u16" | "s16" => 2,
        "u32" | "s32" => 4,
        _ => f.size.unwrap_or(1),
    }
}

/// Reads a field at `base` (a record or element start).
pub fn read_field(bytes: &[u8], base: usize, f: &Field) -> Option<i64> {
    let at = base + f.offset;
    let n = width_bytes(f);
    let raw = bytes.get(at..at + n)?;
    let mut word = 0u64;
    for (i, b) in raw.iter().enumerate() {
        word |= (*b as u64) << (8 * i);
    }
    Some(match f.kind.as_str() {
        "s8" => word as u8 as i8 as i64,
        "s16" => word as u16 as i16 as i64,
        "s32" => word as u32 as i32 as i64,
        "bits" => {
            let width = f.width.unwrap_or(1);
            ((word >> f.bit.unwrap_or(0)) & ((1u64 << width) - 1)) as i64
        }
        _ => word as i64,
    })
}

/// Writes a field, leaving the other bits of a bitfield word unchanged.
pub fn write_field(bytes: &mut [u8], base: usize, f: &Field, value: i64) {
    let at = base + f.offset;
    let n = width_bytes(f);
    let Some(raw) = bytes.get_mut(at..at + n) else {
        return;
    };
    let mut word = 0u64;
    for (i, b) in raw.iter().enumerate() {
        word |= (*b as u64) << (8 * i);
    }
    let word = if f.kind == "bits" {
        let width = f.width.unwrap_or(1);
        let shift = f.bit.unwrap_or(0);
        let mask = ((1u64 << width) - 1) << shift;
        (word & !mask) | (((value as u64) << shift) & mask)
    } else {
        value as u64
    };
    for (i, b) in raw.iter_mut().enumerate() {
        *b = (word >> (8 * i)) as u8;
    }
}

/// The range a field can hold.
pub fn range(f: &Field) -> (i64, i64) {
    match f.kind.as_str() {
        "u8" => (0, 0xFF),
        "s8" => (-128, 127),
        "u16" => (0, 0xFFFF),
        "s16" => (-32768, 32767),
        "u32" => (0, 0xFFFF_FFFF),
        "s32" => (i32::MIN as i64, i32::MAX as i64),
        _ => (0, (1i64 << f.width.unwrap_or(1)) - 1),
    }
}

// ---------------------------------------------------------------- views

/// One editable value.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Value {
    /// The key a save sends back.
    pub key: String,
    pub label: String,
    pub value: i64,
    pub min: i64,
    pub max: i64,
    #[serde(rename = "enum")]
    pub enum_name: Option<String>,
    pub readonly: bool,
    pub inferred: bool,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Column {
    pub name: String,
    pub label: String,
    pub min: i64,
    pub max: i64,
    #[serde(rename = "enum")]
    pub enum_name: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Section {
    #[serde(rename_all = "camelCase")]
    Fields { label: String, values: Vec<Value> },
    /// Rows of values. A fixed list keeps its row count.
    #[serde(rename_all = "camelCase")]
    List {
        key: String,
        label: String,
        columns: Vec<Column>,
        rows: Vec<Vec<i64>>,
        fixed: bool,
        max_rows: usize,
    },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Record {
    pub sections: Vec<Section>,
    /// For seasonal encounter tables: how many seasons the member holds.
    pub seasons: usize,
    pub changed: bool,
}

/// What a save sends back.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Edit {
    #[serde(default)]
    pub values: HashMap<String, i64>,
    #[serde(default)]
    pub lists: HashMap<String, Vec<Vec<i64>>>,
    #[serde(default)]
    pub season: usize,
}

fn values_of(bytes: &[u8], base: usize, fields: &[Field], prefix: &str) -> Vec<Value> {
    let mut out = Vec::new();
    for f in fields {
        if let Some(w) = &f.when {
            let other = fields.iter().find(|o| o.name == w.field);
            if other.and_then(|o| read_field(bytes, base, o)) != Some(w.equals) {
                continue;
            }
        }
        let (count, stride) = f.repeat.as_ref().map_or((1, 0), |r| (r.count, r.stride));
        for n in 0..count {
            let Some(value) = read_field(bytes, base + n * stride, f) else {
                continue;
            };
            let (min, max) = range(f);
            let (key, label) = if count > 1 {
                (
                    format!("{prefix}{}#{n}", f.name),
                    format!("{} {}", f.label, n + 1),
                )
            } else {
                (format!("{prefix}{}", f.name), f.label.clone())
            };
            out.push(Value {
                key,
                label,
                value,
                min,
                max,
                enum_name: f.enum_name.clone(),
                readonly: f.readonly,
                inferred: f.confidence.as_deref() == Some("inferred"),
                note: f.note.clone(),
            });
        }
    }
    out
}

fn columns(fields: &[Field]) -> Vec<Column> {
    fields
        .iter()
        .map(|f| {
            let (min, max) = range(f);
            Column {
                name: f.name.clone(),
                label: f.label.clone(),
                min,
                max,
                enum_name: f.enum_name.clone(),
            }
        })
        .collect()
}

fn row(bytes: &[u8], base: usize, fields: &[Field]) -> Vec<i64> {
    fields
        .iter()
        .map(|f| read_field(bytes, base, f).unwrap_or(0))
        .collect()
}

fn member(t: &Table, index: usize) -> Member {
    Member {
        arc: t.archive,
        index,
    }
}

/// The value of a field of another table's record (trainer party counts).
fn other_field(bridge: &Bridge, link: &TableField, index: usize) -> Result<i64, Error> {
    let t = table(&link.table)?;
    let f = t
        .fields
        .iter()
        .find(|f| f.name == link.field)
        .ok_or_else(|| Error::Format(format!("no field {}", link.field)))?;
    let bytes = bridge.member(member(t, index))?;
    let v = read_field(&bytes, 0, f).unwrap_or(0);
    Ok(link.mask.map_or(v, |m| v & m))
}

/// A record as sections of values and lists.
pub fn record(bridge: &Bridge, id: &str, index: usize, season: usize) -> Result<Record, Error> {
    let t = table(id)?;
    let bytes = bridge.member(member(t, index))?;
    let changed = bridge
        .project()
        .is_some_and(|p| p.member(t.archive, index).is_some());
    let mut sections = Vec::new();
    let mut seasons = 1;
    match t.layout.as_deref() {
        Some("list") => {
            let e = t.element.as_ref().expect("list layout has an element");
            let mut rows = Vec::new();
            let mut at = 0;
            while at + e.size <= bytes.len() {
                let r = row(&bytes, at, &e.fields);
                if r.iter().all(|&v| v == 0xFFFF) {
                    break;
                }
                rows.push(r);
                at += e.size;
            }
            sections.push(Section::List {
                key: "rows".into(),
                label: t.label.clone(),
                columns: columns(&e.fields),
                rows,
                fixed: false,
                max_rows: 25,
            });
        }
        Some("countedList") => {
            let e = t.element.as_ref().expect("counted list has an element");
            let count = bytes
                .get(..2)
                .map_or(0, |b| u16::from_le_bytes([b[0], b[1]]) as usize);
            let rows = (0..count)
                .filter(|i| 2 + (i + 1) * e.size <= bytes.len())
                .map(|i| row(&bytes, 2 + i * e.size, &e.fields))
                .collect();
            sections.push(Section::List {
                key: "rows".into(),
                label: t.label.clone(),
                columns: columns(&e.fields),
                rows,
                fixed: false,
                max_rows: 31,
            });
        }
        Some("variantList") => {
            let kind = other_field(
                bridge,
                t.variant_by.as_ref().expect("variant source"),
                index,
            )?;
            let count = other_field(bridge, t.count_from.as_ref().expect("count source"), index)?;
            let v = &t.variants.as_ref().expect("variants")[&kind.to_string()];
            let rows = (0..count as usize)
                .filter(|i| (i + 1) * v.size <= bytes.len())
                .map(|i| row(&bytes, i * v.size, &v.fields))
                .collect();
            sections.push(Section::List {
                key: "rows".into(),
                label: format!("Party ({})", v.label),
                columns: columns(&v.fields),
                rows,
                fixed: false,
                max_rows: 6,
            });
        }
        _ => {
            let base = match &t.seasons {
                Some(s) => {
                    seasons = (bytes.len() / s.block_size).max(1);
                    season.min(seasons - 1) * s.block_size
                }
                None => 0,
            };
            sections.push(Section::Fields {
                label: t.label.clone(),
                values: values_of(&bytes, base, &t.fields, ""),
            });
            for g in t.groups.iter().flatten() {
                sections.push(Section::List {
                    key: g.name.clone(),
                    label: g.label.clone(),
                    columns: columns(&g.fields),
                    rows: (0..g.count)
                        .map(|i| row(&bytes, base + g.offset + i * g.stride, &g.fields))
                        .collect(),
                    fixed: true,
                    max_rows: g.count,
                });
            }
        }
    }
    Ok(Record {
        sections,
        seasons,
        changed,
    })
}

/// Applies an edit and writes the member into the project.
pub fn save(bridge: &Bridge, id: &str, index: usize, edit: &Edit) -> Result<(), Error> {
    let t = table(id)?;
    // A flat file or a many-records member needs a whole-file or in-place
    // write, which this per-member path does not do.
    if !editable(t) {
        return Err(Error::BadPath(format!("{} cannot be saved yet", t.label)));
    }
    let old = bridge.member(member(t, index))?;
    let bytes = match t.layout.as_deref() {
        Some("list") => {
            let e = t.element.as_ref().expect("list layout has an element");
            let rows = edit.lists.get("rows").cloned().unwrap_or_default();
            let mut out = vec![0u8; (rows.len() + 1) * e.size];
            for (i, r) in rows.iter().enumerate() {
                for (f, &v) in e.fields.iter().zip(r) {
                    write_field(&mut out, i * e.size, f, v);
                }
            }
            for f in &e.fields {
                write_field(&mut out, rows.len() * e.size, f, 0xFFFF);
            }
            out
        }
        Some("countedList") => {
            let e = t.element.as_ref().expect("counted list has an element");
            let rows = edit.lists.get("rows").cloned().unwrap_or_default();
            let mut out = vec![0u8; 2 + rows.len() * e.size];
            out[..2].copy_from_slice(&(rows.len() as u16).to_le_bytes());
            for (i, r) in rows.iter().enumerate() {
                for (f, &v) in e.fields.iter().zip(r) {
                    write_field(&mut out, 2 + i * e.size, f, v);
                }
            }
            out
        }
        Some("variantList") => {
            let by = t.variant_by.as_ref().expect("variant source");
            let kind = other_field(bridge, by, index)?;
            let v = &t.variants.as_ref().expect("variants")[&kind.to_string()];
            let rows = edit.lists.get("rows").cloned().unwrap_or_default();
            let mut out = vec![0u8; rows.len() * v.size];
            for (i, r) in rows.iter().enumerate() {
                for (f, &x) in v.fields.iter().zip(r) {
                    write_field(&mut out, i * v.size, f, x);
                }
            }
            // The trainer record holds the party size.
            let count = t.count_from.as_ref().expect("count source");
            let owner = table(&count.table)?;
            if let Some(f) = owner.fields.iter().find(|f| f.name == count.field) {
                let mut rec = bridge.member(member(owner, index))?;
                if read_field(&rec, 0, f) != Some(rows.len() as i64) {
                    write_field(&mut rec, 0, f, rows.len() as i64);
                    crate::catalog::write_member(bridge, member(owner, index), &rec)?;
                }
            }
            out
        }
        _ => {
            let mut out = old.clone();
            let base = t.seasons.as_ref().map_or(0, |s| {
                edit.season.min((old.len() / s.block_size).max(1) - 1) * s.block_size
            });
            for f in &t.fields {
                if f.readonly {
                    continue;
                }
                let (count, stride) = f.repeat.as_ref().map_or((1, 0), |r| (r.count, r.stride));
                for n in 0..count {
                    let key = if count > 1 {
                        format!("{}#{n}", f.name)
                    } else {
                        f.name.clone()
                    };
                    if let Some(&v) = edit.values.get(&key) {
                        let (min, max) = range(f);
                        write_field(&mut out, base + n * stride, f, v.clamp(min, max));
                    }
                }
            }
            for g in t.groups.iter().flatten() {
                if let Some(rows) = edit.lists.get(&g.name) {
                    for (i, r) in rows.iter().enumerate().take(g.count) {
                        for (f, &v) in g.fields.iter().zip(r) {
                            write_field(&mut out, base + g.offset + i * g.stride, f, v);
                        }
                    }
                }
            }
            out
        }
    };
    if bytes != old {
        crate::catalog::write_member(bridge, member(t, index), &bytes)?;
    }
    Ok(())
}

// ---------------------------------------------------------------- names

/// The strings of a text bank, from the ROM (and the open project).
pub fn bank_strings(bridge: &Bridge, bank: &BankRef) -> Vec<String> {
    let table = bridge.packed_table();
    bridge
        .member(Member {
            arc: bank.archive,
            index: bank.bank,
        })
        .ok()
        .and_then(|b| msg::Bank::parse(&b).ok())
        .map(|b| {
            b.messages()
                .iter()
                .map(|m| readable(&msg::plain(&m.units, table.as_deref())))
                .collect()
        })
        .unwrap_or_default()
}

/// The game's private glyphs as plain text, for names in lists: the "PK"
/// and "MN" glyphs, and the gender marks.
fn readable(s: &str) -> String {
    s.replace('\u{2486}', "PK")
        .replace('\u{2487}', "MN")
        .replace('\u{246D}', "\u{2642}")
        .replace('\u{246E}', "\u{2640}")
}

/// The names of an enum: a text bank of the ROM, or the schema's labels.
pub fn enum_values(bridge: &Bridge, name: &str) -> Result<std::sync::Arc<Vec<String>>, Error> {
    bridge.cached(&format!("enum/{name}"), || make_enum_values(bridge, name))
}

fn make_enum_values(bridge: &Bridge, name: &str) -> Result<Vec<String>, Error> {
    let def = schemas()
        .enums
        .get(name)
        .ok_or_else(|| Error::BadPath(format!("no enum {name}")))?;
    Ok(match (&def.source, &def.values) {
        (Some(bank), _) => bank_strings(bridge, bank),
        (None, Some(values)) => values.clone(),
        (None, None) => Vec::new(),
    })
}

#[derive(Debug, Clone, Serialize)]
pub struct Entry {
    pub index: usize,
    pub name: String,
}

/// The entries of a table, named from the ROM's text where it has names.
pub fn entries(bridge: &Bridge, id: &str) -> Result<std::sync::Arc<Vec<Entry>>, Error> {
    bridge.cached(&format!("entries/{id}"), || make_entries(bridge, id))
}

fn make_entries(bridge: &Bridge, id: &str) -> Result<Vec<Entry>, Error> {
    let t = table(id)?;
    let count = bridge.base_archive(t.archive)?.len();
    let names = t
        .name_bank
        .as_ref()
        .map(|b| bank_strings(bridge, b))
        .unwrap_or_default();
    Ok((0..count)
        .map(|index| Entry {
            index,
            name: names
                .get(index)
                .filter(|n| !n.trim().is_empty())
                .cloned()
                .unwrap_or_else(|| format!("{} {index}", t.label)),
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_loads() {
        assert!(table("species").is_ok());
        assert!(table("moves").unwrap().record_size == Some(36));
    }

    #[test]
    fn fields_read_and_write() {
        let bits = Field {
            name: "x".into(),
            label: "X".into(),
            offset: 1,
            kind: "bits".into(),
            size: Some(2),
            bit: Some(4),
            width: Some(6),
            enum_name: None,
            repeat: None,
            when: None,
            readonly: false,
            confidence: None,
            note: None,
        };
        let mut bytes = vec![0xFF, 0xFF, 0xFF, 0xFF];
        assert_eq!(read_field(&bytes, 0, &bits), Some(63));
        write_field(&mut bytes, 0, &bits, 5);
        assert_eq!(read_field(&bytes, 0, &bits), Some(5));
        // The other bits of the word stay set.
        assert_eq!(bytes, vec![0xFF, 0x5F, 0xFC, 0xFF]);
        let s8 = Field {
            kind: "s8".into(),
            size: None,
            bit: None,
            width: None,
            offset: 0,
            ..bits
        };
        write_field(&mut bytes, 0, &s8, -2);
        assert_eq!(read_field(&bytes, 0, &s8), Some(-2));
        assert_eq!(range(&s8), (-128, 127));
    }
}

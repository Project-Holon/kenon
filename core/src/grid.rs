//! A whole table as one grid, for bulk editing: every entry a row, every
//! single field (not the repeated lists) a column. Built from the same
//! records the entry screens show, so labels, limits and readonly fields
//! are the same, and saved through the same per-entry save.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::bridge::{Bridge, Error};
use crate::tables::{self, Edit, Section};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Column {
    pub key: String,
    pub label: String,
    pub group: Option<String>,
    #[serde(rename = "enum")]
    pub enum_name: Option<String>,
    pub min: i64,
    pub max: i64,
    pub readonly: bool,
    pub scale: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Row {
    pub index: usize,
    pub name: String,
    pub values: Vec<i64>,
    pub changed: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Grid {
    pub columns: Vec<Column>,
    pub rows: Vec<Row>,
}

/// One edited cell.
#[derive(Debug, Clone, Deserialize)]
pub struct Cell {
    pub index: usize,
    pub key: String,
    pub value: i64,
}

/// Whether a table can show as a grid: one record of fields per entry.
pub fn supported(id: &str) -> bool {
    tables::table(id)
        .is_ok_and(|t| t.layout.is_none() && !t.fields.is_empty() && tables::editable(t))
}

/// The table as a grid (entry 0, the game's blank, left out).
pub fn grid(bridge: &Bridge, id: &str) -> Result<Grid, Error> {
    if !supported(id) {
        return Err(Error::BadPath(format!("{id} has no grid view")));
    }
    let entries = tables::entries(bridge, id)?;
    let mut columns: Vec<Column> = Vec::new();
    let mut rows = Vec::new();
    for e in entries.iter().filter(|e| e.index > 0) {
        let rec = tables::record(bridge, id, e.index, 0)?;
        let Some(values) = rec.sections.into_iter().find_map(|s| match s {
            Section::Fields { values, .. } => Some(values),
            _ => None,
        }) else {
            continue;
        };
        if columns.is_empty() {
            columns = values
                .iter()
                .map(|v| Column {
                    key: v.key.clone(),
                    label: v.label.clone(),
                    group: v.group.clone(),
                    enum_name: v.enum_name.clone(),
                    min: v.min,
                    max: v.max,
                    readonly: v.readonly,
                    scale: v.scale,
                })
                .collect();
        }
        let by: BTreeMap<&str, i64> = values.iter().map(|v| (v.key.as_str(), v.value)).collect();
        rows.push(Row {
            index: e.index,
            name: e.name.clone(),
            values: columns
                .iter()
                .map(|c| by.get(c.key.as_str()).copied().unwrap_or(0))
                .collect(),
            changed: rec.changed,
        });
    }
    Ok(Grid { columns, rows })
}

/// Saves edited cells, one save per entry. Values are clamped to each
/// field's range by the save. Returns how many entries were written.
pub fn set_cells(bridge: &Bridge, id: &str, cells: &[Cell]) -> Result<usize, Error> {
    let mut by: BTreeMap<usize, Edit> = BTreeMap::new();
    for c in cells {
        by.entry(c.index)
            .or_default()
            .values
            .insert(c.key.clone(), c.value);
    }
    for (index, edit) in &by {
        tables::save(bridge, id, *index, edit)?;
    }
    bridge.invalidate();
    Ok(by.len())
}

//! Trainer speech: the lines a trainer says before a battle, when beaten,
//! and afterwards. They are not in the field scripts. A script asks for a
//! message kind with opcode 136 and shows it with opcode 135 (trainer id,
//! kind, object); the game looks the line up in two archives.
//!
//! ```text
//! a/0/9/1 member 0  index: u16 per trainer id, the byte offset of the
//!                   trainer's first row in the row table
//! a/0/9/0 member 0  rows of 4 bytes: u16 trainer id, u16 message kind;
//!                   sorted by trainer id; the row at byte offset N has its
//!                   text at message N / 4 of the bank
//! a/0/0/2 bank 189  the lines (the text archive, not the story archive)
//! ```
//!
//! Lookup (`TPOKE_DATA_ReadRecordString`): start at the trainer's offset,
//! read rows until one has this trainer and kind; stop at the end of the
//! row table (no line: empty text). Sources (aletheia `src/game`):
//! arm9/tpoke_data.c:297, ov021/scrcmd_msgwin.c:1230 (opcode 135),
//! ov021/scrcmd_trainer_audio_item.c:390 (opcode 136),
//! ov093/btlv_core.c:2135 (the battle uses the same lookup).

use serde::Serialize;

use crate::bridge::{Bridge, Error, Member};
use crate::{catalog, msg};

/// The row table and the per-trainer index.
const ROWS_ARCHIVE: u16 = 90;
const INDEX_ARCHIVE: u16 = 91;
/// The text archive and bank holding the lines.
pub const BANK_ARCHIVE: u16 = 2;
pub const BANK: u16 = 189;
const ROW: usize = 4;

/// One line of a trainer. Save it with
/// `catalog::save_text_bank(bridge, bank_arc, bank, ..)` after replacing
/// string `id` of the bank (`catalog::text_bank` gives all strings).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrainerLine {
    pub kind: u16,
    pub label: String,
    pub bank_arc: u16,
    pub bank: u16,
    pub id: u16,
    pub text: String,
}

/// A plain name for a message kind. The code names none; the names follow
/// where the game uses each kind and what the data says:
/// - the trainer script (shared script 3001+, a/0/5/7 member 863) shows kind
///   0 (single battle) or 3 and 7 (pairs) before the battle, kind 2 or 5 and
///   9 when the beaten trainer is spoken to, and the third id of opcode 136
///   (24, or 6 and 10) when the player has too few Pokemon for the battle
///   kind; kinds 13 and 14 surround a healing, 15 and 16 a gift;
/// - the battle shows kind 1 when the trainer loses (`BTL_CLIENT_StepShowTrainerResult`, ov093/btl_client.c:4027),
///   and 17, 19, 20 during the fight (`BTL_CLIENT_StepShowSupportItemMessage`);
/// - 8 is the second pair member's defeat line (1 is the first's).
///
/// Other numbers get "Kind N".
pub fn kind_label(kind: u16) -> String {
    match kind {
        0 => "Before battle".into(),
        1 => "Defeated".into(),
        2 => "After battle".into(),
        3 => "Before battle (pair, first)".into(),
        5 => "After battle (pair, first)".into(),
        6 => "Too few Pokémon (pair, first)".into(),
        7 => "Before battle (pair, second)".into(),
        8 => "Defeated (pair, second)".into(),
        9 => "After battle (pair, second)".into(),
        10 => "Too few Pokémon (pair, second)".into(),
        13 => "Before healing".into(),
        14 => "After healing".into(),
        15 => "Gift".into(),
        16 => "After gift".into(),
        17 => "During battle".into(),
        19 => "Last Pokémon".into(),
        20 => "Last Pokémon (alternate)".into(),
        24 => "Too few Pokémon".into(),
        n => format!("Kind {n}"),
    }
}

fn rd16(b: &[u8], at: usize) -> Option<u16> {
    b.get(at..at + 2).map(|x| u16::from_le_bytes([x[0], x[1]]))
}

struct Tables {
    index: Vec<u8>,
    rows: Vec<u8>,
}

fn tables(bridge: &Bridge) -> Result<Tables, Error> {
    Ok(Tables {
        index: bridge.member(Member {
            arc: INDEX_ARCHIVE,
            index: 0,
        })?,
        rows: bridge.member(Member {
            arc: ROWS_ARCHIVE,
            index: 0,
        })?,
    })
}

impl Tables {
    /// Byte offset of the trainer's first row.
    fn start(&self, trainer: u16) -> Result<usize, Error> {
        rd16(&self.index, trainer as usize * 2)
            .map(usize::from)
            .ok_or_else(|| Error::Missing(format!("trainer {trainer} has no text index entry")))
    }

    /// Rows `(byte offset, kind)` of the trainer, from its start while the
    /// rows still belong to it.
    fn lines(&self, trainer: u16) -> Result<Vec<(usize, u16)>, Error> {
        let mut at = self.start(trainer)?;
        let mut out = Vec::new();
        while let (Some(t), Some(k)) = (rd16(&self.rows, at), rd16(&self.rows, at + 2)) {
            if t != trainer {
                break;
            }
            out.push((at, k));
            at += ROW;
        }
        Ok(out)
    }
}

/// The lines of a trainer, in table order. A trainer without any gives an
/// empty list.
pub fn trainer_lines(bridge: &Bridge, trainer: u16) -> Result<Vec<TrainerLine>, Error> {
    let t = tables(bridge)?;
    let rows = t.lines(trainer)?;
    let strings = catalog::text_bank(bridge, BANK_ARCHIVE, BANK as usize)?;
    Ok(rows
        .into_iter()
        .map(|(at, kind)| {
            let id = (at / ROW) as u16;
            TrainerLine {
                kind,
                label: kind_label(kind),
                bank_arc: BANK_ARCHIVE,
                bank: BANK,
                id,
                text: strings.get(id as usize).cloned().unwrap_or_default(),
            }
        })
        .collect())
}

/// Gives a trainer a line of a kind it does not have yet, and returns the
/// new line. The row goes after the trainer's last row, so the message
/// numbers of every later row move up by one: the bank gets a new string at
/// that place, and the index offsets that point at or after that place move
/// by one row. A trainer with no lines gets its row at the end of the table
/// (the game scans on to the end of the table). The game reads the rows only through these two tables and the
/// bank, so nothing else changes. All three are written through
/// `catalog::write_member`.
pub fn add_trainer_line(
    bridge: &Bridge,
    trainer: u16,
    kind: u16,
    text: &str,
) -> Result<TrainerLine, Error> {
    let t = tables(bridge)?;
    let existing = t.lines(trainer)?;
    if existing.iter().any(|&(_, k)| k == kind) {
        return Err(Error::BadPath(format!(
            "trainer {trainer} already has a line of kind {kind}"
        )));
    }
    let table = bridge.packed_table();
    let units = msg::parse(text, table.as_deref()).map_err(Error::BadPath)?;
    // After the trainer's last row; for a trainer with none, at the end of
    // the table (the game scans on to the end, so it still finds the row).
    let at = existing.last().map_or(t.rows.len(), |&(at, _)| at + ROW);
    let mut rows = t.rows.clone();
    let mut row = [0u8; ROW];
    row[..2].copy_from_slice(&trainer.to_le_bytes());
    row[2..].copy_from_slice(&kind.to_le_bytes());
    rows.splice(at..at, row);

    let mut index = t.index.clone();
    for n in 0..index.len() / 2 {
        let v = rd16(&index, n * 2).map_or(0, usize::from);
        let moved = if n == trainer as usize && existing.is_empty() {
            at
        } else if existing.is_empty() {
            v
        } else if v >= at {
            v + ROW
        } else {
            v
        };
        index[n * 2..n * 2 + 2].copy_from_slice(&(moved as u16).to_le_bytes());
    }

    let bank_member = Member {
        arc: BANK_ARCHIVE,
        index: BANK as usize,
    };
    let mut bank =
        msg::Bank::parse(&bridge.member(bank_member)?).map_err(|e| Error::Format(e.to_string()))?;
    let section = bank
        .sections
        .first_mut()
        .ok_or_else(|| Error::Format("the bank has no text".into()))?;
    let id = at / ROW;
    if id > section.messages.len() {
        return Err(Error::Format(
            "the row table is longer than the bank".into(),
        ));
    }
    section
        .messages
        .insert(id, msg::Message { units, reserved: 0 });

    catalog::write_member(
        bridge,
        Member {
            arc: ROWS_ARCHIVE,
            index: 0,
        },
        &rows,
    )?;
    catalog::write_member(
        bridge,
        Member {
            arc: INDEX_ARCHIVE,
            index: 0,
        },
        &index,
    )?;
    catalog::write_member(bridge, bank_member, &bank.build())?;
    Ok(TrainerLine {
        kind,
        label: kind_label(kind),
        bank_arc: BANK_ARCHIVE,
        bank: BANK,
        id: id as u16,
        text: text.to_string(),
    })
}

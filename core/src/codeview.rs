//! What a ROM hack does to the game's code, side by side: the game's
//! instructions and the patch's, named with aletheia's functions
//! (`codenames`) and decoded for the ARM9 (`arm`).
//!
//! The instruction set of the game's side comes from the function that holds
//! the bytes. The patch's side is decoded only where it is known how its code
//! runs: in a changed range that lies inside one game function (the function's
//! set), or where decoded patch code branches into a changed range (the set
//! the branch gives). Anything else is shown as data words.

use std::collections::BTreeSet;

use serde::Serialize;

use crate::arm::{self, Inst};
use crate::codenames;

/// The cartridge around one code part, for naming addresses.
pub struct Context<'a> {
    pub game_code: &'a str,
    /// The ARM9 static module, `[start, end)`.
    pub arm9: (u32, u32),
    /// Each overlay: number, `[start, end)` with its bss.
    pub overlays: Vec<(u16, u32, u32)>,
    /// The game's word at an address of an overlay.
    pub word: &'a dyn Fn(u16, u32) -> Option<u32>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CodeView {
    pub start: u32,
    pub end: u32,
    /// The game functions whose bytes change.
    pub functions: Vec<FunctionRef>,
    pub game: Vec<Line>,
    pub patch: Vec<Line>,
    /// Said when a changed part of the patch could not be decoded.
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FunctionRef {
    pub name: String,
    /// aletheia's source file (src/game/...).
    pub file: String,
    pub address: u32,
    pub thumb: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Line {
    pub address: u32,
    pub bytes: String,
    pub text: String,
    /// What an address in the line is: a function name, an overlay, new code.
    pub note: String,
    pub changed: bool,
}

/// Bytes of context shown before and after a changed range.
const CONTEXT: u32 = 8;

/// One stretch of a side: decoded from `from` in the given set, or data.
#[derive(Debug, Clone, Copy)]
struct Segment {
    start: u32,
    end: u32,
    thumb: Option<bool>,
    from: u32,
}

struct Side<'a> {
    bytes: &'a [u8],
    base: u32,
}

impl Side<'_> {
    fn word(&self, at: u32) -> Option<u32> {
        let off = at.checked_sub(self.base)? as usize;
        Some(u32::from_le_bytes(
            self.bytes.get(off..off + 4)?.try_into().ok()?,
        ))
    }
    fn hex(&self, at: u32, len: u32) -> String {
        let off = (at - self.base) as usize;
        self.bytes
            .get(off..off + len as usize)
            .map(|b| b.iter().map(|x| format!("{x:02x}")).collect())
            .unwrap_or_default()
    }
    fn end(&self) -> u32 {
        self.base + self.bytes.len() as u32
    }
}

/// Decodes from `from` in one set until `end`: every instruction with its
/// address. Stops at bytes that are not an instruction.
fn walk(side: &Side, from: u32, end: u32, thumb: bool) -> Vec<(u32, Inst)> {
    let mut out = Vec::new();
    let mut at = from;
    let end = end.min(side.end());
    while at < end {
        match arm::decode(side.bytes, side.base, at, thumb) {
            Some(i) => {
                let n = i.len;
                out.push((at, i));
                at += n;
            }
            None => at += if thumb { 2 } else { 4 },
        }
    }
    out
}

/// The game side's segments over `[start, end)`: aletheia's functions, and
/// data between them.
fn game_segments(game_code: &str, region: &str, start: u32, end: u32) -> Vec<Segment> {
    let mut out = Vec::new();
    let mut at = start;
    for (f, fstart) in codenames::functions_in(game_code, region, start, end) {
        let fend = fstart + f.size.max(1);
        if fstart > at {
            out.push(Segment {
                start: at,
                end: fstart.min(end),
                thumb: None,
                from: at,
            });
        }
        out.push(Segment {
            start: fstart.max(at),
            end: fend.min(end),
            thumb: Some(f.thumb),
            from: fstart,
        });
        at = fend.min(end).max(at);
    }
    if at < end {
        out.push(Segment {
            start: at,
            end,
            thumb: None,
            from: at,
        });
    }
    out
}

fn inside(ranges: &[(u32, u32)], a: u32) -> bool {
    ranges.iter().any(|&(s, e)| s <= a && a < e)
}

/// How the patch's changed ranges run: (range, thumb, decode from).
fn patch_seeds(
    ctx: &Context,
    region: &str,
    new: &Side,
    ranges: &[(u32, u32)],
) -> Vec<((u32, u32), bool, u32)> {
    let mut seeds: Vec<((u32, u32), bool, u32)> = Vec::new();
    for &(s, e) in ranges {
        if let Some((f, fstart)) = codenames::function_at(ctx.game_code, region, s) {
            if e <= fstart + f.size {
                seeds.push(((s, e), f.thumb, fstart));
            }
        }
    }
    // Branches out of known patch code into changed ranges give their set.
    let mut done = 0;
    while done < seeds.len() {
        let ((s, e), thumb, from) = seeds[done];
        done += 1;
        // the code's own literal words are data, not branches
        let mut literals = BTreeSet::new();
        for (at, i) in walk(new, from, e, thumb) {
            if let Some(l) = i.literal {
                literals.insert(l);
            }
            if at + i.len <= s || literals.contains(&at) {
                continue;
            }
            let Some(b) = i.branch else { continue };
            let Some(&r) = ranges
                .iter()
                .find(|&&(rs, re)| rs <= b.target && b.target < re)
            else {
                continue;
            };
            if seeds.iter().any(|(x, ..)| *x == r) {
                continue;
            }
            seeds.push((r, thumb ^ b.switch, b.target));
        }
    }
    seeds
}

/// The patch side's segments: the game's, with each changed range replaced
/// by how the patch runs there (data when that is not known).
fn patch_segments(
    game: &[Segment],
    ranges: &[(u32, u32)],
    seeds: &[((u32, u32), bool, u32)],
) -> Vec<Segment> {
    let mut cuts: Vec<Segment> = Vec::new();
    for &(s, e) in ranges {
        match seeds.iter().find(|(r, ..)| *r == (s, e)) {
            Some(&(_, thumb, from)) => {
                if from > s {
                    cuts.push(Segment {
                        start: s,
                        end: from,
                        thumb: None,
                        from: s,
                    });
                }
                cuts.push(Segment {
                    start: from.max(s),
                    end: e,
                    thumb: Some(thumb),
                    from,
                });
            }
            None => cuts.push(Segment {
                start: s,
                end: e,
                thumb: None,
                from: s,
            }),
        }
    }
    let mut out = Vec::new();
    for g in game {
        let mut at = g.start;
        for c in cuts.iter().filter(|c| c.end > g.start && c.start < g.end) {
            if c.start > at {
                out.push(Segment {
                    start: at,
                    end: c.start,
                    ..*g
                });
            }
            out.push(Segment {
                start: c.start.max(g.start),
                end: c.end.min(g.end),
                ..*c
            });
            at = c.end.min(g.end);
        }
        if at < g.end {
            out.push(Segment {
                start: at,
                end: g.end,
                ..*g
            });
        }
    }
    out
}

/// Names the addresses one side of a view mentions.
struct Namer<'a> {
    ctx: &'a Context<'a>,
    region: &'a str,
    part: (u32, u32),
    /// The patch's changed ranges (the patch side only): its new code.
    new_code: Option<&'a [(u32, u32)]>,
    /// The words the side's code loads.
    values: BTreeSet<u32>,
}

impl Namer<'_> {
    /// A function, new code, or the overlays loaded there.
    fn name(&self, a: u32) -> String {
        let ctx = self.ctx;
        let a0 = a & !1;
        if self.new_code.is_some_and(|r| inside(r, a0)) {
            return "new code".into();
        }
        if self.part.0 <= a0 && a0 < self.part.1 {
            return codenames::label(ctx.game_code, self.region, a0).unwrap_or_default();
        }
        if ctx.arm9.0 <= a0 && a0 < ctx.arm9.1 {
            return codenames::label(ctx.game_code, "arm9", a0).unwrap_or_default();
        }
        let candidates: Vec<String> = ctx
            .overlays
            .iter()
            .filter(|(_, s, e)| *s <= a0 && a0 < *e)
            .map(|&(n, ..)| {
                let r = codenames::overlay_region(n);
                let mut t = r.clone();
                if let Some(l) = codenames::label(ctx.game_code, &r, a0) {
                    t.push_str(&format!(" {l}"));
                }
                if let Some(w) = (ctx.word)(n, a0) {
                    if self.values.contains(&w) {
                        t.push_str(&format!(" (holds {w:#x}, which this code also loads)"));
                    }
                }
                t
            })
            .collect();
        match candidates.len() {
            0 => String::new(),
            1 => candidates[0].clone(),
            _ => format!("one of: {}", candidates.join("; ")),
        }
    }
}

/// One side of a view. `ranges` are the view's changed ranges; `new_code`
/// all of the part's, on the patch side.
fn render(
    ctx: &Context,
    region: &str,
    side: &Side,
    segments: &[Segment],
    ranges: &[(u32, u32)],
    new_code: Option<&[(u32, u32)]>,
) -> Vec<Line> {
    // Words loaded relative to pc are data, and their values name things.
    let mut literals = BTreeSet::new();
    let mut decoded: Vec<(Segment, Vec<(u32, Inst)>)> = Vec::new();
    for seg in segments {
        let insts = match seg.thumb {
            Some(t) => {
                let end = if seg.from < seg.start {
                    seg.end.max(seg.start)
                } else {
                    seg.end
                };
                // a function is read whole for its literals
                let whole_end = codenames::function_at(ctx.game_code, region, seg.from)
                    .filter(|(_, s)| *s == seg.from)
                    .map_or(end, |(f, s)| (s + f.size).max(end));
                walk(side, seg.from, whole_end, t)
            }
            None => Vec::new(),
        };
        for (_, i) in &insts {
            if let Some(l) = i.literal {
                literals.insert(l);
            }
        }
        decoded.push((*seg, insts));
    }
    let namer = Namer {
        ctx,
        region,
        part: (side.base, side.end()),
        new_code,
        values: literals.iter().filter_map(|&a| side.word(a)).collect(),
    };
    // a game function that starts on a line names it (outside new code)
    let starts = |a: u32| -> Option<String> {
        if new_code.is_some_and(|r| inside(r, a)) {
            return None;
        }
        codenames::function_at(ctx.game_code, region, a)
            .filter(|(_, s)| *s == a)
            .map(|(f, _)| format!("start of {}", f.name))
    };
    let with_start = |a: u32, note: String| match (starts(a), note.is_empty()) {
        (Some(s), true) => s,
        (Some(s), false) => format!("{s}; {note}"),
        (None, _) => note,
    };
    let mut out = Vec::new();
    let changed = |a: u32, n: u32| ranges.iter().any(|&(s, e)| a < e && s < a + n);
    for (seg, insts) in decoded {
        let mut at = seg.start;
        while at < seg.end {
            if literals.contains(&at) && at + 4 <= seg.end {
                let v = side.word(at).unwrap_or(0);
                out.push(Line {
                    address: at,
                    bytes: side.hex(at, 4),
                    text: format!(".word {v:#010x}"),
                    note: namer.name(v),
                    changed: changed(at, 4),
                });
                at += 4;
                continue;
            }
            if let Some((_, i)) = insts.iter().find(|(a, _)| *a == at) {
                let mut note = String::new();
                if let Some(b) = i.branch {
                    note = namer.name(b.target);
                    if b.switch {
                        note = format!("{} (other instruction set)", note)
                            .trim()
                            .to_string();
                    }
                }
                if let Some(l) = i.literal {
                    if let Some(v) = side.word(l) {
                        let named = namer.name(v);
                        note = if named.is_empty() {
                            format!("= {v:#x}")
                        } else {
                            format!("= {v:#x}, {named}")
                        };
                    }
                }
                out.push(Line {
                    address: at,
                    bytes: side.hex(at, i.len),
                    text: i.text.clone(),
                    note: with_start(at, note),
                    changed: changed(at, i.len),
                });
                at += i.len;
                continue;
            }
            // data: a halfword in Thumb code, else a word where aligned
            let n = if seg.thumb != Some(true) && at % 4 == 0 && at + 4 <= seg.end {
                4
            } else {
                2.min(seg.end - at)
            };
            let text = match n {
                4 => format!(".word {:#010x}", side.word(at).unwrap_or(0)),
                _ => format!(".hword {}", side.hex(at, n)),
            };
            out.push(Line {
                address: at,
                bytes: side.hex(at, n),
                text,
                note: String::new(),
                changed: changed(at, n),
            });
            at += n;
        }
    }
    out
}

/// The views of one code part (`arm9`, `ov093`) loaded at `base`: one per
/// group of nearby changed ranges.
pub fn views(
    ctx: &Context,
    region: &str,
    base: u32,
    old: &[u8],
    new: &[u8],
    ranges: &[(u32, u32)],
) -> Vec<CodeView> {
    let (old, new) = (Side { bytes: old, base }, Side { bytes: new, base });
    let seeds = patch_seeds(ctx, region, &new, ranges);
    // nearby ranges share a view
    struct Group {
        start: u32,
        end: u32,
        ranges: Vec<(u32, u32)>,
    }
    let mut groups: Vec<Group> = Vec::new();
    for &(s, e) in ranges {
        let (ws, we) = (
            s.saturating_sub(CONTEXT).max(base),
            (e + CONTEXT).min(new.end().max(old.end())),
        );
        match groups.last_mut() {
            Some(g) if ws <= g.end => {
                g.end = g.end.max(we);
                g.ranges.push((s, e));
            }
            _ => groups.push(Group {
                start: ws,
                end: we,
                ranges: vec![(s, e)],
            }),
        }
    }
    groups
        .into_iter()
        .map(|Group { start: ws, end: we, ranges: rs }| {
            let game_segs = game_segments(ctx.game_code, region, ws, we.min(old.end()));
            let patch_segs = patch_segments(
                &game_segments(ctx.game_code, region, ws, we.min(new.end())),
                &rs,
                &seeds,
            );
            let mut functions: Vec<FunctionRef> = Vec::new();
            for &(s, e) in &rs {
                for (f, start) in codenames::functions_in(ctx.game_code, region, s, e) {
                    if !functions.iter().any(|g| g.address == start) {
                        functions.push(FunctionRef {
                            name: f.name.clone(),
                            file: f.file.clone(),
                            address: start,
                            thumb: f.thumb,
                        });
                    }
                }
            }
            let unknown = rs
                .iter()
                .filter(|r| !seeds.iter().any(|(x, ..)| x == *r))
                .count();
            let note = if !codenames::knows(ctx.game_code) {
                Some("aletheia's names are not mapped for this cartridge, so the code is shown as data words.".into())
            } else if unknown > 0 {
                Some("How the patch's new bytes here are run is not known, so they are shown as data words.".into())
            } else {
                None
            };
            CodeView {
                start: rs[0].0,
                end: rs.last().map_or(0, |r| r.1),
                functions,
                game: render(ctx, region, &old, &game_segs, &rs, None),
                patch: render(ctx, region, &new, &patch_segs, &rs, Some(ranges)),
                note,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Blaze Black's ARM9 change (its own bytes; the game's side is zeros
    /// here, with MIi_UncompressBackward's `bx lr` at 0x02004ea4).
    #[test]
    fn blaze_black_hook() {
        let base = 0x0200_4000;
        let mut old = vec![0u8; 0x1000];
        old[0xEA4..0xEA8].copy_from_slice(&0xE12F_FF1Eu32.to_le_bytes());
        let mut new = old.clone();
        let code: [u32; 23] = [
            0xE59F003C, 0xE5901000, 0xE59F2038, 0xE1510002, 0x059F1034, 0x05801008, 0x05801148,
            0x059F102C, 0x058010A8, 0x058011E8, 0xE59F0024, 0xE5901000, 0xE59F2020, 0xE1510002,
            0x03A01001, 0x05C0100A, 0xE12FFF1E, 0x02187F64, 0xE15F034F, 0x36AAB337, 0x36AAE06A,
            0x021809E0, 0x28AAFF1F,
        ];
        for (i, w) in code.iter().enumerate() {
            new[0x660 + 4 * i..0x664 + 4 * i].copy_from_slice(&w.to_le_bytes());
        }
        new[0xEA4..0xEA8].copy_from_slice(&0xEAFF_FDEDu32.to_le_bytes());
        let word = |_: u16, _: u32| None;
        let ctx = Context {
            game_code: "IRBO",
            arm9: (base, base + 0x1000),
            overlays: vec![(230, 0x0218_7F00, 0x0218_A000)],
            word: &word,
        };
        let ranges = [(0x0200_4660, 0x0200_46BC), (0x0200_4EA4, 0x0200_4EA8)];
        let v = views(&ctx, "arm9", base, &old, &new, &ranges);
        assert_eq!(v.len(), 2);
        // the new code: entered by the ARM branch, so decoded as ARM
        let new_code: Vec<&Line> = v[0].patch.iter().filter(|l| l.changed).collect();
        assert_eq!(new_code[0].text, "ldr r0, [pc, #0x3c]");
        assert!(
            new_code[0].note.starts_with("= 0x2187f64, ov230"),
            "{}",
            new_code[0].note
        );
        let words = new_code
            .iter()
            .filter(|l| l.text.starts_with(".word"))
            .count();
        let insts = new_code.len() - words;
        assert_eq!((insts, words), (17, 6));
        assert_eq!(v[0].functions[0].name, "SVC_UncompressRL8");
        // the hook: the function's last return becomes a jump to it
        assert_eq!(v[1].functions[0].name, "MIi_UncompressBackward");
        let hook = v[1].patch.iter().find(|l| l.changed).unwrap();
        assert_eq!(
            (hook.text.as_str(), hook.note.as_str()),
            ("b 0x2004660", "new code")
        );
        let game = v[1].game.iter().find(|l| l.changed).unwrap();
        assert_eq!(game.text, "bx lr");
    }
}

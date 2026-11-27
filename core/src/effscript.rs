//! Battle effect scripts (move animations): the game's binary format, and a
//! text form kenon edits. a/0/6/6 member n is move n's script; a/0/6/7
//! holds the status, weather and item effects (eidos docs/game-tables.md,
//! aletheia src/game/ov094/btlv_effvm.c).
//!
//! Binary: u8 variant count, 3 pad bytes, then one 0x38-byte record per
//! variant: 14 u32 offsets of its commands, all the same in the game's
//! files (only the first is read). A command is a u16 id and its
//! arguments, a u32 each, unaligned. ReleaseScriptResources (77) ends the
//! game's scripts; an id of 78 or more also stops one. The jumps take an
//! offset from the end of the jump command; a call or archive jump takes a
//! script id.
//!
//! Text, one item per line (`;` starts a comment):
//!
//! ```text
//! variant @v0
//! @v0:
//!     CameraToView 1 0 0 0 0
//!     JumpIfValue 3 0 1 @l_0084
//!     WaitFrames 10
//! @l_0084:
//!     end 65535
//!     bytes 00 00
//! ```
//!
//! `variant @label [extra <hex>]` lists the variants in order (`extra`: the
//! record's other 13 words when they are not the offset repeated).
//! Variant n plays on the battle's turn count n (two-turn moves: 0 charges,
//! 1 strikes; a missing variant plays variant 0). Commands take signed or
//! 0x hex numbers, and
//! a jump's offset as `@label`. `end <id>` is a stop id, `bytes` raw bytes
//! nothing runs. Only code reached from a variant or a jump is decoded, so
//! any file reads back byte for byte.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

/// The first command eidos adds (the game's end at 77, EndScript).
pub const FIRST_EIDOS_COMMAND: usize = 78;

/// A command's editor name and how many u32 arguments it reads. Names
/// say what the command does (eidos research, effect-script-args.md);
/// several of aletheia's names do not (56 and 57 are swapped).
pub const COMMANDS: [(&str, usize); 80] = [
    ("CameraToView", 5),
    ("CameraToPoint", 10),
    ("OrbitCamera", 6),
    ("CameraShake", 6),
    ("SpriteLayoutMode", 2),
    ("SaveCamera", 0),
    ("LoadParticles", 1),
    ("ParticleEffect", 11),
    ("ParticleEffectAtPoint", 15),
    ("ParticleEffectScreenSpace", 11),
    ("AllParticleEffects", 10),
    ("UnloadParticles", 1),
    ("ParticleEffectOnPath", 11),
    ("ParticleEffectOnPathFromPoint", 13),
    ("ParticleEffectOnPathScreenSpace", 11),
    ("ParticleEffectOnPathFromPointScreenSpace", 13),
    ("OrbitingParticleEffect", 10),
    ("OrbitingParticleEffectScreenSpace", 10),
    ("MovePokemon", 7),
    ("ShakePokemon", 9),
    ("BouncePokemon", 6),
    ("ScalePokemon", 7),
    ("RotatePokemon", 6),
    ("FadePokemon", 6),
    ("MosaicPokemon", 6),
    ("PokemonEyeBlink", 4),
    ("PausePokemonAnimation", 2),
    ("TintPokemon", 5),
    ("ShowOrHidePokemon", 2),
    ("ShowOrHideShadow", 2),
    ("ScalePokemonDrawScale", 7),
    ("RemoveSprites", 1),
    ("AddTrainerSprite", 5),
    ("MoveTrainerSprite", 8),
    ("PlaySpriteAnimation", 2),
    ("RemoveTrainerSprite", 1),
    ("EffectBackground", 1),
    ("ScrollBackground", 6),
    ("NoOp4Words", 4),
    ("NoOp5Words", 5),
    ("BG3Priority", 1),
    ("BackgroundBlend", 6),
    ("TintStage", 5),
    ("HideStagePart", 2),
    ("WindowWipe", 7),
    ("CellActorAtBattler", 7),
    ("MoveCellActor", 7),
    ("ScaleCellActor", 7),
    ("CellActorAnimation", 2),
    ("TintCellActor", 5),
    ("RemoveCellActor", 1),
    ("HPGauges", 2),
    ("PlaySoundEffect", 9),
    ("StopSoundEffect", 1),
    ("PanSoundAcross", 8),
    ("RampSound", 9),
    ("WaitUntilIdle", 1),
    ("WaitFrames", 1),
    ("FramePacing", 1),
    ("JumpIfValue", 4),
    ("JumpIfValuesCompare", 4),
    ("JumpIfSpritePresent", 3),
    ("SetScriptVariable", 1),
    ("ReadValueIntoScriptVariable", 1),
    ("SetParameter", 2),
    ("SubstituteDoll", 3),
    ("CopyDefenderLookToAttacker", 0),
    ("PlayCry", 7),
    ("BallSource", 1),
    ("BallCellActorAtBattler", 6),
    ("CallEffect", 3),
    ("Return", 0),
    ("Jump", 1),
    ("WaitForBattle", 0),
    ("GoToEffect", 1),
    ("ReleaseSideSprites", 1),
    ("BlinkDrawOrder", 1),
    ("EndScript", 0),
    // eidos's own (docs/game-tables.md, Effect script commands of eidos):
    // a mod's sound by its `<mod>:<seq>` name, the name's offset from the
    // command's end first (like a jump), then PlaySoundEffect's arguments
    // without its last.
    ("PlaySoundByName", 8),
    // Tints a loaded particle file's emitters (eidos): its key, the colour's
    // source (0 BGR555, 1 the variant's type, 2 type `value`, 3 the move's
    // type) and the value.
    ("ParticleColor", 3),
];

const HEADER: usize = 4;
const VARIANT: usize = 0x38;

/// The argument a jump command takes its offset in (relative to the end of
/// the command: the offset is always its last argument).
fn jump_arg(id: u16) -> Option<usize> {
    match id {
        0x3B | 0x3C => Some(3),
        0x3D => Some(2),
        0x48 => Some(0),
        // PlaySoundByName: the offset of its name (data, not code).
        PLAY_SOUND_BY_NAME => Some(0),
        _ => None,
    }
}

const PLAY_SOUND_BY_NAME: u16 = 78;

/// Whether running falls through to the next command: not after a return,
/// a jump or the release that ends every game script.
fn falls_through(id: u16) -> bool {
    !matches!(id, 0x47 | 0x48 | 0x4A | 0x4D)
}

fn u16_at(b: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes([*b.get(at)?, *b.get(at + 1)?]))
}

fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

/// One decoded item at an offset.
#[derive(Debug, Clone)]
enum Item {
    Command { id: u16, args: Vec<u32> },
    End(u16),
}

impl Item {
    fn size(&self) -> usize {
        match self {
            Item::Command { args, .. } => 2 + 4 * args.len(),
            Item::End(_) => 2,
        }
    }
}

fn decode_at(b: &[u8], at: usize) -> Option<Item> {
    let id = u16_at(b, at)?;
    if id as usize >= COMMANDS.len() {
        return Some(Item::End(id));
    }
    let n = COMMANDS[id as usize].1;
    let args = (0..n)
        .map(|k| u32_at(b, at + 2 + 4 * k))
        .collect::<Option<Vec<_>>>()?;
    Some(Item::Command { id, args })
}

/// How a file decodes: whether every path from a variant start ends on a
/// stop id (a wrong argument count would run off the end or into data),
/// the stop ids seen, and the bytes left undecoded. For checking the
/// command table against the game's scripts.
pub fn check(b: &[u8]) -> (bool, BTreeSet<u16>, usize) {
    let Some(&count) = b.first() else {
        return (false, BTreeSet::new(), 0);
    };
    let code = HEADER + count as usize * VARIANT;
    let mut ends = BTreeSet::new();
    let mut ok = true;
    let mut seen = BTreeSet::new();
    let mut decoded = 0;
    let mut work: Vec<usize> = (0..count as usize)
        .filter_map(|v| u32_at(b, HEADER + v * VARIANT).map(|e| e as usize))
        .collect();
    while let Some(mut at) = work.pop() {
        loop {
            if !seen.insert(at) {
                break;
            }
            match decode_at(b, at) {
                None => {
                    ok = false;
                    break;
                }
                Some(Item::End(id)) => {
                    ends.insert(id);
                    decoded += 2;
                    break;
                }
                Some(item @ Item::Command { .. }) => {
                    let Item::Command { id, args } = &item else {
                        unreachable!()
                    };
                    let end = at + item.size();
                    decoded += item.size();
                    if let Some(k) = jump_arg(*id) {
                        let t = end as i64 + args[k] as i32 as i64;
                        if t < code as i64 || t >= b.len() as i64 {
                            ok = false;
                        } else {
                            work.push(t as usize);
                        }
                    }
                    if !falls_through(*id) {
                        break;
                    }
                    at = end;
                }
            }
        }
    }
    let _ = decoded;
    // What is left once code nothing reaches is decoded too.
    let mut items = BTreeMap::new();
    for at in seen {
        if let Some(i) = decode_at(b, at) {
            items.insert(at, i);
        }
    }
    sweep_gaps(b, code, &mut items);
    let used: usize = items.values().map(Item::size).sum();
    (ok, ends, b.len().saturating_sub(code + used))
}

/// Decodes the code nothing reaches (the game has some: Transform's
/// animation after a release): from each gap's start, commands one after
/// another while they fit in the gap. What does not fit stays bytes.
fn sweep_gaps(b: &[u8], code: usize, items: &mut BTreeMap<usize, Item>) {
    let mut at = code;
    while at < b.len() {
        if let Some(i) = items.get(&at) {
            at += i.size();
            continue;
        }
        let gap_end = items.range(at..).next().map_or(b.len(), |(o, _)| *o);
        match decode_at(b, at) {
            Some(item @ Item::Command { .. }) if at + item.size() <= gap_end => {
                let size = item.size();
                items.insert(at, item);
                at += size;
            }
            _ => at = gap_end,
        }
    }
}

/// The particle files a script loads (LoadParticles, command 6, its key):
/// every decoded command, reached or not.
pub fn particle_keys(b: &[u8]) -> Vec<u32> {
    let Some(&count) = b.first() else {
        return Vec::new();
    };
    let code = HEADER + count as usize * VARIANT;
    let mut items: BTreeMap<usize, Item> = BTreeMap::new();
    let mut work: Vec<usize> = (0..count as usize)
        .filter_map(|v| u32_at(b, HEADER + v * VARIANT).map(|e| e as usize))
        .collect();
    while let Some(mut at) = work.pop() {
        while at >= code && at < b.len() && !items.contains_key(&at) {
            let Some(item) = decode_at(b, at) else { break };
            let end = at + item.size();
            let next = match &item {
                Item::End(_) => None,
                Item::Command { id, args } => {
                    if let Some(k) = jump_arg(*id) {
                        let t = end as i64 + args[k] as i32 as i64;
                        if t >= code as i64 && (t as usize) < b.len() {
                            work.push(t as usize);
                        }
                    }
                    falls_through(*id).then_some(end)
                }
            };
            items.insert(at, item);
            match next {
                Some(n) => at = n,
                None => break,
            }
        }
    }
    let mut keys: Vec<u32> = items
        .values()
        .filter_map(|i| match i {
            Item::Command { id: 6, args } => args.first().copied(),
            _ => None,
        })
        .collect();
    keys.sort_unstable();
    keys.dedup();
    keys
}

/// A file's text form.
pub fn disassemble(b: &[u8]) -> Result<String, String> {
    let count = *b.first().ok_or("empty script")? as usize;
    let code = HEADER + count * VARIANT;
    if b.len() < code {
        return Err("the file is shorter than its variant table".into());
    }
    let entries: Vec<usize> = (0..count)
        .map(|v| u32_at(b, HEADER + v * VARIANT).unwrap() as usize)
        .collect();
    // Decode what runs: from each variant's start, along jumps.
    let mut items: BTreeMap<usize, Item> = BTreeMap::new();
    let mut targets: BTreeSet<usize> = BTreeSet::new();
    // PlaySoundByName's names: text, not code.
    let mut names: BTreeSet<usize> = BTreeSet::new();
    let mut work: Vec<usize> = entries
        .iter()
        .copied()
        .filter(|&e| e >= code && e < b.len())
        .collect();
    while let Some(mut at) = work.pop() {
        while at >= code && at < b.len() && !items.contains_key(&at) {
            let Some(item) = decode_at(b, at) else { break };
            let end = at + item.size();
            // Overlapping another decoded item: leave this one as bytes.
            if items.range(at..end).next().is_some()
                || items
                    .range(..at)
                    .next_back()
                    .is_some_and(|(o, i)| o + i.size() > at)
            {
                break;
            }
            let next = match &item {
                Item::End(_) => None,
                Item::Command { id, args } => {
                    if let Some(k) = jump_arg(*id) {
                        let t = (end as i64 + args[k] as i32 as i64) as usize;
                        if t >= code && t < b.len() {
                            targets.insert(t);
                            if *id == PLAY_SOUND_BY_NAME {
                                names.insert(t);
                            } else {
                                work.push(t);
                            }
                        }
                    }
                    falls_through(*id).then_some(end)
                }
            };
            items.insert(at, item);
            match next {
                Some(n) => at = n,
                None => break,
            }
        }
    }
    sweep_gaps(b, code, &mut items);
    // Labels: variant starts, then jump targets.
    let mut labels: BTreeMap<usize, String> = BTreeMap::new();
    for (v, &e) in entries.iter().enumerate() {
        labels.entry(e).or_insert_with(|| format!("v{v}"));
    }
    for &t in &targets {
        labels.entry(t).or_insert_with(|| format!("l_{t:04x}"));
    }
    let mut out = String::from("; kenon battle effect script\n");
    for (v, &e) in entries.iter().enumerate() {
        let name = labels.get(&e).cloned().unwrap_or_else(|| format!("{e}"));
        let at = if labels.contains_key(&e) {
            format!("@{name}")
        } else {
            format!("{e}")
        };
        // The game writes the offset in all 14 words of the record.
        let extra = &b[HEADER + v * VARIANT + 4..HEADER + (v + 1) * VARIANT];
        let repeated: Vec<u8> = (0..(VARIANT - 4) / 4)
            .flat_map(|_| (e as u32).to_le_bytes())
            .collect();
        if extra == &repeated[..] {
            let _ = writeln!(out, "variant {at}");
        } else {
            let _ = writeln!(out, "variant {at} extra {}", hex(extra));
        }
    }
    if b[1..HEADER].iter().any(|&x| x != 0) {
        let _ = writeln!(out, "pad {}", hex(&b[1..HEADER]));
    }
    let mut at = code;
    let mut raw: Vec<u8> = Vec::new();
    let flush = |out: &mut String, raw: &mut Vec<u8>| {
        for chunk in raw.chunks(16) {
            let _ = writeln!(out, "    bytes {}", hex(chunk));
        }
        raw.clear();
    };
    while at < b.len() {
        if let Some(l) = labels.get(&at) {
            flush(&mut out, &mut raw);
            let _ = writeln!(out, "@{l}:");
        }
        match items.get(&at) {
            Some(item) => {
                flush(&mut out, &mut raw);
                match item {
                    Item::End(id) => {
                        let _ = writeln!(out, "    end {id}");
                    }
                    Item::Command { id, args } => {
                        let end = at + item.size();
                        let mut line = format!("    {}", COMMANDS[*id as usize].0);
                        for (k, &a) in args.iter().enumerate() {
                            let t = (end as i64 + a as i32 as i64) as usize;
                            if jump_arg(*id) == Some(k) && labels.contains_key(&t) {
                                let _ = write!(line, " @{}", labels[&t]);
                            } else {
                                let _ = write!(line, " {}", a as i32);
                            }
                        }
                        out.push_str(&line);
                        out.push('\n');
                    }
                }
                at += item.size();
            }
            None => {
                // A sound's name: printable text up to its NUL.
                if names.contains(&at) {
                    let len = b[at..].iter().position(|&c| c == 0);
                    if let Some(n) = len.filter(|&n| {
                        b[at..at + n]
                            .iter()
                            .all(|c| (0x20..0x7F).contains(c) && *c != b'"')
                    }) {
                        flush(&mut out, &mut raw);
                        let _ = writeln!(
                            out,
                            "    text \"{}\"",
                            String::from_utf8_lossy(&b[at..at + n])
                        );
                        at += n + 1;
                        continue;
                    }
                }
                raw.push(b[at]);
                at += 1;
            }
        }
    }
    flush(&mut out, &mut raw);
    Ok(out)
}

fn hex(b: &[u8]) -> String {
    b.iter()
        .map(|x| format!("{x:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// A text form's binary, or the first error with its line number.
pub fn assemble(text: &str) -> Result<Vec<u8>, String> {
    enum Line<'a> {
        Variant(&'a str, Vec<u8>),
        Label(&'a str),
        Command(u16, Vec<&'a str>),
        End(u16),
        Bytes(Vec<u8>),
        Pad(Vec<u8>),
    }
    let names: BTreeMap<String, u16> = COMMANDS
        .iter()
        .enumerate()
        .map(|(i, (n, _))| (n.to_ascii_lowercase(), i as u16))
        .collect();
    let parse_hex = |words: &[&str], n: usize| -> Result<Vec<u8>, String> {
        words
            .iter()
            .map(|w| u8::from_str_radix(w, 16).map_err(|_| format!("'{w}' is not a hex byte")))
            .collect::<Result<Vec<_>, _>>()
            .and_then(|v| {
                if n == 0 || v.len() == n {
                    Ok(v)
                } else {
                    Err(format!("expected {n} bytes, found {}", v.len()))
                }
            })
    };
    let mut lines: Vec<(usize, Line)> = Vec::new();
    for (no, raw) in text.lines().enumerate() {
        let no = no + 1;
        let err = |e: String| format!("line {no}: {e}");
        let l = raw.split(';').next().unwrap_or("").trim();
        if l.is_empty() {
            continue;
        }
        if let Some(name) = l.strip_prefix('@').and_then(|r| r.strip_suffix(':')) {
            lines.push((no, Line::Label(name.trim())));
            continue;
        }
        let words: Vec<&str> = l.split_whitespace().collect();
        let item = match words[0].to_ascii_lowercase().as_str() {
            "variant" => {
                let at = words
                    .get(1)
                    .ok_or_else(|| err("variant needs a label".into()))?;
                // No extra: the offset repeated, as the game writes it.
                let extra = match words.get(2) {
                    Some(&"extra") => parse_hex(&words[3..], VARIANT - 4).map_err(err)?,
                    Some(w) => return Err(err(format!("unexpected '{w}'"))),
                    None => Vec::new(),
                };
                Line::Variant(at, extra)
            }
            "pad" => Line::Pad(parse_hex(&words[1..], HEADER - 1).map_err(err)?),
            "end" => Line::End(
                words
                    .get(1)
                    .map_or(Ok(0xFFFF), |w| number(w).map(|n| n as u16))
                    .map_err(err)?,
            ),
            "bytes" => Line::Bytes(parse_hex(&words[1..], 0).map_err(err)?),
            // A NUL-terminated name (PlaySoundByName's): `text "mod:seq"`.
            "text" => {
                let rest = l[4..].trim();
                let inner = rest
                    .strip_prefix('"')
                    .and_then(|r| r.strip_suffix('"'))
                    .ok_or_else(|| err("text takes a quoted name".into()))?;
                if !inner
                    .bytes()
                    .all(|c| (0x20..0x7F).contains(&c) && c != b'"')
                {
                    return Err(err("a name is plain ASCII".into()));
                }
                let mut b = inner.as_bytes().to_vec();
                b.push(0);
                Line::Bytes(b)
            }
            name => {
                let id = *names
                    .get(name)
                    .ok_or_else(|| err(format!("unknown command '{}'", words[0])))?;
                let want = COMMANDS[id as usize].1;
                if words.len() - 1 != want {
                    return Err(err(format!(
                        "{} takes {want} arguments, found {}",
                        COMMANDS[id as usize].0,
                        words.len() - 1
                    )));
                }
                Line::Command(id, words[1..].to_vec())
            }
        };
        lines.push((no, item));
    }
    // Offsets: the header, then the items in order.
    let variants: Vec<(usize, &str, &Vec<u8>)> = lines
        .iter()
        .filter_map(|(no, l)| match l {
            Line::Variant(at, extra) => Some((*no, *at, extra)),
            _ => None,
        })
        .collect();
    if variants.is_empty() || variants.len() > 255 {
        return Err("a script has 1 to 255 variants".into());
    }
    let code = HEADER + variants.len() * VARIANT;
    let mut labels: BTreeMap<&str, usize> = BTreeMap::new();
    let mut at = code;
    for (no, l) in &lines {
        match l {
            Line::Label(name) => {
                if labels.insert(name, at).is_some() {
                    return Err(format!("line {no}: label @{name} is defined twice"));
                }
            }
            Line::Command(id, _) => at += 2 + 4 * COMMANDS[*id as usize].1,
            Line::End(_) => at += 2,
            Line::Bytes(b) => at += b.len(),
            _ => {}
        }
    }
    let target = |no: usize, w: &str| -> Result<usize, String> {
        let name = w.trim_start_matches('@');
        labels
            .get(name)
            .copied()
            .ok_or_else(|| format!("line {no}: no label @{name}"))
    };
    let mut out = vec![0u8; code];
    out[0] = variants.len() as u8;
    for (v, (no, at, extra)) in variants.iter().enumerate() {
        let entry = if at.starts_with('@') {
            target(*no, at)?
        } else {
            number(at).map_err(|e| format!("line {no}: {e}"))? as usize
        };
        let rec = &mut out[HEADER + v * VARIANT..HEADER + (v + 1) * VARIANT];
        if extra.is_empty() {
            for w in rec.chunks_mut(4) {
                w.copy_from_slice(&(entry as u32).to_le_bytes());
            }
        } else {
            rec[..4].copy_from_slice(&(entry as u32).to_le_bytes());
            rec[4..].copy_from_slice(extra);
        }
    }
    for (no, l) in &lines {
        match l {
            Line::Pad(p) => out[1..HEADER].copy_from_slice(p),
            Line::Command(id, args) => {
                let end = out.len() + 2 + 4 * args.len();
                out.extend_from_slice(&id.to_le_bytes());
                for (k, w) in args.iter().enumerate() {
                    let v = if w.starts_with('@') {
                        if jump_arg(*id) != Some(k) {
                            return Err(format!("line {no}: only a jump's offset can be a label"));
                        }
                        (target(*no, w)? as i64 - end as i64) as i32 as u32
                    } else {
                        number(w).map_err(|e| format!("line {no}: {e}"))?
                    };
                    out.extend_from_slice(&v.to_le_bytes());
                }
            }
            Line::End(id) => {
                if (*id as usize) < COMMANDS.len() {
                    return Err(format!(
                        "line {no}: an end id is {} or more",
                        COMMANDS.len()
                    ));
                }
                out.extend_from_slice(&id.to_le_bytes());
            }
            Line::Bytes(b) => out.extend_from_slice(b),
            _ => {}
        }
    }
    Ok(out)
}

/// A decimal (signed) or 0x hex number as the u32 the game reads.
fn number(w: &str) -> Result<u32, String> {
    let bad = || format!("'{w}' is not a number");
    if let Some(h) = w.strip_prefix("0x").or_else(|| w.strip_prefix("0X")) {
        return u32::from_str_radix(h, 16).map_err(|_| bad());
    }
    if let Some(h) = w.strip_prefix("-0x") {
        return u32::from_str_radix(h, 16)
            .map(|v| (v as i32).wrapping_neg() as u32)
            .map_err(|_| bad());
    }
    w.parse::<i64>()
        .ok()
        .filter(|v| *v >= i32::MIN as i64 && *v <= u32::MAX as i64)
        .map(|v| v as u32)
        .ok_or_else(bad)
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_named_sound_reads_back() {
        let text = "variant @v0
@v0:
    PlaySoundByName @snd 1 0 0 0 127 0 0
    EndScript
@snd:
    text \"mymod:boom\"
";
        let b = assemble(text).unwrap();
        let code = HEADER + VARIANT;
        // The command, then EndScript, then the name; the offset counts from the command's end.
        assert_eq!(u16::from_le_bytes([b[code], b[code + 1]]), 78);
        let end = code + 2 + 4 * 8;
        let off = u32::from_le_bytes(b[code + 2..code + 6].try_into().unwrap()) as usize;
        assert_eq!(&b[end + off..end + off + 11], b"mymod:boom ");
        let again = disassemble(&b).unwrap();
        assert!(
            again.contains("PlaySoundByName @") && again.contains("text \"mymod:boom\""),
            "{again}"
        );
        assert_eq!(assemble(&again).unwrap(), b);
    }

    use super::*;

    fn script() -> Vec<u8> {
        // One variant; WaitUntilIdle 5; JumpScript to the end; WaitUntilIdle 1
        // (skipped); end.
        let mut b = vec![1, 0, 0, 0];
        for _ in 0..VARIANT / 4 {
            b.extend_from_slice(&(HEADER as u32 + VARIANT as u32).to_le_bytes());
        }
        b.extend_from_slice(&0x38u16.to_le_bytes());
        b.extend_from_slice(&5u32.to_le_bytes());
        b.extend_from_slice(&0x48u16.to_le_bytes());
        b.extend_from_slice(&6u32.to_le_bytes());
        b.extend_from_slice(&0x38u16.to_le_bytes());
        b.extend_from_slice(&1u32.to_le_bytes());
        b.extend_from_slice(&0xFFFFu16.to_le_bytes());
        b
    }

    #[test]
    fn round_trip() {
        let b = script();
        let t = disassemble(&b).unwrap();
        assert!(t.contains("WaitUntilIdle 5"), "{t}");
        assert!(t.contains("Jump @l_"), "{t}");
        // The command the jump skips is decoded too.
        assert!(t.contains("WaitUntilIdle 1\n"), "{t}");
        assert_eq!(assemble(&t).unwrap(), b);
    }

    #[test]
    fn labels_follow_edits() {
        let t = disassemble(&script()).unwrap();
        // One more command before the jump target: the offset grows by 6.
        let t2 = t.replace(
            "    WaitUntilIdle 5\n",
            "    WaitUntilIdle 5\n    WaitUntilIdle 2\n",
        );
        let b2 = assemble(&t2).unwrap();
        let jump_off = HEADER + VARIANT + 12 + 2;
        assert_eq!(u32_at(&b2, jump_off), Some(6));
        assert!(assemble("variant @x\n@x:\n    WaitFrames\n")
            .unwrap_err()
            .contains("line 3"));
        assert!(assemble("variant @x\n@x:\n    Nope 1\n")
            .unwrap_err()
            .contains("unknown command"));
    }
}

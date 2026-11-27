//! A new type's pictures, in eidos `eidos/tables/types.txt` (eidos
//! research types-pictures.md; draft format):
//!
//! ```text
//! 17 = icon 172 colors F6A5D5 9C4A7B label 40 button E6B4D5 ... hof 34 3 color F6A5D5
//! ```
//!
//! - icon: a/0/8/3 member, a 32x16 4bpp RGCN like the game's (member 0x22 +
//!   t), shipped without a palette: index 0 clear, 14 the game's outline,
//!   15 the white lettering, 1..13 the `colors` in order.
//! - label: a/1/5/9 member, six 32x16 Pokedex labels (Japanese, French,
//!   German, Italian, Spanish, Korean), same index rules.
//! - button: the battle move button's six shades.
//! - hof: the Hall of Fame particle and palette of a/2/1/6.
//! - color: the eidos one-screen UI tag colour.
//!
//! kenon makes a new type's icon from the game's own: the body of the type
//! it copies, recoloured, and the type's name spelled with letters cut from
//! the words on the 17 game icons, with the shadow the game draws (one pixel
//! right, below and diagonal of each white pixel).

use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};

use crate::bridge::{Bridge, Error, Member};
use crate::project::Project;

const TABLE: &str = "types";
const ICONS: u16 = 83;
const LABELS: u16 = 159;
const W: usize = 32;
const H: usize = 16;
/// Icon text rows: 7 rows of letters and the shadow row under them.
const TEXT_TOP: usize = 4;
const TEXT_ROWS: usize = 7;
const OUTLINE: u8 = 14;
const WHITE: u8 = 15;
/// The game's per-type palette rows of a/0/8/3 member 0x21 (rows 0..2),
/// with the body colours each icon uses (eidos research 2.3).
const ICON_PALETTE: [[u16; 16]; 3] = [
    [0x0000, 0x62B5, 0x2E97, 0x2EDA, 0x1B1F, 0x4A95, 0x0D4E, 0x256E, 0x18E9, 0x1D54, 0x048E, 0x195E, 0x5F18, 0x227F, 0x294A, 0x7FFF],
    [0x0000, 0x51DF, 0x5176, 0x28E9, 0x598C, 0x7AB3, 0x4948, 0x7E67, 0x732B, 0x1CED, 0x55CA, 0x6ED2, 0x7F78, 0x7C1F, 0x294A, 0x7FFF],
    [0x0000, 0x718F, 0x48E9, 0x2544, 0x4A8D, 0x1DA8, 0x2B2F, 0x12F5, 0x7C1F, 0x7C1F, 0x7C1F, 0x7C1F, 0x7C1F, 0x7C1F, 0x294A, 0x7FFF],
];
const ICON_ROW: [usize; 17] = [0, 0, 1, 1, 0, 0, 2, 1, 0, 0, 1, 2, 0, 1, 1, 2, 0];
/// The battle move button's six shades per game type (a/0/1/1 row 0,
/// indices 8..13).
const BUTTONS: [[u16; 6]; 17] = [
    [0x6B5A, 0x5294, 0x3DEF, 0x318C, 0x2529, 0x18C6],
    [0x0978, 0x0556, 0x0513, 0x00F1, 0x00CE, 0x00AC],
    [0x7E2F, 0x6DED, 0x5D8A, 0x4D48, 0x3CE6, 0x2CA4],
    [0x6113, 0x54F1, 0x48AF, 0x388D, 0x2C6B, 0x2049],
    [0x0E37, 0x09F4, 0x09B1, 0x094D, 0x090A, 0x08C7],
    [0x29F2, 0x21B0, 0x1D6E, 0x192B, 0x10E9, 0x0CA7],
    [0x0E93, 0x0A31, 0x09CE, 0x098C, 0x0929, 0x08C7],
    [0x494B, 0x3D09, 0x30E8, 0x24A6, 0x1C64, 0x1043],
    [0x6AFA, 0x5A76, 0x49F2, 0x398E, 0x290A, 0x1886],
    [0x14DF, 0x10BB, 0x0C97, 0x0C93, 0x086F, 0x086B],
    [0x7DE4, 0x6DA3, 0x5963, 0x4922, 0x34E2, 0x24A2],
    [0x12C5, 0x0E64, 0x09E3, 0x0562, 0x0501, 0x08A2],
    [0x171F, 0x12BB, 0x0E36, 0x0DB1, 0x092C, 0x08C7],
    [0x351F, 0x30FC, 0x28D8, 0x24B5, 0x1C91, 0x186E],
    [0x7F09, 0x72A7, 0x6245, 0x51C3, 0x4161, 0x3100],
    [0x7D2B, 0x6CE9, 0x5CC8, 0x4CA6, 0x3C64, 0x2C43],
    [0x252E, 0x1D0C, 0x18CB, 0x14A9, 0x0C87, 0x0866],
];
/// Hall of Fame particle and palette members of a/2/1/6 per game type.
const HOF: [(u16, u16); 17] = [
    (0x2D, 0x03), (0x26, 0x09), (0x28, 0x0B), (0x2E, 0x10), (0x2B, 0x0E), (0x30, 0x12),
    (0x22, 0x05), (0x29, 0x0C), (0x31, 0x13), (0x27, 0x0A), (0x32, 0x14), (0x2A, 0x0D),
    (0x25, 0x08), (0x2F, 0x11), (0x2C, 0x0F), (0x24, 0x07), (0x23, 0x06),
];

/// A colour as RRGGBB.
fn hex(c: u16) -> String {
    let f = |v: u16| ((v & 31) as u32 * 255 / 31) as u8;
    format!("{:02X}{:02X}{:02X}", f(c), f(c >> 5), f(c >> 10))
}
fn from_hex(s: &str) -> Option<u16> {
    let v = u32::from_str_radix(s.trim_start_matches('#'), 16).ok().filter(|_| s.trim_start_matches('#').len() == 6)?;
    let c = |sh: u32| ((v >> sh) & 0xFF) as u16 >> 3;
    Some(c(16) | c(8) << 5 | c(0) << 10)
}
fn scale(c: u16, k: f32) -> u16 {
    let ch = |sh: u16| (((c >> sh) & 31) as f32 * k).round().clamp(0.0, 31.0) as u16;
    ch(0) | ch(5) << 5 | ch(10) << 10
}

/// One type's pictures as types.txt holds them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeLook {
    /// The word eidos files use for the type (`type=fairy`), kept when the
    /// type is renamed so lines that name it still work.
    #[serde(default)]
    pub name: Option<String>,
    pub icon: usize,
    /// RRGGBB for icon indices 1, 2, ...
    pub colors: Vec<String>,
    pub label: Option<usize>,
    /// Six RRGGBB shades, light to dark.
    pub button: Vec<String>,
    pub hof: (u16, u16),
    pub color: String,
    /// The item that is the type's plate (Arceus, Judgment).
    #[serde(default)]
    pub plate: Option<usize>,
    /// The item that is the type's drive and the Genesect form it gives
    /// (5 and up; 1 to 4 are the game's drives).
    #[serde(default)]
    pub drive: Option<(usize, usize)>,
}

fn parse(line: &str) -> Option<TypeLook> {
    let words: Vec<&str> = line.split('#').next()?.split_whitespace().collect();
    let mut look = TypeLook { name: None, icon: 0, colors: vec![], label: None, button: vec![], hof: (HOF[0].0, HOF[0].1), color: String::new(), plate: None, drive: None };
    let mut i = 0;
    let keys = ["name", "icon", "colors", "label", "button", "hof", "color", "plate", "drive"];
    while i < words.len() {
        let key = words[i];
        let vals: Vec<&str> = words[i + 1..].iter().take_while(|w| !keys.contains(w)).copied().collect();
        match key {
            "name" => look.name = Some(vals.first()?.to_string()),
            "icon" => look.icon = vals.first()?.parse().ok()?,
            "colors" => look.colors = vals.iter().map(|s| s.to_string()).collect(),
            "label" => look.label = vals.first()?.parse().ok(),
            "button" => look.button = vals.iter().map(|s| s.to_string()).collect(),
            "hof" => look.hof = (vals.first()?.parse().ok()?, vals.get(1)?.parse().ok()?),
            "color" => look.color = vals.first()?.to_string(),
            "plate" => look.plate = Some(vals.first()?.parse().ok()?),
            "drive" => look.drive = Some((vals.first()?.parse().ok()?, vals.get(1)?.parse().ok()?)),
            _ => return None,
        }
        i += 1 + vals.len();
    }
    Some(look)
}

fn format(l: &TypeLook) -> String {
    let mut s = l.name.as_ref().map(|n| format!("name {n} ")).unwrap_or_default();
    s += &format!("icon {} colors {}", l.icon, l.colors.join(" "));
    if let Some(m) = l.label {
        s += &format!(" label {m}");
    }
    if !l.button.is_empty() {
        s += &format!(" button {}", l.button.join(" "));
    }
    s += &format!(" hof {} {} color {}", l.hof.0, l.hof.1, l.color);
    if let Some(p) = l.plate {
        s += &format!(" plate {p}");
    }
    if let Some((item, form)) = l.drive {
        s += &format!(" drive {item} {form}");
    }
    s
}

/// The mod's lines: type to its pictures.
pub fn all(project: &Project) -> BTreeMap<usize, TypeLook> {
    project
        .table_lines(TABLE)
        .into_iter()
        .filter_map(|(t, line)| Some((t, parse(&line)?)))
        .collect()
}

fn write_all(project: &Project, rows: &BTreeMap<usize, TypeLook>) -> Result<(), Error> {
    let lines = rows.iter().map(|(t, l)| (*t, format(l))).collect();
    project
        .write_table_lines(TABLE, &lines, "<type> = name <word> [plate <item>] [drive <item> <form>] icon <a/0/8/3 member> colors <RRGGBB...> label <a/1/5/9 member> button <6 RRGGBB> hof <particle> <palette> color <RRGGBB>.")
        .map_err(|e| Error::Missing(e.to_string()))
}

// ---- icon pixels -------------------------------------------------------

/// A 32x16 icon as colour indices, row by row.
pub type Icon = Vec<u8>;

/// The 4bpp 1D tile data of an icon (8 tiles, 4 across, 2 down).
fn tiles_to_icon(data: &[u8]) -> Icon {
    let mut out = vec![0u8; W * H];
    for y in 0..H {
        for x in 0..W {
            let tile = (y / 8) * 4 + x / 8;
            let p = (y % 8) * 8 + x % 8;
            let b = data.get(tile * 32 + p / 2).copied().unwrap_or(0);
            out[y * W + x] = if p % 2 == 0 { b & 15 } else { b >> 4 };
        }
    }
    out
}
fn icon_to_tiles(icon: &Icon) -> Vec<u8> {
    let mut out = vec![0u8; 256];
    for y in 0..H {
        for x in 0..W {
            let tile = (y / 8) * 4 + x / 8;
            let p = (y % 8) * 8 + x % 8;
            let v = icon[y * W + x] & 15;
            out[tile * 32 + p / 2] |= if p % 2 == 0 { v } else { v << 4 };
        }
    }
    out
}

/// A game type's icon (a/0/8/3 0x22 + t), or an added type's.
pub fn icon_of(bridge: &Bridge, t: usize) -> Result<Icon, Error> {
    let member = if t < 17 {
        0x22 + t
    } else {
        bridge
            .project()
            .and_then(|p| all(p).get(&t).map(|l| l.icon))
            .ok_or_else(|| Error::Missing(format!("type {t} has no icon")))?
    };
    let b = bridge.member(Member { arc: ICONS, index: member })?;
    let g = crate::ncgr::Ncgr::parse(&b).map_err(|e| Error::Format(e.to_string()))?;
    Ok(tiles_to_icon(&g.data))
}

/// The colour of each index of a type's icon (RGB555, 16 entries).
pub fn palette_of(bridge: &Bridge, t: usize) -> [u16; 16] {
    if t < 17 {
        return ICON_PALETTE[ICON_ROW[t]];
    }
    let mut pal = [0u16; 16];
    pal[OUTLINE as usize] = 0x294A;
    pal[WHITE as usize] = 0x7FFF;
    if let Some(l) = bridge.project().and_then(|p| all(p).remove(&t)) {
        for (i, c) in l.colors.iter().enumerate().take(13) {
            pal[1 + i] = from_hex(c).unwrap_or(0);
        }
    }
    pal
}

/// An icon as RGBA pixels with a palette.
pub fn rgba(icon: &Icon, pal: &[u16; 16]) -> Vec<u8> {
    icon.iter()
        .flat_map(|&i| {
            let c = pal[i as usize & 15];
            let f = |v: u16| ((v & 31) as u32 * 255 / 31) as u8;
            [f(c), f(c >> 5), f(c >> 10), if i == 0 { 0 } else { 255 }]
        })
        .collect()
}

/// A letter cut from a game icon: its white width, and its white and shadow
/// pixels (column from the letter's left, row from TEXT_TOP, colour index).
/// The shadow is the game's own drawing, so it is cut with the letter.
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord)]
struct Glyph {
    width: usize,
    pixels: Vec<(usize, usize, u8)>,
}

/// The letters of the game's icon words, cut from the 17 icons by their
/// names (the most common drawing of each).
fn letters(bridge: &Bridge) -> Result<HashMap<char, Glyph>, Error> {
    let names = crate::tables::enum_values(bridge, "type")?;
    let mut words = Vec::new();
    for t in 0..17 {
        let icon = icon_of(bridge, t)?;
        let word: Vec<char> = names.get(t).map(|n| n.to_uppercase().chars().filter(|c| !c.is_whitespace()).collect()).unwrap_or_default();
        // Columns holding white text, in runs: one run per letter, or
        // several letters whose white touches.
        let white = |x: usize| (TEXT_TOP..TEXT_TOP + TEXT_ROWS).any(|y| icon[y * W + x] == WHITE);
        let mut runs: Vec<(usize, usize)> = Vec::new();
        let mut x = 0;
        while x < W {
            if white(x) {
                let s = x;
                while x < W && white(x) {
                    x += 1;
                }
                runs.push((s, x));
            } else {
                x += 1;
            }
        }
        words.push((icon, word, runs));
    }
    // A letter's white columns, the shadow column after them and the shadow
    // row under them. Columns past `end` belong to the next letter.
    let cut = |icon: &Icon, s: usize, e: usize, end: usize| {
        let mut pixels = Vec::new();
        for y in TEXT_TOP..=TEXT_TOP + TEXT_ROWS {
            for x in s..=end.min(W - 1) {
                let v = icon[y * W + x];
                if v == WHITE || v == OUTLINE {
                    pixels.push((x - s, y - TEXT_TOP, v));
                }
            }
        }
        Glyph { width: e - s, pixels }
    };
    let mut found: HashMap<char, Vec<Glyph>> = HashMap::new();
    // First the words whose letters all stand apart.
    for (icon, word, runs) in &words {
        if runs.len() == word.len() {
            for (&ch, &(s, e)) in word.iter().zip(runs) {
                found.entry(ch).or_default().push(cut(icon, s, e, e));
            }
        }
    }
    // Then words with touching letters (FLYING's L and Y): a run is split
    // at the known width of its first letter.
    let widths: HashMap<char, usize> = found.iter().map(|(c, g)| (*c, g[0].width)).collect();
    for (icon, word, runs) in &words {
        if runs.len() >= word.len() {
            continue;
        }
        let mut letters = word.iter().peekable();
        let mut cuts: Vec<(char, usize, usize, usize)> = Vec::new();
        for &(s0, e) in runs {
            let mut s = s0;
            while s < e {
                let Some(&ch) = letters.next() else { break };
                let take = match widths.get(&ch) {
                    Some(&w) if s + w < e => w,
                    _ => e - s,
                };
                // A split letter has no shadow column of its own.
                let end = if s + take < e { s + take - 1 } else { e };
                cuts.push((ch, s, s + take, end));
                s += take;
            }
        }
        if cuts.len() == word.len() {
            for (ch, s, e, end) in cuts {
                // Only letters not cut cleanly elsewhere come from here.
                if !widths.contains_key(&ch) {
                    found.entry(ch).or_default().push(cut(icon, s, e, end));
                }
            }
        }
    }
    // The most common drawing of each letter.
    Ok(found
        .into_iter()
        .map(|(ch, mut shapes)| {
            shapes.sort();
            let best = shapes
                .iter()
                .max_by_key(|s| shapes.iter().filter(|o| o == s).count())
                .cloned()
                .unwrap_or_default();
            (ch, best)
        })
        .collect())
}

/// The letters a name needs that no game icon has.
pub fn missing_letters(bridge: &Bridge, name: &str) -> Result<Vec<char>, Error> {
    let lib = letters(bridge)?;
    let mut out: Vec<char> = name.to_uppercase().chars().filter(|c| !c.is_whitespace() && !lib.contains_key(c)).collect();
    out.dedup();
    Ok(out)
}

/// An icon: `from`'s body recoloured to `fill` and `border`, with `name`
/// in the game's letters and shadow. Body indices: 1 fill, 2 border.
pub fn make_icon(bridge: &Bridge, from: usize, name: &str) -> Result<Icon, Error> {
    let lib = letters(bridge)?;
    let word: Vec<char> = name.to_uppercase().chars().filter(|c| !c.is_whitespace()).collect();
    if let Some(c) = word.iter().find(|c| !lib.contains_key(c)) {
        return Err(Error::BadPath(format!(
            "No built-in type icon has the letter {c}. Import a 32x16 PNG instead."
        )));
    }
    let src = icon_of(bridge, from)?;
    // The body: the source's two most used body indices become 1 (fill,
    // the inside) and 2 (border, the frame's colour at the top row).
    let border_idx = src[2 * W];
    let mut icon: Icon = src
        .iter()
        .enumerate()
        .map(|(_, &v)| match v {
            0 => 0,
            OUTLINE | WHITE => 1, // text pixels become fill, then redrawn
            v if v == border_idx => 2,
            _ => 1,
        })
        .collect();
    // Outline pixels of the frame stay (Normal's frame is drawn in it): the
    // frame's top and bottom rows and its outer columns. The rest were the
    // source word's shadow.
    for y in 0..H {
        for x in 0..W {
            let frame = y == 2 || y == 13 || x == 0 || x == W - 1;
            if src[y * W + x] == OUTLINE && frame {
                icon[y * W + x] = OUTLINE;
            }
        }
    }
    // Lay out the word: the shadow column and one body column between
    // letters (the game's usual spacing), else the shadow alone when that
    // would not fit, centred. The game spaced a few words by hand, so a
    // rebuilt game icon can sit a pixel apart from the original.
    let width = |gap: usize| word.iter().map(|c| lib[c].width).sum::<usize>() + gap * word.len().saturating_sub(1) + 1;
    let gap = if width(2) <= W - 4 { 2 } else { 1 };
    let total = width(gap);
    if total > W - 2 {
        return Err(Error::BadPath(format!("\"{name}\" is too long for a type icon.")));
    }
    let mut x0 = (W - total) / 2;
    // Shadows first, then the white over them: a letter's white is never
    // covered by its neighbour's shadow.
    let mut placed: Vec<(usize, usize, u8)> = Vec::new();
    for c in &word {
        let glyph = &lib[c];
        placed.extend(glyph.pixels.iter().map(|&(dx, dy, v)| (x0 + dx, TEXT_TOP + dy, v)));
        x0 += glyph.width + gap;
    }
    for pass in [OUTLINE, WHITE] {
        for &(x, y, v) in &placed {
            if v == pass && x < W && y < H {
                icon[y * W + x] = v;
            }
        }
    }
    Ok(icon)
}

/// Six button shades from a colour, light to dark, as the game's ramps go.
fn button_ramp(fill: u16) -> Vec<String> {
    [1.0, 0.88, 0.76, 0.64, 0.52, 0.40].iter().map(|&k| hex(scale(fill, k))).collect()
}

/// The icon file: a copy of the game's icon RGCN (member 0x22) with new
/// pixels.
fn icon_file(bridge: &Bridge, icon: &Icon) -> Result<Vec<u8>, Error> {
    let template = bridge.member(Member { arc: ICONS, index: 0x22 })?;
    crate::ncgr::Ncgr::with_data(&template, &icon_to_tiles(icon)).map_err(|e| Error::Format(e.to_string()))
}

/// The label file: six labels, each the icon (one word in every language
/// until the user draws them), as a copy of the game's label sheet layout:
/// 48 tiles in one row of 4x2-tile cells, 1D.
fn label_file(bridge: &Bridge, icon: &Icon) -> Result<Vec<u8>, Error> {
    let template = bridge.member(Member { arc: ICONS, index: 0x22 })?;
    let tiles = icon_to_tiles(icon);
    let data: Vec<u8> = (0..6).flat_map(|_| tiles.clone()).collect();
    // The icon RGCN's header with six times the tiles.
    let g = crate::ncgr::Ncgr::parse(&template).map_err(|e| Error::Format(e.to_string()))?;
    let mut out = template[..template.len() - g.data.len()].to_vec();
    out.extend_from_slice(&data);
    // RGCN sizes: file size at 8, the RAHC block size at 0x14 and its data
    // size at 0x28.
    let total = out.len() as u32;
    out[8..12].copy_from_slice(&total.to_le_bytes());
    let rahc = total - 0x10;
    out[0x14..0x18].copy_from_slice(&rahc.to_le_bytes());
    out[0x28..0x2C].copy_from_slice(&(data.len() as u32).to_le_bytes());
    Ok(out)
}

/// The first free member past an archive's end and the mod's own members.
fn free_member(bridge: &Bridge, project: &Project, arc: u16, taken: &[usize]) -> Result<usize, Error> {
    let base = bridge.base_archive(arc)?.len();
    let mut m = base.max(project.last_member(arc).map_or(0, |l| l + 1));
    while taken.contains(&m) {
        m += 1;
    }
    Ok(m)
}

/// Makes type `t`'s pictures: its icon from `from`'s with its name, the
/// labels, the button shades from `fill`, the Hall of Fame of `from`.
pub fn create(bridge: &Bridge, t: usize, from: usize, name: &str, fill: Option<u16>) -> Result<TypeLook, Error> {
    let project = bridge.project().ok_or_else(|| Error::BadPath("no project is open".into()))?;
    let pal = palette_of(bridge, from);
    let src = icon_of(bridge, from)?;
    let border_src = pal[src[2 * W] as usize & 15];
    let fill_src = pal[src[3 * W + 1] as usize & 15];
    let fill = fill.unwrap_or(fill_src);
    // The border keeps the source's darkness relative to its fill.
    let border = if fill == fill_src { border_src } else { scale(fill, 0.6) };
    let icon = make_icon(bridge, from, name)?;
    let rows = all(project);
    let taken_icons: Vec<usize> = rows.iter().filter(|(k, _)| **k != t).map(|(_, l)| l.icon).collect();
    let taken_labels: Vec<usize> = rows.iter().filter(|(k, _)| **k != t).filter_map(|(_, l)| l.label).collect();
    let icon_m = rows.get(&t).map(|l| l.icon).unwrap_or(free_member(bridge, project, ICONS, &taken_icons)?);
    let label_m = rows.get(&t).and_then(|l| l.label).unwrap_or(free_member(bridge, project, LABELS, &taken_labels)?);
    let io = |e: std::io::Error| Error::Missing(e.to_string());
    project.write_member(ICONS, icon_m, &icon_file(bridge, &icon)?).map_err(io)?;
    project.write_member(LABELS, label_m, &label_file(bridge, &icon)?).map_err(io)?;
    let hof = if from < 17 { HOF[from] } else { rows.get(&from).map(|l| l.hof).unwrap_or(HOF[0]) };
    let taken_names: Vec<String> = rows.iter().filter(|(k, _)| **k != t).filter_map(|(_, l)| l.name.clone()).collect();
    let look = TypeLook {
        name: rows.get(&t).and_then(|l| l.name.clone()).or_else(|| Some(key_word(name, &taken_names))),
        icon: icon_m,
        colors: vec![hex(fill), hex(border)],
        label: Some(label_m),
        // The source's own shades when the colour is the source's.
        button: if fill == fill_src && from < 17 { BUTTONS[from].iter().map(|&c| hex(c)).collect() } else { button_ramp(fill) },
        hof,
        color: hex(fill),
        plate: rows.get(&t).and_then(|l| l.plate),
        drive: rows.get(&t).and_then(|l| l.drive),
    };
    let mut rows = rows;
    rows.insert(t, look.clone());
    write_all(project, &rows)?;
    bridge.invalidate();
    Ok(look)
}

/// The word for a type's name in eidos files: lowercase a-z, 0-9 and _,
/// not starting with a digit, unlike the game's 17 and the mod's others.
pub fn key_word(display: &str, taken: &[String]) -> String {
    let mut w: String = display
        .chars()
        .flat_map(char::to_lowercase)
        .map(|c| if c.is_ascii_lowercase() || c.is_ascii_digit() { c } else { '_' })
        .collect();
    w = w.trim_matches('_').to_string();
    if w.is_empty() {
        w = "type".into();
    }
    if w.starts_with(|c: char| c.is_ascii_digit()) {
        w.insert_str(0, "type_");
    }
    let used = |x: &str| GAME_TYPE_WORDS.contains(&x) || taken.iter().any(|t| t == x);
    if !used(&w) {
        return w;
    }
    (2..).map(|n| format!("{w}_{n}")).find(|x| !used(x)).unwrap()
}

/// The game's 17 type words in eidos files.
pub const GAME_TYPE_WORDS: [&str; 17] = [
    "normal", "fighting", "flying", "poison", "ground", "rock", "bug", "ghost", "steel", "fire", "water", "grass",
    "electric", "psychic", "ice", "dragon", "dark",
];

/// The added types' words, by type.
pub fn key_names(project: &Project) -> Vec<(usize, String)> {
    all(project).into_iter().filter_map(|(t, l)| Some((t, l.name?))).collect()
}

/// Sets an added type's colours (fill and border) and the button shades
/// that follow the fill.
pub fn set_colors(bridge: &Bridge, t: usize, fill: &str, border: &str) -> Result<(), Error> {
    let project = bridge.project().ok_or_else(|| Error::BadPath("no project is open".into()))?;
    let (Some(f), Some(_)) = (from_hex(fill), from_hex(border)) else {
        return Err(Error::BadPath("colours are RRGGBB".into()));
    };
    let mut rows = all(project);
    let look = rows.get_mut(&t).ok_or_else(|| Error::Missing(format!("type {t} has no pictures")))?;
    look.colors = vec![fill.to_uppercase(), border.to_uppercase()];
    look.button = button_ramp(f);
    look.color = fill.to_uppercase();
    write_all(project, &rows)?;
    bridge.invalidate();
    Ok(())
}

/// Replaces an added type's icon from RGBA pixels (32x16): clear stays
/// clear, the rest takes the nearest of fill, border, outline and white.
pub fn import_icon(bridge: &Bridge, t: usize, rgba_px: &[u8]) -> Result<(), Error> {
    let project = bridge.project().ok_or_else(|| Error::BadPath("no project is open".into()))?;
    if rgba_px.len() != W * H * 4 {
        return Err(Error::BadPath("a type icon is 32 x 16 pixels".into()));
    }
    let pal = palette_of(bridge, t);
    let choices = [1usize, 2, OUTLINE as usize, WHITE as usize];
    let icon: Icon = rgba_px
        .chunks(4)
        .map(|p| {
            if p[3] < 128 {
                return 0;
            }
            *choices
                .iter()
                .min_by_key(|&&i| {
                    let c = pal[i];
                    let f = |v: u16| (v & 31) as i32 * 255 / 31;
                    (f(c) - p[0] as i32).pow(2) + (f(c >> 5) - p[1] as i32).pow(2) + (f(c >> 10) - p[2] as i32).pow(2)
                })
                .unwrap() as u8
        })
        .collect();
    let look = all(project).remove(&t).ok_or_else(|| Error::Missing(format!("type {t} has no pictures")))?;
    let io = |e: std::io::Error| Error::Missing(e.to_string());
    project.write_member(ICONS, look.icon, &icon_file(bridge, &icon)?).map_err(io)?;
    if let Some(l) = look.label {
        project.write_member(LABELS, l, &label_file(bridge, &icon)?).map_err(io)?;
    }
    bridge.invalidate();
    Ok(())
}

/// After an added type is removed: its line and its pictures go, later
/// types' lines move down one (their members stay).
pub fn remove_type(project: &Project, t: usize) -> Result<(), Error> {
    let mut rows = all(project);
    if let Some(l) = rows.remove(&t) {
        let io = |e: std::io::Error| Error::Missing(e.to_string());
        project.revert_member(ICONS, l.icon).map_err(io)?;
        if let Some(m) = l.label {
            project.revert_member(LABELS, m).map_err(io)?;
        }
    }
    let shifted = rows.into_iter().map(|(k, l)| (if k > t { k - 1 } else { k }, l)).collect();
    write_all(project, &shifted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_read_back() {
        let line = "name fairy icon 172 colors F6A5D5 9C4A7B label 113 button A B C D E F hof 47 17 color F6A5D5 plate 628 drive 629 5";
        let l = parse(line).unwrap();
        assert_eq!((l.name.as_deref(), l.plate, l.drive), (Some("fairy"), Some(628), Some((629, 5))));
        assert_eq!(format(&l), line);
        assert_eq!(key_word("Fairy", &[]), "fairy");
        assert_eq!(key_word("Ice", &[]), "ice_2");
        assert_eq!(key_word("3D Star", &["type_3d_star".into()]), "type_3d_star_2");
    }

    #[test]
    fn lines() {
        let l = parse("icon 172 colors F6A5D5 9C4A7B label 40 button E6B4D5 DE94C5 D57BB4 C562A4 B44A94 9C3983 hof 34 3 color F6A5D5").unwrap();
        assert_eq!((l.icon, l.label, l.hof), (172, Some(40), (34, 3)));
        assert_eq!(l.colors, vec!["F6A5D5", "9C4A7B"]);
        assert_eq!(l.button.len(), 6);
        assert_eq!(parse(&format(&l)), Some(l));
        assert_eq!(from_hex("F6A5D5").map(hex).as_deref(), Some("F6A4D5"));
    }
}

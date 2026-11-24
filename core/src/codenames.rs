//! aletheia's names for the game's functions, in every cartridge it maps.
//!
//! `code_names.tsv.gz` (written by tools/code-names.py from an aletheia
//! checkout) holds every function of English Black with its region, address,
//! size, instruction set and source file, and the runs that place English
//! addresses in the other European cartridges. A Japanese cartridge has no
//! runs, so nothing is named there.

use std::collections::HashMap;
use std::io::Read;
use std::sync::OnceLock;

/// One function, at its English Black address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Function {
    pub region: String,
    pub address: u32,
    pub size: u32,
    pub thumb: bool,
    pub name: String,
    /// The source file in aletheia's src/game.
    pub file: String,
}

/// English `[start, end)` sits at `target` in a cartridge.
#[derive(Debug, Clone, Copy)]
struct Run {
    start: u32,
    end: u32,
    target: u32,
}

struct Table {
    /// Per region, by address.
    functions: HashMap<String, Vec<Function>>,
    /// Per game code, per region, by target address.
    runs: HashMap<String, HashMap<String, Vec<Run>>>,
}

/// The cartridge the table's addresses are from.
pub const ENGLISH_BLACK: &str = "IRBO";

fn table() -> &'static Table {
    static T: OnceLock<Table> = OnceLock::new();
    T.get_or_init(|| {
        let mut text = String::new();
        let packed: &[u8] = include_bytes!("code_names.tsv.gz");
        let _ = flate2::read::GzDecoder::new(packed).read_to_string(&mut text);
        let mut t = Table {
            functions: HashMap::new(),
            runs: HashMap::new(),
        };
        let hex = |s: &str| u32::from_str_radix(s, 16).unwrap_or(0);
        for line in text.lines() {
            let f: Vec<&str> = line.split('\t').collect();
            match f.as_slice() {
                ["F", region, addr, size, isa, name, file] => t
                    .functions
                    .entry(region.to_string())
                    .or_default()
                    .push(Function {
                        region: region.to_string(),
                        address: hex(addr),
                        size: size.parse().unwrap_or(0),
                        thumb: *isa == "t",
                        name: name.to_string(),
                        file: file.to_string(),
                    }),
                ["M", game, region, start, end, target] => t
                    .runs
                    .entry(game.to_string())
                    .or_default()
                    .entry(region.to_string())
                    .or_default()
                    .push(Run {
                        start: hex(start),
                        end: hex(end),
                        target: hex(target),
                    }),
                _ => {}
            }
        }
        for list in t.functions.values_mut() {
            list.sort_by_key(|f| f.address);
        }
        for regions in t.runs.values_mut() {
            for runs in regions.values_mut() {
                runs.sort_by_key(|r| r.target);
            }
        }
        t
    })
}

/// Whether the table names anything in this cartridge.
pub fn knows(game_code: &str) -> bool {
    game_code == ENGLISH_BLACK || table().runs.contains_key(game_code)
}

/// The English address of a cartridge's address in a region.
fn to_english(game_code: &str, region: &str, addr: u32) -> Option<u32> {
    if game_code == ENGLISH_BLACK {
        return Some(addr);
    }
    let runs = table().runs.get(game_code)?.get(region)?;
    let i = runs.partition_point(|r| r.target <= addr).checked_sub(1)?;
    let r = runs[i];
    let off = addr - r.target;
    (off < r.end - r.start).then_some(r.start + off)
}

/// The cartridge's address of an English address in a region.
fn from_english(game_code: &str, region: &str, addr: u32) -> Option<u32> {
    if game_code == ENGLISH_BLACK {
        return Some(addr);
    }
    let runs = table().runs.get(game_code)?.get(region)?;
    runs.iter()
        .find(|r| r.start <= addr && addr < r.end)
        .map(|r| r.target + (addr - r.start))
}

/// The function that holds `addr` in a region (`arm9`, `ov093`) of the
/// cartridge, and where it starts there.
pub fn function_at(game_code: &str, region: &str, addr: u32) -> Option<(&'static Function, u32)> {
    let english = to_english(game_code, region, addr)?;
    let list = table().functions.get(region)?;
    let i = list
        .partition_point(|f| f.address <= english)
        .checked_sub(1)?;
    let f = &list[i];
    if english >= f.address + f.size.max(1) {
        return None;
    }
    let start = from_english(game_code, region, f.address).unwrap_or(addr - (english - f.address));
    Some((f, start))
}

/// The functions of a region that overlap `[start, end)` in the cartridge,
/// each with where it starts there.
pub fn functions_in(
    game_code: &str,
    region: &str,
    start: u32,
    end: u32,
) -> Vec<(&'static Function, u32)> {
    let mut out: Vec<(&'static Function, u32)> = Vec::new();
    let mut at = start;
    while at < end {
        match function_at(game_code, region, at) {
            Some((f, s)) => {
                if !out.iter().any(|(g, _)| std::ptr::eq(*g, f)) {
                    out.push((f, s));
                }
                at = (s + f.size.max(1)).max(at + 1);
            }
            None => at += 2,
        }
    }
    out
}

/// `Name` or `Name+0x1c`.
pub fn label(game_code: &str, region: &str, addr: u32) -> Option<String> {
    let (f, start) = function_at(game_code, region, addr)?;
    Some(if addr == start {
        f.name.clone()
    } else {
        format!("{}+{:#x}", f.name, addr - start)
    })
}

/// The region name of an overlay: `ov093`.
pub fn overlay_region(n: u16) -> String {
    format!("ov{n:03}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_in_black_and_white() {
        let (f, start) = function_at("IRBO", "arm9", 0x0200_4EA4).unwrap();
        assert_eq!(
            (f.name.as_str(), f.thumb),
            ("MIi_UncompressBackward", false)
        );
        assert_eq!(f.file, "arm9/crt0.c");
        assert!(start <= 0x0200_4EA4);
        assert_eq!(
            label("IRBO", "arm9", 0x0200_4694).as_deref(),
            Some("SVC_UncompressRL8")
        );
        // ov230 loads 0x20 higher in English White
        let (f, start) = function_at("IRAO", "ov230", 0x0218_89D0).unwrap();
        assert_eq!(
            (f.name.as_str(), start),
            ("LOADER_ENTRY_Start", 0x0218_89D0)
        );
        assert!(knows("IRAF") && !knows("IRBJ"));
        assert!(function_at("IRBJ", "arm9", 0x0200_4EA4).is_none());
    }
}

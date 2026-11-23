//! Turns a ROM hack patch (xdelta) into a project: the patch is applied to
//! the player's ROM in memory, the result is compared with the ROM, and
//! every changed archive member and file becomes an override in a new mod
//! folder, which kenon then edits like any other project.
//!
//! What a data mod cannot carry is reported, not dropped silently: changed
//! code (ARM9, overlays, ARM7), the header and the banner, members an
//! archive lost, and files the ROM does not have.

use std::fs;
use std::path::Path;

use eidos_mod::manifest::Icon;
use nds::banner::Banner;
use nds::rom::Rom;
use nds::romdiff::{self, FileDiff};
use serde::Serialize;

use crate::bridge::Error;
use crate::project::Project;

/// Where the banner's icon goes in the project.
const ICON_FILE: &str = "icon.png";

/// What an import did and what it left out.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchImport {
    /// The project folder.
    pub root: String,
    /// The mod's name: the hack's banner title when it has its own.
    pub name: String,
    /// The hack's banner icon became the mod's icon.
    pub icon: bool,
    pub game_code: String,
    /// Member overrides written, and the archives they are in.
    pub members: usize,
    pub archives: usize,
    /// Whole files written.
    pub files: Vec<String>,
    /// Code the patch changes, which the mod does not carry: one line each.
    pub code: Vec<String>,
    /// Other things the mod does not carry.
    pub left_out: Vec<String>,
    /// Parts of the ROM past its game data that differ from the ROM the
    /// patch was made on (only filler there).
    pub filler_windows: usize,
}

/// Applies `patch` to `rom` and writes the differences as a new project in
/// `into` (an empty or new folder).
pub fn import_patch(
    rom: &Path,
    patch: &Path,
    into: &Path,
    name: &str,
) -> Result<PatchImport, Error> {
    let io = |what: &str, e: std::io::Error| Error::Missing(format!("{what}: {e}"));
    let source = fs::read(rom).map_err(|e| io("the ROM", e))?;
    let patch_bytes = fs::read(patch).map_err(|e| io("the patch", e))?;
    let base =
        Rom::from_bytes(source.clone()).map_err(|e| Error::Format(format!("the ROM: {e}")))?;

    let (output, bad) =
        xdelta::apply_checked(&source, &patch_bytes).map_err(|e| Error::Format(e.to_string()))?;
    // A window that fails its checksum is fine only past the game data, and
    // only where it copies the ROM's own filler: the patch's maker had other
    // filler bytes there. Anywhere else the ROM is not the one the patch
    // was made for.
    let used = used_size(&source);
    for m in &bad {
        let (s, e) = (m.offset as usize, (m.offset + m.len) as usize);
        if m.offset < used || source.get(s..e) != output.get(s..e) {
            return Err(Error::Format(format!(
                "this patch was made for another ROM: the game data does not match at {:#x}. The open ROM is {}; the patch needs a clean, unchanged dump of the game it was made for.",
                m.offset, base.identity.game_code
            )));
        }
    }
    let hacked =
        Rom::from_bytes(output).map_err(|e| Error::Format(format!("the patched ROM: {e}")))?;
    let diff = romdiff::diff(&base, &hacked)
        .map_err(|e| Error::Format(format!("comparing the ROMs: {e}")))?;

    let mut project =
        Project::create(into, name, Some(rom)).map_err(|e| io("the project folder", e))?;
    project.manifest.game = base.identity.game_code.clone();
    if let Some(file) = patch.file_name() {
        project.manifest.description = format!("Made from {}.", file.to_string_lossy());
    }
    // The hack's own banner names the mod, and its icon becomes the mod's.
    let mut icon = false;
    if diff.banner_changed {
        let (old, new) = (
            base.banner().map_err(|e| Error::Format(e.to_string()))?,
            hacked.banner().map_err(|e| Error::Format(e.to_string()))?,
        );
        if let Some(banner) = Banner::parse(&new) {
            let title = banner.name();
            if !title.is_empty() {
                project.manifest.name = title;
            }
            if Banner::icon_differs(&old, &new) {
                fs::write(
                    project.root.join(ICON_FILE),
                    nds::image::encode_png(&banner.icon),
                )
                .map_err(|e| io(ICON_FILE, e))?;
                project.manifest.icon = Some(Icon::File(ICON_FILE.into()));
                icon = true;
            }
        }
    }
    project.save_meta().map_err(|e| io("the project", e))?;

    let mut report = PatchImport {
        root: into.to_string_lossy().into_owned(),
        name: project.manifest.name.clone(),
        icon,
        game_code: base.identity.game_code.clone(),
        filler_windows: bad.len(),
        ..Default::default()
    };
    for f in &diff.files {
        match f {
            FileDiff::Narc {
                path,
                changed,
                added,
                removed,
            } => {
                let id = archive_id(path).ok_or_else(|| Error::BadPath(path.clone()))?;
                let narc = nds::narc::Narc::parse(
                    hacked
                        .file(path)
                        .map_err(|e| Error::Missing(e.to_string()))?,
                )
                .map_err(|e| Error::Format(format!("{path}: {e:?}")))?;
                for &i in changed.iter().chain(added) {
                    let bytes = narc
                        .raw(i)
                        .map_err(|e| Error::Format(format!("{path}/{i}: {e:?}")))?;
                    project
                        .write_member(id, i, bytes)
                        .map_err(|e| io(path, e))?;
                }
                report.members += changed.len() + added.len();
                report.archives += 1;
                if !added.is_empty() {
                    report.left_out.push(format!(
                        "{path}: {} members past the end of the archive are written, but eidos may not load new members there",
                        added.len()
                    ));
                }
                if *removed > 0 {
                    report.left_out.push(format!("{path}: the patch removes {removed} members at the end; a mod cannot remove members"));
                }
            }
            FileDiff::Whole { path, .. } => {
                write_file(&project, path, &hacked)?;
                report.files.push(path.clone());
            }
        }
    }
    for path in &diff.added_files {
        write_file(&project, path, &hacked)?;
        report.files.push(path.clone());
        report.left_out.push(format!(
            "{path} is a new file: it is written, but eidos only reads paths the game has"
        ));
    }
    for path in &diff.removed_files {
        report.left_out.push(format!(
            "{path}: the patch removes this file; a mod cannot remove files"
        ));
    }
    for c in &diff.code {
        let bytes: u32 = c.ranges.iter().map(|(s, e)| e - s).sum();
        let spans: Vec<String> = c
            .ranges
            .iter()
            .take(6)
            .map(|(s, e)| format!("{s:#010x}-{e:#010x}"))
            .collect();
        let more = if c.ranges.len() > 6 {
            format!(" and {} more", c.ranges.len() - 6)
        } else {
            String::new()
        };
        report.code.push(format!(
            "{}: {bytes} bytes changed at {}{more}",
            c.part,
            spans.join(", ")
        ));
    }
    Ok(report)
}

/// The ROM's used size (header 0x80): where its game data ends.
fn used_size(rom: &[u8]) -> u64 {
    rom.get(0x80..0x84)
        .map_or(0, |b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as u64)
}

/// `a/0/1/6` -> 16.
fn archive_id(path: &str) -> Option<u16> {
    let rest = path.strip_prefix("a/")?;
    let digits: Vec<&str> = rest.split('/').collect();
    if digits.len() != 3
        || digits
            .iter()
            .any(|d| d.len() != 1 || !d.as_bytes()[0].is_ascii_digit())
    {
        return None;
    }
    digits.concat().parse().ok()
}

fn write_file(project: &Project, path: &str, rom: &Rom) -> Result<(), Error> {
    let bytes = rom.file(path).map_err(|e| Error::Missing(e.to_string()))?;
    let target = project.root.join(path.trim_start_matches('/'));
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|e| Error::Missing(format!("{path}: {e}")))?;
    }
    fs::write(&target, bytes).map_err(|e| Error::Missing(format!("{path}: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn archive_ids() {
        assert_eq!(archive_id("a/0/1/6"), Some(16));
        assert_eq!(archive_id("a/1/2/5"), Some(125));
        assert_eq!(archive_id("a/0/1"), None);
        assert_eq!(archive_id("data/x"), None);
    }
}

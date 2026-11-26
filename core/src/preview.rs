//! Starting eidos to preview something from the mod being edited: a move's
//! animation (`--preview-move`, eidos docs/game-tables.md).
//!
//! The program is found the way the eidos launcher finds "latest": the
//! newest installed version under `<user folder>/versions`, else the
//! launcher's local build. `KENON_EIDOS` (a program or its folder) comes
//! first, for a development build. The save comes from the game's own
//! profile (`profiles/Black` or `profiles/White`), which eidos copies and
//! never writes.

use std::cmp::Ordering;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use crate::data_root::eidos_user_folder;

const EXES: [&str; 5] = [
    "eidos.exe",
    "eidos",
    "eidos.AppImage",
    "eidos_boot.exe",
    "eidos_boot",
];

/// The eidos program inside a folder (or one folder below it), or the file
/// itself.
fn find_exe(path: &Path) -> Option<PathBuf> {
    if path.is_file() {
        return Some(path.to_path_buf());
    }
    let in_dir = |d: &Path| EXES.iter().map(|n| d.join(n)).find(|p| p.is_file());
    if let Some(p) = in_dir(path) {
        return Some(p);
    }
    let mut subs: Vec<_> = fs::read_dir(path)
        .ok()?
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .map(|e| e.path())
        .collect();
    subs.sort();
    subs.iter().find_map(|d| in_dir(d))
}

fn numbers(id: &str) -> Vec<u64> {
    id.split('-')
        .next()
        .unwrap_or("")
        .split(|c: char| !c.is_ascii_digit())
        .filter_map(|s| s.parse().ok())
        .collect()
}

/// Version order as the launcher has it: by numbers, a release after its
/// own pre-release.
fn compare(a: &str, b: &str) -> Ordering {
    numbers(a).cmp(&numbers(b)).then_with(|| {
        let pre = |s: &str| s.contains('-');
        pre(b).cmp(&pre(a)).then_with(|| a.cmp(b))
    })
}

/// The eidos program to run, and the version it is ("local" for a local
/// or development build).
pub fn program() -> Option<(PathBuf, String)> {
    if let Some(p) = std::env::var_os("KENON_EIDOS").map(PathBuf::from) {
        if let Some(exe) = find_exe(&p) {
            return Some((exe, "local".into()));
        }
    }
    let user = eidos_user_folder()?;
    let mut ids: Vec<String> = fs::read_dir(user.join("versions"))
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| !n.ends_with(".partial"))
        .collect();
    ids.sort_by(|a, b| compare(b, a));
    for id in ids {
        if let Some(exe) = find_exe(&user.join("versions").join(&id)) {
            return Some((exe, id));
        }
    }
    let settings = fs::read_to_string(user.join("launcher/settings.json")).ok()?;
    let v: serde_json::Value = serde_json::from_str(&settings).ok()?;
    let local = PathBuf::from(v.get("local_build")?.as_str()?);
    find_exe(&local).map(|exe| (exe, "local".into()))
}

/// The game's own profile for a game code (White for IRA*, else Black).
pub fn profile_dir(game_code: &str) -> Option<PathBuf> {
    let name = if game_code.starts_with("IRA") {
        "White"
    } else {
        "Black"
    };
    Some(eidos_user_folder()?.join("profiles").join(name))
}

/// The arguments for a move animation preview. `--window` because eidos
/// runs headless when it gets other arguments.
pub fn move_args(
    profile: &Path,
    project: &Path,
    mv: usize,
    species: Option<(usize, usize)>,
) -> Vec<OsString> {
    let mut args: Vec<OsString> = vec![
        "--profile-dir".into(),
        profile.into(),
        "--window".into(),
        "--mod".into(),
        project.into(),
        "--preview-move".into(),
        mv.to_string().into(),
    ];
    if let Some((a, d)) = species {
        args.push("--preview-species".into());
        args.push(a.to_string().into());
        args.push(d.to_string().into());
    }
    args
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_order() {
        assert_eq!(compare("0.10.0", "0.9.0"), Ordering::Greater);
        assert_eq!(compare("1.0.0", "1.0.0-rc1"), Ordering::Greater);
        assert_eq!(profile_dir("IRAO").unwrap().file_name().unwrap(), "White");
        assert_eq!(profile_dir("IRBJ").unwrap().file_name().unwrap(), "Black");
    }
}

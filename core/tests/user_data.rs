//! Checks against the user's own data folder. Set `KENON_DATA_ROOT` to the
//! extraction folder (or its `data/` folder) to run them; without it they
//! pass without doing anything. No game data is in this repository.

use std::path::PathBuf;

use kenon_core::data_root::{self, DataRoot, DataRootCheck};
use kenon_core::{lz, narc};

fn data_root() -> Option<DataRoot> {
    let path = PathBuf::from(std::env::var_os("KENON_DATA_ROOT")?);
    match data_root::check(&path) {
        DataRootCheck::Ready(root) => Some(root),
        other => panic!("KENON_DATA_ROOT is set but not usable: {other:?}"),
    }
}

#[test]
fn every_archive_parses() {
    let Some(root) = data_root() else {
        eprintln!("KENON_DATA_ROOT not set; skipped");
        return;
    };
    let mut archives = 0;
    let mut members = 0;
    let mut compressed = 0;
    for id in 0..1000u16 {
        let rel = narc::archive_path(id).unwrap();
        let path = root.file(&rel);
        if !path.is_file() {
            continue;
        }
        archives += 1;
        // The one archive that is not a NARC: 282 raw 10-byte records
        // (aletheia docs/ASSETS.md, "a/0/1/3: not a NARC").
        if rel == "a/0/1/3" {
            assert!(narc::Narc::open(&path).is_err());
            continue;
        }
        let archive = narc::Narc::open(&path).unwrap_or_else(|err| panic!("{rel}: {err}"));
        members += archive.len();
        compressed += archive
            .iter_raw()
            .filter(|raw| lz::decompress(raw).is_ok())
            .count();
    }
    eprintln!("{archives} archives, {members} members, {compressed} LZ members");
    // The game's path table has 235 archives (inventory section 0).
    assert_eq!(archives, 235);
}

#[test]
fn font_archive_holds_nftr_members() {
    let Some(root) = data_root() else {
        return;
    };
    let fonts = narc::Narc::open(&root.file("a/0/2/3")).unwrap();
    let nftr = fonts
        .iter_raw()
        .filter(|raw| raw.starts_with(b"RTFN"))
        .count();
    assert!(nftr > 0, "no NFTR member in a/0/2/3");
}

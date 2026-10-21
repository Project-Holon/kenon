//! The `.bw` package: a mod folder as one file (`docs/bw-format.md`).
//!
//! A `.bw` is a zip with deflate. It holds every file of the mod folder
//! except kenon's own notes, with `eidos_mod.txt` first. Packing the same
//! folder twice gives the same bytes.

use std::fs;
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};

use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, DateTime, ZipArchive, ZipWriter};

use crate::project::{Manifest, KENON_FILE, MANIFEST};

/// One file in a package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// The path inside the package, with forward slashes.
    pub path: String,
    /// The size once unpacked.
    pub size: u64,
}

/// What a package holds.
#[derive(Debug, Clone)]
pub struct Info {
    pub manifest: Manifest,
    /// Every file, manifest first.
    pub files: Vec<Entry>,
}

fn bad(msg: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg.into())
}

/// kenon's notes, and the half-written files of its atomic saves.
fn is_bookkeeping(rel: &str) -> bool {
    let name = rel.rsplit('/').next().unwrap_or(rel);
    rel == KENON_FILE || (name.starts_with('~') && name.ends_with(".kenon"))
}

fn collect(dir: &Path, prefix: &str, out: &mut Vec<(String, PathBuf)>) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let rel = if prefix.is_empty() {
            name
        } else {
            format!("{prefix}/{name}")
        };
        let kind = entry.file_type()?;
        if kind.is_dir() {
            collect(&entry.path(), &rel, out)?;
        } else if kind.is_file() && !is_bookkeeping(&rel) {
            out.push((rel, entry.path()));
        }
    }
    Ok(())
}

/// Packs the mod folder `root` into `out_path`.
pub fn pack(root: &Path, out_path: &Path) -> io::Result<()> {
    let mut files = Vec::new();
    collect(root, "", &mut files)?;
    // The package itself may sit inside the folder.
    if let Ok(out) = fs::canonicalize(out_path) {
        files.retain(|(_, p)| fs::canonicalize(p).map_or(true, |c| c != out));
    }
    if !files.iter().any(|(rel, _)| rel == MANIFEST) {
        return Err(bad(format!("the folder has no {MANIFEST}")));
    }
    // Manifest first, then by path.
    files.sort_by(|a, b| {
        (a.0 != MANIFEST)
            .cmp(&(b.0 != MANIFEST))
            .then_with(|| a.0.cmp(&b.0))
    });

    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .last_modified_time(DateTime::default())
        .unix_permissions(0o644);
    let tmp = out_path.with_extension("bw.tmp");
    let result = (|| {
        let mut zip = ZipWriter::new(io::BufWriter::new(fs::File::create(&tmp)?));
        for (rel, path) in &files {
            zip.start_file(rel.as_str(), options)
                .map_err(io::Error::other)?;
            zip.write_all(&fs::read(path)?)?;
        }
        zip.finish().map_err(io::Error::other)?.flush()?;
        fs::rename(&tmp, out_path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

/// A package path as a relative path, or None when it could leave the
/// folder it is unpacked into.
fn safe_path(name: &str) -> Option<PathBuf> {
    if name.is_empty() || name.contains('\\') || name.contains('\0') {
        return None;
    }
    let mut out = PathBuf::new();
    for part in name.split('/') {
        // A drive prefix ("C:") would make the path absolute on Windows.
        if part.is_empty() || part.contains(':') {
            return None;
        }
        match Path::new(part).components().next() {
            Some(Component::Normal(p)) => out.push(p),
            _ => return None,
        }
    }
    Some(out)
}

fn open_archive(bw: &Path) -> io::Result<ZipArchive<io::BufReader<fs::File>>> {
    let file = fs::File::open(bw)?;
    ZipArchive::new(io::BufReader::new(file)).map_err(|e| bad(format!("not a .bw package: {e}")))
}

/// Lists a package and reads its manifest, without unpacking.
pub fn inspect(bw: &Path) -> io::Result<Info> {
    let mut zip = open_archive(bw)?;
    let mut files = Vec::new();
    for i in 0..zip.len() {
        let entry = zip.by_index(i).map_err(|e| bad(e.to_string()))?;
        if entry.is_dir() {
            continue;
        }
        files.push(Entry {
            path: entry.name().to_string(),
            size: entry.size(),
        });
    }
    let mut text = String::new();
    zip.by_name(MANIFEST)
        .map_err(|_| bad(format!("the package has no {MANIFEST}")))?
        .read_to_string(&mut text)?;
    let default_name = bw
        .file_stem()
        .map_or(String::new(), |s| s.to_string_lossy().into_owned());
    Ok(Info {
        manifest: Manifest::parse(&text, &default_name),
        files,
    })
}

/// Unpacks a package into `into_dir` (made if missing). Nothing is written
/// when the package has no manifest or a path that leaves the folder.
pub fn unpack(bw: &Path, into_dir: &Path) -> io::Result<()> {
    let mut zip = open_archive(bw)?;
    let mut targets = Vec::new();
    let mut has_manifest = false;
    for i in 0..zip.len() {
        let entry = zip.by_index(i).map_err(|e| bad(e.to_string()))?;
        let name = entry.name().trim_end_matches('/');
        let rel = safe_path(name).ok_or_else(|| bad(format!("unsafe path in package: {name}")))?;
        has_manifest |= name == MANIFEST && !entry.is_dir();
        targets.push((i, rel, entry.is_dir()));
    }
    if !has_manifest {
        return Err(bad(format!("the package has no {MANIFEST}")));
    }
    fs::create_dir_all(into_dir)?;
    for (i, rel, is_dir) in targets {
        let dest = into_dir.join(rel);
        if is_dir {
            fs::create_dir_all(&dest)?;
            continue;
        }
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut entry = zip.by_index(i).map_err(|e| bad(e.to_string()))?;
        let mut out = fs::File::create(&dest)?;
        io::copy(&mut entry, &mut out)?;
    }
    Ok(())
}

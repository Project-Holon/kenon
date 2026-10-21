//! Code mods: C compiled to WebAssembly with Zig.
//!
//! The design is eidos's `docs/mod-code-wasm.md`. Everything that encodes
//! the contract (the header, the template, the compiler flags, the exports
//! the module must have) is in `modsdk/v4/`. The header is eidos's
//! published `sdk/api/v4/eidos_wasm.h`, copied unchanged; when eidos changes
//! the API, replace it and nothing else here changes.
//!
//! - [`new_code_mod`] writes the template into a project.
//! - [`zig_path`], [`fetch_zig`]: finding or downloading the compiler.
//! - [`build_code_mod`] builds `mod.wasm`, parses the compiler's messages and
//!   checks the module against the contract.
//! - the source file calls keep to `src/` and `include/` of the mod folder.

use std::collections::BTreeSet;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::modcode;
use crate::project::Project;

/// The Zig release kenon downloads. Change it here only.
pub const ZIG_VERSION: &str = "0.15.2";

const HEADER_NAME: &str = "eidos_wasm.h";
const HEADER: &str = include_str!("modsdk/v4/eidos_wasm.h");
const TEMPLATE_MOD: &str = include_str!("modsdk/v4/mod.c");
const TEMPLATE_ALLOC: &str = include_str!("modsdk/v4/alloc.c");
const CONTRACT: &str = include_str!("modsdk/v4/contract.txt");

/// Where the sources live in a mod folder.
pub const SRC_DIR: &str = "src";
pub const INCLUDE_DIR: &str = "include";

fn bad(msg: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, msg.into())
}

// ---------------------------------------------------------------------
// The contract
// ---------------------------------------------------------------------

/// `modsdk/v4/contract.txt`, read.
#[derive(Debug, Clone, Default)]
pub struct Contract {
    pub api: u32,
    pub target: String,
    pub import_module: String,
    pub output: String,
    pub flags: Vec<String>,
    /// Required function exports with their result type (`i32` or none).
    pub export_funcs: Vec<(String, Option<String>)>,
    pub export_memory: String,
    pub export_table: bool,
}

impl Contract {
    pub fn parse(text: &str) -> Self {
        let mut c = Contract::default();
        for line in text.lines() {
            let line = line.split('#').next().unwrap_or("").trim();
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let value = value.trim();
            match key.trim() {
                "api" => c.api = value.parse().unwrap_or(0),
                "target" => c.target = value.to_string(),
                "import_module" => c.import_module = value.to_string(),
                "output" => c.output = value.to_string(),
                "flag" => c.flags.push(value.to_string()),
                "export_func" => {
                    let (name, result) = value.split_once(':').unwrap_or((value, "none"));
                    let result = (result.trim() != "none").then(|| result.trim().to_string());
                    c.export_funcs.push((name.trim().to_string(), result));
                }
                "export_memory" => c.export_memory = value.to_string(),
                "export_table" => c.export_table = value == "yes",
                _ => {}
            }
        }
        c
    }
}

/// The contract the build follows.
pub fn contract() -> Contract {
    Contract::parse(CONTRACT)
}

// ---------------------------------------------------------------------
// The template
// ---------------------------------------------------------------------

/// Writes a code mod's starting files into the project and sets the
/// manifest to `code = mod.wasm` and the contract's `api`. A file that
/// exists is left alone. Returns the files written, relative to the mod.
pub fn new_code_mod(project: &Project) -> io::Result<Vec<String>> {
    let c = contract();
    let mut settings = modcode::read_settings(&project.root)?;
    if let Some(code) = settings.code.as_deref().filter(|v| !v.ends_with(".wasm")) {
        return Err(bad(format!(
            "the mod already names {code} as its code; code mods build a .wasm file"
        )));
    }
    let files = [
        (format!("{SRC_DIR}/mod.c"), TEMPLATE_MOD),
        (format!("{SRC_DIR}/alloc.c"), TEMPLATE_ALLOC),
        (format!("{INCLUDE_DIR}/{HEADER_NAME}"), HEADER),
    ];
    let mut written = Vec::new();
    for (rel, text) in files {
        let path = project.root.join(&rel);
        if path.exists() {
            continue;
        }
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        fs::write(&path, text)?;
        written.push(rel);
    }
    if settings.code.is_none() {
        settings.code = Some(c.output.clone());
    }
    settings.api = Some(c.api);
    modcode::write_settings(&project.root, &settings)?;
    Ok(written)
}

// ---------------------------------------------------------------------
// Source files
// ---------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceFile {
    /// Relative to the mod, forward slashes: `src/mod.c`.
    pub path: String,
    pub bytes: u64,
}

fn is_source_name(name: &str) -> bool {
    name.ends_with(".c") || name.ends_with(".h")
}

/// A source path as a checked relative path: inside `src/` or `include/`,
/// ending in `.c` or `.h`, nothing that leaves the folder.
fn source_path(root: &Path, rel: &str) -> io::Result<PathBuf> {
    let mut parts = Vec::new();
    for part in rel.split('/') {
        let ok = !part.is_empty()
            && part != "."
            && part != ".."
            && !part.contains(['\\', ':', '\0'])
            && !part.starts_with('~')
            && !part.ends_with('.')
            && !part.ends_with(' ');
        if !ok {
            return Err(bad(format!("{rel} is not a valid file name")));
        }
        parts.push(part);
    }
    if parts.len() < 2 || !matches!(parts[0], SRC_DIR | INCLUDE_DIR) {
        return Err(bad("sources go in src/ or include/"));
    }
    if !is_source_name(parts[parts.len() - 1]) {
        return Err(bad("a source file ends in .c or .h"));
    }
    if parts[0] == INCLUDE_DIR && !parts[parts.len() - 1].ends_with(".h") {
        return Err(bad("files in include/ end in .h"));
    }
    Ok(root.join(parts.join("/")))
}

fn walk_sources(root: &Path, dir: &str, out: &mut Vec<SourceFile>) {
    let Ok(entries) = fs::read_dir(root.join(dir)) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let rel = format!("{dir}/{name}");
        match entry.file_type() {
            Ok(t) if t.is_dir() => walk_sources(root, &rel, out),
            Ok(t) if t.is_file() && is_source_name(&name) && !name.starts_with('~') => {
                out.push(SourceFile {
                    path: rel,
                    bytes: entry.metadata().map(|m| m.len()).unwrap_or(0),
                });
            }
            _ => {}
        }
    }
}

/// Every `.c` and `.h` file under `src/` and `include/`, sorted.
pub fn list_sources(root: &Path) -> Vec<SourceFile> {
    let mut out = Vec::new();
    walk_sources(root, SRC_DIR, &mut out);
    walk_sources(root, INCLUDE_DIR, &mut out);
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out
}

pub fn read_source(root: &Path, rel: &str) -> io::Result<String> {
    fs::read_to_string(source_path(root, rel)?)
}

pub fn write_source(root: &Path, rel: &str, text: &str) -> io::Result<()> {
    let path = source_path(root, rel)?;
    if !path.is_file() {
        return Err(bad("no such file"));
    }
    atomic_write(&path, text.as_bytes())
}

/// Makes a new file. A `.c` file starts with an include of the header.
pub fn new_source(root: &Path, rel: &str) -> io::Result<()> {
    let path = source_path(root, rel)?;
    if path.exists() {
        return Err(bad("a file with this name exists"));
    }
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let text = if rel.ends_with(".c") {
        format!("#include \"{HEADER_NAME}\"\n\n")
    } else {
        String::new()
    };
    fs::write(path, text)
}

pub fn rename_source(root: &Path, from: &str, to: &str) -> io::Result<()> {
    let (a, b) = (source_path(root, from)?, source_path(root, to)?);
    if !a.is_file() {
        return Err(bad("no such file"));
    }
    if b.exists() && !from.eq_ignore_ascii_case(to) {
        return Err(bad("a file with this name exists"));
    }
    if let Some(dir) = b.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::rename(a, b)
}

pub fn delete_source(root: &Path, rel: &str) -> io::Result<()> {
    fs::remove_file(source_path(root, rel)?)
}

fn atomic_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let name = path
        .file_name()
        .map_or(String::new(), |n| n.to_string_lossy().into_owned());
    let tmp = path.with_file_name(format!("~{name}.kenon"));
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, path)
}

// ---------------------------------------------------------------------
// The header, for completion
// ---------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiFunction {
    pub name: String,
    /// The declaration without the import macro.
    pub signature: String,
    pub doc: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiConstant {
    pub name: String,
    pub value: String,
    /// The comment above its group (`battle`, `session`).
    pub group: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeApi {
    pub functions: Vec<ApiFunction>,
    pub constants: Vec<ApiConstant>,
}

/// Reads the function declarations and the enum constants out of a header.
pub fn parse_header(text: &str) -> CodeApi {
    let mut api = CodeApi::default();
    let mut doc: Vec<String> = Vec::new();
    let mut in_comment = false;
    let mut group = String::new();
    // A declaration that wraps onto more lines is joined into one.
    let mut joined = String::new();
    for raw in text.lines() {
        let mut trimmed = raw.trim();
        // A declaration may carry its note after it: `int f(void); /* note */`.
        let mut note = None;
        if !in_comment && !trimmed.starts_with("/*") {
            if let Some(at) = trimmed.find("/*").filter(|_| trimmed.ends_with("*/")) {
                let body = trimmed[at..]
                    .trim_start_matches("/*")
                    .trim_end_matches("*/");
                note = Some(body.trim().to_string());
                trimmed = trimmed[..at].trim_end();
            }
        }
        let whole;
        let line: &str = if !joined.is_empty()
            || (trimmed.starts_with("EIDOS_IMPORT(") && !trimmed.ends_with(';'))
        {
            if !joined.is_empty() {
                joined.push(' ');
            }
            joined.push_str(trimmed);
            if !trimmed.ends_with(';') {
                continue;
            }
            whole = std::mem::take(&mut joined);
            &whole
        } else {
            trimmed
        };
        if in_comment || line.starts_with("/*") {
            let body = line
                .trim_start_matches("/*")
                .trim_start_matches('*')
                .trim_end_matches("*/")
                .trim();
            if !in_comment {
                doc.clear();
            }
            if !body.is_empty() {
                doc.push(body.to_string());
            }
            in_comment = !line.ends_with("*/");
            if !in_comment && doc.len() == 1 {
                group = doc[0].clone();
            }
            continue;
        }
        if let Some((name, value)) = line.split_once('=') {
            let name = name.trim();
            let value = value.split("/*").next().unwrap_or("").trim();
            let value = value.trim_end_matches(',').trim();
            if name.starts_with("EIDOS_")
                && name
                    .bytes()
                    .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
                && !value.is_empty()
            {
                api.constants.push(ApiConstant {
                    name: name.to_string(),
                    value: value.to_string(),
                    group: group.clone(),
                });
                continue;
            }
        }
        if let Some(decl) = declaration(line) {
            api.functions.push(ApiFunction {
                name: decl.0,
                signature: decl.1,
                doc: note.unwrap_or_else(|| doc.join(" ")),
            });
        }
        if !line.is_empty() {
            doc.clear();
        }
    }
    api
}

/// `(name, signature)` of a function declaration line, if it is one.
fn declaration(line: &str) -> Option<(String, String)> {
    if line.starts_with('#') || line.starts_with("typedef") || !line.ends_with(");") {
        return None;
    }
    let mut sig = line.trim_end_matches(';').trim().to_string();
    // Drop the import macro: EIDOS_IMPORT("log") void eidos_log(...)
    if let Some(at) = sig.find("EIDOS_IMPORT(") {
        let close = sig[at..].find(')')? + at + 1;
        sig = sig[close..].trim().to_string();
    }
    let open = sig.find('(')?;
    let head = sig[..open].trim_end();
    let name = head
        .rsplit(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .next()?
        .to_string();
    (!name.is_empty()).then_some((name, sig))
}

/// The header of the project (its own copy), else the template's.
pub fn code_api(root: &Path) -> CodeApi {
    let own = fs::read_to_string(root.join(INCLUDE_DIR).join(HEADER_NAME)).ok();
    parse_header(own.as_deref().unwrap_or(HEADER))
}

// ---------------------------------------------------------------------
// Zig
// ---------------------------------------------------------------------

/// One download of the pinned release.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZigRelease {
    pub url: String,
    pub sha256: &'static str,
    pub size: u64,
    pub zip: bool,
}

/// The pinned release for a host (`std::env::consts::OS` and `ARCH`). The
/// sizes and SHA-256 sums are the ones ziglang.org/download/index.json
/// publishes for `ZIG_VERSION`.
pub fn zig_release(os: &str, arch: &str) -> Option<ZigRelease> {
    let (sha256, size, os_name, zip) = match (os, arch) {
        ("windows", "x86_64") => (
            "3a0ed1e8799a2f8ce2a6e6290a9ff22e6906f8227865911fb7ddedc3cc14cb0c",
            92_614_574,
            "windows",
            true,
        ),
        ("windows", "aarch64") => (
            "b926465f8872bf983422257cd9ec248bb2b270996fbe8d57872cca13b56fc370",
            88_464_585,
            "windows",
            true,
        ),
        ("linux", "x86_64") => (
            "02aa270f183da276e5b5920b1dac44a63f1a49e55050ebde3aecc9eb82f93239",
            53_733_924,
            "linux",
            false,
        ),
        ("linux", "aarch64") => (
            "958ed7d1e00d0ea76590d27666efbf7a932281b3d7ba0c6b01b0ff26498f667f",
            49_471_996,
            "linux",
            false,
        ),
        ("macos", "x86_64") => (
            "375b6909fc1495d16fc2c7db9538f707456bfc3373b14ee83fdd3e22b3d43f7f",
            55_800_460,
            "macos",
            false,
        ),
        ("macos", "aarch64") => (
            "3cc2bab367e185cdfb27501c4b30b1b0653c28d9f73df8dc91488e66ece5fa6b",
            50_635_984,
            "macos",
            false,
        ),
        _ => return None,
    };
    let ext = if zip { "zip" } else { "tar.xz" };
    Some(ZigRelease {
        url: format!(
            "https://ziglang.org/download/{ZIG_VERSION}/zig-{arch}-{os_name}-{ZIG_VERSION}.{ext}"
        ),
        sha256,
        size,
        zip,
    })
}

fn zig_exe_name() -> &'static str {
    if cfg!(windows) {
        "zig.exe"
    } else {
        "zig"
    }
}

/// Where the downloaded Zig is kept.
pub fn zig_install_dir(app_data: &Path) -> PathBuf {
    app_data.join("zig").join(ZIG_VERSION)
}

/// The Zig on the search path, if any.
fn zig_on_path() -> Option<PathBuf> {
    let paths = std::env::var_os("PATH")?;
    std::env::split_paths(&paths)
        .map(|dir| dir.join(zig_exe_name()))
        .find(|p| p.is_file())
}

/// Where a usable Zig is, and where it came from: the one the user chose,
/// the one kenon downloaded, the `KENON_ZIG` variable, then the search path.
pub fn zig_path(user: Option<&Path>, app_data: &Path) -> Option<(PathBuf, &'static str)> {
    if let Some(user) = user {
        let exe = if user.is_dir() {
            user.join(zig_exe_name())
        } else {
            user.to_path_buf()
        };
        return exe.is_file().then_some((exe, "own"));
    }
    let own = zig_install_dir(app_data).join(zig_exe_name());
    if own.is_file() {
        return Some((own, "downloaded"));
    }
    if let Some(env) = std::env::var_os("KENON_ZIG").map(PathBuf::from) {
        if env.is_file() {
            return Some((env, "path"));
        }
    }
    zig_on_path().map(|p| (p, "path"))
}

/// A process that opens no console window on Windows.
fn command(program: &Path) -> Command {
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    cmd
}

/// `zig version`, or None when the file does not run as Zig.
pub fn zig_version(zig: &Path) -> Option<String> {
    let out = command(zig).arg("version").output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
        .filter(|v| !v.is_empty())
}

/// Progress of [`fetch_zig`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FetchProgress {
    /// `download`, `check` or `unpack`.
    pub phase: &'static str,
    pub done: u64,
    pub total: u64,
}

/// Downloads the pinned Zig into `<app_data>/zig/<version>/` and returns the
/// compiler's path. The download is checked against the published SHA-256
/// before anything is unpacked. Uses `curl` (Windows 10 and later, macOS and
/// Linux have it) and, off Windows, `tar`.
pub fn fetch_zig(app_data: &Path, progress: &mut dyn FnMut(FetchProgress)) -> io::Result<PathBuf> {
    let release = zig_release(std::env::consts::OS, std::env::consts::ARCH).ok_or_else(|| {
        bad("there is no Zig download for this system; choose a Zig you installed")
    })?;
    let dest = zig_install_dir(app_data);
    let exe = dest.join(zig_exe_name());
    if exe.is_file() && zig_version(&exe).as_deref() == Some(ZIG_VERSION) {
        return Ok(exe);
    }
    let work = app_data.join("zig").join("~download");
    let _ = fs::remove_dir_all(&work);
    fs::create_dir_all(&work)?;
    let result = fetch_into(&release, &work, &dest, progress);
    let _ = fs::remove_dir_all(&work);
    result?;
    match zig_version(&exe) {
        Some(v) if v == ZIG_VERSION => Ok(exe),
        _ => {
            let _ = fs::remove_dir_all(&dest);
            Err(bad("the downloaded compiler does not run on this system"))
        }
    }
}

fn fetch_into(
    release: &ZigRelease,
    work: &Path,
    dest: &Path,
    progress: &mut dyn FnMut(FetchProgress),
) -> io::Result<()> {
    let archive = work.join(if release.zip { "zig.zip" } else { "zig.tar.xz" });
    let total = release.size;
    let mut child = command(Path::new("curl"))
        .args([
            "--fail",
            "--silent",
            "--show-error",
            "--location",
            "--retry",
            "2",
        ])
        .arg("--output")
        .arg(&archive)
        .arg(&release.url)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| bad(format!("could not start curl: {e}")))?;
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        let done = fs::metadata(&archive).map(|m| m.len()).unwrap_or(0);
        progress(FetchProgress {
            phase: "download",
            done,
            total,
        });
        std::thread::sleep(Duration::from_millis(150));
    };
    if !status.success() {
        let mut why = String::new();
        if let Some(mut err) = child.stderr.take() {
            let _ = err.read_to_string(&mut why);
        }
        return Err(bad(format!("the download failed: {}", why.trim())));
    }
    progress(FetchProgress {
        phase: "download",
        done: total,
        total,
    });

    // The sum is checked before the archive is opened.
    let size = fs::metadata(&archive)?.len();
    let mut hasher = Sha256::new();
    let mut file = fs::File::open(&archive)?;
    let mut buf = vec![0u8; 1 << 20];
    let mut done = 0u64;
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        done += n as u64;
        progress(FetchProgress {
            phase: "check",
            done,
            total: size,
        });
    }
    let sum: String = hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    if sum != release.sha256 {
        return Err(bad(
            "the download does not match the published checksum, so it was discarded",
        ));
    }

    let staged = work.join("zig");
    fs::create_dir_all(&staged)?;
    if release.zip {
        unpack_zip(&archive, &staged, progress)?;
    } else {
        progress(FetchProgress {
            phase: "unpack",
            done: 0,
            total: 1,
        });
        let out = command(Path::new("tar"))
            .arg("-xJf")
            .arg(&archive)
            .arg("-C")
            .arg(&staged)
            .arg("--strip-components=1")
            .output()
            .map_err(|e| bad(format!("could not start tar: {e}")))?;
        if !out.status.success() {
            return Err(bad(format!(
                "could not unpack the download: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            )));
        }
        progress(FetchProgress {
            phase: "unpack",
            done: 1,
            total: 1,
        });
    }
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    let _ = fs::remove_dir_all(dest);
    fs::rename(&staged, dest)
}

/// Unpacks the release zip without its top folder.
fn unpack_zip(
    archive: &Path,
    into: &Path,
    progress: &mut dyn FnMut(FetchProgress),
) -> io::Result<()> {
    let mut zip = zip::ZipArchive::new(io::BufReader::new(fs::File::open(archive)?))
        .map_err(io::Error::other)?;
    let total = zip.len() as u64;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(io::Error::other)?;
        let Some(rel) = entry.enclosed_name() else {
            continue;
        };
        let rel: PathBuf = rel.components().skip(1).collect();
        if rel.as_os_str().is_empty() {
            continue;
        }
        let out = into.join(&rel);
        if entry.is_dir() {
            fs::create_dir_all(&out)?;
        } else {
            if let Some(dir) = out.parent() {
                fs::create_dir_all(dir)?;
            }
            io::copy(&mut entry, &mut fs::File::create(&out)?)?;
        }
        if i % 64 == 0 {
            progress(FetchProgress {
                phase: "unpack",
                done: i as u64,
                total,
            });
        }
    }
    progress(FetchProgress {
        phase: "unpack",
        done: total,
        total,
    });
    Ok(())
}

// ---------------------------------------------------------------------
// The build
// ---------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum BuildSeverity {
    Error,
    Warning,
    Note,
}

/// One compiler or linker message. `file` is relative to the mod (empty
/// for a message about the whole build); `line` and `column` start at 1,
/// 0 when unknown.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildDiagnostic {
    pub file: String,
    pub line: u32,
    pub column: u32,
    pub severity: BuildSeverity,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildResult {
    pub ok: bool,
    /// The finished module, relative to the mod, when `ok`.
    pub wasm_path: Option<String>,
    pub size: u64,
    pub millis: u64,
    pub diagnostics: Vec<BuildDiagnostic>,
    /// The compiler's output as it was.
    pub log: String,
}

impl BuildResult {
    fn failed(message: impl Into<String>) -> Self {
        BuildResult {
            ok: false,
            wasm_path: None,
            size: 0,
            millis: 0,
            diagnostics: vec![BuildDiagnostic {
                file: String::new(),
                line: 0,
                column: 0,
                severity: BuildSeverity::Error,
                message: message.into(),
            }],
            log: String::new(),
        }
    }
}

/// Reads clang's and wasm-ld's messages (`file:line:col: error: text`,
/// `error: text`, and the `>>> referenced by` lines that follow a link
/// error). Source excerpts and counts are dropped.
pub fn parse_diagnostics(output: &str, root: &Path) -> Vec<BuildDiagnostic> {
    const KINDS: [(&str, BuildSeverity); 4] = [
        ("fatal error: ", BuildSeverity::Error),
        ("error: ", BuildSeverity::Error),
        ("warning: ", BuildSeverity::Warning),
        ("note: ", BuildSeverity::Note),
    ];
    let root_text = root.to_string_lossy().replace('\\', "/");
    let mut out: Vec<BuildDiagnostic> = Vec::new();
    for raw in output.lines() {
        let line = raw.trim_end();
        if let Some(rest) = line.trim_start().strip_prefix(">>>") {
            if let Some(last) = out.last_mut() {
                if last.line == 0 && !rest.trim().is_empty() {
                    last.message.push('\n');
                    last.message.push_str(rest.trim());
                }
            }
            continue;
        }
        // The earliest marker: `: error: ` inside the line, or at its start.
        let mut best: Option<(usize, usize, BuildSeverity)> = None;
        for (tag, severity) in KINDS {
            let at = if line.starts_with(tag) {
                Some(0)
            } else {
                line.find(&format!(": {tag}")).map(|i| i + 2)
            };
            if let Some(at) = at {
                if best.is_none_or(|(b, _, _)| at < b) {
                    best = Some((at, tag.len(), severity));
                }
            }
        }
        let Some((at, tag_len, severity)) = best else {
            continue;
        };
        let message = line[at + tag_len..].trim().to_string();
        let prefix = line[..at].trim_end_matches([':', ' ']);
        let mut file = String::new();
        let (mut ln, mut col) = (0u32, 0u32);
        // `file:line:col` from the right, so a drive letter stays whole.
        let mut parts = prefix.rsplitn(3, ':');
        let (c, l, f) = (parts.next(), parts.next(), parts.next());
        if let (Some(c), Some(l), Some(f)) = (c, l, f) {
            if let (Ok(c), Ok(l)) = (c.parse::<u32>(), l.parse::<u32>()) {
                (file, ln, col) = (f.to_string(), l, c);
            }
        }
        if file.is_empty() && !prefix.is_empty() {
            // `file:line` without a column.
            if let Some((f, l)) = prefix.rsplit_once(':') {
                if let Ok(l) = l.parse::<u32>() {
                    (file, ln) = (f.to_string(), l);
                }
            }
        }
        let mut file = file.replace('\\', "/");
        if let Some(rel) = file.strip_prefix(&root_text) {
            file = rel.trim_start_matches('/').to_string();
        }
        let file = file.trim_start_matches("./").to_string();
        // A line like `2 errors generated.` has no marker and is skipped
        // above; a warning-count line never reaches here.
        out.push(BuildDiagnostic {
            file,
            line: ln,
            column: col,
            severity,
            message,
        });
    }
    out
}

/// A relative path in the mod folder that does not climb out of it.
fn inside_mod(p: &str) -> bool {
    !p.is_empty()
        && !p.contains(['\\', ':', '\0'])
        && p.split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
}

fn collect_c(root: &Path, dir: &str, out: &mut BTreeSet<String>) {
    let Ok(entries) = fs::read_dir(root.join(dir)) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let rel = format!("{dir}/{name}");
        match entry.file_type() {
            Ok(t) if t.is_dir() => collect_c(root, &rel, out),
            Ok(t) if t.is_file() && name.ends_with(".c") && !name.starts_with('~') => {
                out.insert(rel);
            }
            _ => {}
        }
    }
}

/// The exact arguments of the compiler for a mod, for display and tests.
pub fn build_args(sources: &[String], output: &str) -> Vec<String> {
    let c = contract();
    let mut args = vec!["cc".to_string(), "-target".into(), c.target.clone()];
    args.extend(c.flags.iter().cloned());
    args.push(format!("-I{INCLUDE_DIR}"));
    args.push(format!("-I{SRC_DIR}"));
    args.extend(sources.iter().cloned());
    args.push("-o".into());
    args.push(output.to_string());
    args
}

/// Builds the mod's `.wasm` from `src/*.c`. `cache` is Zig's cache folder;
/// without one Zig uses its default, outside the mod folder either way.
pub fn build_code_mod(project: &Project, zig: &Path, cache: Option<&Path>) -> BuildResult {
    let c = contract();
    let root = &project.root;
    let settings = match modcode::read_settings(root) {
        Ok(s) => s,
        Err(e) => return BuildResult::failed(e.to_string()),
    };
    let output = settings.code.clone().unwrap_or_else(|| c.output.clone());
    if !output.ends_with(".wasm") {
        return BuildResult::failed(format!(
            "the manifest names {output} as the code; a built mod is a .wasm file"
        ));
    }
    if !inside_mod(&output) {
        return BuildResult::failed("the code file must be a path inside the mod folder");
    }
    let mut sources = BTreeSet::new();
    collect_c(root, SRC_DIR, &mut sources);
    if sources.is_empty() {
        return BuildResult::failed("the mod has no .c files in src/");
    }
    let sources: Vec<String> = sources.into_iter().collect();
    // Written beside the target under a name the packer and the file lists
    // skip, then renamed, so a failed build leaves the last good module.
    let tmp_name = format!("~{}.kenon", output.rsplit('/').next().unwrap_or(&output));
    let tmp_rel = match output.rsplit_once('/') {
        Some((dir, _)) => format!("{dir}/{tmp_name}"),
        None => tmp_name,
    };
    let args = build_args(&sources, &tmp_rel);
    if let Some(dir) = Path::new(&output).parent() {
        let _ = fs::create_dir_all(root.join(dir));
    }

    let mut cmd = command(zig);
    cmd.args(&args).current_dir(root);
    if let Some(cache) = cache {
        let _ = fs::create_dir_all(cache);
        cmd.env("ZIG_GLOBAL_CACHE_DIR", cache)
            .env("ZIG_LOCAL_CACHE_DIR", cache);
    }
    let started = Instant::now();
    let out = match cmd.output() {
        Ok(out) => out,
        Err(e) => return BuildResult::failed(format!("could not run Zig: {e}")),
    };
    let millis = started.elapsed().as_millis() as u64;
    let log = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
    );
    let mut diagnostics = parse_diagnostics(&log, root);
    let tmp = root.join(&tmp_rel);
    let mut result = BuildResult {
        ok: false,
        wasm_path: None,
        size: 0,
        millis,
        diagnostics: Vec::new(),
        log,
    };
    let has_error = diagnostics
        .iter()
        .any(|d| d.severity == BuildSeverity::Error);
    if !out.status.success() || !tmp.is_file() {
        if !has_error {
            diagnostics.push(BuildDiagnostic {
                file: String::new(),
                line: 0,
                column: 0,
                severity: BuildSeverity::Error,
                message: "the build failed; the log has the compiler's output".into(),
            });
        }
        result.diagnostics = diagnostics;
        let _ = fs::remove_file(&tmp);
        return result;
    }
    let bytes = match fs::read(&tmp) {
        Ok(b) => b,
        Err(e) => {
            result.diagnostics = vec![BuildDiagnostic {
                file: String::new(),
                line: 0,
                column: 0,
                severity: BuildSeverity::Error,
                message: e.to_string(),
            }];
            return result;
        }
    };
    if let Err(why) = check_module(&bytes, &c) {
        diagnostics.push(BuildDiagnostic {
            file: String::new(),
            line: 0,
            column: 0,
            severity: BuildSeverity::Error,
            message: why,
        });
        result.diagnostics = diagnostics;
        let _ = fs::remove_file(&tmp);
        return result;
    }
    if let Err(e) = fs::rename(&tmp, root.join(&output)) {
        result.diagnostics = vec![BuildDiagnostic {
            file: String::new(),
            line: 0,
            column: 0,
            severity: BuildSeverity::Error,
            message: format!("could not write {output}: {e}"),
        }];
        return result;
    }
    result.ok = true;
    result.size = bytes.len() as u64;
    result.wasm_path = Some(output);
    result.diagnostics = diagnostics;
    result
}

// ---------------------------------------------------------------------
// The module check
// ---------------------------------------------------------------------

/// What the check reads from a module.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ModuleInfo {
    pub imports: Vec<Import>,
    pub exports: Vec<Export>,
    /// Parameter and result types of each type entry.
    types: Vec<(Vec<u8>, Vec<u8>)>,
    /// The type entry of every function, imported ones first.
    func_types: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Import {
    pub module: String,
    pub name: String,
    pub kind: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Export {
    pub name: String,
    pub kind: u8,
    pub index: u32,
}

const KIND_FUNC: u8 = 0;
const KIND_TABLE: u8 = 1;
const KIND_MEMORY: u8 = 2;
const KIND_GLOBAL: u8 = 3;

struct Bytes<'a> {
    b: &'a [u8],
    at: usize,
}

impl<'a> Bytes<'a> {
    fn byte(&mut self) -> Result<u8, String> {
        let v = *self.b.get(self.at).ok_or("the module ends too early")?;
        self.at += 1;
        Ok(v)
    }

    fn leb(&mut self) -> Result<u32, String> {
        let (mut value, mut shift) = (0u64, 0);
        loop {
            let byte = self.byte()?;
            value |= u64::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                return u32::try_from(value)
                    .map_err(|_| "a number in the module is too large".into());
            }
            shift += 7;
            if shift > 35 {
                return Err("a number in the module is too long".into());
            }
        }
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        let end = self.at.checked_add(n).filter(|&e| e <= self.b.len());
        let end = end.ok_or("the module ends too early")?;
        let s = &self.b[self.at..end];
        self.at = end;
        Ok(s)
    }

    fn name(&mut self) -> Result<String, String> {
        let n = self.leb()? as usize;
        Ok(String::from_utf8_lossy(self.take(n)?).into_owned())
    }

    fn limits(&mut self) -> Result<(), String> {
        let flags = self.byte()?;
        self.leb()?;
        if flags & 1 != 0 {
            self.leb()?;
        }
        Ok(())
    }
}

/// Reads the type, import, function and export sections of a module.
pub fn read_module(bytes: &[u8]) -> Result<ModuleInfo, String> {
    if bytes.len() < 8 || bytes[..4] != *b"\0asm" {
        return Err("the output is not a WebAssembly module".into());
    }
    if bytes[4..8] != [1, 0, 0, 0] {
        return Err("the module is not WebAssembly version 1".into());
    }
    let mut info = ModuleInfo::default();
    let mut r = Bytes { b: bytes, at: 8 };
    while r.at < bytes.len() {
        let id = r.byte()?;
        let size = r.leb()? as usize;
        let body = r.take(size)?;
        let mut s = Bytes { b: body, at: 0 };
        match id {
            1 => {
                for _ in 0..s.leb()? {
                    if s.byte()? != 0x60 {
                        return Err("the module has a type this check does not know".into());
                    }
                    let np = s.leb()? as usize;
                    let params = s.take(np)?.to_vec();
                    let nr = s.leb()? as usize;
                    let results = s.take(nr)?.to_vec();
                    info.types.push((params, results));
                }
            }
            2 => {
                for _ in 0..s.leb()? {
                    let module = s.name()?;
                    let name = s.name()?;
                    let kind = s.byte()?;
                    match kind {
                        KIND_FUNC => info.func_types.push(s.leb()?),
                        KIND_TABLE => {
                            s.byte()?;
                            s.limits()?;
                        }
                        KIND_MEMORY => s.limits()?,
                        KIND_GLOBAL => {
                            s.byte()?;
                            s.byte()?;
                        }
                        _ => {
                            s.byte()?;
                            s.leb()?;
                        }
                    }
                    info.imports.push(Import { module, name, kind });
                }
            }
            3 => {
                for _ in 0..s.leb()? {
                    info.func_types.push(s.leb()?);
                }
            }
            7 => {
                for _ in 0..s.leb()? {
                    let name = s.name()?;
                    let kind = s.byte()?;
                    let index = s.leb()?;
                    info.exports.push(Export { name, kind, index });
                }
            }
            _ => {}
        }
    }
    Ok(info)
}

/// Checks a built module against the contract. The messages are for the
/// person who wrote the mod.
pub fn check_module(bytes: &[u8], contract: &Contract) -> Result<ModuleInfo, String> {
    let info = read_module(bytes)?;
    for i in &info.imports {
        if i.kind == KIND_MEMORY {
            return Err("the module imports its memory; it has to export it".into());
        }
        if i.module != contract.import_module {
            return Err(format!(
                "the module imports {}.{}; a code mod may import only from \"{}\"",
                i.module, i.name, contract.import_module
            ));
        }
    }
    for (name, result) in &contract.export_funcs {
        let Some(e) = info
            .exports
            .iter()
            .find(|e| e.name == *name && e.kind == KIND_FUNC)
        else {
            return Err(format!("the module does not export the function {name}"));
        };
        let ty = info
            .func_types
            .get(e.index as usize)
            .and_then(|t| info.types.get(*t as usize));
        let wanted: &[u8] = match result.as_deref() {
            Some("i32") => &[0x7f],
            _ => &[],
        };
        if ty.is_none_or(|(params, results)| !params.is_empty() || results != wanted) {
            let shape = if result.is_some() {
                "returns an i32"
            } else {
                "returns nothing"
            };
            return Err(format!("{name} takes no arguments and {shape}"));
        }
    }
    if !contract.export_memory.is_empty()
        && !info
            .exports
            .iter()
            .any(|e| e.name == contract.export_memory && e.kind == KIND_MEMORY)
    {
        return Err(format!(
            "the module does not export its memory as \"{}\"",
            contract.export_memory
        ));
    }
    if contract.export_table && !info.exports.iter().any(|e| e.kind == KIND_TABLE) {
        return Err(
            "the module does not export its function table (link with --export-table)".into(),
        );
    }
    Ok(info)
}

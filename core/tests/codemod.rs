use std::fs;
use std::path::{Path, PathBuf};

use kenon_core::codemod::*;
use kenon_core::modcode;
use kenon_core::package;
use kenon_core::project::Project;

fn new_project(dir: &Path) -> Project {
    Project::create(&dir.join("mod"), "Code test", None).unwrap()
}

#[test]
fn template_is_written_once() {
    let dir = tempfile::tempdir().unwrap();
    let project = new_project(dir.path());
    let written = new_code_mod(&project).unwrap();
    assert_eq!(
        written,
        ["src/mod.c", "src/alloc.c", "include/eidos_wasm.h"]
    );
    let settings = modcode::read_settings(&project.root).unwrap();
    assert_eq!(settings.code.as_deref(), Some("mod.wasm"));
    assert_eq!(settings.api, Some(4));

    // Edits stay; a second call writes nothing.
    fs::write(project.root.join("src/mod.c"), "// mine\n").unwrap();
    assert!(new_code_mod(&project).unwrap().is_empty());
    assert_eq!(
        read_source(&project.root, "src/mod.c").unwrap(),
        "// mine\n"
    );

    // A mod that names a native library is not changed.
    let other = Project::create(&dir.path().join("native"), "Native", None).unwrap();
    let mut s = modcode::read_settings(&other.root).unwrap();
    s.code = Some("mod.dll".into());
    s.api = Some(3);
    modcode::write_settings(&other.root, &s).unwrap();
    assert!(new_code_mod(&other).is_err());
    assert!(!other.root.join("src").exists());
}

#[test]
fn source_files() {
    let dir = tempfile::tempdir().unwrap();
    let project = new_project(dir.path());
    new_code_mod(&project).unwrap();
    let root = &project.root;
    let list: Vec<_> = list_sources(root).into_iter().map(|f| f.path).collect();
    assert_eq!(list, ["include/eidos_wasm.h", "src/alloc.c", "src/mod.c"]);

    new_source(root, "src/ai.c").unwrap();
    assert!(read_source(root, "src/ai.c")
        .unwrap()
        .contains("eidos_wasm.h"));
    assert!(new_source(root, "src/ai.c").is_err());
    write_source(root, "src/ai.c", "int x;\n").unwrap();
    rename_source(root, "src/ai.c", "src/battle.c").unwrap();
    assert_eq!(read_source(root, "src/battle.c").unwrap(), "int x;\n");
    assert!(rename_source(root, "src/battle.c", "src/mod.c").is_err());
    delete_source(root, "src/battle.c").unwrap();
    assert!(read_source(root, "src/battle.c").is_err());

    for bad in [
        "mod.c",
        "src/../eidos_mod.txt",
        "src/a.txt",
        "src//a.c",
        "../x.c",
        "src/a\\b.c",
        "C:/x.c",
        "include/a.c",
        "src/~a.c",
    ] {
        assert!(new_source(root, bad).is_err(), "{bad}");
    }
}

#[test]
fn zig_releases_are_pinned() {
    for (os, arch) in [
        ("windows", "x86_64"),
        ("windows", "aarch64"),
        ("linux", "x86_64"),
        ("linux", "aarch64"),
        ("macos", "x86_64"),
        ("macos", "aarch64"),
    ] {
        let r = zig_release(os, arch).unwrap();
        assert!(r.url.starts_with(&format!(
            "https://ziglang.org/download/{ZIG_VERSION}/zig-{arch}-{os}-{ZIG_VERSION}."
        )));
        assert_eq!(r.sha256.len(), 64);
        assert_eq!(r.zip, os == "windows");
    }
    assert!(zig_release("freebsd", "x86_64").is_none());
}

#[test]
fn zig_path_order() {
    let dir = tempfile::tempdir().unwrap();
    let app = dir.path().join("app");
    let exe = if cfg!(windows) { "zig.exe" } else { "zig" };
    // The user's choice wins and must exist.
    let own = dir.path().join(exe);
    assert!(zig_path(Some(&own), &app).is_none());
    fs::write(&own, b"x").unwrap();
    assert_eq!(zig_path(Some(&own), &app), Some((own.clone(), "own")));
    assert_eq!(zig_path(Some(dir.path()), &app), Some((own.clone(), "own")));
    // Then the downloaded one.
    let installed = zig_install_dir(&app);
    fs::create_dir_all(&installed).unwrap();
    fs::write(installed.join(exe), b"x").unwrap();
    assert_eq!(
        zig_path(None, &app),
        Some((installed.join(exe), "downloaded"))
    );
}

#[test]
fn clang_messages() {
    let root = Path::new("C:/mods/test");
    let text = "\
src/bad.c:1:21: error: use of undeclared identifier 'x'
    1 | int f(void){ return x; }
      |                     ^
src\\mod.c:7:3: warning: unused variable 'y' [-Wunused-variable]
C:\\mods\\test\\src\\mod.c:7:9: note: declared here
In file included from src/mod.c:2:
include/eidos_wasm.h:12:5: fatal error: 'stdint.h' file not found
wasm-ld: error: undefined symbol: helper
>>> referenced by src/mod.c:12
>>> referenced by src/alloc.c:30
error: the following command failed with 1 compiler errors
2 errors generated.
";
    let d = parse_diagnostics(text, root);
    let shown: Vec<_> = d
        .iter()
        .map(|d| (d.file.as_str(), d.line, d.column, d.severity))
        .collect();
    use BuildSeverity::*;
    assert_eq!(
        shown,
        [
            ("src/bad.c", 1, 21, Error),
            ("src/mod.c", 7, 3, Warning),
            ("src/mod.c", 7, 9, Note),
            ("include/eidos_wasm.h", 12, 5, Error),
            ("", 0, 0, Error),
            ("", 0, 0, Error),
        ]
    );
    assert_eq!(d[0].message, "use of undeclared identifier 'x'");
    assert_eq!(
        d[4].message,
        "undefined symbol: helper\nreferenced by src/mod.c:12\nreferenced by src/alloc.c:30"
    );
}

// --- handcrafted modules ---

fn leb(mut v: u32) -> Vec<u8> {
    let mut out = Vec::new();
    loop {
        let b = (v & 0x7f) as u8;
        v >>= 7;
        if v == 0 {
            out.push(b);
            return out;
        }
        out.push(b | 0x80);
    }
}

fn name(s: &str) -> Vec<u8> {
    let mut out = leb(s.len() as u32);
    out.extend(s.as_bytes());
    out
}

fn section(id: u8, body: Vec<u8>) -> Vec<u8> {
    let mut out = vec![id];
    out.extend(leb(body.len() as u32));
    out.extend(body);
    out
}

struct Module {
    import_module: &'static str,
    start_result: bool,
    export_memory: bool,
    export_table: bool,
    export_stop: bool,
    import_memory: bool,
}

impl Module {
    fn good() -> Self {
        Module {
            import_module: "eidos",
            start_result: true,
            export_memory: true,
            export_table: true,
            export_stop: true,
            import_memory: false,
        }
    }

    fn bytes(&self) -> Vec<u8> {
        let mut m = b"\0asm\x01\0\0\0".to_vec();
        // Types: 0 = (i32, i32) -> (), 1 = () -> i32, 2 = () -> ().
        m.extend(section(
            1,
            [
                vec![3],
                vec![0x60, 2, 0x7f, 0x7f, 0],
                vec![0x60, 0, 1, 0x7f],
                vec![0x60, 0, 0],
            ]
            .concat(),
        ));
        let mut imports = vec![if self.import_memory { 2 } else { 1 }];
        imports.extend(name(self.import_module));
        imports.extend(name("log"));
        imports.extend([0, 0]);
        if self.import_memory {
            imports.extend(name("env"));
            imports.extend(name("memory"));
            imports.extend([2, 0, 1]);
        }
        m.extend(section(2, imports));
        // Functions: start, stop (func indices 1 and 2).
        let start_type = if self.start_result { 1 } else { 2 };
        m.extend(section(3, vec![2, start_type, 2]));
        m.extend(section(4, vec![1, 0x70, 0, 1]));
        m.extend(section(5, vec![1, 0, 1]));
        let mut exports = Vec::new();
        let mut count = 0;
        let mut add = |n: &str, kind: u8, index: u8| {
            exports.extend(name(n));
            exports.extend([kind, index]);
            count += 1;
        };
        if self.export_memory {
            add("memory", 2, 0);
        }
        add("eidos_mod_start", 0, 1);
        if self.export_stop {
            add("eidos_mod_stop", 0, 2);
        }
        if self.export_table {
            add("__indirect_function_table", 1, 0);
        }
        m.extend(section(7, [vec![count], exports].concat()));
        m
    }
}

#[test]
fn module_check() {
    let c = contract();
    let info = check_module(&Module::good().bytes(), &c).unwrap();
    assert_eq!(info.imports.len(), 1);
    assert_eq!(info.exports.len(), 4);

    let fail = |m: Module| check_module(&m.bytes(), &c).unwrap_err();
    assert!(fail(Module {
        import_module: "env",
        ..Module::good()
    })
    .contains("\"eidos\""));
    assert!(fail(Module {
        start_result: false,
        ..Module::good()
    })
    .contains("eidos_mod_start"));
    assert!(fail(Module {
        export_memory: false,
        ..Module::good()
    })
    .contains("memory"));
    assert!(fail(Module {
        export_table: false,
        ..Module::good()
    })
    .contains("table"));
    assert!(fail(Module {
        export_stop: false,
        ..Module::good()
    })
    .contains("eidos_mod_stop"));
    assert!(fail(Module {
        import_memory: true,
        ..Module::good()
    })
    .contains("memory"));

    assert!(check_module(b"", &c).is_err());
    assert!(check_module(b"MZ\x90\0\x03\0\0\0", &c)
        .unwrap_err()
        .contains("WebAssembly"));
    let good = Module::good().bytes();
    assert!(check_module(&good[..good.len() - 3], &c).is_err());
}

// --- the header ---

#[test]
fn header_names() {
    let api = code_api(Path::new("no/such/mod"));
    let f = |n: &str| api.functions.iter().find(|f| f.name == n).unwrap();
    assert_eq!(
        f("eidos_log").signature,
        "void eidos_log(const char *text, uint32_t length)"
    );
    assert!(f("eidos_log").doc.contains("mod log"));
    assert!(api.functions.iter().any(|f| f.name == "eidos_on"));
    assert!(api.functions.iter().any(|f| f.name == "eidos_mod_start"));
    let c = |n: &str| api.constants.iter().find(|c| c.name == n).unwrap();
    assert_eq!(c("EIDOS_EVENT_BATTLE_START").value, "70");
    assert_eq!(c("EIDOS_EVENT_BATTLE_START").group, "battle");
    assert_eq!(c("EIDOS_CONTINUE").value, "0");

    // Every event the rule language names has an id in the header.
    let ids: Vec<_> = api.constants.iter().map(|c| c.name.as_str()).collect();
    let missing: Vec<_> = modcode::rule_names()
        .events
        .iter()
        .map(|e| format!("EIDOS_EVENT_{}", e.name.replace('.', "_").to_uppercase()))
        .filter(|n| !ids.contains(&n.as_str()))
        .collect();
    assert!(missing.is_empty(), "{missing:?}");
}

// --- builds ---

fn find_zig() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("KENON_ZIG").map(PathBuf::from) {
        return p.is_file().then_some(p);
    }
    zig_path(None, Path::new("no/such/app/data")).map(|(p, _)| p)
}

#[test]
fn real_build() {
    let Some(zig) = find_zig() else {
        eprintln!("skipped: no Zig (set KENON_ZIG or put zig on PATH)");
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let cache = dir.path().join("cache");
    let project = new_project(dir.path());
    new_code_mod(&project).unwrap();

    let result = build_code_mod(&project, &zig, Some(&cache));
    assert!(result.ok, "{:?}\n{}", result.diagnostics, result.log);
    assert_eq!(result.wasm_path.as_deref(), Some("mod.wasm"));
    let bytes = fs::read(project.root.join("mod.wasm")).unwrap();
    assert_eq!(result.size, bytes.len() as u64);
    let info = check_module(&bytes, &contract()).unwrap();
    assert!(info.imports.iter().all(|i| i.module == "eidos"));
    let imported: Vec<_> = info.imports.iter().map(|i| i.name.as_str()).collect();
    assert!(imported.contains(&"log") && imported.contains(&"on"));
    // Only the manifest, the module and the sources are in the folder.
    assert!(!project.root.join(".zig-cache").exists());

    // An error: located, and the last good module stays.
    fs::write(
        project.root.join("src/bad.c"),
        "#include \"eidos_wasm.h\"\nint f(void) { return nope; }\n",
    )
    .unwrap();
    let bad = build_code_mod(&project, &zig, Some(&cache));
    assert!(!bad.ok);
    let d = bad
        .diagnostics
        .iter()
        .find(|d| d.severity == BuildSeverity::Error)
        .unwrap();
    assert_eq!(
        (d.file.as_str(), d.line),
        ("src/bad.c", 2),
        "{:?}",
        bad.diagnostics
    );
    assert!(d.message.contains("nope"));
    assert_eq!(fs::read(project.root.join("mod.wasm")).unwrap(), bytes);
    fs::remove_file(project.root.join("src/bad.c")).unwrap();

    // A link error has no line.
    fs::write(
        project.root.join("src/link.c"),
        "#include \"eidos_wasm.h\"\nint g(void);\nint f(void) { return g(); }\n\
         EIDOS_EXPORT(\"f\") int h(void) { return f(); }\n",
    )
    .unwrap();
    let link = build_code_mod(&project, &zig, Some(&cache));
    assert!(!link.ok);
    assert!(
        link.diagnostics
            .iter()
            .any(|d| d.message.contains("undefined symbol")),
        "{:?}",
        link.diagnostics
    );
    fs::remove_file(project.root.join("src/link.c")).unwrap();

    // A module that imports from elsewhere is refused.
    fs::write(
        project.root.join("src/env.c"),
        "__attribute__((import_module(\"env\"), import_name(\"x\"))) void x(void);\n\
         __attribute__((export_name(\"y\"))) void y(void) { x(); }\n",
    )
    .unwrap();
    let env = build_code_mod(&project, &zig, Some(&cache));
    assert!(!env.ok);
    assert!(
        env.diagnostics
            .iter()
            .any(|d| d.message.contains("\"eidos\"")),
        "{:?}",
        env.diagnostics
    );
    fs::remove_file(project.root.join("src/env.c")).unwrap();

    // The package holds the module and the sources, not bookkeeping.
    let bw = dir.path().join("out.bw");
    package::pack(&project.root, &bw).unwrap();
    let files: Vec<_> = package::inspect(&bw)
        .unwrap()
        .files
        .into_iter()
        .map(|f| f.path)
        .collect();
    assert_eq!(
        files,
        [
            "eidos_mod.txt",
            "include/eidos_wasm.h",
            "mod.wasm",
            "src/alloc.c",
            "src/mod.c"
        ]
    );
}

#[test]
fn build_without_sources_or_with_native_code() {
    let dir = tempfile::tempdir().unwrap();
    let project = new_project(dir.path());
    let zig = Path::new("zig-that-does-not-exist");
    let r = build_code_mod(&project, zig, None);
    assert!(!r.ok);
    assert!(r.diagnostics[0].message.contains("no .c files"));

    let mut s = modcode::read_settings(&project.root).unwrap();
    s.code = Some("mod.dll".into());
    s.api = Some(3);
    modcode::write_settings(&project.root, &s).unwrap();
    let project = Project::open(&project.root).unwrap();
    let r = build_code_mod(&project, zig, None);
    assert!(r.diagnostics[0].message.contains("mod.dll"));

    new_source(&project.root, "src/a.c").unwrap();
    s.code = Some("mod.wasm".into());
    s.api = Some(4);
    modcode::write_settings(&project.root, &s).unwrap();
    let project = Project::open(&project.root).unwrap();
    let r = build_code_mod(&project, zig, None);
    assert!(r.diagnostics[0].message.contains("could not run Zig"));
}

/// The pinned download, end to end. Slow and needs the network, so it runs
/// only when `KENON_FETCH_ZIG` names a scratch folder.
#[test]
fn fetch_pinned_zig() {
    let Some(app) = std::env::var_os("KENON_FETCH_ZIG").map(PathBuf::from) else {
        return;
    };
    let mut phases = Vec::new();
    let exe = fetch_zig(&app, &mut |p| {
        if phases.last() != Some(&p.phase) {
            phases.push(p.phase);
        }
    })
    .unwrap();
    assert_eq!(zig_version(&exe).as_deref(), Some(ZIG_VERSION));
    assert_eq!(phases, ["download", "check", "unpack"]);
}

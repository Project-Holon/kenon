use std::fs;
use std::io::Write;
use std::path::Path;

use kenon_core::package::{inspect, pack, unpack};

fn write(root: &Path, rel: &str, bytes: &[u8]) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
}

fn project(root: &Path) {
    write(
        root,
        "eidos_mod.txt",
        b"# Written by kenon.\nname = Test mod\norder = 3\n",
    );
    write(root, "kenon.json", b"{\"rom\":\"black.nds\"}");
    write(root, "a/0/1/6/25", &[1, 2, 3, 4, 5, 0, 0, 0, 0, 0, 0, 0]);
    write(
        root,
        "a/0/1/6/26.bin",
        &(0..=255u8).cycle().take(5000).collect::<Vec<_>>(),
    );
    write(root, "a/0/0/2", b"whole file");
    write(root, "a/0/0/~3.kenon", b"half written");
}

#[test]
fn round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("mod");
    project(&src);
    let bw = dir.path().join("test.bw");
    pack(&src, &bw).unwrap();

    let info = inspect(&bw).unwrap();
    assert_eq!(info.manifest.name, "Test mod");
    assert_eq!(info.manifest.order, 3);
    let names: Vec<_> = info.files.iter().map(|e| e.path.as_str()).collect();
    assert_eq!(
        names,
        ["eidos_mod.txt", "a/0/0/2", "a/0/1/6/25", "a/0/1/6/26.bin"]
    );
    assert_eq!(info.files[3].size, 5000);

    let out = dir.path().join("out");
    unpack(&bw, &out).unwrap();
    for rel in names {
        assert_eq!(
            fs::read(src.join(rel)).unwrap(),
            fs::read(out.join(rel)).unwrap()
        );
    }
    assert!(!out.join("kenon.json").exists());
    assert!(!out.join("a/0/0/~3.kenon").exists());
}

#[test]
fn same_mod_same_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("mod");
    project(&src);
    let (a, b) = (dir.path().join("a.bw"), dir.path().join("b.bw"));
    pack(&src, &a).unwrap();
    // Rewrite a file so its timestamp differs.
    std::thread::sleep(std::time::Duration::from_millis(1100));
    write(&src, "a/0/0/2", b"whole file");
    pack(&src, &b).unwrap();
    assert_eq!(fs::read(a).unwrap(), fs::read(b).unwrap());
}

#[test]
fn deflate_shrinks_repetitive_members() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "mod/eidos_mod.txt", b"name = x\n");
    write(dir.path(), "mod/a/1", &vec![7u8; 20000]);
    let bw = dir.path().join("x.bw");
    pack(&dir.path().join("mod"), &bw).unwrap();
    assert!(fs::metadata(&bw).unwrap().len() < 2000);
}

#[test]
fn pack_inside_the_folder_skips_itself() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("mod");
    project(&src);
    let bw = src.join("self.bw");
    pack(&src, &bw).unwrap();
    pack(&src, &bw).unwrap();
    assert_eq!(inspect(&bw).unwrap().files.len(), 4);
}

#[test]
fn folder_without_manifest_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "mod/a/1", b"x");
    assert!(pack(&dir.path().join("mod"), &dir.path().join("x.bw")).is_err());
}

fn raw_zip(path: &Path, entries: &[(&str, &[u8])]) {
    let mut zip = zip::ZipWriter::new(fs::File::create(path).unwrap());
    let options = zip::write::SimpleFileOptions::default();
    for (name, bytes) in entries {
        zip.start_file(*name, options).unwrap();
        zip.write_all(bytes).unwrap();
    }
    zip.finish().unwrap();
}

#[test]
fn escaping_paths_are_refused() {
    let dir = tempfile::tempdir().unwrap();
    for bad in ["../evil", "a/../../evil", "/abs", "C:/evil", "a\\..\\evil"] {
        let bw = dir.path().join("bad.bw");
        raw_zip(&bw, &[("eidos_mod.txt", b"name = x\n"), (bad, b"x")]);
        let into = dir.path().join("into");
        assert!(unpack(&bw, &into).is_err(), "{bad}");
        assert!(!dir.path().join("evil").exists());
        assert!(
            !into.join("eidos_mod.txt").exists(),
            "{bad}: wrote before refusing"
        );
    }
}

#[test]
fn package_without_manifest_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let bw = dir.path().join("none.bw");
    raw_zip(&bw, &[("a/1", b"x")]);
    assert!(inspect(&bw).is_err());
    assert!(unpack(&bw, &dir.path().join("into")).is_err());
}

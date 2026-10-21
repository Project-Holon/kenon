//! Checks for the mod code editors: the manifest, the hook lines, the rule
//! language and the conflicts between mods. The declared-set check reads
//! eidos's own file when `KENON_EIDOS` names the eidos folder.

use std::fs;
use std::path::PathBuf;

use kenon_core::modcode::*;
use kenon_core::project::{Project, MANIFEST};

fn errors(text: &str, ev: Option<&str>) -> Vec<String> {
    check_rule(text, ev)
        .into_iter()
        .map(|d| d.message)
        .collect()
}

#[test]
fn tables_have_the_ports_counts() {
    let n = rule_names();
    assert_eq!(n.events.len(), 51);
    assert_eq!(n.states.len(), 20);
    let wild = n.event("encounter.wild").unwrap();
    assert!(wild.fields.iter().any(|f| f.name == "double"));
    assert!(n.event("battle.faint").unwrap().fields.len() == 6);
}

#[test]
fn language_examples_pass() {
    for (text, ev) in [
        ("language 1\nrule faint_penalty on battle.faint\n    when event.side == 0\n    do sub money 1000\n", "battle.faint"),
        ("language 1\nrule one on catch.attempt\n    when state.route_used[zone] == 1\n    do refuse\n    do message \"You already caught one.\"\n", "catch.attempt"),
        ("language 1\nrule mark on catch.result\n    when event.caught == 1\n    do set state.route_used[zone] 1\n", "catch.result"),
        ("language 1\nrule cap on mon.exp\n    when level[event.mon] >= state.cap\n    do set event.amount 0\n", "mon.exp"),
        ("language 1\nrule no_evo on mon.evolve\n    do refuse\n", "mon.evolve"),
    ] {
        assert!(errors(text, Some(ev)).is_empty(), "{text}: {:?}", errors(text, Some(ev)));
    }
}

#[test]
fn bad_rules_are_caught() {
    let rule = |body: &str| format!("language 1\nrule r on battle.faint\n{body}");
    let one = |body: &str| errors(&rule(body), None);
    assert!(
        errors("language 1\nrule r on battle.fainted\n do refuse\n", None)[0]
            .contains("not one of the port's events")
    );
    assert!(one(" do refuse\n")[0].contains("cannot be refused"));
    assert!(one(" do set event.side 1\n")[0].contains("cannot be changed"));
    assert!(one(" do sub bogus 1\n")[0].contains("not a name"));
    assert!(one(" when event.nope == 1\n do message \"x\"\n")[0].contains("no field"));
    assert!(one(" when event.side = 1\n do message \"x\"\n")[0].contains("'=='"));
    assert!(one(" when event.side == \"a\"\n do message \"x\"\n")[0].contains("different types"));
    assert!(one(" when 1\n")[0].contains("no action"));
    assert!(one(" do message \"open\n")[0].contains("not closed"));
    assert!(one(" do sub money 1 2\n")[0].contains("more than it needs"));
    assert!(one(" do sub item_count[1] 1\n")[0].contains("cannot be changed"));
    assert!(one(" when item_count[1] > 0\n do message 1\n")[0].contains("cannot read"));
    assert!(one(" do jump\n")[0].contains("not an action"));
    assert!(one(" else\n")[0].contains("not a line of a rule"));
    assert!(errors("rule r on battle.faint\n", None)[0].contains("start with 'language 1'"));
    assert!(
        errors("language 2\nrule r on battle.faint\n do message 1\n", None)[0]
            .contains("version 2")
    );
    assert!(errors(
        "language 1\nrule r on mon.exp\n do message 1\n",
        Some("battle.faint")
    )[0]
    .contains("manifest names"));
    assert!(errors("", None)[0].contains("empty"));
}

#[test]
fn positions_point_at_the_token() {
    let d = check_rule(
        "language 1\nrule r on battle.faint\n    when event.nope == 1\n    do message 1\n",
        None,
    );
    assert_eq!(d.len(), 1);
    assert_eq!((d[0].line, d[0].start, d[0].end), (3, 16, 20));
}

#[test]
fn manifest_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("fairy");
    Project::create(&root, "Fairy type", None).unwrap();
    fs::write(
        root.join(MANIFEST),
        "# keep me\nname = Fairy type\norder = 10\napi = 3\ncode = fairy.dll\nport = 0.4\nstate = fairy\nsettings = opts.txt\nfuture = 7\noverride = ov93:0x021CBD48 wrap hook\nrule = battle.faint rules/a.rule\n",
    )
    .unwrap();
    let s = read_settings(&root).unwrap();
    assert_eq!(s.name, "Fairy type");
    assert_eq!(s.order, 10);
    assert_eq!(s.api, Some(3));
    assert_eq!(s.code.as_deref(), Some("fairy.dll"));
    assert_eq!(s.port.as_deref(), Some("0.4"));
    assert_eq!(s.state.as_deref(), Some("fairy"));
    assert_eq!(s.settings.as_deref(), Some("opts.txt"));
    write_settings(&root, &s).unwrap();
    assert_eq!(read_settings(&root).unwrap(), s);
    let text = fs::read_to_string(root.join(MANIFEST)).unwrap();
    for kept in [
        "# keep me",
        "future = 7",
        "override = ov93:0x021CBD48 wrap hook",
        "rule = battle.faint rules/a.rule",
    ] {
        assert!(text.contains(kept), "{kept} lost:\n{text}");
    }
    // A change reads back, and api goes with the code.
    let mut t = s.clone();
    t.name = "Fairy".into();
    t.order = -5;
    t.code = None;
    t.port = None;
    write_settings(&root, &t).unwrap();
    let back = read_settings(&root).unwrap();
    assert_eq!(
        (back.name.as_str(), back.order, back.api, back.code),
        ("Fairy", -5, None, None)
    );
    assert_eq!(back.port, None);
    assert_eq!(back.state.as_deref(), Some("fairy"));
    assert!(write_settings(&root, &ModSettings::default()).is_err());
}

#[test]
fn settings_checks() {
    let mut s = ModSettings {
        name: "m".into(),
        code: Some("m.dll".into()),
        ..Default::default()
    };
    let fields = |d: Vec<Diagnostic>| d.into_iter().filter_map(|d| d.field).collect::<Vec<_>>();
    assert_eq!(fields(check_settings(&s, 0)), ["api"]);
    s.api = Some(2);
    assert_eq!(fields(check_settings(&s, 0)), ["api"]);
    s.api = Some(API_VERSION);
    assert!(check_settings(&s, 0).is_empty());
    s.state = Some("../x".into());
    s.code = Some("../m.dll".into());
    assert_eq!(fields(check_settings(&s, 1)), ["port", "state", "code"]);
    s.state = Some("CON.txt".into());
    assert!(fields(check_settings(&s, 0)).contains(&"state".to_string()));
}

const DECLARED: &str =
    "# c\n0x02007150\tSAVE_CONTROL_StartSave\tarm9\tsave.begin\n0x021CBD48\tBTL_X\tov093\t\n";

#[test]
fn hook_lines_are_checked() {
    let set = DeclaredSet::parse(DECLARED);
    assert_eq!(set.len(), 2);
    assert_eq!(set.find("ov93", 0x021CBD49).unwrap().function, "BTL_X");
    let good = "arm9:0x02007150 wrap my_hook sig=\"int (int)\" switch=mine\nov093:0x021CBD48 replace other # note\n";
    let r = check_overrides(good, Some(&set));
    assert!(r.diagnostics.is_empty(), "{:?}", r.diagnostics);
    assert_eq!(r.rows[0].events, ["save.begin"]);
    let bad = "0x02007150 wrap h\narm9:0x02007150 swap h\nov93:0x021CBD48 wrap 9h\narm9:0x02007154 wrap h\narm9:0x02007150 wrap h extra\nnonsense\n";
    let msgs: Vec<_> = check_overrides(bad, Some(&set))
        .diagnostics
        .into_iter()
        .map(|d| (d.line, d.message))
        .collect();
    assert!(msgs[0].1.contains("bare address") && msgs[0].0 == 1);
    assert!(msgs[1].1.contains("replace") && msgs[1].0 == 2);
    assert!(msgs[2].1.contains("C name"));
    assert!(msgs[3].1.contains("not a declared address"));
    assert!(msgs[4].1.contains("only sig="));
    assert!(msgs[5].1.contains("needs"));
    // Without the declared set only the grammar is checked.
    assert!(check_overrides("arm9:0x02007154 wrap h\n", None)
        .diagnostics
        .is_empty());
}

#[test]
fn rule_files_bind_rename_and_delete() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("m");
    Project::create(&root, "m", None).unwrap();
    let path = new_rule(&root, "faint", "battle.faint").unwrap();
    assert_eq!(path, "rules/faint.rule");
    assert!(new_rule(&root, "faint", "battle.faint").is_err());
    assert!(new_rule(&root, "x y", "battle.faint").is_err());
    assert!(new_rule(&root, "z", "no.such").is_err());
    let list = list_rules(&root).unwrap();
    assert_eq!(list[0].bound.as_deref(), Some("battle.faint"));
    // A template has no action yet.
    let text = read_rule(&root, &path).unwrap();
    assert!(check_rule(&text, None)[0].message.contains("no action"));
    // Saving a rule for another event rebinds it.
    let diags = write_rule(
        &root,
        &path,
        "language 1\nrule faint on mon.evolve\n    do refuse\n",
    )
    .unwrap();
    assert!(diags.is_empty());
    assert_eq!(
        list_rules(&root).unwrap()[0].bound.as_deref(),
        Some("mon.evolve")
    );
    let moved = rename_rule(&root, &path, "rules/sub/evo").unwrap();
    assert_eq!(moved, "rules/sub/evo.rule");
    let list = list_rules(&root).unwrap();
    assert_eq!(
        (list.len(), list[0].path.as_str(), list[0].bound.as_deref()),
        (1, "rules/sub/evo.rule", Some("mon.evolve"))
    );
    assert!(read_rule(&root, "../x").is_err());
    delete_rule(&root, &moved).unwrap();
    assert!(list_rules(&root).unwrap().is_empty());
    assert!(!fs::read_to_string(root.join(MANIFEST))
        .unwrap()
        .contains("rule ="));
    write_overrides(&root, "# c\narm9:0x02007150 wrap h\n\n").unwrap();
    assert_eq!(read_overrides(&root).unwrap(), "arm9:0x02007150 wrap h\n");
}

#[test]
fn two_mods_conflict() {
    let dir = tempfile::tempdir().unwrap();
    let mods = dir.path().join("mods");
    let make = |name: &str, order: i32, files: &[&str], extra: &str| {
        let root = mods.join(name);
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join(MANIFEST),
            format!("name = {name}\norder = {order}\n{extra}"),
        )
        .unwrap();
        for f in files {
            let p = root.join(f);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, b"x").unwrap();
        }
        root
    };
    let mine = make(
        "mine",
        5,
        &["a/0/1/6/25", "a/0/0/2", "msg.bin", "rules/r.rule"],
        "override = ov93:0x021CBD48 wrap a\nrule = mon.exp rules/r.rule\n",
    );
    make(
        "other",
        9,
        &["a/0/1/6/25.bin", "rules/r.rule", "z"],
        "override = ov093:0x021cbd49 replace b\n",
    );
    make("early", 1, &["a/0/0/2/7", "msg.bin"], "");
    make("clean", 3, &["unrelated"], "");
    let r = find_conflicts(&mine, &mods).unwrap();
    let order: Vec<_> = r.mods.iter().map(|m| m.dir.as_str()).collect();
    assert_eq!(order, ["early", "clean", "mine", "other"]);
    let find = |kind: &str, key: &str| r.conflicts.iter().find(|c| c.kind == kind && c.key == key);
    let member = find("file", "a/0/1/6/25").unwrap();
    assert_eq!(member.winner, "other");
    assert_eq!(member.mods.len(), 2);
    // A whole archive file over another mod's members.
    assert!(find("file", "a/0/0/2").is_some());
    assert_eq!(find("file", "msg.bin").unwrap().winner, "mine");
    // Rule files and the manifests are not game files.
    assert!(find("file", "rules/r.rule").is_none());
    let hook = find("hook", "ov93:0x021CBD48").unwrap();
    assert_eq!(hook.detail, ["wrap", "replace"]);
    assert!(r.conflicts.iter().all(|c| c.mods.iter().any(|m| m.this)));
    assert_eq!(r.conflicts.len(), 4);
}

/// Reads eidos's real declared set when `KENON_EIDOS` names its folder.
#[test]
fn real_declared_set() {
    let Some(dir) = std::env::var_os("KENON_EIDOS") else {
        eprintln!("KENON_EIDOS not set; skipped");
        return;
    };
    let set = DeclaredSet::load(&PathBuf::from(dir).join("mod/declared_set.txt")).unwrap();
    assert!(set.len() > 250, "{}", set.len());
    let row = set.find("arm9", 0x02007150).unwrap();
    assert_eq!(row.function, "SAVE_CONTROL_StartSave");
    assert!(set.find("ov93", 0x021CBD48).is_some());
}

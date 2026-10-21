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
    assert_eq!(n.events.len(), 52);
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
fn actions_state_and_heal_hp() {
    let ok = |text: &str, ev: &str| {
        let e = errors(text, Some(ev));
        assert!(e.is_empty(), "{text}: {e:?}");
    };
    // The Nuzlocke rule.
    ok("language 1\nrule nuzlocke on battle.faint\n  when event.side == 0\n  do release event.mon\n", "battle.faint");
    // A party position releases too.
    ok(
        "language 1\nrule r on box.deposit\n  do release 0\n",
        "box.deposit",
    );
    // A gift on entering a map: the item and the count are numbers.
    ok("language 1\nrule gift on route.change\n  when event.zone == 10 && item_count[17] < 5\n  do give 17 5\n", "route.change");
    // heal.hp can be refused and its amount written.
    ok(
        "language 1\nrule no_heal on heal.hp\n  when event.source == 1\n  do refuse\n",
        "heal.hp",
    );
    ok(
        "language 1\nrule half on heal.hp\n  do set event.amount event.amount / 2\n",
        "heal.hp",
    );
    ok(
        "language 1\nrule opt on heal.hp\n  when setting.hard == 1\n  do set event.amount 0\n",
        "heal.hp",
    );
    ok(
        "language 1\nrule fill on heal.hp\n  do set event.amount event.max_hp - event.hp\n",
        "heal.hp",
    );
    ok(
        "language 1\nrule r on heal.hp\n  when in_battle == 1\n  do message \"x\"\n",
        "heal.hp",
    );

    let bad = |body: &str| errors(&format!("language 1\nrule r on battle.faint\n{body}"), None);
    assert!(bad(" do release\n")[0].contains("value was expected"));
    assert!(bad(" do release \"a\"\n")[0].contains("Pokemon or a party position"));
    assert!(bad(" do release event.mon 1\n")[0].contains("more than it needs"));
    assert!(bad(" do give 1\n")[0].contains("value was expected"));
    assert!(bad(" do give 1 2 3\n")[0].contains("more than it needs"));
    assert!(bad(" do give event.mon 1\n")[0].contains("must be numbers"));
    assert!(bad(" when item_count > 0\n do message 1\n")[0].contains("needs a subscript"));
    assert!(bad(" when setting == 1\n do message 1\n")[0].contains("'.' was expected"));
    assert!(bad(" when setting.\n do message 1\n")[0].contains("key was expected"));
    assert!(bad(" do set event.amount 1\n")[0].contains("no field"));
    // Every action the port names is one the checker knows.
    let table = include_str!("../src/modnames/rules.txt");
    for line in table.lines().filter(|l| l.starts_with("action ")) {
        let kw = line.split_whitespace().nth(1).unwrap();
        let body = match kw {
            "refuse" => " do refuse".to_string(),
            "set" | "add" | "sub" => format!(" do {kw} money 1"),
            "message" | "send" => format!(" do {kw} 1"),
            "release" => " do release 0".to_string(),
            "give" => " do give 1 1".to_string(),
            other => panic!("the port has an action kenon does not check: {other}"),
        };
        let text = format!("language 1\nrule r on dex.register\n{body}\n");
        let e = errors(&text, None);
        assert!(
            e.iter().all(|m| !m.contains("not an action")),
            "{kw}: {e:?}"
        );
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

/// Eidos's own test rules and the rules of its language document pass the
/// checker. `KENON_EIDOS` names an eidos folder (the commit the names were
/// generated from).
#[test]
fn eidos_own_rules() {
    let Some(dir) = std::env::var_os("KENON_EIDOS") else {
        eprintln!("KENON_EIDOS not set; skipped");
        return;
    };
    let dir = PathBuf::from(dir);
    let mut seen = 0;
    for file in ["docs/mod-rules-language.md", "docs/mod-rules.md"] {
        let Ok(doc) = fs::read_to_string(dir.join(file)) else {
            continue;
        };
        let doc = doc.replace("\r\n", "\n");
        for block in doc.split("```").skip(1).step_by(2) {
            let block = block.trim_start_matches(|c: char| c.is_ascii_alphabetic());
            let block = block.trim_start_matches('\n');
            if block.starts_with("language 1") && block.contains("\nrule ") && !block.contains('<')
            {
                let e = errors(block, None);
                assert!(e.is_empty(), "{file}: {block}: {e:?}");
                seen += 1;
            }
        }
    }
    assert!(seen >= 5, "{seen} rules read");
    // The rules of tests/test_mod_rule.c, as that file writes them.
    let test = fs::read_to_string(dir.join("tests/test_mod_rule.c")).unwrap();
    assert!(test.contains("rule bump_wild on encounter.wild"));
    for text in [
        "language 1\nrule bump_wild on encounter.wild\n    when event.level < 5\n    do set event.level 5\n    do set event.shiny 1\n",
        "language 1\nrule no_wild on encounter.wild\n    do refuse\n",
    ] {
        assert!(errors(text, Some("encounter.wild")).is_empty());
    }
    for text in [
        "language 1\nrule broken on encounter.wild\n    when event.level >\n    do set event.level 0\n",
        "language 1\nrule bad_event on no.such.event\n    do refuse\n",
        "language 2\nrule future on encounter.wild\n    do refuse\n",
    ] {
        assert!(!errors(text, None).is_empty(), "{text}");
    }
}

/// Every rule in the mod docs (recipes and the code blocks of the rules
/// pages) passes the checker, and the actions and events the docs list are
/// the port's.
#[test]
fn doc_rules_pass_the_checker() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../ui/src/lib/moddocs/api.json");
    let api: serde_json::Value = serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
    let mut rules: Vec<(String, String)> = Vec::new();
    for r in api["recipes"].as_array().unwrap() {
        for key in ["rule", "rule2"] {
            if let Some(text) = r[key].as_str() {
                rules.push((r["slug"].as_str().unwrap().to_string(), text.to_string()));
            }
        }
    }
    for page in api["pages"].as_array().unwrap() {
        for block in page["blocks"].as_array().unwrap() {
            if block["t"] == "code" && block["lang"] == "rule" {
                rules.push((
                    page["slug"].as_str().unwrap().to_string(),
                    block["text"].as_str().unwrap().to_string(),
                ));
            }
        }
    }
    assert!(rules.len() >= 15, "{} rules found", rules.len());
    let names = rule_names();
    for (what, text) in &rules {
        let e = errors(text, None);
        assert!(e.is_empty(), "{what}: {text}: {e:?}");
        let ev = rule_event(text).expect("a rule names its event");
        assert!(names.event(&ev).is_some(), "{what}: {ev}");
    }
    // The Nuzlocke and heal.hp recipes are among them.
    assert!(rules
        .iter()
        .any(|(w, t)| w == "release-on-faint" && t.contains("do release event.mon")));
    assert!(rules.iter().any(|(_, t)| t.contains("on heal.hp")));
    // Every event the docs list is a rule event of the port, and the docs
    // list the actions the port has.
    for ev in api["events"].as_array().unwrap() {
        assert_eq!(ev["ruleEvent"], true, "{}", ev["name"]);
    }
    let docs: Vec<&str> = api["actions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["name"].as_str().unwrap())
        .collect();
    for line in include_str!("../src/modnames/rules.txt")
        .lines()
        .filter(|l| l.starts_with("action "))
    {
        let kw = line.split_whitespace().nth(1).unwrap();
        assert!(docs.contains(&kw), "the docs lack the action {kw}");
    }
}

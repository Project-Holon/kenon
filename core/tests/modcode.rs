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
    assert_eq!(n.events.len(), 58);
    assert_eq!(n.states.len(), 62);
    let wild = n.event("encounter.wild").unwrap();
    assert!(wild.fields.iter().any(|f| f.name == "double"));
    assert!(n.event("battle.faint").unwrap().fields.len() == 6);
}

#[test]
fn cure_pp_and_form() {
    let ok = |text: &str, ev: &str| {
        let e = errors(text, Some(ev));
        assert!(e.is_empty(), "{text}: {e:?}");
    };
    ok(
        "language 1
rule r on mon.cure
    when event.source == 1
    do refuse
",
        "mon.cure",
    );
    ok(
        "language 1
rule r on mon.pp
    do set event.amount event.amount / 2
",
        "mon.pp",
    );
    ok(
        "language 1
rule r on mon.form
    when event.source == 1
    do set event.to 0
",
        "mon.form",
    );
    // Only amount is writable on mon.pp.
    let bad = errors(
        "language 1
rule r on mon.pp
    do set event.slot 0
",
        Some("mon.pp"),
    );
    assert!(!bad.is_empty());
    // battle_style is declared writable, but the port cannot write it yet.
    let style = errors(
        "language 1
rule r on battle.start
    do set battle_style 1
",
        Some("battle.start"),
    );
    assert!(!style.is_empty());
    let line = include_str!("../src/modnames/rules.txt")
        .lines()
        .find(|l| l.starts_with("state battle_style"))
        .unwrap();
    assert_eq!(line, "state battle_style plain 1 1 0");
}

#[test]
fn revive_and_species_base() {
    let ok = |text: &str, ev: &str| {
        let e = errors(text, Some(ev));
        assert!(e.is_empty(), "{text}: {e:?}");
    };
    ok(
        "language 1
rule r on mon.revive
    when event.source == 1
    do refuse
",
        "mon.revive",
    );
    ok(
        "language 1
rule r on mon.revive
    do set event.hp event.max_hp / 2
",
        "mon.revive",
    );
    ok(
        "language 1
rule r on encounter.wild
    when species_base[species[0]] == species_base[event.species]
    do set event.reroll 1
",
        "encounter.wild",
    );
    assert!(rule_names().event("mon.revive").unwrap().refusable);
    // Only the HP is writable.
    let e = errors(
        "language 1
rule r on mon.revive
    do set event.source 0
",
        Some("mon.revive"),
    );
    assert!(!e.is_empty());
    // A whole party heal and the battle EXP hand-out can be refused.
    ok(
        "language 1
rule r on heal
    do refuse
",
        "heal",
    );
    ok(
        "language 1
rule r on battle.exp
    when event.wild == 1
    do refuse
",
        "battle.exp",
    );
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
            "box" => " do box 0 3".to_string(),
            "withdraw" => " do withdraw 0 1".to_string(),
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
    // A function name stands for its address; the message says which.
    let mut named = DeclaredSet::parse(DECLARED);
    named.add_aliases(
        r#"{"hooks":[{"address":"0x02007150","region":"arm9","aliases":["SAVE_StartSave"]}]}"#,
    );
    assert_eq!(
        named.find("arm9", 0x02007150).unwrap().aliases,
        ["SAVE_StartSave"]
    );
    assert_eq!(named.find_name("arm9", "SAVE_StartSave"), Some(0x02007150));
    assert_eq!(
        named.find_name("arm9", "SAVE_CONTROL_StartSave"),
        Some(0x02007150)
    );
    assert_eq!(named.find_name("ov93", "SAVE_StartSave"), None);
    let r = check_overrides(
        "arm9:SAVE_StartSave wrap h
",
        Some(&named),
    );
    assert!(r.diagnostics[0].message.contains("arm9:0x02007150"));
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
    // The aliases come from sdk/api/v4/hooks.json beside it.
    assert_eq!(
        set.find("arm9", 0x0200C248).unwrap().aliases,
        ["MISC_SetBadge"]
    );
    assert_eq!(set.find_name("arm9", "MISC_SetBadge"), Some(0x0200C248));
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
    // The interface 4 rules of that file: an operator next to a minus, and a
    // table read (with the table declared, and then not).
    if test.contains("rule negops on encounter.wild") {
        let have = tables(&[("caps", true)]);
        let negops = "language 1\nrule negops on encounter.wild\n    when event.level*-1 <-1\n    do set event.level 7\n";
        assert!(errors_in(negops, "encounter.wild", Some(&have)).is_empty());
        let read = "language 1\nrule cap on encounter.wild\n    do set event.level table.caps[event.level] + table.caps[50]\n";
        assert!(errors_in(read, "encounter.wild", Some(&have)).is_empty());
        let none = "language 1\nrule cap on encounter.wild\n    do set event.level table.nope[1]\n";
        assert!(!errors_in(none, "encounter.wild", Some(&have)).is_empty());
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

// ---------------------------------------------------------------------
// Interface 4: more reads, data tables
// ---------------------------------------------------------------------

fn tables(names: &[(&str, bool)]) -> TableSet {
    names.iter().map(|(n, ok)| (n.to_string(), *ok)).collect()
}

fn errors_in(text: &str, ev: &str, t: Option<&TableSet>) -> Vec<String> {
    check_rule_in(text, Some(ev), t)
        .into_iter()
        .map(|d| d.message)
        .collect()
}

#[test]
fn operators_read_as_pairs_or_singles() {
    let one = |body: &str| {
        errors(
            &format!("language 1\nrule r on mon.exp\n  when {body}\n  do message 1\n"),
            Some("mon.exp"),
        )
    };
    assert!(one("level[event.mon] * -1 < 0").is_empty());
    assert!(one("event.level*-1 < 0").is_empty());
    assert!(one("event.level<-1").is_empty());
    assert!(one("event.level >= 0 && event.level<=-1").is_empty());
    assert!(one("event.level =- 1")[0].contains("'=='"));
    assert!(one("event.level & 1")[0].contains("'&&'"));
}

#[test]
fn new_reads_check() {
    let ok = |ev: &str, body: &str| {
        let text = format!("language 1\nrule r on {ev}\n{body}");
        let e = errors(&text, Some(ev));
        assert!(e.is_empty(), "{text}: {e:?}");
    };
    // Every new state name reads with the subscript its kind needs.
    for (name, sub) in [
        ("personality", "event.mon"),
        ("ot_id", "0"),
        ("exp", "event.mon"),
        ("friendship", "event.mon"),
        ("ability", "event.mon"),
        ("move1", "event.mon"),
        ("move2", "event.mon"),
        ("move3", "event.mon"),
        ("move4", "event.mon"),
        ("is_egg", "event.mon"),
        ("gender", "event.mon"),
        ("form", "event.mon"),
        ("nature", "event.mon"),
        ("has_nickname", "event.mon"),
        ("place", "event.mon"),
        ("side", "event.mon"),
        ("species_stage", "event.level"),
        ("catch_rate", "25"),
        ("gender_ratio", "25"),
        ("growth", "25"),
        ("egg_group1", "25"),
        ("egg_group2", "25"),
        ("area_of", "event.level"),
        ("owned", "25"),
        ("party_has_type", "10"),
        ("item_pocket", "17"),
        ("move_type", "33"),
        ("move_power", "33"),
        ("move_class", "33"),
    ] {
        ok(
            "mon.exp",
            &format!("  when {name}[{sub}] >= 0\n  do message 1\n"),
        );
    }
    for name in [
        "area",
        "wild_battle",
        "play_seconds",
        "party_max_level",
        "party_alive",
    ] {
        ok("mon.exp", &format!("  when {name} >= 0\n  do message 1\n"));
    }
    let bad = |body: &str| {
        errors(
            &format!("language 1\nrule r on mon.exp\n  when {body}\n  do message 1\n"),
            Some("mon.exp"),
        )
    };
    assert!(bad("owned > 0")[0].contains("needs a subscript"));
    assert!(bad("play_seconds[1] > 0")[0].contains("takes no subscript"));
    assert!(bad("party_alive[0] > 0")[0].contains("takes no subscript"));
    // None of them can be written.
    let write = errors(
        "language 1\nrule r on mon.exp\n  do set play_seconds 0\n",
        Some("mon.exp"),
    );
    assert!(write[0].contains("cannot be changed"), "{write:?}");
}

#[test]
fn tables_in_rules() {
    let one =
        |body: &str| format!("language 1\nrule r on mon.exp\n  when {body}\n  do message 1\n");
    let have = tables(&[("caps", true), ("bad", false)]);
    // The level cap by badge rule.
    let cap = "language 1\nrule cap on mon.exp\n  when level[event.mon] >= table.caps[badges]\n  do set event.amount 0\n";
    assert!(errors_in(cap, "mon.exp", Some(&have)).is_empty());
    // Any number expression is a key.
    assert!(errors_in(
        &one("table.caps[badges * 2 + 1] > 0"),
        "mon.exp",
        Some(&have)
    )
    .is_empty());
    assert!(errors_in(
        &one("table.caps[event.level] == table.caps[0]"),
        "mon.exp",
        Some(&have)
    )
    .is_empty());
    // A table the mod lacks is refused, as the port does at bind.
    let e = errors_in(cap, "mon.exp", Some(&tables(&[("other", true)])));
    assert!(
        e[0].contains("no table 'caps'") && e[0].contains("manifest"),
        "{e:?}"
    );
    let e = errors_in(cap, "mon.exp", Some(&tables(&[])));
    assert!(e[0].contains("no table 'caps'"), "{e:?}");
    // A table whose file does not load is not there either.
    let e = errors_in(&one("table.bad[0] > 0"), "mon.exp", Some(&have));
    assert!(e[0].contains("does not load"), "{e:?}");
    // Without a mod the names are not checked.
    assert!(errors_in(cap, "mon.exp", None).is_empty());
    // Shape errors.
    assert!(errors_in(&one("table.caps > 0"), "mon.exp", Some(&have))[0].contains("with a key"));
    assert!(
        errors_in(&one("table.caps[\"a\"] > 0"), "mon.exp", Some(&have))[0]
            .contains("key must be a number")
    );
    assert!(errors_in(&one("table > 0"), "mon.exp", Some(&have))[0].contains("'.' was expected"));
    assert!(
        errors_in(&one("table.[0] > 0"), "mon.exp", Some(&have))[0].contains("name was expected")
    );
}

#[test]
fn capability_rules_pass() {
    let have = tables(&[("caps", true)]);
    for (ev, text) in [
        // Level cap by badge.
        (
            "mon.exp",
            "language 1\nrule cap on mon.exp\n  when level[event.mon] >= table.caps[badges]\n  do set event.amount 0\n",
        ),
        // Rare Candy refused at the cap.
        (
            "item.use",
            "language 1\nrule candy_cap on item.use\n  when event.item == 50 && level[event.target] >= table.caps[badges]\n  do refuse\n  do message \"Too high a level for now.\"\n",
        ),
        // Dupes clause: no second catch of a species the player owns.
        (
            "catch.attempt",
            "language 1\nrule dupes on catch.attempt\n  when owned[event.species] == 1\n  do refuse\n  do message \"Already owned.\"\n",
        ),
        // Dupes by evolution line: only a first stage counts as new.
        (
            "catch.attempt",
            "language 1\nrule stage on catch.attempt\n  when species_stage[event.species] > 0 && owned[event.species] == 1\n  do refuse\n",
        ),
        // Monotype: the party must hold the type.
        (
            "encounter.wild",
            "language 1\nrule mono on encounter.wild\n  when party_has_type[10] == 0\n  do refuse\n",
        ),
        // Play time and the area.
        (
            "route.change",
            "language 1\nrule late on route.change\n  when play_seconds > 3600 && area == area_of[event.zone]\n  do message \"An hour in.\"\n",
        ),
        // Move data.
        (
            "battle.move",
            "language 1\nrule weak on battle.move\n  when move_power[event.move] > 0 && move_class[event.move] == 2\n  do message 1\n",
        ),
    ] {
        let e = errors_in(text, ev, Some(&have));
        assert!(e.is_empty(), "{text}: {e:?}");
    }
}

#[test]
fn table_files() {
    let msgs = |text: &str| check_table(text);
    let ok = "# caps\n0 = 12\n1 = 20   # first gym\n0x2 = 25\n-1 = 0\n+3 = 0x1E\ndefault = 100\n";
    assert!(msgs(ok).is_empty(), "{:?}", msgs(ok));
    // Both ends of the signed range.
    assert!(msgs("2147483647 = -2147483648\n-2147483648 = 2147483647\n").is_empty());
    let d = msgs("1 = 2147483648\n");
    assert_eq!((d[0].line, d[0].severity), (1, Severity::Error));
    assert!(d[0].message.contains("32 bits"));
    assert!(msgs("-2147483649 = 1\n")[0].message.contains("32 bits"));
    // Syntax.
    assert!(msgs("12\n")[0].message.contains("'=' is missing"));
    assert!(msgs("a = 1\n")[0].message.contains("key is not a number"));
    assert!(msgs("1 = x\n")[0].message.contains("value is not a number"));
    assert!(msgs("1 = 2 3\n")[0]
        .message
        .contains("value is not a number"));
    assert!(msgs("1 = \n")[0].message.contains("value is not a number"));
    assert!(msgs("0x = 1\n")[0].message.contains("key is not a number"));
    assert!(msgs("= 1\n")[0].message.contains("key is not a number"));
    // The position points at the offending text.
    let d = msgs("# c\n  7 = 0xZZ  # no\n");
    assert_eq!((d[0].line, d[0].start, d[0].end), (2, 7, 11));
    // A repeated key or default is a warning, and the file still loads.
    let d = msgs("1 = 1\n0x1 = 2\ndefault = 0\ndefault = 5\n");
    assert_eq!(d.len(), 2);
    assert!(d.iter().all(|x| x.severity == Severity::Warning));
    assert!(d[0].message.contains("line 1") && d[1].message.contains("default"));
    assert!(!has_errors(&d));
    // The entry limit: 65536 entries are fine, one more is not.
    let big = |n: usize| (0..n).map(|i| format!("{i} = 1\n")).collect::<String>();
    assert!(msgs(&big(TABLE_ENTRIES_MAX)).is_empty());
    let d = msgs(&big(TABLE_ENTRIES_MAX + 1));
    assert_eq!(d.len(), 1);
    assert!(d[0].message.contains("65536") && d[0].line as usize == TABLE_ENTRIES_MAX + 1);
}

#[test]
fn table_names_and_lines() {
    for ok in ["caps", "_x", "A1_b", &"a".repeat(31)] {
        assert!(table_name_valid(ok), "{ok}");
    }
    for bad in ["", "1a", "a-b", "a b", "a.b", &"a".repeat(32)] {
        assert!(!table_name_valid(bad), "{bad}");
    }
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("m");
    Project::create(&root, "M", None).unwrap();
    let check = |s: &ModSettings| check_settings(s, 0);
    assert!(read_settings(&root).unwrap().tables.is_empty());
    let with = |lines: &str| {
        fs::write(root.join(MANIFEST), format!("name = M\n{lines}")).unwrap();
        read_settings(&root).unwrap()
    };
    let s = with("table = caps tables/caps.txt\ntable = ban tables/ban.txt # note\n");
    assert_eq!(s.tables.len(), 2);
    assert_eq!(s.tables[1].name, "ban");
    assert!(check(&s).is_empty());
    for (line, what) in [
        ("table = 1caps t.txt", "not a table name"),
        ("table = caps", "needs one path"),
        ("table = caps a b", "needs one path"),
        ("table = caps ../x.txt", "inside the mod folder"),
        ("table = caps /abs.txt", "inside the mod folder"),
        ("table = caps C:/x.txt", "inside the mod folder"),
        ("table = a-b t.txt", "not a table name"),
    ] {
        let d = check(&with(&format!("{line}\n")));
        assert!(
            d.iter()
                .any(|x| x.message.contains(what) && x.field.as_deref() == Some("tables")),
            "{line}: {d:?}"
        );
    }
    let d = check(&with("table = caps a.txt\ntable = caps b.txt\n"));
    assert!(d[0].severity == Severity::Warning && d[0].message.contains("twice"));
    // The settings form does not touch the table lines.
    let mut s = with("table = caps a.txt\nfuture = 1\n");
    s.order = 4;
    write_settings(&root, &s).unwrap();
    let text = fs::read_to_string(root.join(MANIFEST)).unwrap();
    assert!(
        text.contains("table = caps a.txt") && text.contains("future = 1"),
        "{text}"
    );
}

#[test]
fn table_files_on_disk() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("m");
    Project::create(&root, "M", None).unwrap();
    assert!(list_tables(&root).unwrap().is_empty());
    let made = new_table(&root, "caps").unwrap();
    assert_eq!((made.path.as_str(), made.exists), ("tables/caps.txt", true));
    assert!(fs::read_to_string(root.join(MANIFEST))
        .unwrap()
        .contains("table = caps tables/caps.txt"));
    assert!(new_table(&root, "caps").is_err());
    assert!(new_table(&root, "9bad").is_err());
    assert!(new_table(&root, &"a".repeat(32)).is_err());
    new_table(&root, "ban").unwrap();
    assert_eq!(list_tables(&root).unwrap().len(), 2);
    // A new table loads; an edit reports its problems.
    assert!(check_table_files(&root).unwrap().is_empty());
    assert!(
        write_table(&root, "caps", "0 = 12\n1 = 20\ndefault = 100\n")
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        read_table(&root, "caps").unwrap(),
        "0 = 12\n1 = 20\ndefault = 100\n"
    );
    let d = write_table(&root, "ban", "x = 1\n").unwrap();
    assert!(has_errors(&d));
    let d = check_table_files(&root).unwrap();
    assert_eq!(d.len(), 1);
    assert!(
        d[0].message.contains("'ban'") && d[0].message.contains("line 1"),
        "{d:?}"
    );
    assert_eq!(
        table_set(&root).unwrap(),
        tables(&[("caps", true), ("ban", false)])
    );
    assert!(read_table(&root, "nope").is_err());
    assert!(write_table(&root, "nope", "").is_err());

    // A rule that reads the table: checked against the manifest on save.
    let rule = "language 1\nrule cap on mon.exp\n  when level[event.mon] >= table.caps[badges]\n  do set event.amount 0\n  # table.caps[1] in a comment\n  do message \"table.caps[1]\"\n";
    let d = write_rule(&root, "rules/cap.rule", rule).unwrap();
    assert!(d.is_empty(), "{d:?}");
    let other = rule.replace("table.caps[badges]", "table.nope[badges]");
    let d = write_rule(&root, "rules/other.rule", &other).unwrap();
    assert!(d[0].message.contains("no table 'nope'"), "{d:?}");

    // Rename: the line, the standard file and the reads move; comments and
    // texts stay.
    let (entry, reads) = rename_table(&root, "caps", "levelcaps").unwrap();
    assert_eq!(
        (entry.path.as_str(), entry.exists, reads),
        ("tables/levelcaps.txt", true, 1)
    );
    assert!(root.join("tables/levelcaps.txt").is_file() && !root.join("tables/caps.txt").exists());
    let m = fs::read_to_string(root.join(MANIFEST)).unwrap();
    assert!(
        m.contains("table = levelcaps tables/levelcaps.txt") && !m.contains("table = caps "),
        "{m}"
    );
    let r = read_rule(&root, "rules/cap.rule").unwrap();
    assert!(
        r.contains("table.levelcaps[badges]")
            && r.contains("# table.caps[1]")
            && r.contains("\"table.caps[1]\""),
        "{r}"
    );
    assert!(check_rule_in(&r, Some("mon.exp"), Some(&table_set(&root).unwrap())).is_empty());
    assert!(rename_table(&root, "levelcaps", "ban").is_err());
    assert!(rename_table(&root, "levelcaps", "1x").is_err());
    assert!(rename_table(&root, "gone", "x").is_err());

    // Delete: line and file go; the rule that reads it now fails.
    delete_table(&root, "levelcaps").unwrap();
    assert!(!root.join("tables/levelcaps.txt").exists());
    assert_eq!(list_tables(&root).unwrap().len(), 1);
    let now = table_set(&root).unwrap();
    let d = check_rule_in(&r, Some("mon.exp"), Some(&now));
    assert!(d[0].message.contains("no table 'levelcaps'"));
}

/// A rename changes the reads of exactly that table.
#[test]
fn rename_leaves_other_names_alone() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("m");
    Project::create(&root, "M", None).unwrap();
    new_table(&root, "caps").unwrap();
    new_table(&root, "capsule").unwrap();
    let body = "  when table.caps[0] > table.capsule[0] && table.caps [2] > 0\n";
    write_rule(
        &root,
        "rules/r.rule",
        &format!("language 1\nrule r on mon.exp\n{body}  do refuse\n"),
    )
    .unwrap();
    let (_, n) = rename_table(&root, "caps", "cap").unwrap();
    assert_eq!(n, 2);
    let r = read_rule(&root, "rules/r.rule").unwrap();
    assert!(
        r.contains("table.cap[0] > table.capsule[0] && table.cap [2]"),
        "{r}"
    );
}

/// The mod from the docs example passes every check.
#[test]
fn level_cap_mod_end_to_end() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("m");
    Project::create(&root, "Caps", None).unwrap();
    new_table(&root, "caps").unwrap();
    write_table(&root, "caps", "0 = 12\n1 = 20\n2 = 25\ndefault = 100\n").unwrap();
    let d = write_rule(
        &root,
        "rules/cap.rule",
        "language 1\nrule cap on mon.exp\n  when level[event.mon] >= table.caps[badges]\n  do set event.amount 0\n",
    )
    .unwrap();
    assert!(d.is_empty());
    let m = fs::read_to_string(root.join(MANIFEST)).unwrap();
    assert!(
        m.contains("rule = mon.exp rules/cap.rule") && m.contains("table = caps tables/caps.txt"),
        "{m}"
    );
    assert!(check_table_files(&root).unwrap().is_empty());
    assert!(check_settings(&read_settings(&root).unwrap(), 0).is_empty());
}

#[test]
fn multiplayer_number_follows_the_port() {
    for (text, want) in [
        ("1", Some(1)),
        ("3", Some(3)),
        ("+7", Some(7)),
        ("1000000", Some(1_000_000)),
        ("0", None),
        ("-2", None),
        ("1000001", None),
        ("99999999999", None),
        ("2.5", None),
        ("abc", None),
        ("+", None),
        ("", None),
    ] {
        assert_eq!(parse_multiplayer(text).ok(), want, "{text:?}");
    }
    let mut s = ModSettings {
        name: "m".into(),
        ..Default::default()
    };
    let fields = |d: Vec<Diagnostic>| d.into_iter().filter_map(|d| d.field).collect::<Vec<_>>();
    assert!(check_settings(&s, 0).is_empty());
    s.multiplayer = Some("3".into());
    assert!(check_settings(&s, 0).is_empty());
    for bad in ["0", "x", "1.5", "2000000"] {
        s.multiplayer = Some(bad.into());
        assert_eq!(fields(check_settings(&s, 0)), ["multiplayer"], "{bad}");
    }
}

#[test]
fn multiplayer_key_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("soul");
    Project::create(&root, "Soul", None).unwrap();
    fs::write(
        root.join(MANIFEST),
        "name = Soul\nmultiplayer = 2\nmultiplayer = 4 # later wins\nfuture = 1\n",
    )
    .unwrap();
    let mut s = read_settings(&root).unwrap();
    assert_eq!(s.multiplayer.as_deref(), Some("4"));
    s.multiplayer = Some(" +5 ".into());
    write_settings(&root, &s).unwrap();
    let text = fs::read_to_string(root.join(MANIFEST)).unwrap();
    assert_eq!(text.matches("multiplayer").count(), 1, "{text}");
    assert!(text.contains("multiplayer = 5"), "{text}");
    assert!(text.contains("future = 1"), "{text}");
    assert_eq!(
        read_settings(&root).unwrap().multiplayer.as_deref(),
        Some("5")
    );
    // A value the port refuses is not written.
    s.multiplayer = Some("0".into());
    assert!(write_settings(&root, &s).is_err());
    // Empty removes the key.
    s.multiplayer = None;
    write_settings(&root, &s).unwrap();
    let text = fs::read_to_string(root.join(MANIFEST)).unwrap();
    assert!(!text.contains("multiplayer"), "{text}");
}

#[test]
fn session_use_without_a_number_warns() {
    // A module that imports eidos.<name> as a function of type () -> ().
    let module = |import: &str| {
        let name = |s: &str| [vec![s.len() as u8], s.as_bytes().to_vec()].concat();
        let mut m = b"\0asm\x01\0\0\0".to_vec();
        m.extend([1, 4, 1, 0x60, 0, 0]);
        let mut imports = vec![1];
        imports.extend(name("eidos"));
        imports.extend(name(import));
        imports.extend([0, 0]);
        m.push(2);
        m.push(imports.len() as u8);
        m.extend(imports);
        m
    };
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();
    let mut s = ModSettings {
        name: "m".into(),
        code: Some("mod.wasm".into()),
        ..Default::default()
    };
    // No file yet: nothing to say.
    assert!(check_session_use(&root, &s).is_empty());
    fs::write(root.join("mod.wasm"), module("session_leave")).unwrap();
    let found = check_session_use(&root, &s);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].field.as_deref(), Some("multiplayer"));
    s.multiplayer = Some("1".into());
    assert!(check_session_use(&root, &s).is_empty());
    s.multiplayer = None;
    fs::write(root.join("mod.wasm"), module("on_session")).unwrap();
    assert_eq!(check_session_use(&root, &s).len(), 1);
    fs::write(root.join("mod.wasm"), module("log")).unwrap();
    assert!(check_session_use(&root, &s).is_empty());
}

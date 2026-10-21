//! Checks for the options file: the grammar of eidos `sdk/mod_menu.c` at
//! 7524fd1, the round trip, and the `setting.<key>` check of rules.

use kenon_core::modcode::{check_rule_ctx, RuleContext, Severity};
use kenon_core::modopts::*;
use kenon_core::project::Project;

/// The example of eidos `docs/mod-services.md`, "Mod settings".
const DOC: &str = "option nuzlocke.dupes   bool    \"Dupes clause\"         on
option nuzlocke.cap     choice  \"Level caps\"           off|gym|ace
option nuzlocke.panel   bool    \"Show run status\"      on
option nuzlocke.lives   int     \"Lives\"                0 10 3
";

/// The file eidos `tests/test_mod_menu.c` writes.
const TEST: &str = "option nuzlocke.dupes bool \"Dupes clause\" on
option nuzlocke.cap choice \"Level caps\" off|gym|ace
";

fn msgs(text: &str) -> Vec<String> {
    parse_options(text)
        .diagnostics
        .into_iter()
        .map(|d| d.message)
        .collect()
}

#[test]
fn eidos_examples_parse_and_round_trip() {
    for text in [DOC, TEST] {
        let p = parse_options(text);
        assert!(p
            .diagnostics
            .iter()
            .all(|d| d.severity == Severity::Warning));
        assert_eq!(render_options(text, &p.options), text);
    }
    let p = parse_options(DOC);
    assert_eq!(p.options.len(), 4);
    assert_eq!(p.options[0].kind, OptionKind::Bool);
    assert_eq!(p.options[0].default, 1);
    assert_eq!(p.options[1].values, ["off", "gym", "ace"]);
    let lives = &p.options[3];
    assert_eq!((lives.min, lives.max, lives.default), (0, 10, 3));
    assert_eq!(lives.label, "Lives");
}

#[test]
fn comments_order_and_crlf_survive() {
    let text = "# Options of the run\r\n\r\n# Drop duplicates\r\noption a bool \"A\" on   # trailing\r\nnot a line\r\n# range\r\noption b int \"B\" 1 5 2\r\n";
    let mut p = parse_options(text);
    assert!(p.diagnostics.is_empty());
    assert_eq!(p.options[0].lead, ["# Drop duplicates"]);
    assert_eq!(p.options[0].comment, "# trailing");
    assert_eq!(render_options(text, &p.options), text);
    // An edit changes its line and keeps the rest.
    p.options[1].max = 9;
    let out = render_options(text, &p.options);
    assert!(out.contains("# range\r\noption b"), "{out}");
    assert!(out.contains("1 9 2"));
    assert!(out.contains("# Options of the run\r\n\r\n# Drop duplicates\r\noption a bool \"A\" on   # trailing\r\nnot a line"));
    // The comment above an option moves with it.
    p.options.swap(0, 1);
    let out = render_options(text, &p.options);
    let first = out.find("option b").unwrap();
    assert!(
        out[..first].ends_with("# range\r\n") || out[..first].contains("# range"),
        "{out}"
    );
    let again = parse_options(&out);
    assert!(again.diagnostics.is_empty());
    assert_eq!(again.options[0].key, "b");
    assert_eq!(again.options[0].lead, ["# range"]);
    assert_eq!(again.options[1].lead, ["# Drop duplicates"]);
    // A new option and a removed one.
    let mut q = parse_options(DOC).options;
    q.remove(1);
    q.push(OptionDecl {
        key: "extra".into(),
        label: "Extra".into(),
        kind: OptionKind::Choice,
        values: vec!["x".into(), "y".into()],
        ..OptionDecl::default()
    });
    let out = render_options(DOC, &q);
    assert!(!out.contains("nuzlocke.cap"));
    let again = parse_options(&out);
    assert!(again
        .diagnostics
        .iter()
        .all(|d| d.severity == Severity::Warning));
    assert_eq!(again.options.len(), 4);
    assert_eq!(again.options[3].key, "extra");
    assert_eq!(again.options[3].values, ["x", "y"]);
}

#[test]
fn the_ports_refusals() {
    let m = msgs("option a bool\n");
    assert!(m[0].contains("no label"), "{m:?}");
    assert!(msgs("option a color \"A\" red")[0].contains("unknown kind 'color'"));
    assert!(msgs("option a choice \"A\" ||")[0].contains("names no choice"));
    for bad in [
        "5 1 3",
        "0 10 11",
        "0 10 -1",
        "0 10",
        "a b c",
        "0 10 3 4",
        "0 2000000000 1",
    ] {
        let m = msgs(&format!("option a int \"A\" {bad}"));
        assert!(m[0].contains("min <= default <= max"), "{bad}: {m:?}");
    }
    // Another keyword and a comment are skipped without a word.
    assert!(msgs("hello x\n# option a bool \"A\" on\n\n").is_empty());
    // The line is cut at '#', quotes or not: the label is cut too.
    let o = parse_options("option a bool \"A # B\" on").options;
    assert!(o.is_empty() || !o[0].label.contains('#'));
    // Bool values.
    for (v, d) in [("on", 1), ("true", 1), ("1", 1), ("off", 0), ("yes", 0)] {
        let o = parse_options(&format!("option a bool \"A\" {v}")).options;
        assert_eq!(o[0].default, d, "{v}");
    }
}

#[test]
fn list_checks() {
    let mk = |key: &str| OptionDecl {
        key: key.into(),
        label: "L".into(),
        ..OptionDecl::default()
    };
    let d = check_options(&[mk("a"), mk("a")]);
    assert_eq!(d.len(), 1);
    assert_eq!(d[0].severity, Severity::Warning);
    assert_eq!(d[0].field.as_deref(), Some("option:1"));
    let d = check_options(&[mk("bad key")]);
    assert_eq!(d[0].severity, Severity::Error);
    assert_eq!(check_options(&[mk("a=b")])[0].severity, Severity::Error);
    // A dotted key works for code but not for rules.
    let d = check_options(&[mk("nuzlocke.dupes")]);
    assert_eq!(d.len(), 1);
    assert_eq!(d[0].severity, Severity::Warning);
    let many: Vec<_> = (0..17).map(|i| mk(&format!("k{i}"))).collect();
    let d = check_options(&many);
    assert_eq!(d.len(), 1);
    assert_eq!(d[0].field.as_deref(), Some("option:16"));
    let mut long = mk("a");
    long.label = "x".repeat(64);
    assert_eq!(check_options(&[long])[0].severity, Severity::Warning);
    let mut ch = mk("a");
    ch.kind = OptionKind::Choice;
    assert_eq!(check_options(&[ch.clone()])[0].severity, Severity::Error);
    ch.values = (0..9).map(|i| format!("v{i}")).collect();
    assert_eq!(check_options(&[ch.clone()])[0].severity, Severity::Warning);
    ch.values = vec!["a|b".into()];
    assert_eq!(check_options(&[ch])[0].severity, Severity::Error);
    let mut int = mk("a");
    int.kind = OptionKind::Int;
    int.min = 5;
    int.max = 1;
    assert_eq!(check_options(&[int.clone()])[0].severity, Severity::Error);
    int.min = 0;
    int.max = 4;
    int.default = 9;
    assert_eq!(check_options(&[int.clone()])[0].severity, Severity::Error);
    int.default = 4;
    assert!(check_options(&[int]).is_empty());
}

#[test]
fn the_file_is_made_and_declared() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("m");
    Project::create(&root, "Run rules", None).unwrap();
    let f = read_options_file(&root).unwrap();
    assert!(f.path.is_none() && f.options.is_empty());
    let mut opts = parse_options(DOC).options;
    for o in &mut opts {
        o.line = 0;
        o.raw.clear();
    }
    let f = write_options_file(&root, &opts).unwrap();
    assert_eq!(f.path.as_deref(), Some("options.txt"));
    assert_eq!(f.options.len(), 4);
    assert!(f
        .diagnostics
        .iter()
        .all(|d| d.severity == Severity::Warning));
    let manifest = std::fs::read_to_string(root.join("eidos_mod.txt")).unwrap();
    assert!(manifest.contains("settings = options.txt"), "{manifest}");
    // Saved again, the file stays the same.
    let text = std::fs::read_to_string(root.join("options.txt")).unwrap();
    write_options_file(&root, &f.options).unwrap();
    assert_eq!(
        std::fs::read_to_string(root.join("options.txt")).unwrap(),
        text
    );
    // A list the port would skip a line of is refused.
    let mut bad = f.options.clone();
    bad[3].max = -5;
    assert!(write_options_file(&root, &bad).is_err());
    let ctx = settings_ctx(&root).unwrap();
    assert!(ctx.namespace && ctx.keys.contains("nuzlocke.lives"));
}

#[test]
fn rules_read_declared_settings() {
    let rule = |key: &str| {
        format!("language 1\nrule cap on encounter.wild\n    when setting.{key} == 1\n    do set event.level setting.{key}\n")
    };
    let ctx = SettingsCtx {
        namespace: true,
        keys: ["lives".to_string()].into_iter().collect(),
    };
    let rc = RuleContext {
        settings: Some(&ctx),
        ..RuleContext::default()
    };
    assert!(check_rule_ctx(&rule("lives"), Some("encounter.wild"), rc).is_empty());
    let d = check_rule_ctx(&rule("nope"), Some("encounter.wild"), rc);
    assert_eq!(d.len(), 2);
    assert!(d.iter().all(|d| d.severity == Severity::Warning));
    assert!(d[0].message.contains("declares no option 'nope'"));
    let none = SettingsCtx::default();
    let rc = RuleContext {
        settings: Some(&none),
        ..RuleContext::default()
    };
    let d = check_rule_ctx(&rule("lives"), Some("encounter.wild"), rc);
    assert!(d.iter().all(|d| d.severity == Severity::Error));
    assert!(d[0].message.contains("no namespace"));
    // Unchecked without a context.
    assert!(check_rule_ctx(
        &rule("nope"),
        Some("encounter.wild"),
        RuleContext::default()
    )
    .is_empty());
}

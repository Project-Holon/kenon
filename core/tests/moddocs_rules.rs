//! The mod docs' rules pass eidos-mod's rule checker. The docs are kenon's
//! (`ui/src/lib/moddocs/api.json`), so this check lives here.

use std::fs;
use std::path::PathBuf;

use eidos_mod::modcode::{check_rule, rule_event, rule_names, RULES_TABLE};

fn errors(text: &str, ev: Option<&str>) -> Vec<String> {
    check_rule(text, ev)
        .into_iter()
        .map(|d| d.message)
        .collect()
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
    for line in RULES_TABLE.lines().filter(|l| l.starts_with("action ")) {
        let kw = line.split_whitespace().nth(1).unwrap();
        assert!(docs.contains(&kw), "the docs lack the action {kw}");
    }
}

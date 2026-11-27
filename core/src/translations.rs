//! A mod's text in other languages (`<bank>.<lang>.msg`, eidos 0.2.8): what
//! it changes in its own language, and each line's translation.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::bridge::Error;
use crate::project::Project;

/// One line the mod sets, with its translation in a language.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Row {
    /// Where it lives: "text" (a/0/0/2 or a/0/0/3 line), "mod" (the
    /// manifest's name and description), "strings" or "ui".
    pub source: String,
    /// Its key in that source: "<arc>/<bank>/<line>", "name", a strings key
    /// or an interface word.
    pub key: String,
    pub arc: u16,
    pub bank: usize,
    pub line: usize,
    /// The mod's line in its own language.
    pub text: String,
    /// The line in the language, when the mod has one.
    pub translation: Option<String>,
}

/// Every line the mod sets in its own language (text archives a/0/0/2 and
/// a/0/0/3), with its translation in `lang`.
pub fn rows(project: &Project, lang: &str) -> Vec<Row> {
    let mut out = Vec::new();
    for arc in [2u16, 3] {
        for bank in project.msg_banks(arc, None) {
            let theirs = project.msg_lines_in(arc, bank, Some(lang));
            for (line, text) in project.msg_lines(arc, bank) {
                out.push(Row {
                    source: "text".into(),
                    key: format!("{arc}/{bank}/{line}"),
                    arc,
                    bank,
                    line,
                    text,
                    translation: theirs.get(&line).cloned(),
                });
            }
        }
    }
    // The mod's name and description.
    let other = |k: &str| {
        project.manifest.other.iter().find_map(|l| {
            let (key, v) = l.split('#').next()?.split_once('=')?;
            (key.trim() == k).then(|| v.trim().to_string())
        })
    };
    for (key, text) in [
        ("name", project.manifest.name.clone()),
        ("description", project.manifest.description.clone()),
    ] {
        if !text.is_empty() {
            out.push(Row {
                source: "mod".into(),
                key: key.into(),
                arc: 0,
                bank: 0,
                line: 0,
                text,
                translation: other(&format!("{key}.{lang}")),
            });
        }
    }
    // Strings files and interface words.
    for (source, plain) in [("strings", "strings"), ("ui", "ui_text")] {
        let base = kv_lines(&project.root.join(format!("{plain}.txt")));
        let theirs = kv_lines(&project.root.join(format!("{plain}.{lang}.txt")));
        for (key, text) in base {
            // An interface word's plain line may be empty: the word is its own text.
            let text = if text.is_empty() && source == "ui" {
                key.clone()
            } else {
                text
            };
            let translation = theirs
                .iter()
                .find(|(k, _)| *k == key)
                .map(|(_, v)| v.clone())
                .filter(|v| !v.is_empty());
            out.push(Row {
                source: source.into(),
                key,
                arc: 0,
                bank: 0,
                line: 0,
                text,
                translation,
            });
        }
    }
    out
}

/// A `key = text` file's lines in order (strings, ui_text), comments left out.
fn kv_lines(path: &std::path::Path) -> Vec<(String, String)> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .filter_map(|l| {
            let (k, v) = l.split_once('=')?;
            Some((k.trim().to_string(), v.trim().to_string()))
        })
        .filter(|(k, _)| !k.is_empty())
        .collect()
}

/// Sets one key's line in a `key = text` file, in place: the line is
/// replaced, removed (empty text) or added at the end; other lines and
/// comments stay.
fn set_kv_line(path: &std::path::Path, key: &str, text: &str) -> Result<(), Error> {
    let old = std::fs::read_to_string(path).unwrap_or_default();
    let mut lines: Vec<String> = old.lines().map(str::to_string).collect();
    let at = lines.iter().position(|l| {
        !l.trim_start().starts_with('#') && l.split_once('=').is_some_and(|(k, _)| k.trim() == key)
    });
    match (at, text.is_empty()) {
        (Some(i), true) => {
            lines.remove(i);
        }
        (Some(i), false) => lines[i] = format!("{key} = {text}"),
        (None, false) => lines.push(format!("{key} = {text}")),
        (None, true) => return Ok(()),
    }
    let io = |e: std::io::Error| Error::Missing(e.to_string());
    if lines
        .iter()
        .all(|l| l.trim().is_empty() || l.trim_start().starts_with('#'))
    {
        if path.is_file() {
            std::fs::remove_file(path).map_err(io)?;
        }
        return Ok(());
    }
    eidos_mod::manifest::write_atomic(path, (lines.join("\n") + "\n").as_bytes()).map_err(io)
}

/// Sets one translation of any source; an empty text removes it.
pub fn set_one(
    project: &Project,
    lang: &str,
    source: &str,
    key: &str,
    text: &str,
) -> Result<(), Error> {
    if !crate::project::LANGUAGES.contains(&lang) {
        return Err(Error::BadPath(format!(
            "{lang} is not a language eidos runs"
        )));
    }
    match source {
        "text" => {
            let n: Vec<usize> = key.split('/').filter_map(|x| x.parse().ok()).collect();
            let [arc, bank, line] = n[..] else {
                return Err(Error::BadPath(format!("bad text key {key}")));
            };
            set(project, arc as u16, bank, lang, &[(line, text.to_string())])
        }
        "mod" if key == "name" || key == "description" => {
            let mut fresh =
                Project::open(&project.root).map_err(|e| Error::Missing(e.to_string()))?;
            let k = format!("{key}.{lang}");
            fresh.manifest.other.retain(|l| {
                l.split('#')
                    .next()
                    .unwrap_or("")
                    .split_once('=')
                    .map(|(x, _)| x.trim())
                    != Some(k.as_str())
            });
            if !text.is_empty() {
                fresh
                    .manifest
                    .other
                    .push(format!("{k} = {}", text.replace(['\r', '\n'], " ")));
            }
            fresh.save_meta().map_err(|e| Error::Missing(e.to_string()))
        }
        "strings" => set_kv_line(&project.root.join(format!("strings.{lang}.txt")), key, text),
        "ui" => set_kv_line(&project.root.join(format!("ui_text.{lang}.txt")), key, text),
        _ => Err(Error::BadPath(format!("no translations in {source}"))),
    }
}

/// Name lines (a/0/0/2 bank) and what they name.
const NAME_BANKS: [(usize, &str); 5] = [
    (203, "move"),
    (54, "item"),
    (182, "ability"),
    (199, "type"),
    (70, "species"),
];

/// Official names in `lang` for the mod's untranslated name lines whose
/// English matches a name in any game (Moonblast, Pixie Plate...):
/// (bank, line, name). The mod's own language must be English.
pub fn official_names(project: &Project, lang: &str) -> Vec<(usize, usize, String)> {
    rows(project, lang)
        .into_iter()
        .filter(|r| r.source == "text" && r.arc == 2 && r.translation.is_none())
        .filter_map(|r| {
            let kind = NAME_BANKS.iter().find(|(b, _)| *b == r.bank)?.1;
            let name = crate::reference::official_name(kind, &r.text, lang)?;
            Some((r.bank, r.line, name))
        })
        .collect()
}

/// How many of the mod's lines `lang` has.
pub fn progress(project: &Project, lang: &str) -> (usize, usize) {
    let r = rows(project, lang);
    (
        r.iter().filter(|r| r.translation.is_some()).count(),
        r.len(),
    )
}

/// Sets lines of a bank in `lang`; an empty text removes the line, so the
/// game shows the mod's own line.
pub fn set(
    project: &Project,
    arc: u16,
    bank: usize,
    lang: &str,
    lines: &[(usize, String)],
) -> Result<(), Error> {
    if !crate::project::LANGUAGES.contains(&lang) {
        return Err(Error::BadPath(format!(
            "{lang} is not a language eidos runs"
        )));
    }
    let mut all: BTreeMap<usize, String> = project.msg_lines_in(arc, bank, Some(lang));
    for (line, text) in lines {
        if text.is_empty() {
            all.remove(line);
        } else {
            all.insert(*line, text.clone());
        }
    }
    project
        .write_msg_lines_in(arc, bank, Some(lang), &all)
        .map_err(|e| Error::Missing(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_follow_the_mods_lines() {
        let dir = tempfile::tempdir().unwrap();
        let project = Project::create(&dir.path().join("m"), "Test", None).unwrap();
        let plain = [(17usize, "Fairy".to_string())].into_iter().collect();
        project.write_msg_lines(2, 199, &plain).unwrap();
        assert_eq!(
            progress(&project, "fr"),
            (0, 2),
            "the type name and the mod name"
        );
        set(&project, 2, 199, "fr", &[(17, "Fée".into())]).unwrap();
        let r: Vec<Row> = rows(&project, "fr")
            .into_iter()
            .filter(|r| r.source == "text")
            .collect();
        assert_eq!(
            (r[0].text.as_str(), r[0].translation.as_deref()),
            ("Fairy", Some("Fée"))
        );
        assert_eq!(progress(&project, "fr"), (1, 2));
        set(&project, 2, 199, "fr", &[(17, String::new())]).unwrap();
        assert_eq!(
            progress(&project, "fr"),
            (0, 2),
            "the type name and the mod name"
        );
        assert!(set(&project, 2, 199, "ja", &[(17, "x".into())]).is_err());

        // The name, a string and an interface word.
        std::fs::write(
            project.root.join("strings.txt"),
            "# lives\nnuz.lives = Lives\n",
        )
        .unwrap();
        std::fs::write(project.root.join("ui_text.txt"), "BAG =\n").unwrap();
        let project = Project::open(&project.root).unwrap();
        set_one(&project, "fr", "strings", "nuz.lives", "Vies").unwrap();
        set_one(&project, "fr", "ui", "BAG", "SAC").unwrap();
        set_one(&project, "fr", "mod", "name", "Essai").unwrap();
        let project = Project::open(&project.root).unwrap();
        let r = rows(&project, "fr");
        let t = |s: &str, k: &str| {
            r.iter()
                .find(|x| x.source == s && x.key == k)
                .map(|x| (x.text.clone(), x.translation.clone()))
        };
        assert_eq!(
            t("strings", "nuz.lives"),
            Some(("Lives".into(), Some("Vies".into())))
        );
        assert_eq!(t("ui", "BAG"), Some(("BAG".into(), Some("SAC".into()))));
        assert_eq!(
            t("mod", "name"),
            Some(("Test".into(), Some("Essai".into())))
        );
        assert!(std::fs::read_to_string(project.root.join("eidos_mod.txt"))
            .unwrap()
            .contains("name.fr = Essai"));
        set_one(&project, "fr", "strings", "nuz.lives", "").unwrap();
        assert!(!project.root.join("strings.fr.txt").exists());
    }
}

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
                    arc,
                    bank,
                    line,
                    text,
                    translation: theirs.get(&line).cloned(),
                });
            }
        }
    }
    out
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
        .filter(|r| r.arc == 2 && r.translation.is_none())
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
        assert_eq!(progress(&project, "fr"), (0, 1));
        set(&project, 2, 199, "fr", &[(17, "Fée".into())]).unwrap();
        let r = rows(&project, "fr");
        assert_eq!(
            (r[0].text.as_str(), r[0].translation.as_deref()),
            ("Fairy", Some("Fée"))
        );
        assert_eq!(progress(&project, "fr"), (1, 1));
        set(&project, 2, 199, "fr", &[(17, String::new())]).unwrap();
        assert_eq!(progress(&project, "fr"), (0, 1));
        assert!(set(&project, 2, 199, "ja", &[(17, "x".into())]).is_err());
    }
}

//! A mod's options file: the player-facing settings the manifest's
//! `settings` key points at.
//!
//! The grammar follows eidos's `sdk/mod_menu.c` (`parse_option`,
//! `read_declaration`, `bw_mod_menu_setting_get`) at 7524fd1:
//!
//! ```text
//! option <key> bool   "<label>" on|off
//! option <key> choice "<label>" first|second|third
//! option <key> int    "<label>" <min> <max> <default>
//! ```
//!
//! - a line is cut at the first `#`, quotes or not; a line that does not
//!   start with `option` is skipped; a line is read up to 511 bytes;
//! - the key and the kind are words, the label is a quoted text (or a word),
//!   and the value is the rest of the line;
//! - a bool is on for `on`, `true` and `1`, and off for any other text;
//! - a choice lists up to 8 names of up to 31 characters, split on `|`; empty
//!   names are dropped and the first name is the default;
//! - an int has three whole numbers within a billion of zero, with
//!   `min <= default <= max`;
//! - a mod has up to 16 options, keys and labels are cut at 63 characters,
//!   and the first option of a key answers `setting_get`.
//!
//! The port skips a line it refuses. The player's choices are stored as
//! `key = number` lines, so a key has no `=`. A rule reads a key with
//! `setting.<key>`, and the rule language has no dot in a name, so a key with
//! one is for code only.
//!
//! [`parse_options`] and [`render_options`] round-trip a file: comments, blank
//! lines, the order and the text of an unchanged line stay as they are.

use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::modcode::{Diagnostic, Severity};
use crate::project::Project;

/// The most options one mod has; the port ignores the rest.
pub const OPTIONS_MAX: usize = 16;
/// The longest key and label, in bytes (the port keeps 63).
pub const KEY_MAX: usize = 63;
pub const LABEL_MAX: usize = 63;
/// The most names of a choice, and the longest name, in bytes.
pub const CHOICES_MAX: usize = 8;
pub const CHOICE_LEN_MAX: usize = 31;
/// The widest an int bound goes.
pub const INT_LIMIT: i32 = 1_000_000_000;
/// The bytes of a line the port reads.
const LINE_MAX: usize = 511;
/// The file a new options list goes in.
pub const OPTIONS_FILE: &str = "options.txt";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OptionKind {
    #[default]
    Bool,
    Choice,
    Int,
}

impl OptionKind {
    fn word(self) -> &'static str {
        match self {
            OptionKind::Bool => "bool",
            OptionKind::Choice => "choice",
            OptionKind::Int => "int",
        }
    }
}

/// One option. `default` is 0 or 1 for a bool, 0 for a choice (the first
/// name) and the number for an int. `line`, `lead`, `comment` and `raw` come
/// from the file and go back to [`render_options`] unchanged: they keep the
/// comments and the text of a line nobody edited.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct OptionDecl {
    pub key: String,
    pub label: String,
    pub kind: OptionKind,
    pub default: i32,
    pub min: i32,
    pub max: i32,
    pub values: Vec<String>,
    /// 1-based line in the file, 0 for a new option.
    pub line: u32,
    /// The comment lines right above the option.
    pub lead: Vec<String>,
    /// The comment after the value, with its `#`.
    pub comment: String,
    /// The line as the file has it.
    pub raw: String,
}

/// What a file holds: the options, and the problems of the lines the port
/// skips and of the options themselves.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Parsed {
    pub options: Vec<OptionDecl>,
    pub diagnostics: Vec<Diagnostic>,
}

// ---------------------------------------------------------------------
// Reading
// ---------------------------------------------------------------------

fn is_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\r' | '\x0b' | '\x0c' | '\n')
}

/// The next token of `s` from `at`: a quoted text or a word. Returns the
/// text and the byte after it, or `None` at the end of the line.
fn next_token(s: &str, mut at: usize) -> Option<(&str, usize)> {
    let b = s.as_bytes();
    while at < b.len() && is_space(b[at] as char) {
        at += 1;
    }
    if at >= b.len() {
        return None;
    }
    if b[at] == b'"' {
        let start = at + 1;
        let end = s[start..].find('"').map_or(s.len(), |i| start + i);
        let after = if end < s.len() { end + 1 } else { end };
        return Some((&s[start..end], after));
    }
    let end = s[at..].find(is_space).map_or(s.len(), |i| at + i);
    Some((&s[at..end], end))
}

/// The line the port reads: cut at 511 bytes (on a character boundary), then
/// at the first `#`. Returns the body and the comment (with its `#`).
fn split_comment(line: &str) -> (&str, &str) {
    let mut cut = line.len().min(LINE_MAX);
    while !line.is_char_boundary(cut) {
        cut -= 1;
    }
    let read = &line[..cut];
    match read.find('#') {
        Some(i) => (&read[..i], line[i..].trim_end()),
        None => (read, ""),
    }
}

fn bool_value(text: &str) -> bool {
    matches!(text, "on" | "true" | "1")
}

/// A whole number the way `strtol` reads it in base 10: an optional sign and
/// digits. Returns it and the rest.
fn long_prefix(s: &str) -> Option<(i64, &str)> {
    let t = s.trim_start_matches(is_space);
    let digits = t.strip_prefix(['-', '+']).unwrap_or(t);
    let n = digits.bytes().take_while(u8::is_ascii_digit).count();
    if n == 0 {
        return None;
    }
    let sign_len = t.len() - digits.len();
    let v: i64 =
        t[..sign_len + n]
            .trim_start_matches('+')
            .parse()
            .unwrap_or(if t.starts_with('-') {
                i64::MIN
            } else {
                i64::MAX
            });
    Some((v, &t[sign_len + n..]))
}

/// One line. `Ok(None)` is a line the port skips without a word (blank, a
/// comment, another keyword), `Err` is an option line it refuses.
fn parse_line(line: &str) -> Result<Option<OptionDecl>, (usize, usize, String)> {
    let (body, comment) = split_comment(line);
    let Some((word, mut at)) = next_token(body, 0) else {
        return Ok(None);
    };
    if word != "option" {
        return Ok(None);
    }
    let whole = (0, body.trim_end().len());
    let mut take = |what: &str| -> Result<(&str, usize, usize), (usize, usize, String)> {
        let start = at;
        match next_token(body, at) {
            Some((t, next)) => {
                at = next;
                Ok((t, start, next))
            }
            None => Err((
                whole.0,
                whole.1,
                format!(
                    "the option line has no {what}: option <key> <bool|choice|int> \"<label>\" <value>; the game skips it"
                ),
            )),
        }
    };
    let (key, _, _) = take("key")?;
    let (kind_word, kind_from, kind_to) = take("kind")?;
    let (label, _, _) = take("label")?;
    let value = body[at..].trim();
    let value_at = body[at..].find(value).map_or(at, |i| at + i);
    let mut o = OptionDecl {
        key: key.to_string(),
        label: label.to_string(),
        comment: comment.to_string(),
        raw: line.to_string(),
        ..OptionDecl::default()
    };
    match kind_word {
        "bool" => {
            o.kind = OptionKind::Bool;
            o.default = i32::from(bool_value(value));
        }
        "choice" => {
            o.kind = OptionKind::Choice;
            o.values = value
                .split('|')
                .map(str::trim)
                .filter(|n| !n.is_empty())
                .map(str::to_string)
                .collect();
            if o.values.is_empty() {
                return Err((
                    value_at,
                    value_at + value.len().max(1),
                    format!(
                        "the option '{key}' names no choice: list the names with '|'; the game skips it"
                    ),
                ));
            }
        }
        "int" => {
            let bad = |msg: &str| (value_at, value_at + value.len().max(1), msg.to_string());
            let need = format!(
                "the int option '{key}' needs '<min> <max> <default>' with min <= default <= max; the game skips it"
            );
            let Some((lo, rest)) = long_prefix(value) else {
                return Err(bad(&need));
            };
            let Some((hi, rest)) = long_prefix(rest) else {
                return Err(bad(&need));
            };
            let Some((def, rest)) = long_prefix(rest) else {
                return Err(bad(&need));
            };
            let limit = i64::from(INT_LIMIT);
            if !rest.trim().is_empty()
                || lo > hi
                || def < lo
                || def > hi
                || lo < -limit
                || hi > limit
            {
                return Err(bad(&need));
            }
            o.kind = OptionKind::Int;
            o.min = lo as i32;
            o.max = hi as i32;
            o.default = def as i32;
        }
        other => {
            return Err((
                kind_from,
                kind_to,
                format!(
                    "the option '{key}' has the unknown kind '{other}' (bool, choice or int); the game skips it"
                ),
            ));
        }
    }
    Ok(Some(o))
}

/// The pieces of a file, in order.
enum Piece {
    Raw(String),
    Slot,
}

/// Splits `text` into raw lines and option slots. The comment lines right
/// above an option are the option's `lead`.
fn pieces(text: &str) -> (Vec<Piece>, Vec<OptionDecl>, Vec<Diagnostic>) {
    let mut out: Vec<Piece> = Vec::new();
    let mut opts: Vec<OptionDecl> = Vec::new();
    let mut diags: Vec<Diagnostic> = Vec::new();
    let mut pending: Vec<String> = Vec::new();
    let flush = |pending: &mut Vec<String>, out: &mut Vec<Piece>| {
        out.extend(pending.drain(..).map(Piece::Raw));
    };
    for (i, line) in text.lines().enumerate() {
        let n = i + 1;
        if line.trim_start().starts_with('#') {
            pending.push(line.to_string());
            continue;
        }
        match parse_line(line) {
            Ok(Some(mut o)) => {
                o.line = n as u32;
                o.lead = std::mem::take(&mut pending);
                opts.push(o);
                out.push(Piece::Slot);
            }
            Ok(None) => {
                flush(&mut pending, &mut out);
                out.push(Piece::Raw(line.to_string()));
            }
            Err((a, b, msg)) => {
                flush(&mut pending, &mut out);
                out.push(Piece::Raw(line.to_string()));
                diags.push(Diagnostic::at(n, line, a, b, Severity::Error, &msg));
            }
        }
    }
    flush(&mut pending, &mut out);
    (out, opts, diags)
}

/// Reads an options file. The problems are the port's refusals of a line,
/// then the checks of [`check_options`].
pub fn parse_options(text: &str) -> Parsed {
    let (_, options, mut diagnostics) = pieces(text);
    diagnostics.extend(check_options(&options));
    Parsed {
        options,
        diagnostics,
    }
}

// ---------------------------------------------------------------------
// Checking
// ---------------------------------------------------------------------

/// A name a rule can read: letters, digits and `_`, not starting with a
/// digit.
pub fn rule_readable(key: &str) -> bool {
    !key.is_empty()
        && !key.starts_with(|c: char| c.is_ascii_digit())
        && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// The problems of a list of options. A problem of option `n` (0-based) has
/// the field `option:n`, and a problem of the list the field `options`. When
/// the option comes from a file the diagnostic also has its line.
pub fn check_options(options: &[OptionDecl]) -> Vec<Diagnostic> {
    use Severity::{Error, Warning};
    let mut out = Vec::new();
    let mut at = |i: usize, sev: Severity, msg: &str| {
        let mut d = Diagnostic::field(&format!("option:{i}"), sev, msg);
        d.line = options[i].line;
        out.push(d);
    };
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for (i, o) in options.iter().enumerate() {
        if i >= OPTIONS_MAX {
            at(
                i,
                Error,
                &format!("a mod has {OPTIONS_MAX} options at most; the game ignores this one"),
            );
        }
        if o.key.is_empty()
            || o.key
                .contains(|c: char| is_space(c) || matches!(c, '"' | '#' | '='))
        {
            at(
                i,
                Error,
                "a key is one word without spaces, quotes, '#' or '='",
            );
        } else {
            if o.key.len() > KEY_MAX {
                at(
                    i,
                    Error,
                    &format!("a key is {KEY_MAX} characters at most; the game cuts it"),
                );
            }
            if !rule_readable(&o.key) {
                at(
                    i,
                    Warning,
                    "rules read keys of letters, digits and '_' (not starting with a digit); only code can read this one",
                );
            }
            if !seen.insert(&o.key) {
                at(
                    i,
                    Warning,
                    "this key is declared before; the game answers with the first",
                );
            }
        }
        if o.label.contains(['"', '#']) {
            at(i, Error, "a label has no quote or '#'");
        } else if o.label.trim().is_empty() {
            at(i, Warning, "the option has no label to show");
        } else if o.label.len() > LABEL_MAX {
            at(
                i,
                Warning,
                &format!("a label is {LABEL_MAX} characters at most; the game cuts it"),
            );
        }
        match o.kind {
            OptionKind::Bool => {
                if !(0..=1).contains(&o.default) {
                    at(i, Error, "a bool is on or off");
                }
            }
            OptionKind::Choice => {
                if o.values.is_empty() {
                    at(i, Error, "a choice names at least one value");
                }
                if o.values.len() > CHOICES_MAX {
                    at(
                        i,
                        Warning,
                        &format!(
                            "a choice has {CHOICES_MAX} names at most; the game drops the rest"
                        ),
                    );
                }
                for v in &o.values {
                    if v.trim().is_empty() || v.contains(['|', '#', '"']) {
                        at(i, Error, "a name is text without '|', '#' or quotes");
                    } else if v.len() > CHOICE_LEN_MAX {
                        at(
                            i,
                            Warning,
                            &format!(
                                "a name is {CHOICE_LEN_MAX} characters at most; the game cuts '{v}'"
                            ),
                        );
                    }
                }
            }
            OptionKind::Int => {
                let limit = INT_LIMIT;
                if o.min > o.max {
                    at(i, Error, "the lowest value is above the highest");
                } else if o.default < o.min || o.default > o.max {
                    at(
                        i,
                        Error,
                        "the default is outside the lowest and highest value",
                    );
                }
                if o.min < -limit || o.max > limit {
                    at(i, Error, "an int is within a billion of zero");
                }
            }
        }
    }
    out
}

// ---------------------------------------------------------------------
// Writing
// ---------------------------------------------------------------------

fn same(a: &OptionDecl, b: &OptionDecl) -> bool {
    a.key == b.key
        && a.label == b.label
        && a.kind == b.kind
        && a.default == b.default
        && a.min == b.min
        && a.max == b.max
        && a.values == b.values
}

fn value_text(o: &OptionDecl) -> String {
    match o.kind {
        OptionKind::Bool => (if o.default != 0 { "on" } else { "off" }).to_string(),
        OptionKind::Choice => o.values.join("|"),
        OptionKind::Int => format!("{} {} {}", o.min, o.max, o.default),
    }
}

/// The text of one option line, its columns padded to `widths` (key, label).
fn render_line(o: &OptionDecl, widths: (usize, usize)) -> String {
    let label = format!("\"{}\"", o.label);
    let mut s = format!(
        "option {:<kw$}  {:<6}  {:<lw$}  {}",
        o.key,
        o.kind.word(),
        label,
        value_text(o),
        kw = widths.0,
        lw = widths.1,
    );
    if !o.comment.trim().is_empty() {
        s.push_str("  ");
        s.push_str(o.comment.trim());
    }
    s.trim_end().to_string()
}

/// `text` with its options replaced by `options`, in that order. Comments,
/// blank lines and lines the port skips stay where they are; the comment lines
/// above an option move with it; a line whose option nobody edited stays as
/// it is. The options fill the places of the old ones in order, and extra
/// ones go after the last.
pub fn render_options(text: &str, options: &[OptionDecl]) -> String {
    let crlf = text.contains("\r\n");
    let (parts, _, _) = pieces(text);
    let widths = (
        options.iter().map(|o| o.key.len()).max().unwrap_or(0),
        options.iter().map(|o| o.label.len() + 2).max().unwrap_or(0),
    );
    let line_of = |o: &OptionDecl, out: &mut Vec<String>| {
        out.extend(o.lead.iter().cloned());
        let keep = !o.raw.is_empty() && matches!(parse_line(&o.raw), Ok(Some(p)) if same(&p, o));
        out.push(if keep {
            o.raw.clone()
        } else {
            render_line(o, widths)
        });
    };
    let mut lines: Vec<String> = Vec::new();
    let mut next = options.iter();
    let last_slot = parts.iter().rposition(|p| matches!(p, Piece::Slot));
    for (i, p) in parts.iter().enumerate() {
        match p {
            Piece::Raw(l) => lines.push(l.clone()),
            Piece::Slot => {
                if let Some(o) = next.next() {
                    line_of(o, &mut lines);
                }
                if Some(i) == last_slot {
                    for o in next.by_ref() {
                        line_of(o, &mut lines);
                    }
                }
            }
        }
    }
    for o in next {
        line_of(o, &mut lines);
    }
    let nl = if crlf { "\r\n" } else { "\n" };
    let mut out = lines.join(nl);
    if !out.is_empty() {
        out.push_str(nl);
    }
    out
}

// ---------------------------------------------------------------------
// The mod
// ---------------------------------------------------------------------

/// The options of the open mod.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OptionsFile {
    /// The manifest's `settings` path, `None` when it names none.
    pub path: Option<String>,
    pub exists: bool,
    pub options: Vec<OptionDecl>,
    pub diagnostics: Vec<Diagnostic>,
}

fn bad_input(msg: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, msg)
}

/// The `settings` path of the manifest (the last line wins).
fn settings_path(project: &Project) -> Option<String> {
    project
        .manifest
        .other
        .iter()
        .filter_map(|l| {
            let body = l.split('#').next().unwrap_or("");
            let (k, v) = body.split_once('=')?;
            (k.trim() == "settings").then(|| v.trim().to_string())
        })
        .rfind(|v| !v.is_empty())
}

fn inside(rel: &str) -> io::Result<String> {
    use std::path::Component;
    let p = Path::new(rel);
    let ok = !rel.is_empty()
        && !rel.contains(':')
        && p.components()
            .all(|c| matches!(c, Component::Normal(_) | Component::CurDir));
    if ok {
        Ok(rel.replace('\\', "/"))
    } else {
        Err(bad_input(
            "the options file is a path inside the mod folder",
        ))
    }
}

/// Reads the mod's options file; a mod with no `settings` line, or a missing
/// file, has no options.
pub fn read_options_file(root: &Path) -> io::Result<OptionsFile> {
    let project = Project::open(root)?;
    let Some(path) = settings_path(&project) else {
        return Ok(OptionsFile::default());
    };
    let text = inside(&path).and_then(|rel| fs::read_to_string(root.join(rel)));
    Ok(match text {
        Ok(text) => {
            let parsed = parse_options(&text);
            OptionsFile {
                path: Some(path),
                exists: true,
                options: parsed.options,
                diagnostics: parsed.diagnostics,
            }
        }
        Err(_) => OptionsFile {
            path: Some(path),
            ..OptionsFile::default()
        },
    })
}

/// Writes the options into the mod's options file. A mod with no `settings`
/// line gets `options.txt` and the line. A list the port would skip a line of
/// is refused.
pub fn write_options_file(root: &Path, options: &[OptionDecl]) -> io::Result<OptionsFile> {
    if let Some(d) = check_options(options)
        .into_iter()
        .find(|d| d.severity == Severity::Error)
    {
        return Err(bad_input(&d.message));
    }
    let mut project = Project::open(root)?;
    let rel = match settings_path(&project) {
        Some(p) => inside(&p)?,
        None => {
            project
                .manifest
                .other
                .push(format!("settings = {OPTIONS_FILE}"));
            project.save_meta()?;
            OPTIONS_FILE.to_string()
        }
    };
    let file = root.join(&rel);
    let old = fs::read_to_string(&file).unwrap_or_default();
    let text = render_options(&old, options);
    if let Some(dir) = file.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = file.with_extension("tmp");
    fs::write(&tmp, text)?;
    fs::rename(&tmp, &file)?;
    read_options_file(root)
}

/// What a rule check needs of the mod's options.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SettingsCtx {
    /// The mod has a namespace, so it has settings.
    pub namespace: bool,
    /// The keys of the options the port loads.
    pub keys: BTreeSet<String>,
}

/// The settings a rule of the mod may read. The port gives every mod a
/// namespace (its `state` name, or one made from the mod name, or the folder
/// name), so a mod has none only when all three are empty.
pub fn settings_ctx(root: &Path) -> io::Result<SettingsCtx> {
    let project = Project::open(root)?;
    let file = read_options_file(root)?;
    let state = crate::modcode::read_settings(root)?.state;
    let named = state.is_some_and(|s| !s.trim().is_empty())
        || !project.manifest.name.trim().is_empty()
        || root.file_name().is_some();
    Ok(SettingsCtx {
        namespace: named,
        keys: file
            .options
            .into_iter()
            .take(OPTIONS_MAX)
            .map(|o| o.key)
            .collect(),
    })
}

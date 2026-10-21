//! A mod's code and settings: the manifest keys beyond the data layer's
//! (`code`, `api`, `port`, `state`, `settings`), the raw hook lines
//! (`override`), the rule files (`rule`) and the checks eidos makes on them.
//!
//! The rules follow eidos's code, not its prose: `sdk/mod_manifest.c`
//! (`bw_mod_manifest_parse`, `parse_override`, `parse_rule`,
//! `bw_mod_manifest_apply`) and `sdk/mod_rule.c` (the scanner, the parser and
//! the name tables). Where `docs/mod-rules-language.md` differs from that
//! code, the code wins:
//!
//! - the port knows 58 events (`modnames/rules.txt`), not the 359 entries of
//!   `docs/mod-events.md`, which count raw hooks too;
//! - a rule file holds exactly one rule;
//! - an operator is read as two characters at most, and a pair that is not
//!   an operator reads as two, so `x*-1` and `a<-1` mean what they look like;
//! - `battle_style` cannot be written.
//!
//! A mod's "hook code" is a native library named by `code`, built outside
//! kenon. The text kenon edits for it is the `override` lines of the
//! manifest. The manifest is read and written through [`Project`], and every
//! line kenon does not own is kept.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use crate::modopts::SettingsCtx;
use crate::project::{Manifest, Project, KENON_FILE, MANIFEST, ORDER_LIMIT};

/// The newest interface version the port is built against (eidos
/// `sdk/mod_api.h`, `BW_MOD_API_VERSION`): 4 adds WebAssembly code.
pub const API_VERSION: u32 = 4;
/// Every interface version the port accepts. It refuses a manifest that
/// states another.
pub const API_VERSIONS: &[u32] = &[3, 4];
/// The rule-language version the port reads.
pub const RULE_LANGUAGE: i64 = 1;
/// The longest namespace the saved-data layer accepts.
const NAMESPACE_MAX: usize = 64;
/// Where new rule files go.
pub const RULES_DIR: &str = "rules";
/// The extension of a rule file.
pub const RULE_EXT: &str = "rule";

// ---------------------------------------------------------------------
// Diagnostics
// ---------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
}

/// One problem. `line` is 1-based, 0 for a problem with a whole file or with
/// What a rule is checked against besides the language: the mod's tables and
/// its options. A `None` part is not checked.
#[derive(Debug, Clone, Copy, Default)]
pub struct RuleContext<'a> {
    pub tables: Option<&'a TableSet>,
    pub settings: Option<&'a SettingsCtx>,
}

/// a form field (then `field` names it). `start` and `end` are 1-based
/// columns, `end` exclusive.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostic {
    pub line: u32,
    pub start: u32,
    pub end: u32,
    pub severity: Severity,
    pub message: String,
    pub field: Option<String>,
}

impl Diagnostic {
    pub(crate) fn at(
        line: usize,
        text: &str,
        from: usize,
        to: usize,
        severity: Severity,
        msg: &str,
    ) -> Self {
        let col = |byte: usize| text[..byte.min(text.len())].chars().count() as u32 + 1;
        let start = col(from);
        Diagnostic {
            line: line as u32,
            start,
            end: col(to).max(start + 1),
            severity,
            message: msg.to_string(),
            field: None,
        }
    }

    pub(crate) fn field(field: &str, severity: Severity, msg: &str) -> Self {
        Diagnostic {
            line: 0,
            start: 0,
            end: 0,
            severity,
            message: msg.to_string(),
            field: Some(field.to_string()),
        }
    }
}

pub fn has_errors(list: &[Diagnostic]) -> bool {
    list.iter().any(|d| d.severity == Severity::Error)
}

// ---------------------------------------------------------------------
// The name tables
// ---------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FieldKind {
    Int,
    Mon,
    List,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleField {
    pub name: String,
    pub kind: FieldKind,
    /// A rule may write it.
    pub writable: bool,
    pub signed: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleEvent {
    pub name: String,
    /// A rule may `refuse` it.
    pub refusable: bool,
    pub fields: Vec<RuleField>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SubKind {
    /// No subscript.
    Plain,
    /// The subscript is a Pokemon (or a party position).
    Mon,
    /// The subscript is a number.
    Int,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StateField {
    pub name: String,
    pub sub: SubKind,
    pub writable: bool,
    pub readable: bool,
    /// The port can write it today.
    pub writable_ok: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Names {
    pub events: Vec<RuleEvent>,
    pub states: Vec<StateField>,
}

impl Names {
    pub fn event(&self, name: &str) -> Option<&RuleEvent> {
        self.events.iter().find(|e| e.name == name)
    }

    fn state(&self, name: &str) -> Option<&StateField> {
        self.states.iter().find(|s| s.name == name)
    }
}

/// The rule names, from `modnames/rules.txt` (eidos `sdk/mod_rule.c`).
pub fn rule_names() -> &'static Names {
    static NAMES: OnceLock<Names> = OnceLock::new();
    NAMES.get_or_init(|| {
        let mut names = Names {
            events: Vec::new(),
            states: Vec::new(),
        };
        for line in include_str!("modnames/rules.txt").lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            match parts.as_slice() {
                ["event", name, refusable] => names.events.push(RuleEvent {
                    name: name.to_string(),
                    refusable: *refusable == "1",
                    fields: Vec::new(),
                }),
                ["field", name, kind, flags] => {
                    if let Some(event) = names.events.last_mut() {
                        event.fields.push(RuleField {
                            name: name.to_string(),
                            kind: match *kind {
                                "mon" => FieldKind::Mon,
                                "list" => FieldKind::List,
                                _ => FieldKind::Int,
                            },
                            writable: flags.contains('m'),
                            signed: flags.contains('s'),
                        });
                    }
                }
                ["state", name, sub, writable, readable, ok] => names.states.push(StateField {
                    name: name.to_string(),
                    sub: match *sub {
                        "mon" => SubKind::Mon,
                        "int" => SubKind::Int,
                        _ => SubKind::Plain,
                    },
                    writable: *writable == "1",
                    readable: *readable == "1",
                    writable_ok: *ok == "1",
                }),
                _ => {}
            }
        }
        names
    })
}

// ---------------------------------------------------------------------
// The rule language
// ---------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ty {
    Int,
    Bool,
    Text,
    Mon,
}

impl Ty {
    fn is_number(self) -> bool {
        matches!(self, Ty::Int | Ty::Bool)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tk {
    End,
    Ident,
    Number,
    Str,
    LParen,
    RParen,
    LBracket,
    RBracket,
    Dot,
    Op,
}

#[derive(Debug, Clone)]
struct Tok {
    kind: Tk,
    start: usize,
    end: usize,
    text: String,
}

/// An error with the byte range it covers inside the scanned text.
struct PErr {
    start: usize,
    end: usize,
    msg: String,
}

type PResult<T> = Result<T, PErr>;

const DEPTH_MAX: u32 = 32;
const TEXT_MAX: usize = 256;
const LINE_MAX: usize = 4096;
const FILE_MAX: usize = 64 * 1024;
const GUARDS_MAX: usize = 32;
const ACTIONS_MAX: usize = 32;
const NAME_MAX: usize = 64;

struct Scan<'a> {
    s: &'a str,
    p: usize,
}

impl Scan<'_> {
    fn next(&mut self) -> PResult<Tok> {
        let b = self.s.as_bytes();
        while self.p < b.len() && matches!(b[self.p], b' ' | b'\t' | b'\r') {
            self.p += 1;
        }
        let start = self.p;
        let tok = |kind, end: usize, text: &str| Tok {
            kind,
            start,
            end,
            text: text.to_string(),
        };
        if self.p >= b.len() {
            return Ok(tok(Tk::End, start, ""));
        }
        let c = b[self.p];
        let err = |end: usize, msg: &str| PErr {
            start,
            end,
            msg: msg.to_string(),
        };
        if c.is_ascii_alphabetic() || c == b'_' {
            while self.p < b.len() && (b[self.p].is_ascii_alphanumeric() || b[self.p] == b'_') {
                self.p += 1;
            }
            if self.p - start >= TEXT_MAX {
                return Err(err(self.p, "the name is too long"));
            }
            return Ok(tok(Tk::Ident, self.p, &self.s[start..self.p]));
        }
        if c.is_ascii_digit() {
            let mut v: u64 = 0;
            while self.p < b.len() && b[self.p].is_ascii_digit() {
                v = v * 10 + u64::from(b[self.p] - b'0');
                if v > 2_147_483_647 {
                    return Err(err(self.p + 1, "the number is out of range"));
                }
                self.p += 1;
            }
            return Ok(tok(Tk::Number, self.p, &self.s[start..self.p]));
        }
        if c == b'"' {
            self.p += 1;
            let mut out = String::new();
            let mut chars = self.s[self.p..].char_indices();
            loop {
                let Some((i, ch)) = chars.next() else {
                    return Err(err(self.s.len(), "the text is not closed"));
                };
                if ch == '"' {
                    self.p += i + 1;
                    break;
                }
                let ch = if ch == '\\' {
                    match chars.next() {
                        Some((_, 'n')) => '\n',
                        Some((_, 't')) => '\t',
                        Some((_, other)) => other,
                        None => '\\',
                    }
                } else {
                    ch
                };
                out.push(ch);
                if out.len() >= TEXT_MAX {
                    return Err(err(self.p + i + 1, "the text is too long"));
                }
            }
            return Ok(tok(Tk::Str, self.p, &out));
        }
        let single = match c {
            b'(' => Some(Tk::LParen),
            b')' => Some(Tk::RParen),
            b'[' => Some(Tk::LBracket),
            b']' => Some(Tk::RBracket),
            b'.' => Some(Tk::Dot),
            _ => None,
        };
        if let Some(kind) = single {
            self.p += 1;
            return Ok(tok(kind, self.p, ""));
        }
        // Operators: at most two characters from the operator set.
        let is_op = |ch: u8| b"+-*/%<>=!&|".contains(&ch);
        let mut n = 0;
        while self.p + n < b.len() && n < 2 && is_op(b[self.p + n]) {
            n += 1;
        }
        if n == 0 {
            let ch = self.s[start..].chars().next().unwrap_or('?');
            return Err(err(
                start + ch.len_utf8(),
                &format!("the character '{ch}' is not in the language"),
            ));
        }
        // A pair that is not an operator reads as two: the first character
        // stands alone and the second starts the next token, so `x*-1` and
        // `a<-1` mean what they look like.
        if n == 2 && !["==", "!=", "<=", ">=", "&&", "||"].contains(&&self.s[start..start + 2]) {
            n = 1;
        }
        let op = &self.s[start..start + n];
        self.p += n;
        if n == 2 {
            return Ok(tok(Tk::Op, self.p, op));
        }
        match op {
            "=" => Err(err(self.p, "'=' is not an operator; the language has '=='")),
            "&" | "|" => Err(err(
                self.p,
                &format!("'{op}' is not an operator; the language has '{op}{op}'"),
            )),
            _ => Ok(tok(Tk::Op, self.p, op)),
        }
    }
}

/// The tables a mod declares: each name, and whether its file loads. Eidos
/// refuses a rule that reads a table the mod does not have, and a table
/// that fails to load is not there.
pub type TableSet = BTreeMap<String, bool>;

struct Parser<'a> {
    scan: Scan<'a>,
    tok: Tok,
    event: Option<&'a RuleEvent>,
    names: &'static Names,
    ctx: RuleContext<'a>,
    /// Problems that do not stop the parse.
    warns: Vec<PErr>,
    depth: u32,
}

impl<'a> Parser<'a> {
    fn new(
        s: &'a str,
        event: Option<&'a RuleEvent>,
        names: &'static Names,
        ctx: RuleContext<'a>,
    ) -> PResult<Parser<'a>> {
        let mut scan = Scan { s, p: 0 };
        let tok = scan.next()?;
        Ok(Parser {
            scan,
            tok,
            event,
            names,
            ctx,
            warns: Vec::new(),
            depth: 0,
        })
    }

    fn advance(&mut self) -> PResult<()> {
        self.tok = self.scan.next()?;
        Ok(())
    }

    fn fail<T>(&self, msg: &str) -> PResult<T> {
        Err(PErr {
            start: self.tok.start,
            end: self.tok.end.max(self.tok.start + 1),
            msg: msg.to_string(),
        })
    }

    fn is_op(&self, op: &str) -> bool {
        self.tok.kind == Tk::Op && self.tok.text == op
    }

    fn is_ident(&self, id: &str) -> bool {
        self.tok.kind == Tk::Ident && self.tok.text == id
    }

    fn expr(&mut self) -> PResult<Ty> {
        if self.depth >= DEPTH_MAX {
            return self.fail("the expression is too deep");
        }
        self.depth += 1;
        let r = self.or();
        self.depth -= 1;
        r
    }

    fn or(&mut self) -> PResult<Ty> {
        let mut left = self.and()?;
        while self.is_op("||") {
            if !left.is_number() {
                return self.fail("'||' needs true or false");
            }
            self.advance()?;
            let right = self.and()?;
            if !right.is_number() {
                return self.fail("'||' needs true or false");
            }
            left = Ty::Bool;
        }
        Ok(left)
    }

    fn and(&mut self) -> PResult<Ty> {
        let mut left = self.not()?;
        while self.is_op("&&") {
            if !left.is_number() {
                return self.fail("'&&' needs true or false");
            }
            self.advance()?;
            let right = self.not()?;
            if !right.is_number() {
                return self.fail("'&&' needs true or false");
            }
            left = Ty::Bool;
        }
        Ok(left)
    }

    fn not(&mut self) -> PResult<Ty> {
        if !self.is_op("!") {
            return self.cmp();
        }
        self.advance()?;
        let t = self.cmp()?;
        if !t.is_number() {
            return self.fail("'!' needs true or false");
        }
        Ok(Ty::Bool)
    }

    fn cmp(&mut self) -> PResult<Ty> {
        let left = self.sum()?;
        if !["==", "!=", "<", "<=", ">", ">="]
            .iter()
            .any(|o| self.is_op(o))
        {
            return Ok(left);
        }
        self.advance()?;
        let right = self.sum()?;
        if left != right && !(left.is_number() && right.is_number()) {
            return self.fail("the two sides of the comparison have different types");
        }
        Ok(Ty::Bool)
    }

    fn sum(&mut self) -> PResult<Ty> {
        let mut left = self.product()?;
        while self.is_op("+") || self.is_op("-") {
            self.advance()?;
            let right = self.product()?;
            if left != Ty::Int || right != Ty::Int {
                return self.fail("arithmetic needs numbers");
            }
            left = Ty::Int;
        }
        Ok(left)
    }

    fn product(&mut self) -> PResult<Ty> {
        let mut left = self.unary()?;
        while self.is_op("*") || self.is_op("/") || self.is_op("%") {
            self.advance()?;
            let right = self.unary()?;
            if left != Ty::Int || right != Ty::Int {
                return self.fail("arithmetic needs numbers");
            }
            left = Ty::Int;
        }
        Ok(left)
    }

    fn unary(&mut self) -> PResult<Ty> {
        if self.is_op("-") {
            self.advance()?;
            let t = self.atom()?;
            if t != Ty::Int {
                return self.fail("'-' needs a number");
            }
            return Ok(Ty::Int);
        }
        self.atom()
    }

    fn atom(&mut self) -> PResult<Ty> {
        match self.tok.kind {
            Tk::Number => {
                self.advance()?;
                Ok(Ty::Int)
            }
            Tk::Str => {
                self.advance()?;
                Ok(Ty::Text)
            }
            Tk::Ident if self.tok.text == "true" || self.tok.text == "false" => {
                self.advance()?;
                Ok(Ty::Bool)
            }
            Tk::Ident => self.name(),
            Tk::LParen => {
                self.advance()?;
                let t = self.expr()?;
                if self.tok.kind != Tk::RParen {
                    return self.fail("the parenthesis is not closed");
                }
                self.advance()?;
                Ok(t)
            }
            _ => self.fail("a value was expected"),
        }
    }

    /// `[ <expr> ]`, or nothing.
    fn subscript(&mut self) -> PResult<Option<Ty>> {
        if self.tok.kind != Tk::LBracket {
            return Ok(None);
        }
        self.advance()?;
        let t = self.expr()?;
        if self.tok.kind != Tk::RBracket {
            return self.fail("the subscript is not closed");
        }
        self.advance()?;
        Ok(Some(t))
    }

    fn expect_dot(&mut self, after: &str) -> PResult<()> {
        self.advance()?;
        if self.tok.kind != Tk::Dot {
            return self.fail(&format!("a '.' was expected after '{after}'"));
        }
        self.advance()
    }

    fn event_field(&mut self, write: bool) -> PResult<Ty> {
        self.expect_dot("event")?;
        if self.tok.kind != Tk::Ident {
            return self.fail("a payload field's name was expected");
        }
        let name = self.tok.text.clone();
        let Some(event) = self.event else {
            // The event is unknown (already reported): accept any field.
            self.advance()?;
            return Ok(Ty::Int);
        };
        let Some(field) = event.fields.iter().find(|f| f.name == name) else {
            return self.fail(&format!("the event {} has no field '{}'", event.name, name));
        };
        if write {
            if !field.writable {
                return self.fail(&format!("the field '{name}' cannot be changed by a rule"));
            }
        } else if field.kind == FieldKind::List {
            return self.fail(&format!(
                "the field '{name}' is a list, and a rule cannot read it"
            ));
        }
        let ty = if field.kind == FieldKind::Mon {
            Ty::Mon
        } else {
            Ty::Int
        };
        self.advance()?;
        Ok(ty)
    }

    fn cell(&mut self) -> PResult<()> {
        self.expect_dot("state")?;
        if self.tok.kind != Tk::Ident {
            return self.fail("a cell's name was expected");
        }
        self.advance()?;
        if let Some(t) = self.subscript()? {
            if t != Ty::Int {
                return self.fail("a cell's key must be a number");
            }
        }
        Ok(())
    }

    /// A state field with its subscript, read or written.
    fn state_field(&mut self, write: bool) -> PResult<()> {
        let name = self.tok.text.clone();
        let names = self.names;
        let Some(sf) = names.state(&name) else {
            return self.fail(&format!("'{name}' is not a name of the language"));
        };
        if write {
            if !sf.writable {
                return self.fail(&format!("'{name}' cannot be changed by a rule"));
            }
            if !sf.writable_ok {
                return self.fail(&format!(
                    "the port cannot write '{name}' yet, so no rule may write it"
                ));
            }
        } else if !sf.readable {
            return self.fail(&format!(
                "the port cannot read '{name}' yet, so no rule may use it"
            ));
        }
        let at = self.tok.clone();
        self.advance()?;
        let sub = self.subscript()?;
        let bad = |msg: String| PErr {
            start: at.start,
            end: at.end,
            msg,
        };
        match (sf.sub, sub) {
            (SubKind::Plain, Some(_)) => Err(bad(format!("'{name}' takes no subscript"))),
            (SubKind::Plain, None) => Ok(()),
            (_, None) => Err(bad(format!("'{name}' needs a subscript"))),
            (SubKind::Mon, Some(t)) if t != Ty::Mon && t != Ty::Int => {
                Err(bad(format!("'{name}' takes a Pokemon or a party position")))
            }
            (SubKind::Int, Some(t)) if t != Ty::Int => Err(bad(format!("'{name}' takes a number"))),
            _ => Ok(()),
        }
    }

    fn name(&mut self) -> PResult<Ty> {
        if self.is_ident("event") {
            return self.event_field(false);
        }
        if self.is_ident("state") {
            self.cell()?;
            return Ok(Ty::Int);
        }
        if self.is_ident("setting") {
            // setting.<key>: one of the mod's own options, read when the rule runs.
            self.advance()?;
            if self.tok.kind != Tk::Dot {
                return self.fail("a '.' was expected after 'setting'");
            }
            self.advance()?;
            if self.tok.kind != Tk::Ident {
                return self.fail("a setting's key was expected");
            }
            // A key may hold dots (`setting.nuzlocke.dupes`): the words are
            // joined back with them.
            let mut at = self.tok.clone();
            let mut key = at.text.clone();
            self.advance()?;
            while self.tok.kind == Tk::Dot {
                self.advance()?;
                if self.tok.kind != Tk::Ident {
                    return self.fail("a word was expected after '.' in the setting's key");
                }
                key.push('.');
                key.push_str(&self.tok.text);
                at.end = self.tok.end;
                self.advance()?;
            }
            if let Some(s) = self.ctx.settings {
                let err = |msg: String| PErr {
                    start: at.start,
                    end: at.end,
                    msg,
                };
                if !s.namespace {
                    return Err(err(
                        "the rule has no namespace, so it has no settings".to_string()
                    ));
                }
                if !s.keys.contains(&key) {
                    self.warns.push(err(format!(
                        "the mod declares no option '{key}' (an `option` line in its options file); it reads -1"
                    )));
                }
            }
            return Ok(Ty::Int);
        }
        if self.is_ident("table") {
            return self.table();
        }
        self.state_field(false)?;
        Ok(Ty::Int)
    }

    /// `table.<name>[key]`: a value of one of the mod's own tables.
    fn table(&mut self) -> PResult<Ty> {
        self.expect_dot("table")?;
        if self.tok.kind != Tk::Ident {
            return self.fail("a table's name was expected");
        }
        let at = self.tok.clone();
        let name = at.text.clone();
        let bad = |msg: String| PErr {
            start: at.start,
            end: at.end,
            msg,
        };
        match self.ctx.tables.and_then(|t| t.get(&name)) {
            None if self.ctx.tables.is_some() => {
                return Err(bad(format!(
                    "the mod has no table '{name}' (a `table` line in its manifest)"
                )));
            }
            Some(false) => {
                return Err(bad(format!(
                    "the mod's table '{name}' does not load, so the port refuses this rule"
                )));
            }
            _ => {}
        }
        self.advance()?;
        match self.subscript()? {
            None => Err(bad(
                "a table is read with a key: table.<name>[key]".to_string()
            )),
            Some(t) if !t.is_number() => self.fail("a table's key must be a number"),
            Some(_) => Ok(Ty::Int),
        }
    }

    fn target(&mut self) -> PResult<()> {
        if self.tok.kind != Tk::Ident {
            return self.fail("a target was expected");
        }
        if self.is_ident("event") {
            self.event_field(true)?;
            return Ok(());
        }
        if self.is_ident("state") {
            return self.cell();
        }
        self.state_field(true)
    }

    fn at_end(&self, msg: &str) -> PResult<()> {
        if self.tok.kind != Tk::End {
            return self.fail(msg);
        }
        Ok(())
    }

    fn guard(&mut self) -> PResult<()> {
        let t = self.expr()?;
        self.at_end("the guard has more than it needs")?;
        if !t.is_number() {
            return self.fail("a guard must be true or false");
        }
        Ok(())
    }

    /// One action. `refusable` is whether the event can be refused (true
    /// when the event is unknown, so a wrong event name gives one error).
    fn action(&mut self, keyword: &str, refusable: bool) -> PResult<()> {
        match keyword {
            "refuse" => {
                self.at_end("'refuse' takes nothing")?;
                if !refusable {
                    let name = self.event.map_or("", |e| e.name.as_str());
                    return Err(PErr {
                        start: 0,
                        end: 0,
                        msg: format!("the event {name} cannot be refused"),
                    });
                }
                Ok(())
            }
            "set" | "add" | "sub" => {
                self.target()?;
                let t = self.expr()?;
                self.at_end("the action has more than it needs")?;
                if !t.is_number() {
                    return self.fail("the value must be a number");
                }
                Ok(())
            }
            "message" | "send" => {
                let t = self.expr()?;
                self.at_end("the action has more than it needs")?;
                if t != Ty::Text && t != Ty::Int {
                    return self.fail("the value must be a text or a number");
                }
                Ok(())
            }
            "release" => {
                let t = self.expr()?;
                self.at_end("the action has more than it needs")?;
                if t != Ty::Mon && t != Ty::Int {
                    return self.fail("the value must be a Pokemon or a party position");
                }
                Ok(())
            }
            "box" => {
                let mon = self.expr()?;
                let n = self.expr()?;
                self.at_end("the action has more than it needs")?;
                if (mon != Ty::Mon && mon != Ty::Int) || !n.is_number() {
                    return self
                        .fail("the action needs a Pokemon or a party position, and a box number");
                }
                Ok(())
            }
            "withdraw" => {
                let box_no = self.expr()?;
                let slot = self.expr()?;
                self.at_end("the action has more than it needs")?;
                if !box_no.is_number() || !slot.is_number() {
                    return self.fail("the box and the slot must be numbers");
                }
                Ok(())
            }
            "give" => {
                let item = self.expr()?;
                let count = self.expr()?;
                self.at_end("the action has more than it needs")?;
                if !item.is_number() || !count.is_number() {
                    return self.fail("the item and the count must be numbers");
                }
                Ok(())
            }
            _ => Err(PErr {
                start: 0,
                end: 0,
                msg: format!("'{keyword}' is not an action of the language"),
            }),
        }
    }
}

/// A line of a rule file without its comment and outer blanks, and where it
/// starts in the original line. The port cuts at the first `#`, even inside
/// a text.
fn trim_line(line: &str) -> (usize, &str) {
    let cut = line.find('#').unwrap_or(line.len());
    let body = &line[..cut];
    let lead = body.len() - body.trim_start_matches([' ', '\t']).len();
    (lead, body.trim_matches([' ', '\t', '\r']))
}

/// The first word of a trimmed line and the rest of it (with its offset).
fn split_word(s: &str) -> (&str, usize) {
    let end = s.find([' ', '\t']).unwrap_or(s.len());
    (&s[..end], end)
}

/// The event a rule file names in its `rule <name> on <event>` line, if the
/// file has one.
pub fn rule_event(text: &str) -> Option<String> {
    let mut seen_language = false;
    for line in text.lines() {
        let (_, body) = trim_line(line);
        if body.is_empty() {
            continue;
        }
        let (word, at) = split_word(body);
        if !seen_language {
            seen_language = true;
            if word != "language" {
                return None;
            }
            continue;
        }
        if word != "rule" {
            return None;
        }
        let mut parts = body[at..].split_whitespace();
        let _name = parts.next()?;
        if parts.next()? != "on" {
            return None;
        }
        return parts.next().map(str::to_string);
    }
    None
}

/// Checks a rule file the way the port does when it loads it. `expected` is
/// the event the manifest binds the file to, when known.
pub fn check_rule(text: &str, expected: Option<&str>) -> Vec<Diagnostic> {
    check_rule_in(text, expected, None)
}

/// [`check_rule`] for a mod that declares `tables`: a rule that reads another
/// table is an error. With `None` the table names are not checked.
pub fn check_rule_in(
    text: &str,
    expected: Option<&str>,
    tables: Option<&TableSet>,
) -> Vec<Diagnostic> {
    let ctx = RuleContext {
        tables,
        settings: None,
    };
    check_rule_ctx(text, expected, ctx)
}

/// [`check_rule_in`] with the mod's options too: a `setting.<key>` the
/// options do not declare is a warning (it reads -1), and a mod with no
/// namespace has no settings, which the port refuses.
pub fn check_rule_ctx(text: &str, expected: Option<&str>, ctx: RuleContext) -> Vec<Diagnostic> {
    let names = rule_names();
    let mut out = Vec::new();
    let err = |out: &mut Vec<Diagnostic>, n: usize, line: &str, a: usize, b: usize, m: &str| {
        out.push(Diagnostic::at(n, line, a, b, Severity::Error, m));
    };
    if text.len() > FILE_MAX {
        out.push(Diagnostic::field(
            "file",
            Severity::Error,
            "the file is larger than the port reads (64 KiB)",
        ));
    }
    let mut have_language = false;
    let mut have_rule = false;
    let mut event: Option<&RuleEvent> = None;
    let (mut guards, mut actions, mut any) = (0usize, 0usize, false);
    let mut last_line = 1usize;
    for (i, line) in text.lines().enumerate() {
        let n = i + 1;
        last_line = n;
        let (lead, body) = trim_line(line);
        if body.is_empty() {
            continue;
        }
        any = true;
        let whole = (lead, lead + body.len());
        if body.len() >= LINE_MAX {
            err(&mut out, n, line, whole.0, whole.1, "the line is too long");
            continue;
        }
        let (word, at) = split_word(body);
        let rest = &body[at..];
        let rest_at = lead + at;
        if !have_language {
            have_language = true;
            if word != "language" {
                err(
                    &mut out,
                    n,
                    line,
                    lead,
                    lead + word.len(),
                    &format!("the file must start with 'language {RULE_LANGUAGE}'"),
                );
                // Treat the line as the next one, so the rest is still read.
                have_rule = false;
            } else {
                let digits = rest.trim_start();
                let sign = usize::from(digits.starts_with(['-', '+']));
                let len = digits[sign..]
                    .find(|c: char| !c.is_ascii_digit())
                    .unwrap_or(digits.len() - sign);
                match digits[..sign + len].parse::<i64>() {
                    Err(_) => err(
                        &mut out,
                        n,
                        line,
                        rest_at,
                        whole.1,
                        "the language version is not a number",
                    ),
                    Ok(v) if v != RULE_LANGUAGE => err(
                        &mut out,
                        n,
                        line,
                        rest_at,
                        whole.1,
                        &format!(
                            "the rule is written for the language version {v}, and the port reads version {RULE_LANGUAGE}"
                        ),
                    ),
                    Ok(_) => {}
                }
                continue;
            }
        }
        if !have_rule {
            have_rule = true;
            if word != "rule" {
                err(
                    &mut out,
                    n,
                    line,
                    lead,
                    lead + word.len(),
                    "the file must name its rule with 'rule <name> on <event>'",
                );
                continue;
            }
            let mut parts = rest.split_whitespace();
            let name = parts.next();
            let Some(name) = name else {
                err(&mut out, n, line, lead, whole.1, "the rule has no name");
                continue;
            };
            if name.len() >= NAME_MAX {
                err(
                    &mut out,
                    n,
                    line,
                    lead,
                    whole.1,
                    "the rule's name is too long",
                );
            }
            if parts.next() != Some("on") {
                err(
                    &mut out,
                    n,
                    line,
                    lead,
                    whole.1,
                    "the rule must name its event: 'rule <name> on <event>'",
                );
                continue;
            }
            let Some(ev) = parts.next() else {
                err(&mut out, n, line, lead, whole.1, "the rule has no event");
                continue;
            };
            let ev_at = body.rfind(ev).map_or(lead, |p| lead + p);
            let ev_end = ev_at + ev.len();
            match names.event(ev) {
                None => err(
                    &mut out,
                    n,
                    line,
                    ev_at,
                    ev_end,
                    &format!("'{ev}' is not one of the port's events"),
                ),
                Some(found) => {
                    event = Some(found);
                    if let Some(want) = expected {
                        if want != ev {
                            err(
                                &mut out,
                                n,
                                line,
                                ev_at,
                                ev_end,
                                &format!(
                                    "the rule is written for the event {ev}, and the manifest names {want}"
                                ),
                            );
                        }
                    }
                }
            }
            continue;
        }
        match word {
            "when" => {
                guards += 1;
                if guards > GUARDS_MAX {
                    err(
                        &mut out,
                        n,
                        line,
                        lead,
                        whole.1,
                        "the rule has too many guards",
                    );
                    continue;
                }
                let mut warns = Vec::new();
                let r = Parser::new(rest, event, names, ctx).and_then(|mut p| {
                    let r = p.guard();
                    warns = std::mem::take(&mut p.warns);
                    r
                });
                for w in warns {
                    out.push(Diagnostic::at(
                        n,
                        line,
                        rest_at + w.start,
                        rest_at + w.end,
                        Severity::Warning,
                        &w.msg,
                    ));
                }
                if let Err(e) = r {
                    err(
                        &mut out,
                        n,
                        line,
                        rest_at + e.start,
                        rest_at + e.end,
                        &e.msg,
                    );
                }
            }
            "do" => {
                let (kw_lead, kw_rest) = {
                    let t = rest.trim_start_matches([' ', '\t']);
                    (rest.len() - t.len(), t)
                };
                let (kw, kw_end) = split_word(kw_rest);
                if kw.is_empty() {
                    err(&mut out, n, line, lead, whole.1, "the action is missing");
                    continue;
                }
                actions += 1;
                if actions > ACTIONS_MAX {
                    err(
                        &mut out,
                        n,
                        line,
                        lead,
                        whole.1,
                        "the rule has too many actions",
                    );
                    continue;
                }
                let args = &kw_rest[kw_end..];
                let args_at = rest_at + kw_lead + kw_end;
                let mut warns = Vec::new();
                let r = Parser::new(args, event, names, ctx).and_then(|mut p| {
                    let r = p.action(kw, event.is_none_or(|e| e.refusable));
                    warns = std::mem::take(&mut p.warns);
                    r
                });
                for w in warns {
                    out.push(Diagnostic::at(
                        n,
                        line,
                        args_at + w.start,
                        args_at + w.end,
                        Severity::Warning,
                        &w.msg,
                    ));
                }
                if let Err(e) = r {
                    let (a, b) = if e.start == 0 && e.end == 0 {
                        (rest_at + kw_lead, rest_at + kw_lead + kw.len())
                    } else {
                        (args_at + e.start, args_at + e.end)
                    };
                    err(&mut out, n, line, a, b, &e.msg);
                }
            }
            _ => err(
                &mut out,
                n,
                line,
                lead,
                lead + word.len(),
                &format!("'{word}' is not a line of a rule"),
            ),
        }
    }
    if !any {
        out.push(Diagnostic::field(
            "file",
            Severity::Error,
            "the file is empty",
        ));
        out.last_mut().unwrap().line = 1;
    } else if have_rule && event.is_some() && actions == 0 {
        out.push(Diagnostic {
            line: last_line as u32,
            start: 1,
            end: 2,
            severity: Severity::Error,
            message: "the rule has no action".to_string(),
            field: None,
        });
    } else if !have_rule {
        out.push(Diagnostic {
            line: last_line as u32,
            start: 1,
            end: 2,
            severity: Severity::Error,
            message: "the file names no rule".to_string(),
            field: None,
        });
    }
    out
}

// ---------------------------------------------------------------------
// The declared set and the override lines
// ---------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DeclaredRow {
    pub function: String,
    pub events: Vec<String>,
}

/// The game functions a mod can hook: eidos `mod/declared_set.txt`.
#[derive(Debug, Clone, Default)]
pub struct DeclaredSet {
    rows: HashMap<(String, u32), DeclaredRow>,
}

/// `arm9`, or `ov<n>` with the number written without leading zeros.
fn region_key(region: &str) -> Option<String> {
    if region == "arm9" {
        return Some(region.to_string());
    }
    let n = region.strip_prefix("ov")?;
    if n.is_empty() || !n.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let v: u32 = n.parse().ok()?;
    (v <= 0xFFFF).then(|| format!("ov{v}"))
}

fn parse_addr(s: &str) -> Option<u32> {
    let digits = s
        .strip_prefix("0x")
        .or_else(|| s.strip_prefix("0X"))
        .unwrap_or(s);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    u32::from_str_radix(digits, 16).ok()
}

impl DeclaredSet {
    pub fn parse(text: &str) -> Self {
        let mut rows = HashMap::new();
        for line in text.lines() {
            let line = line.split('#').next().unwrap_or("");
            let cols: Vec<&str> = line.split('\t').collect();
            if cols.len() < 3 {
                continue;
            }
            let (Some(addr), Some(region)) =
                (parse_addr(cols[0].trim()), region_key(cols[2].trim()))
            else {
                continue;
            };
            let events = cols
                .get(3)
                .map(|e| {
                    e.split(',')
                        .map(str::trim)
                        .filter(|e| !e.is_empty())
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_default();
            rows.insert(
                (region, addr & !1),
                DeclaredRow {
                    function: cols[1].trim().to_string(),
                    events,
                },
            );
        }
        DeclaredSet { rows }
    }

    pub fn load(path: &Path) -> io::Result<Self> {
        Ok(Self::parse(&fs::read_to_string(path)?))
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    pub fn find(&self, region: &str, addr: u32) -> Option<&DeclaredRow> {
        self.rows.get(&(region_key(region)?, addr & !1))
    }

    /// Every row as `(region, address, row)`, sorted, for completion.
    pub fn rows(&self) -> Vec<(String, u32, &DeclaredRow)> {
        let mut v: Vec<_> = self
            .rows
            .iter()
            .map(|((r, a), row)| (r.clone(), *a, row))
            .collect();
        v.sort_by(|a, b| (&a.0, a.1).cmp(&(&b.0, b.1)));
        v
    }
}

/// Where eidos keeps its declared set, found from the mods folder (the mods
/// folder sits in the eidos folder, or beside its `mod/` folder).
pub fn declared_set_near(mods_dir: &Path) -> Option<PathBuf> {
    let mut candidates = vec![mods_dir.join("declared_set.txt")];
    if let Some(parent) = mods_dir.parent() {
        candidates.push(parent.join("mod").join("declared_set.txt"));
    }
    candidates.into_iter().find(|p| p.is_file())
}

/// Splits a value into blank-separated tokens; a quoted part may hold
/// blanks. At most `max` tokens, as the port reads them. Each token is
/// `(start, end)` in bytes.
fn tokenize(s: &str, max: usize) -> Vec<(usize, usize)> {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    let blank = |c: u8| matches!(c, b' ' | b'\t' | b'\r' | b'\n');
    while i < b.len() && out.len() < max {
        while i < b.len() && blank(b[i]) {
            i += 1;
        }
        if i >= b.len() {
            break;
        }
        let start = i;
        while i < b.len() {
            if b[i] == b'"' {
                i += 1;
                while i < b.len() && b[i] != b'"' {
                    i += 1;
                }
                i = (i + 1).min(b.len());
                continue;
            }
            if blank(b[i]) {
                break;
            }
            i += 1;
        }
        out.push((start, i));
    }
    out
}

fn is_ident(s: &str) -> bool {
    let mut chars = s.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// One override line, parsed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverrideLine {
    pub region: String,
    pub addr: u32,
    pub mode: String,
    pub symbol: String,
    pub sig: Option<String>,
    pub switch: Option<String>,
}

impl OverrideLine {
    /// The key a conflict is found on: `ov93:0x021CBA40`.
    pub fn key(&self) -> String {
        format!("{}:0x{:08X}", self.region, self.addr & !1)
    }
}

/// Parses an override value (`<region>:<address> <mode> <symbol> ...`) the
/// way the port does, or says what is wrong and where (byte range).
fn parse_override(value: &str) -> Result<OverrideLine, (usize, usize, String)> {
    let toks = tokenize(value, 6);
    let whole = (0, value.len());
    if toks.len() < 3 {
        return Err((
            whole.0,
            whole.1,
            "an override needs '<region>:<address> <mode> <symbol>'".to_string(),
        ));
    }
    let t = |i: usize| &value[toks[i].0..toks[i].1];
    let (key_text, mode, symbol) = (t(0), t(1), t(2));
    let Some((region_text, addr_text)) = key_text.split_once(':') else {
        return Err((
            toks[0].0,
            toks[0].1,
            "a bare address is refused: write '<region>:<address>', for example ov93:0x021CBA40"
                .to_string(),
        ));
    };
    let Some(region) = region_key(region_text) else {
        return Err((
            toks[0].0,
            toks[0].0 + region_text.len(),
            "the region is 'arm9' or 'ov' and an overlay number".to_string(),
        ));
    };
    let Some(addr) = parse_addr(addr_text) else {
        return Err((
            toks[0].0 + region_text.len() + 1,
            toks[0].1,
            "the address is hexadecimal, up to 32 bits".to_string(),
        ));
    };
    if mode != "replace" && mode != "wrap" {
        return Err((
            toks[1].0,
            toks[1].1,
            "the mode is 'replace' or 'wrap'".to_string(),
        ));
    }
    if !is_ident(symbol) {
        return Err((
            toks[2].0,
            toks[2].1,
            "the symbol is a C name: letters, digits and '_'".to_string(),
        ));
    }
    let mut line = OverrideLine {
        region,
        addr,
        mode: mode.to_string(),
        symbol: symbol.to_string(),
        sig: None,
        switch: None,
    };
    for &(a, b) in &toks[3..] {
        let tok = &value[a..b];
        let unquote = |v: &str| {
            v.strip_prefix('"')
                .and_then(|v| v.strip_suffix('"'))
                .unwrap_or(v)
                .to_string()
        };
        if let Some(v) = tok.strip_prefix("sig=").filter(|v| !v.is_empty()) {
            if line.sig.is_some() {
                return Err((a, b, "'sig=' is given twice".to_string()));
            }
            line.sig = Some(unquote(v));
        } else if let Some(v) = tok.strip_prefix("switch=").filter(|v| !v.is_empty()) {
            if line.switch.is_some() {
                return Err((a, b, "'switch=' is given twice".to_string()));
            }
            line.switch = Some(unquote(v));
        } else {
            return Err((
                a,
                b,
                "after the symbol only sig=\"<prototype>\" and switch=<name> are read".to_string(),
            ));
        }
    }
    Ok(line)
}

/// What the hooks editor shows beside a valid line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HookRow {
    pub line: u32,
    pub function: String,
    pub events: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct HooksCheck {
    pub diagnostics: Vec<Diagnostic>,
    pub rows: Vec<HookRow>,
}

/// Checks the override lines (one per line, without the `override =`). With
/// the declared set, an address that is not in it is an error, as the port
/// drops such an override at load.
pub fn check_overrides(text: &str, declared: Option<&DeclaredSet>) -> HooksCheck {
    let mut out = HooksCheck::default();
    let mut seen: HashMap<String, usize> = HashMap::new();
    for (i, line) in text.lines().enumerate() {
        let n = i + 1;
        let body = line.split('#').next().unwrap_or("");
        let lead = body.len() - body.trim_start().len();
        let value = body.trim();
        if value.is_empty() {
            continue;
        }
        let o = match parse_override(value) {
            Ok(o) => o,
            Err((a, b, msg)) => {
                out.diagnostics.push(Diagnostic::at(
                    n,
                    line,
                    lead + a,
                    lead + b,
                    Severity::Error,
                    &msg,
                ));
                continue;
            }
        };
        let key_end = lead + value.find(' ').unwrap_or(value.len());
        let key_start = lead;
        if let Some(prev) = seen.insert(o.key(), n) {
            out.diagnostics.push(Diagnostic::at(
                n,
                line,
                key_start,
                key_end,
                Severity::Warning,
                &format!("line {prev} already hooks this function; the later one wins"),
            ));
        }
        if let Some(sig) = &o.sig {
            if !sig.contains('(') || !sig.contains(')') {
                out.diagnostics.push(Diagnostic::at(
                    n,
                    line,
                    lead,
                    lead + value.len(),
                    Severity::Warning,
                    "sig= is '<return type> (<parameter types>)'",
                ));
            }
        }
        if let Some(set) = declared {
            match set.find(&o.region, o.addr) {
                None => out.diagnostics.push(Diagnostic::at(
                    n,
                    line,
                    key_start,
                    key_end,
                    Severity::Error,
                    &format!(
                        "{} is not a declared address; the port drops this override",
                        o.key()
                    ),
                )),
                Some(row) => out.rows.push(HookRow {
                    line: n as u32,
                    function: row.function.clone(),
                    events: row.events.clone(),
                }),
            }
        }
    }
    out
}

// ---------------------------------------------------------------------
// The manifest
// ---------------------------------------------------------------------

/// The keys of `eidos_mod.txt` kenon edits as a form. `override` and `rule`
/// lines have their own calls.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ModSettings {
    pub name: String,
    pub order: i32,
    /// Only a mod with code states one.
    pub api: Option<u32>,
    pub port: Option<String>,
    pub code: Option<String>,
    pub state: Option<String>,
    pub settings: Option<String>,
    /// The `table` lines. `write_settings` leaves them alone: the table
    /// calls own them.
    pub tables: Vec<TableDecl>,
}

/// `(key, value)` of a manifest line, comment cut, or `None`.
fn key_value(line: &str) -> Option<(&str, &str)> {
    let body = line.split('#').next().unwrap_or("");
    let (k, v) = body.split_once('=')?;
    Some((k.trim(), v.trim()))
}

fn other_values(manifest: &Manifest, key: &str) -> Vec<String> {
    manifest
        .other
        .iter()
        .filter_map(|l| key_value(l))
        .filter(|(k, _)| *k == key)
        .map(|(_, v)| v.to_string())
        .collect()
}

fn drop_keys(manifest: &mut Manifest, keys: &[&str]) {
    manifest
        .other
        .retain(|l| key_value(l).is_none_or(|(k, _)| !keys.contains(&k)));
}

fn project_of(root: &Path) -> io::Result<Project> {
    Project::open(root)
}

pub fn read_settings(root: &Path) -> io::Result<ModSettings> {
    let m = project_of(root)?.manifest;
    // A later line replaces an earlier one, as in the port.
    let last = |key: &str| other_values(&m, key).into_iter().rfind(|v| !v.is_empty());
    Ok(ModSettings {
        name: m.name.clone(),
        order: m.order,
        api: m.api,
        port: last("port"),
        code: last("code"),
        state: last("state"),
        settings: last("settings"),
        tables: table_decls(&m),
    })
}

fn namespace_valid(ns: &str) -> bool {
    if ns.is_empty() || ns.len() > NAMESPACE_MAX || ns == "." || ns == ".." {
        return false;
    }
    if !ns
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    {
        return false;
    }
    let stem = ns.split('.').next().unwrap_or("").to_ascii_uppercase();
    const RESERVED: [&str; 4] = ["CON", "PRN", "AUX", "NUL"];
    let numbered = |p: &str| {
        stem.strip_prefix(p)
            .is_some_and(|n| n.len() == 1 && n != "0" && n.bytes().all(|b| b.is_ascii_digit()))
    };
    !(RESERVED.contains(&stem.as_str()) || numbered("COM") || numbered("LPT"))
}

/// Checks the form the way the port reads the manifest. `overrides` is how
/// many `override` lines the mod has.
pub fn check_settings(s: &ModSettings, overrides: usize) -> Vec<Diagnostic> {
    use Severity::{Error, Warning};
    let mut out = Vec::new();
    if s.name.trim().is_empty() {
        out.push(Diagnostic::field(
            "name",
            Error,
            "an empty name is ignored; the folder name shows instead",
        ));
    }
    if s.order.abs() > ORDER_LIMIT {
        out.push(Diagnostic::field(
            "order",
            Error,
            "the load order is between -1000000 and 1000000",
        ));
    }
    let code = s.code.as_deref().is_some_and(|c| !c.is_empty());
    if code && s.api.is_none() {
        out.push(Diagnostic::field(
            "api",
            Error,
            "a mod with code states the api it is built against; the port refuses it otherwise",
        ));
    }
    if let Some(api) = s.api {
        if code && !API_VERSIONS.contains(&api) {
            out.push(Diagnostic::field(
                "api",
                Error,
                &format!("the port accepts api 3 and 4; it refuses a mod built for {api}"),
            ));
        }
    }
    if overrides > 0 && !code {
        out.push(Diagnostic::field(
            "code",
            Warning,
            "the hook lines name symbols of a code library, and the mod has none",
        ));
    }
    if overrides > 0 && s.port.is_none() {
        out.push(Diagnostic::field(
            "port",
            Warning,
            "a mod with hooks states the port version it is built against",
        ));
    }
    if let Some(state) = s.state.as_deref().filter(|v| !v.is_empty()) {
        if !namespace_valid(state) {
            out.push(Diagnostic::field(
                "state",
                Error,
                "the state name is one folder name: letters, digits, '-', '_' and '.' (64 at most); the port uses a name made from the mod name instead",
            ));
        }
    }
    let mut named: Vec<&str> = Vec::new();
    for t in &s.tables {
        let label = if t.name.is_empty() {
            "a table"
        } else {
            &t.name
        };
        if !table_name_valid(&t.name) {
            out.push(Diagnostic::field(
                "tables",
                Error,
                &format!(
                    "'{}' is not a table name: letters, digits and '_', not starting with a digit, {TABLE_NAME_MAX} at most; the port ignores the line",
                    t.name
                ),
            ));
        } else if t.path.is_empty() || t.path.contains(char::is_whitespace) {
            out.push(Diagnostic::field(
                "tables",
                Error,
                &format!(
                    "{label} needs one path: table = <name> <path>; the port ignores the line"
                ),
            ));
        } else if !is_relative(&t.path) {
            out.push(Diagnostic::field(
                "tables",
                Error,
                &format!("the path of {label} is a path inside the mod folder"),
            ));
        }
        if table_name_valid(&t.name) {
            if named.contains(&t.name.as_str()) {
                out.push(Diagnostic::field(
                    "tables",
                    Warning,
                    &format!(
                        "the table {} is declared twice; the later line wins",
                        t.name
                    ),
                ));
            }
            named.push(&t.name);
        }
    }
    for (field, value) in [("code", &s.code), ("settings", &s.settings)] {
        if let Some(v) = value.as_deref().filter(|v| !v.is_empty()) {
            if !is_relative(v) {
                out.push(Diagnostic::field(
                    field,
                    Error,
                    "this is a path inside the mod folder",
                ));
            }
        }
    }
    out
}

/// A path inside the mod: relative, without `..`.
fn is_relative(p: &str) -> bool {
    let path = Path::new(p);
    !p.is_empty()
        && !p.contains(':')
        && path
            .components()
            .all(|c| matches!(c, Component::Normal(_) | Component::CurDir))
}

fn bad_input(msg: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, msg)
}

/// Writes the form. `api` is kept only for a mod with code, since the port
/// checks it only then and refuses a wrong one. A line kenon does not own
/// stays as it is.
pub fn write_settings(root: &Path, s: &ModSettings) -> io::Result<()> {
    if s.name.trim().is_empty() {
        return Err(bad_input("the mod needs a name"));
    }
    if s.order.abs() > ORDER_LIMIT {
        return Err(bad_input("the load order is out of range"));
    }
    let mut project = project_of(root)?;
    let m = &mut project.manifest;
    m.name = s.name.trim().to_string();
    m.order = s.order;
    let code = s.code.as_deref().map(str::trim).filter(|c| !c.is_empty());
    m.api = if code.is_some() { s.api } else { None };
    drop_keys(m, &["port", "code", "state", "settings"]);
    let opt = |key: &str, v: Option<&str>, other: &mut Vec<String>| {
        if let Some(v) = v.map(str::trim).filter(|v| !v.is_empty()) {
            other.push(format!("{key} = {v}"));
        }
    };
    opt("port", s.port.as_deref(), &mut m.other);
    opt("code", code, &mut m.other);
    opt("state", s.state.as_deref(), &mut m.other);
    opt("settings", s.settings.as_deref(), &mut m.other);
    project.save_meta()
}

fn edit_manifest(root: &Path, f: impl FnOnce(&mut Manifest)) -> io::Result<()> {
    let mut project = project_of(root)?;
    f(&mut project.manifest);
    project.save_meta()
}

// ---------------------------------------------------------------------
// The hook lines
// ---------------------------------------------------------------------

/// The `override` lines of the manifest, one per line.
pub fn read_overrides(root: &Path) -> io::Result<String> {
    let m = project_of(root)?.manifest;
    let mut text = other_values(&m, "override").join("\n");
    if !text.is_empty() {
        text.push('\n');
    }
    Ok(text)
}

/// Replaces the `override` lines with the lines of `text`. Blank lines and
/// comments are dropped.
pub fn write_overrides(root: &Path, text: &str) -> io::Result<()> {
    let lines: Vec<String> = text
        .lines()
        .map(|l| l.split('#').next().unwrap_or("").trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();
    edit_manifest(root, |m| {
        drop_keys(m, &["override"]);
        m.other
            .extend(lines.iter().map(|l| format!("override = {l}")));
    })
}

// ---------------------------------------------------------------------
// The rule files
// ---------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleEntry {
    /// The path inside the mod, with `/`.
    pub path: String,
    /// The event the manifest binds it to.
    pub bound: Option<String>,
    /// The event the file names.
    pub file_event: Option<String>,
    pub exists: bool,
}

fn bindings(m: &Manifest) -> Vec<(String, String)> {
    other_values(m, "rule")
        .iter()
        .filter_map(|v| {
            let t = tokenize(v, 4);
            (t.len() == 2).then(|| (v[t[0].0..t[0].1].to_string(), v[t[1].0..t[1].1].to_string()))
        })
        .collect()
}

/// A mod-relative path that stays inside the mod.
fn safe_rel(rel: &str) -> io::Result<String> {
    let rel = rel.replace('\\', "/");
    if !is_relative(&rel) {
        return Err(bad_input("the path must be inside the mod folder"));
    }
    Ok(rel.trim_start_matches("./").to_string())
}

fn rule_files(dir: &Path, root: &Path, out: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            rule_files(&p, root, out);
        } else if p.extension().is_some_and(|x| x == RULE_EXT) {
            if let Ok(rel) = p.strip_prefix(root) {
                out.push(rel.to_string_lossy().replace('\\', "/"));
            }
        }
    }
}

/// The mod's rule files: the ones the manifest binds and the `.rule` files
/// under `rules/` it does not.
pub fn list_rules(root: &Path) -> io::Result<Vec<RuleEntry>> {
    let m = project_of(root)?.manifest;
    let mut found = Vec::new();
    rule_files(&root.join(RULES_DIR), root, &mut found);
    let bound = bindings(&m);
    let mut paths: BTreeSet<String> = found.into_iter().collect();
    paths.extend(bound.iter().map(|(_, p)| p.replace('\\', "/")));
    Ok(paths
        .into_iter()
        .map(|path| {
            let file = root.join(&path);
            let text = fs::read_to_string(&file).ok();
            RuleEntry {
                bound: bound
                    .iter()
                    .rev()
                    .find(|(_, p)| p.replace('\\', "/") == path)
                    .map(|(e, _)| e.clone()),
                file_event: text.as_deref().and_then(rule_event),
                exists: text.is_some(),
                path,
            }
        })
        .collect())
}

pub fn read_rule(root: &Path, path: &str) -> io::Result<String> {
    fs::read_to_string(root.join(safe_rel(path)?))
}

fn set_binding(m: &mut Manifest, path: &str, event: Option<&str>) {
    m.other.retain(|l| {
        let Some(("rule", v)) = key_value(l) else {
            return true;
        };
        let t = tokenize(v, 4);
        !(t.len() == 2 && v[t[1].0..t[1].1].replace('\\', "/") == path)
    });
    if let Some(ev) = event {
        m.other.push(format!("rule = {ev} {path}"));
    }
}

fn write_text(path: &Path, text: &str) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("rule.tmp");
    fs::write(&tmp, text)?;
    fs::rename(&tmp, path)
}

/// Writes a rule file and binds it to the event it names, so the manifest
/// and the file agree. A file that names no known event keeps its old
/// binding. Returns what the port would report for the file.
pub fn write_rule(root: &Path, path: &str, text: &str) -> io::Result<Vec<Diagnostic>> {
    let rel = safe_rel(path)?;
    write_text(&root.join(&rel), text)?;
    let event = rule_event(text).filter(|e| rule_names().event(e).is_some());
    if let Some(ev) = &event {
        edit_manifest(root, |m| set_binding(m, &rel, Some(ev)))?;
    }
    let tables = table_set(root)?;
    let settings = crate::modopts::settings_ctx(root)?;
    let ctx = RuleContext {
        tables: Some(&tables),
        settings: Some(&settings),
    };
    Ok(check_rule_ctx(text, event.as_deref(), ctx))
}

/// Makes a rule file in `rules/` for an event and binds it.
pub fn new_rule(root: &Path, name: &str, event: &str) -> io::Result<String> {
    if name.is_empty()
        || name.len() >= NAME_MAX
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(bad_input("a rule name is letters, digits, '-' and '_'"));
    }
    let Some(ev) = rule_names().event(event) else {
        return Err(bad_input("that is not one of the port's events"));
    };
    let rel = format!("{RULES_DIR}/{name}.{RULE_EXT}");
    if root.join(&rel).exists() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "a rule with that name exists",
        ));
    }
    let text = format!(
        "language {RULE_LANGUAGE}\nrule {name} on {}\n    # when <condition>\n    # do <action>\n",
        ev.name
    );
    write_text(&root.join(&rel), &text)?;
    edit_manifest(root, |m| set_binding(m, &rel, Some(&ev.name)))?;
    Ok(rel)
}

/// Moves a rule file and its binding.
pub fn rename_rule(root: &Path, from: &str, to: &str) -> io::Result<String> {
    let (from, mut to) = (safe_rel(from)?, safe_rel(to)?);
    if !to.ends_with(&format!(".{RULE_EXT}")) {
        to.push_str(&format!(".{RULE_EXT}"));
    }
    if from == to {
        return Ok(to);
    }
    if root.join(&to).exists() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "a file with that name exists",
        ));
    }
    if let Some(dir) = root.join(&to).parent() {
        fs::create_dir_all(dir)?;
    }
    if root.join(&from).is_file() {
        fs::rename(root.join(&from), root.join(&to))?;
    }
    edit_manifest(root, |m| {
        let event = bindings(m)
            .into_iter()
            .rev()
            .find(|(_, p)| p.replace('\\', "/") == from)
            .map(|(e, _)| e);
        set_binding(m, &from, None);
        if let Some(ev) = event {
            set_binding(m, &to, Some(&ev));
        }
    })?;
    Ok(to)
}

/// Deletes a rule file and its binding.
pub fn delete_rule(root: &Path, path: &str) -> io::Result<()> {
    let rel = safe_rel(path)?;
    let file = root.join(&rel);
    if file.is_file() {
        fs::remove_file(file)?;
    }
    edit_manifest(root, |m| set_binding(m, &rel, None))
}

// ---------------------------------------------------------------------
// The data tables
// ---------------------------------------------------------------------

/// The longest table name the port takes, in characters (it refuses 32).
pub const TABLE_NAME_MAX: usize = 31;
/// The most entries one table file holds.
pub const TABLE_ENTRIES_MAX: usize = 65536;
/// The largest table file the port reads.
pub const TABLE_FILE_MAX: usize = 16 * 1024 * 1024;
/// Where new table files go.
pub const TABLES_DIR: &str = "tables";
/// The extension of a new table file.
pub const TABLE_EXT: &str = "txt";

/// One `table = <name> <path>` line of the manifest.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TableDecl {
    pub name: String,
    pub path: String,
}

/// A declared table with whether its file is there.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableEntry {
    pub name: String,
    pub path: String,
    pub exists: bool,
}

/// A table name: letters, digits and `_`, not starting with a digit, up to
/// 31 characters.
pub fn table_name_valid(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= TABLE_NAME_MAX
        && !name.starts_with(|c: char| c.is_ascii_digit())
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// The `table` lines of the manifest, all of them: a line the port would
/// ignore shows up here too, and [`check_settings`] reports it.
fn table_decls(m: &Manifest) -> Vec<TableDecl> {
    other_values(m, "table")
        .iter()
        .map(|v| {
            let mut words = v.split_whitespace();
            TableDecl {
                name: words.next().unwrap_or("").to_string(),
                path: words.collect::<Vec<_>>().join(" "),
            }
        })
        .collect()
}

/// The declarations the port accepts. A name declared twice keeps the later
/// line.
fn valid_decls(m: &Manifest) -> Vec<TableDecl> {
    let mut out: Vec<TableDecl> = Vec::new();
    for d in table_decls(m) {
        if !table_name_valid(&d.name) || d.path.is_empty() || d.path.contains(char::is_whitespace) {
            continue;
        }
        match out.iter_mut().find(|o| o.name == d.name) {
            Some(o) => *o = d,
            None => out.push(d),
        }
    }
    out
}

/// A signed 32-bit number, decimal or `0x` hex, the way the port reads a
/// table. `Err(true)` is a number that does not fit.
fn parse_s32(s: &str) -> Result<i32, bool> {
    let (neg, body) = match s.as_bytes().first() {
        Some(b'-') => (true, &s[1..]),
        Some(b'+') => (false, &s[1..]),
        _ => (false, s),
    };
    let (radix, digits) = match body.get(..2) {
        Some("0x" | "0X") if body.len() > 2 => (16, &body[2..]),
        _ => (10, body),
    };
    if digits.is_empty() || !digits.chars().all(|c| c.is_digit(radix)) {
        return Err(false);
    }
    let mut v: u64 = 0;
    for c in digits.chars() {
        v = v * u64::from(radix) + u64::from(c.to_digit(radix).unwrap_or(0));
        if v > 0x8000_0000 {
            return Err(true);
        }
    }
    if neg {
        Ok((0i64 - v as i64) as i32)
    } else if v > 0x7FFF_FFFF {
        Err(true)
    } else {
        Ok(v as i32)
    }
}

/// Checks a table file the way the port reads it: one `key = value` per
/// line, `#` comments, an optional `default = N`. A key listed twice is a
/// warning, because the later line wins.
pub fn check_table(text: &str) -> Vec<Diagnostic> {
    use Severity::{Error, Warning};
    let mut out = Vec::new();
    if text.len() > TABLE_FILE_MAX {
        out.push(Diagnostic::field(
            "file",
            Error,
            "the file is larger than the port reads (16 MiB)",
        ));
        return out;
    }
    let mut seen: HashMap<i32, usize> = HashMap::new();
    let mut default_at: Option<usize> = None;
    let mut entries = 0usize;
    let mut limit_said = false;
    for (i, line) in text.lines().enumerate() {
        let n = i + 1;
        let (lead, body) = trim_line(line);
        if body.is_empty() {
            continue;
        }
        let whole = (lead, lead + body.len());
        let Some(eq) = body.find('=') else {
            out.push(Diagnostic::at(
                n,
                line,
                whole.0,
                whole.1,
                Error,
                "'=' is missing: write key = value",
            ));
            continue;
        };
        let (key, value) = (body[..eq].trim_end(), body[eq + 1..].trim_start());
        let value_at = lead + body.len() - value.len();
        if let Err(wide) = parse_s32(value) {
            let msg = if wide {
                "the value does not fit in 32 bits"
            } else {
                "the value is not a number"
            };
            out.push(Diagnostic::at(n, line, value_at, whole.1, Error, msg));
            continue;
        }
        if key == "default" {
            if let Some(first) = default_at {
                out.push(Diagnostic::at(
                    n,
                    line,
                    lead,
                    lead + key.len(),
                    Warning,
                    &format!("the default is also set on line {first}; this later line wins"),
                ));
            }
            default_at = Some(n);
            continue;
        }
        let key_n = match parse_s32(key) {
            Ok(k) => k,
            Err(wide) => {
                let msg = if wide {
                    "the key does not fit in 32 bits"
                } else {
                    "the key is not a number (or the word default)"
                };
                out.push(Diagnostic::at(n, line, lead, lead + key.len(), Error, msg));
                continue;
            }
        };
        entries += 1;
        if entries > TABLE_ENTRIES_MAX && !limit_said {
            limit_said = true;
            out.push(Diagnostic::at(
                n,
                line,
                whole.0,
                whole.1,
                Error,
                &format!("a table holds {TABLE_ENTRIES_MAX} entries at most"),
            ));
        }
        if let Some(first) = seen.insert(key_n, n) {
            out.push(Diagnostic::at(
                n,
                line,
                lead,
                lead + key.len(),
                Warning,
                &format!("the key {key_n} is also on line {first}; this later line wins"),
            ));
        }
    }
    out
}

fn decl_of(root: &Path, name: &str) -> io::Result<TableDecl> {
    valid_decls(&project_of(root)?.manifest)
        .into_iter()
        .find(|d| d.name == name)
        .ok_or_else(|| bad_input("the mod has no table with that name"))
}

/// The mod's declared tables, each with whether its file is there.
pub fn list_tables(root: &Path) -> io::Result<Vec<TableEntry>> {
    Ok(valid_decls(&project_of(root)?.manifest)
        .into_iter()
        .filter(|d| is_relative(&d.path))
        .map(|d| TableEntry {
            exists: root.join(&d.path).is_file(),
            name: d.name,
            path: d.path,
        })
        .collect())
}

/// The declared table names with whether the file loads, for checking rules.
pub fn table_set(root: &Path) -> io::Result<TableSet> {
    Ok(list_tables(root)?
        .into_iter()
        .map(|t| {
            let loads = fs::read_to_string(root.join(&t.path))
                .is_ok_and(|text| !has_errors(&check_table(&text)));
            (t.name, loads)
        })
        .collect())
}

/// Checks the mod's table lines against the disk: the file is there and
/// loads. The lines themselves are checked by [`check_settings`].
pub fn check_table_files(root: &Path) -> io::Result<Vec<Diagnostic>> {
    let mut out = Vec::new();
    for t in list_tables(root)? {
        match fs::read_to_string(root.join(&t.path)) {
            Err(_) => out.push(Diagnostic::field(
                "tables",
                Severity::Error,
                &format!("the table '{}' cannot be read from {}", t.name, t.path),
            )),
            Ok(text) => {
                let first = check_table(&text)
                    .into_iter()
                    .find(|d| d.severity == Severity::Error);
                if let Some(d) = first {
                    let at = if d.line > 0 {
                        format!(" line {}", d.line)
                    } else {
                        String::new()
                    };
                    out.push(Diagnostic::field(
                        "tables",
                        Severity::Error,
                        &format!(
                            "the table '{}' does not load ({}{at}: {})",
                            t.name, t.path, d.message
                        ),
                    ));
                }
            }
        }
    }
    Ok(out)
}

pub fn read_table(root: &Path, name: &str) -> io::Result<String> {
    let d = decl_of(root, name)?;
    fs::read_to_string(root.join(safe_rel(&d.path)?))
}

fn set_table_line(m: &mut Manifest, name: &str, path: Option<&str>) {
    m.other.retain(|l| {
        let Some(("table", v)) = key_value(l) else {
            return true;
        };
        v.split_whitespace().next() != Some(name)
    });
    if let Some(path) = path {
        m.other.push(format!("table = {name} {path}"));
    }
}

/// Writes a table file. Returns what the port would report for it.
pub fn write_table(root: &Path, name: &str, text: &str) -> io::Result<Vec<Diagnostic>> {
    let d = decl_of(root, name)?;
    write_text(&root.join(safe_rel(&d.path)?), text)?;
    Ok(check_table(text))
}

/// Makes an empty table in `tables/` and declares it.
pub fn new_table(root: &Path, name: &str) -> io::Result<TableEntry> {
    if !table_name_valid(name) {
        return Err(bad_input(
            "a table name is letters, digits and '_', not starting with a digit, 31 at most",
        ));
    }
    let mut project = project_of(root)?;
    if valid_decls(&project.manifest)
        .iter()
        .any(|d| d.name == name)
    {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "a table with that name exists",
        ));
    }
    let path = format!("{TABLES_DIR}/{name}.{TABLE_EXT}");
    if root.join(&path).exists() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "a file with that name exists",
        ));
    }
    write_text(&root.join(&path), "# key = value\n")?;
    set_table_line(&mut project.manifest, name, Some(&path));
    project.save_meta()?;
    Ok(TableEntry {
        name: name.to_string(),
        path,
        exists: true,
    })
}

/// `text` with every `table.<from>` read in the code of a rule changed to
/// `table.<to>`, and how many there were. Comments and texts stay.
fn rename_table_reads(text: &str, from: &str, to: &str) -> (String, usize) {
    let head = format!("table.{from}");
    let ident = |c: u8| c.is_ascii_alphanumeric() || c == b'_';
    let mut count = 0;
    let mut out = String::with_capacity(text.len());
    for line in text.split_inclusive('\n') {
        let (code, comment) = line.split_at(line.find('#').unwrap_or(line.len()));
        let b = code.as_bytes();
        let mut quoted = false;
        let mut i = 0;
        while i < b.len() {
            if b[i] == b'"' {
                quoted = !quoted;
            }
            if !quoted
                && code[i..].starts_with(&head)
                && (i == 0 || !(ident(b[i - 1]) || b[i - 1] == b'.'))
                && b.get(i + head.len()).is_none_or(|c| !ident(*c))
            {
                out.push_str(&format!("table.{to}"));
                i += head.len();
                count += 1;
                continue;
            }
            let len = code[i..].chars().next().map_or(1, char::len_utf8);
            out.push_str(&code[i..i + len]);
            i += len;
        }
        out.push_str(comment);
    }
    (out, count)
}

/// Renames a table: its manifest line, its file when the file has the
/// standard name, and the reads in the mod's rule files. Returns the entry
/// and how many reads changed.
pub fn rename_table(root: &Path, from: &str, to: &str) -> io::Result<(TableEntry, usize)> {
    if !table_name_valid(to) {
        return Err(bad_input(
            "a table name is letters, digits and '_', not starting with a digit, 31 at most",
        ));
    }
    let d = decl_of(root, from)?;
    let mut project = project_of(root)?;
    if from != to && valid_decls(&project.manifest).iter().any(|o| o.name == to) {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "a table with that name exists",
        ));
    }
    let mut path = safe_rel(&d.path)?;
    let standard = format!("{TABLES_DIR}/{from}.{TABLE_EXT}");
    let moved = format!("{TABLES_DIR}/{to}.{TABLE_EXT}");
    if from != to && path == standard && !root.join(&moved).exists() {
        if root.join(&path).is_file() {
            fs::rename(root.join(&path), root.join(&moved))?;
        }
        path = moved;
    }
    set_table_line(&mut project.manifest, from, None);
    set_table_line(&mut project.manifest, to, Some(&path));
    project.save_meta()?;
    let mut changed = 0;
    if from != to {
        for rule in list_rules(root)? {
            let file = root.join(&rule.path);
            let Ok(text) = fs::read_to_string(&file) else {
                continue;
            };
            let (next, n) = rename_table_reads(&text, from, to);
            if n > 0 {
                write_text(&file, &next)?;
                changed += n;
            }
        }
    }
    let exists = root.join(&path).is_file();
    Ok((
        TableEntry {
            name: to.to_string(),
            path,
            exists,
        },
        changed,
    ))
}

/// Deletes a table: its manifest line and its file. A rule that still reads
/// it is reported by the rule check.
pub fn delete_table(root: &Path, name: &str) -> io::Result<()> {
    let d = decl_of(root, name)?;
    let file = root.join(safe_rel(&d.path)?);
    if file.is_file() {
        fs::remove_file(file)?;
    }
    edit_manifest(root, |m| set_table_line(m, name, None))
}

// ---------------------------------------------------------------------
// Conflicts with other mods
// ---------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModRef {
    pub name: String,
    /// The folder name.
    pub dir: String,
    pub order: i32,
    /// The mod being edited.
    pub this: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Conflict {
    /// `file` for a game file, `hook` for a function hooked twice.
    pub kind: String,
    pub key: String,
    /// The mods that change it, in load order.
    pub mods: Vec<ModRef>,
    /// The mod whose version the player gets (the last in load order).
    pub winner: String,
    /// For a hook, each mod's mode in the same order.
    pub detail: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ConflictReport {
    /// Every mod compared, in load order.
    pub mods: Vec<ModRef>,
    pub conflicts: Vec<Conflict>,
}

struct Footprint {
    info: ModRef,
    files: BTreeSet<String>,
    hooks: BTreeMap<String, String>,
}

/// The files a mod puts over the game's, as the port resolves them. A member
/// `25.bin` of an archive folder is member 25. Files the mod loads from its
/// own folder (rules, code, settings) and the manifests are left out.
fn footprint(dir: &Path, this: bool) -> Footprint {
    let dir_name = dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let text = fs::read_to_string(dir.join(MANIFEST)).unwrap_or_default();
    let m = Manifest::parse(&text, &dir_name);
    let own: BTreeSet<String> = ["rule", "code", "settings"]
        .iter()
        .flat_map(|k| {
            other_values(&m, k).into_iter().map(|v| {
                let v = v.replace('\\', "/");
                // A `rule` value is `<event> <path>`.
                v.split_whitespace().next_back().unwrap_or("").to_string()
            })
        })
        .collect();
    let mut files = BTreeSet::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(entries) = fs::read_dir(&d) else {
            continue;
        };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
                continue;
            }
            let Ok(rel) = p.strip_prefix(dir) else {
                continue;
            };
            let mut rel = rel.to_string_lossy().replace('\\', "/");
            let file = rel.rsplit('/').next().unwrap_or("").to_string();
            if rel == MANIFEST || rel == KENON_FILE || file.starts_with('~') || own.contains(&rel) {
                continue;
            }
            if let Some((stem, _)) = file.split_once('.') {
                if !stem.is_empty() && stem.bytes().all(|b| b.is_ascii_digit()) {
                    rel.truncate(rel.len() - file.len());
                    rel.push_str(stem);
                }
            }
            files.insert(rel);
        }
    }
    let mut hooks = BTreeMap::new();
    for v in other_values(&m, "override") {
        if let Ok(o) = parse_override(&v) {
            hooks.insert(o.key(), o.mode);
        }
    }
    Footprint {
        info: ModRef {
            name: m.name.clone(),
            dir: dir_name,
            order: m.order,
            this,
        },
        files,
        hooks,
    }
}

/// Finds the files and hooks that `this` changes and another mod in
/// `mods_dir` changes as well. Mods are in load order: ascending `order`,
/// then folder name (the port keeps the order mods were added in, which a
/// folder does not record).
pub fn find_conflicts(this: &Path, mods_dir: &Path) -> io::Result<ConflictReport> {
    let this_canon = fs::canonicalize(this)?;
    let mut mods = vec![footprint(this, true)];
    for e in fs::read_dir(mods_dir)?.flatten() {
        let p = e.path();
        let hidden = e.file_name().to_string_lossy().starts_with('.');
        if !p.is_dir() || hidden || fs::canonicalize(&p).ok().as_ref() == Some(&this_canon) {
            continue;
        }
        mods.push(footprint(&p, false));
    }
    mods.sort_by(|a, b| (a.info.order, &a.info.dir).cmp(&(b.info.order, &b.info.dir)));

    let mut report = ConflictReport {
        mods: mods.iter().map(|m| m.info.clone()).collect(),
        conflicts: Vec::new(),
    };
    let me = mods.iter().find(|m| m.info.this).unwrap();
    let mut add = |kind: &str, key: String, with: Vec<&Footprint>, detail: Vec<String>| {
        if with.len() < 2 {
            return;
        }
        report.conflicts.push(Conflict {
            kind: kind.to_string(),
            key,
            winner: with.last().unwrap().info.name.clone(),
            mods: with.iter().map(|m| m.info.clone()).collect(),
            detail,
        });
    };
    for file in &me.files {
        // The same file, or a whole archive file over the other's members
        // (or the reverse).
        let with: Vec<&Footprint> = mods
            .iter()
            .filter(|m| {
                m.files.contains(file)
                    || m.files.iter().any(|f| f.starts_with(&format!("{file}/")))
                    || file
                        .match_indices('/')
                        .any(|(i, _)| m.files.contains(&file[..i]))
            })
            .collect();
        add("file", file.clone(), with, Vec::new());
    }
    for (key, mode) in &me.hooks {
        let with: Vec<&Footprint> = mods.iter().filter(|m| m.hooks.contains_key(key)).collect();
        let detail = with.iter().map(|m| m.hooks[key].clone()).collect();
        let _ = mode;
        add("hook", key.clone(), with, detail);
    }
    Ok(report)
}

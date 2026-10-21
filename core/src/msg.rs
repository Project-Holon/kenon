//! Message banks: the game's text (`a/0/0/2` system and UI text, `a/0/0/3`
//! field text; each NARC member is one bank).
//!
//! The rules follow the decompiled `msgdata.c` (aletheia
//! `src/game/arm9/msgdata.c`):
//!
//! ```text
//! file:     u16 language count  u16 string count  u32 size  u32 reserved
//!           u32 section offset per language (from the start of the file)
//! section:  u32 section size, then one 8-byte entry per string:
//!           u32 offset (from the section start)  u16 length  u16 reserved
//!           then the strings
//! string:   `length` UTF-16 units, the last being the 0xFFFF terminator,
//!           ciphered; strings are back to back and the section is padded
//!           with zeros to 4 bytes
//! ```
//!
//! The cipher (MSGDATA_DecryptUnits): `key = (index + 3) * 0x2983` as a
//! u16; each unit is XORed with the key, then the key rotates left by 3.
//! XOR is its own inverse, so the same pass encrypts. Black and White have
//! one language section per file; MSGDATA_GetLanguageIndex falls back to
//! section 0 when the current language is past the count.

use std::fmt;

use crate::nitro::{read_u16, read_u32, Error};

/// Ends a string.
pub const END: u16 = 0xFFFF;
/// A line break.
pub const NEWLINE: u16 = 0xFFFE;
/// Starts a control code: kind, argument count, then the arguments.
pub const CONTROL: u16 = 0xF000;

/// Applies the bank cipher to `units` for string `index`, in place.
pub fn cipher(units: &mut [u16], index: u32) {
    let mut key = (index.wrapping_add(3).wrapping_mul(0x2983)) as u16;
    for unit in units {
        *unit ^= key;
        key = key.rotate_left(3);
    }
}

/// One entry of a bank: the string's units (without the terminator) and
/// the entry's reserved field, kept for writing the bank back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub units: Vec<u16>,
    pub reserved: u16,
}

/// One language section: its messages, and the bytes that pad it to 4
/// bytes. The original files fill that pad with an arbitrary repeated byte,
/// which no code reads; it is kept so a bank writes back byte for byte.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Section {
    pub messages: Vec<Message>,
    pub padding: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bank {
    /// One section per language; Black and White have one.
    pub sections: Vec<Section>,
    /// The file header's reserved word.
    pub reserved: u32,
}

impl Bank {
    pub fn parse(bytes: &[u8]) -> Result<Self, Error> {
        let lang_count = read_u16(bytes, 0)? as usize;
        let count = read_u16(bytes, 2)? as usize;
        let reserved = read_u32(bytes, 8)?;
        let sections = (0..lang_count)
            .map(|lang| {
                let section = read_u32(bytes, 12 + lang * 4)? as usize;
                let size = read_u32(bytes, section)? as usize;
                let mut text_end = 4 + count * 8;
                let messages = (0..count)
                    .map(|index| {
                        let entry = section + 4 + index * 8;
                        let offset = read_u32(bytes, entry)? as usize;
                        let length = read_u16(bytes, entry + 4)? as usize;
                        let reserved = read_u16(bytes, entry + 6)?;
                        text_end = text_end.max(offset + length * 2);
                        let start = section + offset;
                        let mut units = (0..length)
                            .map(|i| read_u16(bytes, start + i * 2))
                            .collect::<Result<Vec<_>, _>>()?;
                        cipher(&mut units, index as u32);
                        // The stored length counts the terminator; the game
                        // stops at the first one.
                        if let Some(end) = units.iter().position(|&u| u == END) {
                            units.truncate(end);
                        }
                        Ok(Message { units, reserved })
                    })
                    .collect::<Result<Vec<_>, Error>>()?;
                let padding = bytes
                    .get(section + text_end.min(size)..section + size)
                    .unwrap_or(&[])
                    .to_vec();
                Ok(Section { messages, padding })
            })
            .collect::<Result<_, Error>>()?;
        Ok(Bank { sections, reserved })
    }

    /// The messages of section 0, the only one in Black and White.
    pub fn messages(&self) -> &[Message] {
        self.sections.first().map_or(&[], |s| s.messages.as_slice())
    }

    /// Writes the bank: header, section offsets, then each section with its
    /// entry table and its strings back to back, each with its terminator,
    /// padded to 4 bytes (with the kept padding when it still fits).
    pub fn build(&self) -> Vec<u8> {
        let count = self.sections.first().map_or(0, |s| s.messages.len());
        let mut sections = Vec::new();
        for Section { messages, padding } in &self.sections {
            let table = 4 + count * 8;
            let mut entries = Vec::with_capacity(count * 8);
            let mut text = Vec::new();
            for (index, message) in messages.iter().enumerate() {
                let mut units = message.units.clone();
                units.push(END);
                cipher(&mut units, index as u32);
                entries.extend_from_slice(&((table + text.len()) as u32).to_le_bytes());
                entries.extend_from_slice(&(units.len() as u16).to_le_bytes());
                entries.extend_from_slice(&message.reserved.to_le_bytes());
                for unit in units {
                    text.extend_from_slice(&unit.to_le_bytes());
                }
            }
            let used = table + text.len();
            let size = used.next_multiple_of(4);
            let mut section = Vec::with_capacity(size);
            section.extend_from_slice(&(size as u32).to_le_bytes());
            section.extend_from_slice(&entries);
            section.extend_from_slice(&text);
            if padding.len() == size - used {
                section.extend_from_slice(padding);
            }
            section.resize(size, 0);
            sections.push(section);
        }

        let header = 12 + self.sections.len() * 4;
        let total: usize = sections.iter().map(Vec::len).sum();
        let mut out = Vec::with_capacity(header + total);
        out.extend_from_slice(&(self.sections.len() as u16).to_le_bytes());
        out.extend_from_slice(&(count as u16).to_le_bytes());
        out.extend_from_slice(&(total as u32).to_le_bytes());
        out.extend_from_slice(&self.reserved.to_le_bytes());
        let mut at = header;
        for section in &sections {
            out.extend_from_slice(&(at as u32).to_le_bytes());
            at += section.len();
        }
        for section in sections {
            out.extend_from_slice(&section);
        }
        out
    }
}

/// A piece of decoded text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    Char(u16),
    Newline,
    /// A control code (names, colours, waits, scrolls, ...), interpreted by
    /// the game's print code.
    Control {
        kind: u16,
        args: Vec<u16>,
    },
}

/// Splits units into tokens. A control code cut short by the end of the
/// string keeps the arguments that are there.
pub fn tokens(units: &[u16]) -> Vec<Token> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < units.len() {
        match units[i] {
            NEWLINE => out.push(Token::Newline),
            CONTROL => {
                let kind = units.get(i + 1).copied().unwrap_or(0);
                let count = units.get(i + 2).copied().unwrap_or(0) as usize;
                let end = (i + 3 + count).min(units.len());
                let args = units.get(i + 3..end).unwrap_or(&[]).to_vec();
                out.push(Token::Control { kind, args });
                i = end;
                continue;
            }
            c => out.push(Token::Char(c)),
        }
        i += 1;
    }
    out
}

/// Marks a packed string: nine-bit codes follow in the halfwords after it
/// (trainer names and other short names). WORDSET_IsPackedString and
/// WORDSET_AppendUnpackedString in the decompiled `wordset.c`.
pub const PACKED: u16 = 0xF100;

/// The first code that goes through the game's packed-character table, and
/// how many the table holds (0x100..0x131). The table itself is read from
/// the ROM's ARM9 image.
pub const PACKED_TABLE_FIRST: u16 = 0x100;
pub const PACKED_TABLE_LEN: usize = 0x32;

/// The characters of a packed string, as the game unpacks them: nine bits
/// at a time, least significant first, 0x1FF ends; codes 0x100..0x131 map
/// through `table`, codes above become '?'. `None` when not packed.
pub fn unpack(units: &[u16], table: Option<&[u16]>) -> Option<Vec<u16>> {
    if units.first() != Some(&PACKED) {
        return None;
    }
    let words = &units[1..];
    let mut out = Vec::new();
    let mut bit = 0usize;
    // The game reads on into the 0xFFFF end unit when the packed words stop
    // before the terminator code is complete.
    let word = |w: usize| words.get(w).copied().unwrap_or(0xFFFF) as u32;
    loop {
        let (w, b) = (bit / 16, bit % 16);
        if w > words.len() {
            return Some(out);
        }
        let lo = word(w) >> b;
        let hi = (word(w + 1) << 16) >> b;
        let code = ((lo | hi) & 0x1FF) as u16;
        bit += 9;
        if code == 0x1FF {
            return Some(out);
        }
        let first = PACKED_TABLE_FIRST;
        out.push(match code {
            c if (first..first + PACKED_TABLE_LEN as u16).contains(&c) => table
                .and_then(|t| t.get((c - first) as usize).copied())
                .unwrap_or(c),
            c if c >= first + PACKED_TABLE_LEN as u16 => b'?' as u16,
            c => c,
        });
    }
}

/// Packs characters back into the game's packed form, or `None` when a
/// character has no nine-bit code. Like the game's own strings, only the
/// words the characters need are written: the end unit after them
/// completes the terminator code.
pub fn pack(chars: &[u16], table: Option<&[u16]>) -> Option<Vec<u16>> {
    let mut codes = Vec::with_capacity(chars.len() + 1);
    for &c in chars {
        codes.push(if c < PACKED_TABLE_FIRST {
            c
        } else {
            PACKED_TABLE_FIRST + table?.iter().position(|&t| t == c)? as u16
        });
    }
    let mut words = vec![0u16; (codes.len() * 9).div_ceil(16)];
    codes.push(0x1FF);
    for (i, &code) in codes.iter().enumerate() {
        let bit = i * 9;
        let v = (code as u32) << (bit % 16);
        if let Some(w) = words.get_mut(bit / 16) {
            *w |= v as u16;
        }
        if let Some(w) = words.get_mut(bit / 16 + 1) {
            *w |= (v >> 16) as u16;
        }
    }
    let mut out = vec![PACKED];
    out.extend(words);
    Some(out)
}

/// The display form of a message, unpacking a packed string. A packed one
/// starts with `{packed}`, so a save packs it again.
pub fn display(units: &[u16], table: Option<&[u16]>) -> String {
    match unpack(units, table) {
        Some(chars) => format!("{{packed}}{}", Display(&chars)),
        None => Display(units).to_string(),
    }
}

/// The text of a message for names in lists: unpacked, no marker.
pub fn plain(units: &[u16], table: Option<&[u16]>) -> String {
    match unpack(units, table) {
        Some(chars) => Display(&chars).to_string(),
        None => Display(units).to_string(),
    }
}

/// Parses the form `display` writes.
pub fn parse(text: &str, table: Option<&[u16]>) -> Result<Vec<u16>, String> {
    match text.strip_prefix("{packed}") {
        Some(rest) => {
            let chars = parse_display(rest)?;
            Ok(pack(&chars, table).unwrap_or(chars))
        }
        None => parse_display(text),
    }
}

/// Text for display and editing: characters as themselves, `\n` for a line
/// break, `{XXXX:a,b}` (hex) for a control code, and `{U+XXXX}` for a unit
/// that is not a valid character (an unpaired surrogate). `\`, `{` and `}`
/// are escaped with `\`. [`parse_display`] reverses it exactly.
pub struct Display<'a>(pub &'a [u16]);

impl fmt::Display for Display<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for token in tokens(self.0) {
            match token {
                Token::Newline => f.write_str("\\n")?,
                Token::Control { kind, args } => {
                    write!(f, "{{{kind:04X}")?;
                    for (i, arg) in args.iter().enumerate() {
                        write!(f, "{}{arg:X}", if i == 0 { ':' } else { ',' })?;
                    }
                    f.write_str("}")?;
                }
                Token::Char(c) => match char::from_u32(c as u32) {
                    Some('\\') => f.write_str("\\\\")?,
                    Some('{') => f.write_str("\\{")?,
                    Some('}') => f.write_str("\\}")?,
                    Some(ch) => write!(f, "{ch}")?,
                    None => write!(f, "{{U+{c:04X}}}")?,
                },
            }
        }
        Ok(())
    }
}

/// Parses text in the [`Display`] form back to units.
pub fn parse_display(text: &str) -> Result<Vec<u16>, String> {
    let mut out = Vec::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' => match chars.next() {
                Some('n') => out.push(NEWLINE),
                Some(e @ ('\\' | '{' | '}')) => out.push(e as u16),
                other => return Err(format!("unknown escape \\{}", other.unwrap_or(' '))),
            },
            '{' => {
                let mut body = String::new();
                for c in chars.by_ref() {
                    if c == '}' {
                        break;
                    }
                    body.push(c);
                }
                if let Some(hex) = body.strip_prefix("U+") {
                    out.push(u16::from_str_radix(hex, 16).map_err(|e| e.to_string())?);
                    continue;
                }
                let (kind, args) = body.split_once(':').unwrap_or((&body, ""));
                let kind = u16::from_str_radix(kind, 16).map_err(|e| format!("{{{body}}}: {e}"))?;
                let args: Vec<u16> = if args.is_empty() {
                    Vec::new()
                } else {
                    args.split(',')
                        .map(|a| u16::from_str_radix(a, 16))
                        .collect::<Result<_, _>>()
                        .map_err(|e| format!("{{{body}}}: {e}"))?
                };
                out.extend_from_slice(&[CONTROL, kind, args.len() as u16]);
                out.extend_from_slice(&args);
            }
            c => {
                let mut buf = [0u16; 2];
                out.extend_from_slice(c.encode_utf16(&mut buf));
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn units(s: &str) -> Vec<u16> {
        s.encode_utf16().collect()
    }

    #[test]
    fn cipher_is_its_own_inverse_and_matches_the_key_rule() {
        let mut u = vec![0u16; 3];
        cipher(&mut u, 0);
        // key = 3 * 0x2983 = 0x7C89, then rotated left by 3 twice.
        assert_eq!(u, vec![0x7C89, 0xE44B, 0x225F]);
        let plain = units("Bulbasaur");
        let mut c = plain.clone();
        cipher(&mut c, 1);
        assert_ne!(c, plain);
        cipher(&mut c, 1);
        assert_eq!(c, plain);
    }

    #[test]
    fn bank_round_trip() {
        let bank = Bank {
            sections: vec![Section {
                messages: vec![
                    Message {
                        units: units("Yes"),
                        reserved: 0,
                    },
                    Message {
                        units: vec![],
                        reserved: 0x3C,
                    },
                    Message {
                        units: units("Hello\u{FFFE}there"),
                        reserved: 0,
                    },
                ],
                // 62 bytes used, so 2 bytes of padding.
                padding: vec![0, 0],
            }],
            reserved: 0,
        };
        let bytes = bank.build();
        assert_eq!(read_u16(&bytes, 2).unwrap(), 3);
        assert_eq!(Bank::parse(&bytes).unwrap(), bank);
        assert_eq!(bytes.len() % 4, 0);
    }

    #[test]
    fn packed_strings() {
        // "Mali" with code 0x100 mapped to U+00E9 by the table.
        let table = [0x00E9u16];
        let chars: Vec<u16> = "Mal".encode_utf16().chain([0x00E9]).collect();
        let packed = pack(&chars, Some(&table)).unwrap();
        assert_eq!(packed[0], PACKED);
        assert_eq!(unpack(&packed, Some(&table)).unwrap(), chars);
        assert_eq!(display(&packed, Some(&table)), "{packed}Mal\u{e9}");
        assert_eq!(parse("{packed}Mal\u{e9}", Some(&table)).unwrap(), packed);
        // From the ROM's trainer bank: [F100, C24D, 49B0, 1FF3] is "Mali".
        let real = [PACKED, 0xC24D, 0x49B0, 0x1FF3];
        assert_eq!(plain(&real, None), "Mali");
        assert_eq!(
            pack(&"Mali".encode_utf16().collect::<Vec<_>>(), None).unwrap(),
            real
        );
        assert_eq!(unpack(&[1, 2], None), None);
        // Five characters: the terminator runs into the end unit.
        let jimmy = [PACKED, 0xD24A, 0x69B4, 0xE793];
        assert_eq!(plain(&jimmy, None), "Jimmy");
        assert_eq!(
            pack(&"Jimmy".encode_utf16().collect::<Vec<_>>(), None).unwrap(),
            jimmy
        );
    }

    #[test]
    fn tokens_and_display() {
        let mut u = units("Hi ");
        u.extend_from_slice(&[CONTROL, 0x0100, 2, 0, 7, NEWLINE]);
        u.extend(units("{ok}\\"));
        u.push(0xD800);
        assert_eq!(
            tokens(&u)[3],
            Token::Control {
                kind: 0x0100,
                args: vec![0, 7]
            }
        );
        let text = Display(&u).to_string();
        assert_eq!(text, "Hi {0100:0,7}\\n\\{ok\\}\\\\{U+D800}");
        assert_eq!(parse_display(&text).unwrap(), u);
        assert_eq!(parse_display("{BE01}").unwrap(), vec![CONTROL, 0xBE01, 0]);
        assert!(parse_display("\\q").is_err());
    }
}

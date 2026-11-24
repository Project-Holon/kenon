//! An ARMv5TE decoder for the DS's main processor (ARM946E-S): ARM and Thumb
//! instructions as that CPU runs them, written the way capstone writes them
//! (UAL). A word or halfword the CPU does not run as an instruction (an ARMv6
//! or Thumb-2 form, an undefined pattern) decodes to `None`: it is data.

/// One decoded instruction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inst {
    /// 4 (ARM, Thumb BL/BLX pair) or 2 (Thumb).
    pub len: u32,
    pub text: String,
    /// A branch with an address in the instruction.
    pub branch: Option<Branch>,
    /// The address of a word the instruction loads relative to pc (a literal).
    pub literal: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Branch {
    pub target: u32,
    /// The target runs in the other instruction set (BLX with an address).
    pub switch: bool,
    /// BL / BLX: a call that comes back.
    pub link: bool,
}

const COND: [&str; 15] = [
    "eq", "ne", "hs", "lo", "mi", "pl", "vs", "vc", "hi", "ls", "ge", "lt", "gt", "le", "",
];

fn reg(r: u32) -> &'static str {
    [
        "r0", "r1", "r2", "r3", "r4", "r5", "r6", "r7", "r8", "sb", "sl", "fp", "ip", "sp", "lr",
        "pc",
    ][(r & 15) as usize]
}

fn imm(v: u32) -> String {
    if v > 9 {
        format!("#{v:#x}")
    } else {
        format!("#{v}")
    }
}

fn signed(up: bool, v: u32) -> String {
    match (up, v > 9) {
        (true, _) => imm(v),
        (false, true) => format!("#-{v:#x}"),
        (false, false) => format!("#-{v}"),
    }
}

fn list(mask: u32) -> String {
    let regs: Vec<&str> = (0..16).filter(|r| mask & (1 << r) != 0).map(reg).collect();
    format!("{{{}}}", regs.join(", "))
}

fn sext(v: u32, bits: u32) -> i32 {
    ((v << (32 - bits)) as i32) >> (32 - bits)
}

fn plain(len: u32, text: String) -> Inst {
    Inst {
        len,
        text,
        branch: None,
        literal: None,
    }
}

/// Decodes one ARM instruction at `pc`.
pub fn arm(w: u32, pc: u32) -> Option<Inst> {
    let cond = w >> 28;
    if cond == 0xF {
        return unconditional(w, pc);
    }
    let c = COND[cond as usize];
    let bit = |n: u32| (w >> n) & 1 == 1;
    let (rn, rd, rs, rm) = ((w >> 16) & 15, (w >> 12) & 15, (w >> 8) & 15, w & 15);
    let text = |s: String| Some(plain(4, s));
    match (w >> 25) & 7 {
        0 if w & 0x90 == 0x90 => {
            let sh = (w >> 5) & 3;
            if sh == 0 {
                let s = if bit(20) { "s" } else { "" };
                return match (w >> 23) & 0x1F {
                    0 if !bit(21) => {
                        text(format!("mul{s}{c} {}, {}, {}", reg(rn), reg(rm), reg(rs)))
                    }
                    0 => text(format!(
                        "mla{s}{c} {}, {}, {}, {}",
                        reg(rn),
                        reg(rm),
                        reg(rs),
                        reg(rd)
                    )),
                    1 => {
                        let name = ["umull", "umlal", "smull", "smlal"][((w >> 21) & 3) as usize];
                        text(format!(
                            "{name}{s}{c} {}, {}, {}, {}",
                            reg(rd),
                            reg(rn),
                            reg(rm),
                            reg(rs)
                        ))
                    }
                    2 if (w >> 20) & 3 == 0 && rs == 0 => {
                        let b = if bit(22) { "b" } else { "" };
                        text(format!("swp{b}{c} {}, {}, [{}]", reg(rd), reg(rm), reg(rn)))
                    }
                    _ => None,
                };
            }
            // halfword, signed and doubleword transfers
            let (l, p, u, i, wb) = (bit(20), bit(24), bit(23), bit(22), bit(21));
            let name = match (sh, l) {
                (1, true) => "ldrh",
                (1, false) => "strh",
                (2, true) => "ldrsb",
                (2, false) => "ldrd",
                (3, true) => "ldrsh",
                _ => "strd",
            };
            if !p && wb {
                return None;
            }
            let dst = if name.ends_with('d') {
                if rd % 2 == 1 {
                    return None;
                }
                format!("{}, {}", reg(rd), reg(rd + 1))
            } else {
                reg(rd).to_string()
            };
            let off = if i {
                let v = ((w >> 4) & 0xF0) | (w & 0xF);
                (v != 0 || !u).then(|| signed(u, v))
            } else {
                if rs != 0 {
                    return None;
                }
                Some(format!("{}{}", if u { "" } else { "-" }, reg(rm)))
            };
            text(format!("{name}{c} {dst}, {}", address(rn, p, wb, off)))
        }
        0 if (w >> 23) & 3 == 2 && !bit(20) => misc(w, c),
        0 => data_processing(w, c, shifter_reg(w)?),
        1 if (w >> 23) & 3 == 2 && !bit(20) => {
            if !bit(21) {
                return None;
            }
            let v = (w & 0xFF).rotate_right(((w >> 8) & 15) * 2);
            text(format!("msr{c} {}, {}", psr_fields(w), imm(v)))
        }
        1 => {
            let v = (w & 0xFF).rotate_right(((w >> 8) & 15) * 2);
            data_processing(w, c, imm(v))
        }
        2 | 3 => {
            if (w >> 25) & 1 == 1 && bit(4) {
                return None;
            }
            let (p, u, b, wb, l) = (bit(24), bit(23), bit(22), bit(21), bit(20));
            let t = if !p && wb { "t" } else { "" };
            let name = format!(
                "{}{}{t}{c}",
                if l { "ldr" } else { "str" },
                if b { "b" } else { "" }
            );
            let off = if (w >> 25) & 1 == 0 {
                let v = w & 0xFFF;
                (v != 0 || !u).then(|| signed(u, v))
            } else {
                let shift = shift_imm(w);
                Some(format!("{}{}{shift}", if u { "" } else { "-" }, reg(rm)))
            };
            let mut inst = plain(
                4,
                format!("{name} {}, {}", reg(rd), address(rn, p, wb && p, off)),
            );
            if rn == 15 && p && !wb && (w >> 25) & 1 == 0 && l && !b {
                let v = w & 0xFFF;
                inst.literal = Some(if u {
                    pc.wrapping_add(8 + v)
                } else {
                    pc.wrapping_add(8).wrapping_sub(v)
                });
            }
            Some(inst)
        }
        4 => {
            let mask = w & 0xFFFF;
            if mask == 0 {
                return None;
            }
            let (p, u, s, wb, l) = (bit(24), bit(23), bit(22), bit(21), bit(20));
            let caret = if s { " ^" } else { "" };
            if rn == 13 && wb && !s && mask.count_ones() > 1 {
                if l && !p && u {
                    return text(format!("pop{c} {}", list(mask)));
                }
                if !l && p && !u {
                    return text(format!("push{c} {}", list(mask)));
                }
            }
            let mode = match (p, u) {
                (false, true) => "",
                (true, true) => "ib",
                (false, false) => "da",
                (true, false) => "db",
            };
            let name = if l { "ldm" } else { "stm" };
            let bang = if wb { "!" } else { "" };
            text(format!(
                "{name}{mode}{c} {}{bang}, {}{caret}",
                reg(rn),
                list(mask)
            ))
        }
        5 => {
            let link = bit(24);
            let target = pc
                .wrapping_add(8)
                .wrapping_add((sext(w & 0xFF_FFFF, 24) << 2) as u32);
            Some(Inst {
                len: 4,
                text: format!("{}{c} {target:#x}", if link { "bl" } else { "b" }),
                branch: Some(Branch {
                    target,
                    switch: false,
                    link,
                }),
                literal: None,
            })
        }
        6 => coproc_transfer(w, c, ""),
        _ => {
            if bit(24) {
                text(format!("svc{c} {}", imm(w & 0xFF_FFFF)))
            } else if bit(4) {
                text(format!(
                    "{}{c} p{}, #{}, {}, c{}, c{}, #{}",
                    if bit(20) { "mrc" } else { "mcr" },
                    (w >> 8) & 15,
                    (w >> 21) & 7,
                    coproc_reg(rd, bit(20)),
                    rn,
                    rm,
                    (w >> 5) & 7
                ))
            } else {
                text(format!(
                    "cdp{c} p{}, #{}, c{}, c{}, c{}, #{}",
                    (w >> 8) & 15,
                    (w >> 20) & 15,
                    rd,
                    rn,
                    rm,
                    (w >> 5) & 7
                ))
            }
        }
    }
}

/// The ARM register of MRC/MCR: an MRC into pc sets the flags.
fn coproc_reg(r: u32, read: bool) -> &'static str {
    if r == 15 && read {
        "apsr_nzcv"
    } else {
        reg(r)
    }
}

/// `[rn, off]`, `[rn, off]!` or `[rn], off`.
fn address(rn: u32, pre: bool, wb: bool, off: Option<String>) -> String {
    match (pre, off) {
        (true, None) => format!("[{}]{}", reg(rn), if wb { "!" } else { "" }),
        (true, Some(o)) => format!("[{}, {o}]{}", reg(rn), if wb { "!" } else { "" }),
        (false, None) => format!("[{}]", reg(rn)),
        (false, Some(o)) => format!("[{}], {o}", reg(rn)),
    }
}

/// `, lsl #2` and the like for a register offset (empty for none).
fn shift_imm(w: u32) -> String {
    let (kind, amount) = ((w >> 5) & 3, (w >> 7) & 0x1F);
    match (kind, amount) {
        (0, 0) => String::new(),
        (3, 0) => ", rrx".into(),
        (1 | 2, 0) => format!(", {} #32", ["lsl", "lsr", "asr", "ror"][kind as usize]),
        _ => format!(
            ", {} #{amount}",
            ["lsl", "lsr", "asr", "ror"][kind as usize]
        ),
    }
}

/// The register form of a data-processing operand.
fn shifter_reg(w: u32) -> Option<String> {
    let rm = reg(w & 15);
    if (w >> 4) & 1 == 1 {
        if (w >> 7) & 1 == 1 {
            return None;
        }
        let kind = ["lsl", "lsr", "asr", "ror"][((w >> 5) & 3) as usize];
        return Some(format!("{rm}, {kind} {}", reg((w >> 8) & 15)));
    }
    Some(format!("{rm}{}", shift_imm(w)))
}

fn data_processing(w: u32, c: &str, op2: String) -> Option<Inst> {
    const NAMES: [&str; 16] = [
        "and", "eor", "sub", "rsb", "add", "adc", "sbc", "rsc", "tst", "teq", "cmp", "cmn", "orr",
        "mov", "bic", "mvn",
    ];
    let op = (w >> 21) & 15;
    let s = if (w >> 20) & 1 == 1 { "s" } else { "" };
    let (rn, rd) = (reg((w >> 16) & 15), reg((w >> 12) & 15));
    let name = NAMES[op as usize];
    let text = match op {
        8..=11 => format!("{name}{c} {rn}, {op2}"),
        13 => {
            // UAL: a shifted move is written as the shift.
            if let Some((rm, rest)) = op2.split_once(", ") {
                let (kind, amount) = rest.split_once(' ').unwrap_or((rest, ""));
                if kind == "rrx" {
                    format!("rrx{s}{c} {rd}, {rm}")
                } else {
                    format!("{kind}{s}{c} {rd}, {rm}, {amount}")
                }
            } else {
                format!("mov{s}{c} {rd}, {op2}")
            }
        }
        15 => format!("mvn{s}{c} {rd}, {op2}"),
        _ => format!("{name}{s}{c} {rd}, {rn}, {op2}"),
    };
    Some(plain(4, text))
}

fn psr_fields(w: u32) -> String {
    let psr = if (w >> 22) & 1 == 1 { "spsr" } else { "cpsr" };
    let fields: String = [(3, 'f'), (2, 's'), (1, 'x'), (0, 'c')]
        .iter()
        .filter(|(b, _)| (w >> (16 + b)) & 1 == 1)
        .map(|(_, f)| *f)
        .collect();
    format!("{psr}_{fields}")
}

/// The miscellaneous space: MRS, MSR, BX, BLX, CLZ, saturating and the
/// 16-bit multiplies, BKPT.
fn misc(w: u32, c: &str) -> Option<Inst> {
    let (rn, rd, rs, rm) = (
        reg((w >> 16) & 15),
        reg((w >> 12) & 15),
        reg((w >> 8) & 15),
        reg(w & 15),
    );
    let op1 = (w >> 21) & 3;
    let xy = |b: u32| if (w >> b) & 1 == 1 { "t" } else { "b" };
    let text = match (w >> 4) & 15 {
        0 if op1 & 1 == 0 => {
            let psr = if op1 == 2 { "spsr" } else { "apsr" };
            format!("mrs{c} {rd}, {psr}")
        }
        0 => format!("msr{c} {}, {rm}", psr_fields(w)),
        1 if op1 == 1 => format!("bx{c} {rm}"),
        1 if op1 == 3 => format!("clz{c} {rd}, {rm}"),
        3 if op1 == 1 => format!("blx{c} {rm}"),
        5 => format!(
            "{}{c} {rd}, {rm}, {rn}",
            ["qadd", "qsub", "qdadd", "qdsub"][op1 as usize]
        ),
        7 if op1 == 1 && c.is_empty() => {
            format!("bkpt {}", imm(((w >> 4) & 0xFFF0) | (w & 0xF)))
        }
        8 | 0xA | 0xC | 0xE => match op1 {
            0 => format!("smla{}{}{c} {rn}, {rm}, {rs}, {rd}", xy(5), xy(6)),
            1 if (w >> 5) & 1 == 0 => format!("smlaw{}{c} {rn}, {rm}, {rs}, {rd}", xy(6)),
            1 => format!("smulw{}{c} {rn}, {rm}, {rs}", xy(6)),
            2 => format!("smlal{}{}{c} {rd}, {rn}, {rm}, {rs}", xy(5), xy(6)),
            _ => format!("smul{}{}{c} {rn}, {rm}, {rs}", xy(5), xy(6)),
        },
        _ => return None,
    };
    Some(plain(4, text))
}

fn coproc_transfer(w: u32, c: &str, two: &str) -> Option<Inst> {
    let bit = |n: u32| (w >> n) & 1 == 1;
    let (p, u, n, wb, l) = (bit(24), bit(23), bit(22), bit(21), bit(20));
    if !p && !wb && !u {
        return None;
    }
    let name = format!(
        "{}{two}{}{c}",
        if l { "ldc" } else { "stc" },
        if n { "l" } else { "" }
    );
    let rn = reg((w >> 16) & 15);
    let off = (w & 0xFF) * 4;
    let addr = match (p, wb) {
        (true, _) => format!("[{rn}, {}]{}", signed(u, off), if wb { "!" } else { "" }),
        (false, true) => format!("[{rn}], {}", signed(u, off)),
        (false, false) => format!("[{rn}], {{{}}}", w & 0xFF),
    };
    Some(plain(
        4,
        format!("{name} p{}, c{}, {addr}", (w >> 8) & 15, (w >> 12) & 15),
    ))
}

/// The cond = 1111 space of ARMv5TE: BLX with an address, PLD and the
/// second coprocessor forms.
fn unconditional(w: u32, pc: u32) -> Option<Inst> {
    if w & 0x0E00_0000 == 0x0A00_0000 {
        let target = pc
            .wrapping_add(8)
            .wrapping_add((sext(w & 0xFF_FFFF, 24) << 2) as u32)
            .wrapping_add(((w >> 24) & 1) << 1);
        return Some(Inst {
            len: 4,
            text: format!("blx {target:#x}"),
            branch: Some(Branch {
                target,
                switch: true,
                link: true,
            }),
            literal: None,
        });
    }
    if w & 0x0D70_F000 == 0x0550_F000 {
        let (rn, u) = ((w >> 16) & 15, (w >> 23) & 1 == 1);
        let off = if (w >> 25) & 1 == 0 {
            let v = w & 0xFFF;
            (v != 0 || !u).then(|| signed(u, v))
        } else {
            if (w >> 4) & 1 == 1 {
                return None;
            }
            Some(format!(
                "{}{}{}",
                if u { "" } else { "-" },
                reg(w & 15),
                shift_imm(w)
            ))
        };
        return Some(plain(4, format!("pld {}", address(rn, true, false, off))));
    }
    match (w >> 24) & 0xF {
        0xC | 0xD => coproc_transfer(w, "", "2"),
        0xE if (w >> 4) & 1 == 1 => Some(plain(
            4,
            format!(
                "{}2 p{}, #{}, {}, c{}, c{}, #{}",
                if (w >> 20) & 1 == 1 { "mrc" } else { "mcr" },
                (w >> 8) & 15,
                (w >> 21) & 7,
                coproc_reg((w >> 12) & 15, (w >> 20) & 1 == 1),
                (w >> 16) & 15,
                w & 15,
                (w >> 5) & 7
            ),
        )),
        0xE => Some(plain(
            4,
            format!(
                "cdp2 p{}, #{}, c{}, c{}, c{}, #{}",
                (w >> 8) & 15,
                (w >> 20) & 15,
                (w >> 12) & 15,
                (w >> 16) & 15,
                w & 15,
                (w >> 5) & 7
            ),
        )),
        _ => None,
    }
}

/// Decodes one Thumb instruction at `pc`; `next` is the halfword after it
/// (a BL or BLX takes two).
pub fn thumb(h: u16, next: Option<u16>, pc: u32) -> Option<Inst> {
    let h = h as u32;
    let (r0, r3, r6, r8) = (h & 7, (h >> 3) & 7, (h >> 6) & 7, (h >> 8) & 7);
    let text = |s: String| Some(plain(2, s));
    match h >> 13 {
        0 if (h >> 11) & 3 != 3 => {
            let (kind, amount) = ((h >> 11) & 3, (h >> 6) & 0x1F);
            if kind == 0 && amount == 0 {
                return text(format!("movs {}, {}", reg(r0), reg(r3)));
            }
            let amount = if amount == 0 { 32 } else { amount };
            let name = ["lsls", "lsrs", "asrs"][kind as usize];
            text(format!("{name} {}, {}, #{amount}", reg(r0), reg(r3)))
        }
        0 => {
            let name = if (h >> 9) & 1 == 1 { "subs" } else { "adds" };
            let operand = if (h >> 10) & 1 == 1 {
                format!("#{r6}")
            } else {
                reg(r6).to_string()
            };
            text(format!("{name} {}, {}, {operand}", reg(r0), reg(r3)))
        }
        1 => {
            let name = ["movs", "cmp", "adds", "subs"][((h >> 11) & 3) as usize];
            text(format!("{name} {}, {}", reg(r8), imm(h & 0xFF)))
        }
        2 if h >> 10 == 0b010000 => {
            let (rd, rm) = (reg(r0), reg(r3));
            text(match (h >> 6) & 15 {
                9 => format!("rsbs {rd}, {rm}, #0"),
                13 => format!("muls {rd}, {rm}, {rd}"),
                op => {
                    let name = [
                        "ands", "eors", "lsls", "lsrs", "asrs", "adcs", "sbcs", "rors", "tst", "",
                        "cmp", "cmn", "orrs", "", "bics", "mvns",
                    ][op as usize];
                    format!("{name} {rd}, {rm}")
                }
            })
        }
        2 if h >> 10 == 0b010001 => {
            let rm = reg((h >> 3) & 15);
            let rd = reg((h & 7) | ((h >> 4) & 8));
            match (h >> 8) & 3 {
                0 => text(format!("add {rd}, {rm}")),
                1 => text(format!("cmp {rd}, {rm}")),
                2 => text(format!("mov {rd}, {rm}")),
                _ if (h >> 7) & 1 == 1 => text(format!("blx {rm}")),
                _ => text(format!("bx {rm}")),
            }
        }
        2 if h >> 11 == 0b01001 => {
            let v = (h & 0xFF) * 4;
            let mut inst = plain(2, format!("ldr {}, [pc, {}]", reg(r8), imm(v)));
            inst.literal = Some((pc.wrapping_add(4) & !3).wrapping_add(v));
            Some(inst)
        }
        2 if (h >> 12) & 1 == 1 => {
            let name = [
                "str", "strh", "strb", "ldrsb", "ldr", "ldrh", "ldrb", "ldrsh",
            ][((h >> 9) & 7) as usize];
            text(format!("{name} {}, [{}, {}]", reg(r0), reg(r3), reg(r6)))
        }
        2 => None,
        3 => {
            let (byte, load) = ((h >> 12) & 1 == 1, (h >> 11) & 1 == 1);
            let v = ((h >> 6) & 0x1F) * if byte { 1 } else { 4 };
            let name = format!(
                "{}{}",
                if load { "ldr" } else { "str" },
                if byte { "b" } else { "" }
            );
            text(format!("{name} {}, {}", reg(r0), offset(r3, v)))
        }
        4 if (h >> 12) & 1 == 0 => {
            let name = if (h >> 11) & 1 == 1 { "ldrh" } else { "strh" };
            text(format!(
                "{name} {}, {}",
                reg(r0),
                offset(r3, ((h >> 6) & 0x1F) * 2)
            ))
        }
        4 => {
            let name = if (h >> 11) & 1 == 1 { "ldr" } else { "str" };
            text(format!(
                "{name} {}, {}",
                reg(r8),
                offset(13, (h & 0xFF) * 4)
            ))
        }
        5 if (h >> 12) & 1 == 0 => {
            let v = (h & 0xFF) * 4;
            if (h >> 11) & 1 == 1 {
                text(format!("add {}, sp, {}", reg(r8), imm(v)))
            } else {
                text(format!("adr {}, {}", reg(r8), imm(v)))
            }
        }
        5 if h >> 8 == 0xB0 => {
            let name = if (h >> 7) & 1 == 1 { "sub" } else { "add" };
            text(format!("{name} sp, {}", imm((h & 0x7F) * 4)))
        }
        5 if h & 0x0600 == 0x0400 => {
            let load = (h >> 11) & 1 == 1;
            let extra = if (h >> 8) & 1 == 1 {
                1 << if load { 15 } else { 14 }
            } else {
                0
            };
            let mask = (h & 0xFF) | extra;
            if mask == 0 {
                return None;
            }
            text(format!(
                "{} {}",
                if load { "pop" } else { "push" },
                list(mask)
            ))
        }
        5 if h >> 8 == 0xBE => text(format!("bkpt {}", imm(h & 0xFF))),
        5 => None,
        6 if (h >> 12) & 1 == 0 => {
            let mask = h & 0xFF;
            if mask == 0 {
                return None;
            }
            let load = (h >> 11) & 1 == 1;
            let bang = if load && mask & (1 << r8) != 0 {
                ""
            } else {
                "!"
            };
            text(format!(
                "{} {}{bang}, {}",
                if load { "ldm" } else { "stm" },
                reg(r8),
                list(mask)
            ))
        }
        6 => match (h >> 8) & 15 {
            0xE => None,
            0xF => text(format!("svc {}", imm(h & 0xFF))),
            cond => {
                let target = pc
                    .wrapping_add(4)
                    .wrapping_add((sext(h & 0xFF, 8) << 1) as u32);
                Some(Inst {
                    len: 2,
                    text: format!("b{} {target:#x}", COND[cond as usize]),
                    branch: Some(Branch {
                        target,
                        switch: false,
                        link: false,
                    }),
                    literal: None,
                })
            }
        },
        _ => match (h >> 11) & 3 {
            0 => {
                let target = pc
                    .wrapping_add(4)
                    .wrapping_add((sext(h & 0x7FF, 11) << 1) as u32);
                Some(Inst {
                    len: 2,
                    text: format!("b {target:#x}"),
                    branch: Some(Branch {
                        target,
                        switch: false,
                        link: false,
                    }),
                    literal: None,
                })
            }
            2 => {
                let n = next? as u32;
                let switch = match n >> 11 {
                    0b11111 => false,
                    0b11101 if n & 1 == 0 => true,
                    _ => return None,
                };
                let offset = (sext(h & 0x7FF, 11) << 12) | (((n & 0x7FF) << 1) as i32);
                let mut target = pc.wrapping_add(4).wrapping_add(offset as u32);
                if switch {
                    target &= !3;
                }
                Some(Inst {
                    len: 4,
                    text: format!("{} {target:#x}", if switch { "blx" } else { "bl" }),
                    branch: Some(Branch {
                        target,
                        switch,
                        link: true,
                    }),
                    literal: None,
                })
            }
            _ => None,
        },
    }
}

fn offset(rn: u32, v: u32) -> String {
    if v == 0 {
        format!("[{}]", reg(rn))
    } else {
        format!("[{}, {}]", reg(rn), imm(v))
    }
}

/// Decodes the instruction at `at` of `code` (loaded at `base`) in the given
/// instruction set.
pub fn decode(code: &[u8], base: u32, at: u32, thumb_mode: bool) -> Option<Inst> {
    let off = at.checked_sub(base)? as usize;
    if thumb_mode {
        let h = u16::from_le_bytes(code.get(off..off + 2)?.try_into().ok()?);
        let next = code
            .get(off + 2..off + 4)
            .map(|b| u16::from_le_bytes([b[0], b[1]]));
        thumb(h, next, at)
    } else {
        let w = u32::from_le_bytes(code.get(off..off + 4)?.try_into().ok()?);
        arm(w, at)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a(w: u32, pc: u32) -> String {
        arm(w, pc).map_or("-".into(), |i| i.text)
    }

    #[test]
    fn arm_forms() {
        // the Blaze Black hook and its code (ARM9 0x02004660 and 0x02004ea4)
        assert_eq!(a(0xE59F003C, 0x02004660), "ldr r0, [pc, #0x3c]");
        assert_eq!(
            arm(0xE59F003C, 0x02004660).unwrap().literal,
            Some(0x020046A4)
        );
        assert_eq!(a(0x05801008, 0), "streq r1, [r0, #8]");
        assert_eq!(a(0xE1510002, 0), "cmp r1, r2");
        assert_eq!(a(0x03A01001, 0), "moveq r1, #1");
        assert_eq!(a(0x05C0100A, 0), "strbeq r1, [r0, #0xa]");
        assert_eq!(a(0xE12FFF1E, 0), "bx lr");
        let b = arm(0xEAFFFDED, 0x02004EA4).unwrap();
        assert_eq!(b.text, "b 0x2004660");
        assert_eq!(b.branch.unwrap().target, 0x02004660);
        assert_eq!(a(0xE8BD01F0, 0), "pop {r4, r5, r6, r7, r8}");
        assert_eq!(a(0xE92D4000, 0), "stmdb sp!, {lr}");
        assert_eq!(a(0xE1A00102, 0), "lsl r0, r2, #2");
        assert_eq!(a(0xEE070F9A, 0), "mcr p15, #0, r0, c7, c10, #4");
        assert_eq!(a(0xE16F0F11, 0), "clz r0, r1");
        // ARMv6 forms are data on this CPU
        assert_eq!(a(0xE1900F9F, 0), "-"); // ldrex
        assert_eq!(a(0xE3001234, 0), "-"); // movw
    }

    #[test]
    fn thumb_forms() {
        let t = |h: u16, n: Option<u16>, pc: u32| thumb(h, n, pc).map_or("-".into(), |i| i.text);
        assert_eq!(t(0xDF14, None, 0), "svc #0x14");
        assert_eq!(t(0x4770, None, 0), "bx lr");
        assert_eq!(t(0xB510, None, 0), "push {r4, lr}");
        assert_eq!(t(0x4801, None, 0x100), "ldr r0, [pc, #4]");
        assert_eq!(thumb(0x4801, None, 0x102).unwrap().literal, Some(0x108));
        let bl = thumb(0xF000, Some(0xF802), 0x1000).unwrap();
        assert_eq!((bl.len, bl.branch.unwrap().target), (4, 0x1008));
        // a lone BL half, and a Thumb-2 form, are data
        assert_eq!(t(0xF800, None, 0), "-");
        assert_eq!(t(0xF000, Some(0x0000), 0), "-");
    }
}

//! Battle particle resources: the members of a/0/0/6 (" APS12_1"), per
//! eidos research kenon-needs/spa-particles.md (from aletheia's
//! spl_emitter.c and particle.c and the game's files).
//!
//! A file is a 0x20-byte header, then one resource block per emitter, then
//! the texture blocks. A resource block is a 0x58-byte header, then the
//! sub-blocks its flags name (scale, color, alpha and texture animation,
//! child), then its behaviors (gravity, random, magnet, spin, collision
//! plane, convergence). Every block is kept as its bytes, and an edit
//! changes only the bits of its field, so a file reads back byte for byte
//! and the bytes the game does not read stay as they were.

use serde::{Deserialize, Serialize};

use crate::bridge::Error;

const FILE_HEADER: usize = 0x20;
const RES_HEADER: usize = 0x58;
const TEX_HEADER: usize = 0x20;

/// The optional sub-blocks of a resource: flag bit and size, in file order.
pub const SUBBLOCKS: [(&str, u32, usize); 5] = [
    ("scaleAnim", 8, 0x0C),
    ("colorAnim", 9, 0x0C),
    ("alphaAnim", 10, 0x08),
    ("texAnim", 11, 0x0C),
    ("child", 16, 0x14),
];
/// The behaviors: flag bit and size, in file order.
pub const BEHAVIORS: [(&str, u32, usize); 6] = [
    ("gravity", 24, 0x08),
    ("random", 25, 0x08),
    ("magnet", 26, 0x10),
    ("spin", 27, 0x04),
    ("collision", 28, 0x08),
    ("convergence", 29, 0x10),
];

/// One emitter: its header, and each optional block it has, by name.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Resource {
    pub header: Vec<u8>,
    /// SUBBLOCKS then BEHAVIORS order: (name, bytes) for those present.
    pub blocks: Vec<(String, Vec<u8>)>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Spa {
    /// The file header's first 8 bytes (" APS12_1") and its reserved words.
    pub magic: Vec<u8>,
    pub reserved: [u32; 2],
    pub resources: Vec<Resource>,
    /// Whole texture blocks (header, texels, palette).
    pub textures: Vec<Vec<u8>>,
}

fn rd16(b: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(at..at + 2)?.try_into().ok()?))
}
fn rd32(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?))
}
fn bad(m: impl Into<String>) -> Error {
    Error::Format(m.into())
}

pub fn parse(b: &[u8]) -> Result<Spa, Error> {
    if b.len() < FILE_HEADER || &b[..4] != b" APS" {
        return Err(bad("not a particle resource (no \" APS\" header)"));
    }
    let res_count = rd16(b, 8).unwrap() as usize;
    let tex_count = rd16(b, 10).unwrap() as usize;
    let mut at = FILE_HEADER;
    let mut resources = Vec::new();
    for r in 0..res_count {
        let header = b
            .get(at..at + RES_HEADER)
            .ok_or_else(|| bad(format!("emitter {r} is cut short")))?
            .to_vec();
        let flags = rd32(&header, 0).unwrap();
        at += RES_HEADER;
        let mut blocks = Vec::new();
        for &(name, bit, size) in SUBBLOCKS.iter().chain(BEHAVIORS.iter()) {
            if flags & (1 << bit) != 0 {
                let block = b
                    .get(at..at + size)
                    .ok_or_else(|| bad(format!("emitter {r}: {name} is cut short")))?;
                blocks.push((name.to_string(), block.to_vec()));
                at += size;
            }
        }
        resources.push(Resource { header, blocks });
    }
    let mut textures = Vec::new();
    for t in 0..tex_count {
        let size =
            rd32(b, at + 0x1C).ok_or_else(|| bad(format!("texture {t} is cut short")))? as usize;
        let block = b
            .get(at..at + size)
            .filter(|_| size >= TEX_HEADER)
            .ok_or_else(|| bad(format!("texture {t} is cut short")))?;
        textures.push(block.to_vec());
        at += size;
    }
    if at != b.len() {
        return Err(bad(format!(
            "{} bytes after the last texture",
            b.len() - at
        )));
    }
    Ok(Spa {
        magic: b[..8].to_vec(),
        reserved: [rd32(b, 0x0C).unwrap(), rd32(b, 0x1C).unwrap()],
        resources,
        textures,
    })
}

pub fn build(s: &Spa) -> Vec<u8> {
    let res: Vec<u8> = s
        .resources
        .iter()
        .flat_map(|r| {
            let mut v = r.header.clone();
            for (_, b) in &r.blocks {
                v.extend_from_slice(b);
            }
            v
        })
        .collect();
    let tex: Vec<u8> = s.textures.concat();
    let mut out = s.magic.clone();
    out.extend_from_slice(&(s.resources.len() as u16).to_le_bytes());
    out.extend_from_slice(&(s.textures.len() as u16).to_le_bytes());
    out.extend_from_slice(&s.reserved[0].to_le_bytes());
    out.extend_from_slice(&(res.len() as u32).to_le_bytes());
    out.extend_from_slice(&(tex.len() as u32).to_le_bytes());
    out.extend_from_slice(&((FILE_HEADER + res.len()) as u32).to_le_bytes());
    out.extend_from_slice(&s.reserved[1].to_le_bytes());
    out.extend(res);
    out.extend(tex);
    out
}

// ---- fields ------------------------------------------------------------

/// How a field's raw bits read.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Kind {
    /// Unsigned integer.
    Uint,
    /// Signed integer.
    Int,
    /// 20.12 or 4.12 fixed point (signed): value / 4096.
    Fixed,
    /// 0x10000 = one turn.
    Angle,
    /// RGB555.
    Color,
    /// One of `choices`.
    Choice,
    Flag,
}

/// One editable field: in which block, where, and how it reads.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Field {
    /// "header" or a SUBBLOCKS / BEHAVIORS name.
    pub block: &'static str,
    pub key: &'static str,
    pub label: &'static str,
    pub group: &'static str,
    /// Byte offset of the word, word size in bytes, then bit range in it.
    pub at: usize,
    pub size: usize,
    pub bit: u32,
    pub width: u32,
    pub kind: Kind,
    pub choices: &'static [&'static str],
    pub note: &'static str,
}

#[allow(clippy::too_many_arguments)] // one table row per call
const fn f(
    block: &'static str,
    key: &'static str,
    label: &'static str,
    group: &'static str,
    at: usize,
    size: usize,
    bit: u32,
    width: u32,
    kind: Kind,
) -> Field {
    Field {
        block,
        key,
        label,
        group,
        at,
        size,
        bit,
        width,
        kind,
        choices: &[],
        note: "",
    }
}
const fn c(mut x: Field, choices: &'static [&'static str]) -> Field {
    x.choices = choices;
    x
}
const fn n(mut x: Field, note: &'static str) -> Field {
    x.note = note;
    x
}

use Kind::*;

pub const SHAPES: &[&str] = &[
    "Point",
    "Sphere surface",
    "Circle edge",
    "Circle edge (even)",
    "Sphere",
    "Disc",
    "Cylinder surface",
    "Cylinder",
    "Hemisphere surface",
    "Hemisphere",
];
pub const DRAW: &[&str] = &[
    "Billboard",
    "Stretched billboard",
    "Polygon",
    "Directional polygon",
];
pub const AXES: &[&str] = &["Z axis", "Y axis", "X axis", "Emitter axis"];
pub const SCALE_DIR: &[&str] = &["Both", "Width only", "Height only"];

/// Every field kenon edits, from the research doc.
pub const FIELDS: &[Field] = &[
    // Emission
    c(
        f(
            "header", "shape", "Shape", "Emission", 0x00, 4, 0, 4, Choice,
        ),
        SHAPES,
    ),
    c(
        f(
            "header",
            "circleAxis",
            "Circle axis",
            "Emission",
            0x00,
            4,
            6,
            2,
            Choice,
        ),
        AXES,
    ),
    f(
        "header", "posX", "Offset X", "Emission", 0x04, 4, 0, 32, Fixed,
    ),
    f(
        "header", "posY", "Offset Y", "Emission", 0x08, 4, 0, 32, Fixed,
    ),
    f(
        "header", "posZ", "Offset Z", "Emission", 0x0C, 4, 0, 32, Fixed,
    ),
    n(
        f(
            "header",
            "count",
            "Particles per emission",
            "Emission",
            0x10,
            4,
            0,
            32,
            Fixed,
        ),
        "Fractions carry over: 1.5 emits 1, 2, 1, 2.",
    ),
    f(
        "header", "radius", "Radius", "Emission", 0x14, 4, 0, 32, Fixed,
    ),
    f(
        "header",
        "length",
        "Cylinder length",
        "Emission",
        0x18,
        4,
        0,
        32,
        Fixed,
    ),
    f(
        "header", "axisX", "Axis X", "Emission", 0x1C, 2, 0, 16, Fixed,
    ),
    f(
        "header", "axisY", "Axis Y", "Emission", 0x1E, 2, 0, 16, Fixed,
    ),
    f(
        "header", "axisZ", "Axis Z", "Emission", 0x20, 2, 0, 16, Fixed,
    ),
    n(
        f(
            "header",
            "interval",
            "Emission interval (frames)",
            "Emission",
            0x44,
            1,
            0,
            8,
            Uint,
        ),
        "1 or more.",
    ),
    f(
        "header",
        "startDelay",
        "Start delay (frames)",
        "Emission",
        0x32,
        2,
        0,
        16,
        Uint,
    ),
    n(
        f(
            "header",
            "emitterLife",
            "Emitter lifetime (frames)",
            "Emission",
            0x3C,
            2,
            0,
            16,
            Uint,
        ),
        "0: forever.",
    ),
    f(
        "header",
        "selfMaintaining",
        "Auto-stop",
        "Emission",
        0x00,
        4,
        14,
        1,
        Flag,
    ),
    // Motion
    f(
        "header",
        "speedOut",
        "Initial outward speed",
        "Motion",
        0x24,
        4,
        0,
        32,
        Fixed,
    ),
    f(
        "header",
        "speedAxis",
        "Initial axis speed",
        "Motion",
        0x28,
        4,
        0,
        32,
        Fixed,
    ),
    n(
        f(
            "header",
            "spreadSpeed",
            "Speed spread (0-255)",
            "Motion",
            0x42,
            1,
            0,
            8,
            Uint,
        ),
        "Applies to both initial speeds.",
    ),
    n(
        f(
            "header",
            "airResistance",
            "Drag",
            "Motion",
            0x46,
            1,
            0,
            8,
            Uint,
        ),
        "128 is no drag. Lower slows, higher speeds up.",
    ),
    f(
        "header",
        "followEmitter",
        "Follow emitter",
        "Motion",
        0x00,
        4,
        15,
        1,
        Flag,
    ),
    // Particle
    f(
        "header",
        "life",
        "Lifetime (frames)",
        "Particle",
        0x3E,
        2,
        0,
        16,
        Uint,
    ),
    n(
        f(
            "header",
            "spreadLife",
            "Lifetime spread (0-255)",
            "Particle",
            0x41,
            1,
            0,
            8,
            Uint,
        ),
        "Randomly shortens lifetime.",
    ),
    n(
        f("header", "scale", "Size", "Particle", 0x2C, 4, 0, 32, Fixed),
        "The quad is 2 units across at size 1.",
    ),
    f(
        "header",
        "spreadScale",
        "Size spread (0-255)",
        "Particle",
        0x40,
        1,
        0,
        8,
        Uint,
    ),
    f(
        "header",
        "aspect",
        "Aspect ratio",
        "Particle",
        0x30,
        2,
        0,
        16,
        Fixed,
    ),
    f(
        "header", "color", "Color", "Particle", 0x22, 2, 0, 15, Color,
    ),
    n(
        f(
            "header",
            "alpha",
            "Opacity (0-31)",
            "Particle",
            0x45,
            1,
            0,
            8,
            Uint,
        ),
        "31: opaque. 0 hides the particle.",
    ),
    c(
        f(
            "header",
            "draw",
            "Render mode",
            "Particle",
            0x00,
            4,
            4,
            2,
            Choice,
        ),
        DRAW,
    ),
    f(
        "header", "texture", "Texture", "Particle", 0x47, 1, 0, 8, Uint,
    ),
    f(
        "header",
        "tileS",
        "Tile X (power of 2)",
        "Particle",
        0x48,
        4,
        24,
        2,
        Uint,
    ),
    f(
        "header",
        "tileT",
        "Tile Y (power of 2)",
        "Particle",
        0x48,
        4,
        26,
        2,
        Uint,
    ),
    f(
        "header",
        "flipS",
        "Mirror texture X",
        "Particle",
        0x4C,
        4,
        0,
        1,
        Flag,
    ),
    f(
        "header",
        "flipT",
        "Mirror texture Y",
        "Particle",
        0x4C,
        4,
        1,
        1,
        Flag,
    ),
    f(
        "header",
        "dbbScale",
        "Billboard stretch",
        "Particle",
        0x48,
        4,
        8,
        16,
        Fixed,
    ),
    f(
        "header",
        "polyX",
        "Quad offset X",
        "Particle",
        0x50,
        2,
        0,
        16,
        Fixed,
    ),
    f(
        "header",
        "polyY",
        "Quad offset Y",
        "Particle",
        0x52,
        2,
        0,
        16,
        Fixed,
    ),
    f(
        "header",
        "viewSpace",
        "View-space offset",
        "Particle",
        0x00,
        4,
        23,
        1,
        Flag,
    ),
    // Rotation
    f(
        "header",
        "hasRotation",
        "Rotate",
        "Rotation",
        0x00,
        4,
        12,
        1,
        Flag,
    ),
    f(
        "header",
        "minRotation",
        "Min rotation speed",
        "Rotation",
        0x34,
        2,
        0,
        16,
        Int,
    ),
    f(
        "header",
        "maxRotation",
        "Max rotation speed",
        "Rotation",
        0x36,
        2,
        0,
        16,
        Int,
    ),
    f(
        "header",
        "randomAngle",
        "Random start angle",
        "Rotation",
        0x00,
        4,
        13,
        1,
        Flag,
    ),
    f(
        "header",
        "initAngle",
        "Start angle",
        "Rotation",
        0x38,
        2,
        0,
        16,
        Angle,
    ),
    c(
        f(
            "header",
            "polyAxis",
            "Polygon rotation axis",
            "Rotation",
            0x00,
            4,
            17,
            2,
            Choice,
        ),
        &["Y axis", "Diagonal"],
    ),
    c(
        f(
            "header",
            "polyPlane",
            "Polygon plane",
            "Rotation",
            0x00,
            4,
            19,
            1,
            Choice,
        ),
        &["XY", "XZ"],
    ),
    // Animation
    n(
        f(
            "header",
            "loopFrames",
            "Loop length (frames)",
            "Animation",
            0x48,
            4,
            0,
            8,
            Uint,
        ),
        "1 or more.",
    ),
    f(
        "header",
        "randomLoop",
        "Random loop phase",
        "Animation",
        0x00,
        4,
        20,
        1,
        Flag,
    ),
    c(
        f(
            "header",
            "scaleDir",
            "Size animation axes",
            "Animation",
            0x48,
            4,
            28,
            3,
            Choice,
        ),
        SCALE_DIR,
    ),
    f(
        "scaleAnim",
        "scaleStart",
        "Start size",
        "Size animation",
        0x00,
        2,
        0,
        16,
        Fixed,
    ),
    f(
        "scaleAnim",
        "scaleMid",
        "Mid size",
        "Size animation",
        0x02,
        2,
        0,
        16,
        Fixed,
    ),
    f(
        "scaleAnim",
        "scaleEnd",
        "End size",
        "Size animation",
        0x04,
        2,
        0,
        16,
        Fixed,
    ),
    f(
        "scaleAnim",
        "scaleIn",
        "Mid start (0-255)",
        "Size animation",
        0x06,
        1,
        0,
        8,
        Uint,
    ),
    f(
        "scaleAnim",
        "scaleOut",
        "Mid end (0-255)",
        "Size animation",
        0x07,
        1,
        0,
        8,
        Uint,
    ),
    f(
        "scaleAnim",
        "scaleLoop",
        "Loop",
        "Size animation",
        0x08,
        2,
        0,
        1,
        Flag,
    ),
    f(
        "colorAnim",
        "colorStart",
        "Start color",
        "Color animation",
        0x00,
        2,
        0,
        15,
        Color,
    ),
    f(
        "colorAnim",
        "colorEnd",
        "End color",
        "Color animation",
        0x02,
        2,
        0,
        15,
        Color,
    ),
    f(
        "colorAnim",
        "colorIn",
        "Start color until (0-255)",
        "Color animation",
        0x04,
        1,
        0,
        8,
        Uint,
    ),
    f(
        "colorAnim",
        "colorPeak",
        "Main color at (0-255)",
        "Color animation",
        0x05,
        1,
        0,
        8,
        Uint,
    ),
    f(
        "colorAnim",
        "colorOut",
        "End color from (0-255)",
        "Color animation",
        0x06,
        1,
        0,
        8,
        Uint,
    ),
    f(
        "colorAnim",
        "colorRandom",
        "Random of 3 colors",
        "Color animation",
        0x08,
        2,
        0,
        1,
        Flag,
    ),
    f(
        "colorAnim",
        "colorLoop",
        "Loop",
        "Color animation",
        0x08,
        2,
        1,
        1,
        Flag,
    ),
    f(
        "colorAnim",
        "colorBlend",
        "Blend colors",
        "Color animation",
        0x08,
        2,
        2,
        1,
        Flag,
    ),
    f(
        "alphaAnim",
        "alphaStart",
        "Start opacity (0-31)",
        "Opacity animation",
        0x00,
        2,
        0,
        5,
        Uint,
    ),
    f(
        "alphaAnim",
        "alphaMid",
        "Mid opacity (0-31)",
        "Opacity animation",
        0x00,
        2,
        5,
        5,
        Uint,
    ),
    f(
        "alphaAnim",
        "alphaEnd",
        "End opacity (0-31)",
        "Opacity animation",
        0x00,
        2,
        10,
        5,
        Uint,
    ),
    f(
        "alphaAnim",
        "alphaFlicker",
        "Flicker (0-255)",
        "Opacity animation",
        0x02,
        2,
        0,
        8,
        Uint,
    ),
    f(
        "alphaAnim",
        "alphaLoop",
        "Loop",
        "Opacity animation",
        0x02,
        2,
        8,
        1,
        Flag,
    ),
    f(
        "alphaAnim",
        "alphaIn",
        "Mid start (0-255)",
        "Opacity animation",
        0x04,
        1,
        0,
        8,
        Uint,
    ),
    f(
        "alphaAnim",
        "alphaOut",
        "Mid end (0-255)",
        "Opacity animation",
        0x05,
        1,
        0,
        8,
        Uint,
    ),
    f(
        "texAnim",
        "frame0",
        "Frame 1",
        "Texture animation",
        0x00,
        1,
        0,
        8,
        Uint,
    ),
    f(
        "texAnim",
        "frame1",
        "Frame 2",
        "Texture animation",
        0x01,
        1,
        0,
        8,
        Uint,
    ),
    f(
        "texAnim",
        "frame2",
        "Frame 3",
        "Texture animation",
        0x02,
        1,
        0,
        8,
        Uint,
    ),
    f(
        "texAnim",
        "frame3",
        "Frame 4",
        "Texture animation",
        0x03,
        1,
        0,
        8,
        Uint,
    ),
    f(
        "texAnim",
        "frame4",
        "Frame 5",
        "Texture animation",
        0x04,
        1,
        0,
        8,
        Uint,
    ),
    f(
        "texAnim",
        "frame5",
        "Frame 6",
        "Texture animation",
        0x05,
        1,
        0,
        8,
        Uint,
    ),
    f(
        "texAnim",
        "frame6",
        "Frame 7",
        "Texture animation",
        0x06,
        1,
        0,
        8,
        Uint,
    ),
    f(
        "texAnim",
        "frame7",
        "Frame 8",
        "Texture animation",
        0x07,
        1,
        0,
        8,
        Uint,
    ),
    n(
        f(
            "texAnim",
            "frames",
            "Frame count (1-8)",
            "Texture animation",
            0x08,
            4,
            0,
            8,
            Uint,
        ),
        "1 to 8.",
    ),
    n(
        f(
            "texAnim",
            "step",
            "Frame duration (0-255 of life)",
            "Texture animation",
            0x08,
            4,
            8,
            8,
            Uint,
        ),
        "In life units: 4 frames over a life use 64.",
    ),
    f(
        "texAnim",
        "texRandom",
        "Random frame per particle",
        "Texture animation",
        0x08,
        4,
        16,
        1,
        Flag,
    ),
    f(
        "texAnim",
        "texLoop",
        "Loop",
        "Texture animation",
        0x08,
        4,
        17,
        1,
        Flag,
    ),
    // Children
    f(
        "child",
        "childLife",
        "Lifetime (frames)",
        "Children",
        0x06,
        2,
        0,
        16,
        Uint,
    ),
    f(
        "child",
        "childCount",
        "Children per emission",
        "Children",
        0x0C,
        1,
        0,
        8,
        Uint,
    ),
    f(
        "child",
        "childDelay",
        "Start delay (0-255 of parent life)",
        "Children",
        0x0D,
        1,
        0,
        8,
        Uint,
    ),
    n(
        f(
            "child",
            "childInterval",
            "Emission interval (frames)",
            "Children",
            0x0E,
            1,
            0,
            8,
            Uint,
        ),
        "1 or more.",
    ),
    f(
        "child",
        "childTexture",
        "Texture",
        "Children",
        0x0F,
        1,
        0,
        8,
        Uint,
    ),
    f(
        "child",
        "childSpeed",
        "Inherited speed (0-255)",
        "Children",
        0x08,
        1,
        0,
        8,
        Uint,
    ),
    n(
        f(
            "child",
            "childScale",
            "Size (63 = parent)",
            "Children",
            0x09,
            1,
            0,
            8,
            Uint,
        ),
        "Parent size * (value + 1) / 64.",
    ),
    f(
        "child",
        "childRandom",
        "Random speed",
        "Children",
        0x02,
        2,
        0,
        16,
        Fixed,
    ),
    f(
        "child",
        "childUseColor",
        "Custom color",
        "Children",
        0x00,
        2,
        6,
        1,
        Flag,
    ),
    f(
        "child",
        "childColor",
        "Color",
        "Children",
        0x0A,
        2,
        0,
        15,
        Color,
    ),
    f(
        "child",
        "childScaleAnim",
        "Animate size",
        "Children",
        0x00,
        2,
        1,
        1,
        Flag,
    ),
    f(
        "child",
        "childEndScale",
        "End size",
        "Children",
        0x04,
        2,
        0,
        16,
        Fixed,
    ),
    f(
        "child",
        "childFade",
        "Fade out",
        "Children",
        0x00,
        2,
        2,
        1,
        Flag,
    ),
    f(
        "child",
        "childBehaviors",
        "Apply behaviors",
        "Children",
        0x00,
        2,
        0,
        1,
        Flag,
    ),
    c(
        f(
            "child",
            "childRotation",
            "Rotation",
            "Children",
            0x00,
            2,
            3,
            2,
            Choice,
        ),
        &["None", "Parent's angle", "Parent's angle and spin"],
    ),
    f(
        "child",
        "childFollow",
        "Follow emitter",
        "Children",
        0x00,
        2,
        5,
        1,
        Flag,
    ),
    c(
        f(
            "child",
            "childDraw",
            "Render mode",
            "Children",
            0x00,
            2,
            7,
            2,
            Choice,
        ),
        DRAW,
    ),
    f(
        "header",
        "childrenFirst",
        "Draw children first",
        "Children",
        0x00,
        4,
        21,
        1,
        Flag,
    ),
    f(
        "header",
        "hideParent",
        "Hide parent",
        "Children",
        0x00,
        4,
        22,
        1,
        Flag,
    ),
    // Behaviors
    f(
        "gravity",
        "gravityX",
        "Gravity X",
        "Gravity",
        0x00,
        2,
        0,
        16,
        Fixed,
    ),
    f(
        "gravity",
        "gravityY",
        "Gravity Y",
        "Gravity",
        0x02,
        2,
        0,
        16,
        Fixed,
    ),
    f(
        "gravity",
        "gravityZ",
        "Gravity Z",
        "Gravity",
        0x04,
        2,
        0,
        16,
        Fixed,
    ),
    f(
        "random",
        "randomX",
        "Force X",
        "Random force",
        0x00,
        2,
        0,
        16,
        Fixed,
    ),
    f(
        "random",
        "randomY",
        "Force Y",
        "Random force",
        0x02,
        2,
        0,
        16,
        Fixed,
    ),
    f(
        "random",
        "randomZ",
        "Force Z",
        "Random force",
        0x04,
        2,
        0,
        16,
        Fixed,
    ),
    n(
        f(
            "random",
            "randomEvery",
            "Interval (frames)",
            "Random force",
            0x06,
            2,
            0,
            16,
            Uint,
        ),
        "1 or more.",
    ),
    f(
        "magnet", "magnetX", "Target X", "Magnet", 0x00, 4, 0, 32, Fixed,
    ),
    f(
        "magnet", "magnetY", "Target Y", "Magnet", 0x04, 4, 0, 32, Fixed,
    ),
    f(
        "magnet", "magnetZ", "Target Z", "Magnet", 0x08, 4, 0, 32, Fixed,
    ),
    f(
        "magnet",
        "magnetForce",
        "Force",
        "Magnet",
        0x0C,
        2,
        0,
        16,
        Fixed,
    ),
    f(
        "spin",
        "spinAngle",
        "Angle per frame",
        "Orbit",
        0x00,
        2,
        0,
        16,
        Angle,
    ),
    c(
        f("spin", "spinAxis", "Axis", "Orbit", 0x02, 2, 0, 16, Choice),
        &["X axis", "Y axis", "Z axis"],
    ),
    f(
        "collision",
        "floorY",
        "Floor height",
        "Floor",
        0x00,
        4,
        0,
        32,
        Fixed,
    ),
    f(
        "collision",
        "bounce",
        "Bounce",
        "Floor",
        0x04,
        2,
        0,
        16,
        Fixed,
    ),
    c(
        f(
            "collision",
            "floorMode",
            "On contact",
            "Floor",
            0x06,
            2,
            0,
            2,
            Choice,
        ),
        &["Disappear", "Bounce"],
    ),
    f(
        "convergence",
        "pullX",
        "Target X",
        "Pull",
        0x00,
        4,
        0,
        32,
        Fixed,
    ),
    f(
        "convergence",
        "pullY",
        "Target Y",
        "Pull",
        0x04,
        4,
        0,
        32,
        Fixed,
    ),
    f(
        "convergence",
        "pullZ",
        "Target Z",
        "Pull",
        0x08,
        4,
        0,
        32,
        Fixed,
    ),
    f(
        "convergence",
        "pullForce",
        "Force",
        "Pull",
        0x0C,
        2,
        0,
        16,
        Fixed,
    ),
];

fn word(b: &[u8], at: usize, size: usize) -> u32 {
    match size {
        1 => b[at] as u32,
        2 => rd16(b, at).unwrap() as u32,
        _ => rd32(b, at).unwrap(),
    }
}
fn put_word(b: &mut [u8], at: usize, size: usize, v: u32) {
    match size {
        1 => b[at] = v as u8,
        2 => b[at..at + 2].copy_from_slice(&(v as u16).to_le_bytes()),
        _ => b[at..at + 4].copy_from_slice(&v.to_le_bytes()),
    }
}

/// A field's value: the bits as a signed or unsigned integer (fixed point
/// stays raw: 4096 = 1.0).
pub fn read(r: &Resource, field: &Field) -> Option<i64> {
    let bytes = if field.block == "header" {
        &r.header
    } else {
        &r.blocks.iter().find(|(n, _)| n == field.block)?.1
    };
    let w = word(bytes, field.at, field.size);
    let mask = if field.width == 32 {
        u32::MAX
    } else {
        (1u32 << field.width) - 1
    };
    let raw = (w >> field.bit) & mask;
    Some(match field.kind {
        Int | Fixed if field.width < 64 => {
            let shift = 32 - field.width;
            (((raw << shift) as i32) >> shift) as i64
        }
        _ => raw as i64,
    })
}

/// Sets a field's bits (the value is cut to the field's width).
pub fn write(r: &mut Resource, field: &Field, v: i64) -> Result<(), Error> {
    let bytes = if field.block == "header" {
        &mut r.header
    } else {
        &mut r
            .blocks
            .iter_mut()
            .find(|(n, _)| n == field.block)
            .ok_or_else(|| bad(format!("this emitter has no {}", field.block)))?
            .1
    };
    let mask = if field.width == 32 {
        u32::MAX
    } else {
        (1u32 << field.width) - 1
    };
    let w = word(bytes, field.at, field.size);
    let new = (w & !(mask << field.bit)) | (((v as u32) & mask) << field.bit);
    put_word(bytes, field.at, field.size, new);
    Ok(())
}

/// Adds or removes an optional block (and its flag bit). A new block starts
/// with sensible values: animations that do nothing, an interval of 1.
pub fn set_block(r: &mut Resource, name: &str, on: bool) -> Result<(), Error> {
    let order: Vec<(&str, u32, usize)> =
        SUBBLOCKS.iter().chain(BEHAVIORS.iter()).copied().collect();
    let &(_, bit, size) = order
        .iter()
        .find(|(n, _, _)| *n == name)
        .ok_or_else(|| bad(format!("no block {name}")))?;
    let mut flags = rd32(&r.header, 0).unwrap();
    let has = r.blocks.iter().any(|(n, _)| n == name);
    if on == has {
        return Ok(());
    }
    if on {
        let mut b = vec![0u8; size];
        match name {
            // Size 1.0 throughout, middle over the whole life.
            "scaleAnim" => {
                for at in [0, 2, 4] {
                    b[at..at + 2].copy_from_slice(&0x1000u16.to_le_bytes());
                }
                b[7] = 255;
            }
            "colorAnim" => {
                let c = rd16(&r.header, 0x22).unwrap();
                b[0..2].copy_from_slice(&c.to_le_bytes());
                b[2..4].copy_from_slice(&c.to_le_bytes());
                b[5] = 128;
                b[6] = 255;
            }
            "alphaAnim" => {
                b[0..2].copy_from_slice(&(31u16 | 31 << 5 | 31 << 10).to_le_bytes());
                b[5] = 255;
            }
            "texAnim" => b[8..12].copy_from_slice(&(1u32 | 255 << 8).to_le_bytes()),
            "child" => {
                b[6..8].copy_from_slice(&30u16.to_le_bytes());
                b[9] = 63;
                b[0x0C] = 1;
                b[0x0E] = 1;
            }
            "random" => b[6..8].copy_from_slice(&1u16.to_le_bytes()),
            "spin" => b[2..4].copy_from_slice(&1u16.to_le_bytes()),
            "collision" => b[6..8].copy_from_slice(&1u16.to_le_bytes()),
            _ => {}
        }
        // Keep the file order.
        let rank = |n: &str| order.iter().position(|(x, _, _)| *x == n).unwrap();
        let at = r
            .blocks
            .iter()
            .position(|(n, _)| rank(n) > rank(name))
            .unwrap_or(r.blocks.len());
        r.blocks.insert(at, (name.to_string(), b));
        flags |= 1 << bit;
    } else {
        r.blocks.retain(|(n, _)| n != name);
        flags &= !(1 << bit);
    }
    r.header[0..4].copy_from_slice(&flags.to_le_bytes());
    Ok(())
}

/// Checks the values the game divides by or indexes with (eidos research
/// 3.1-3.8): returns what is wrong, empty when nothing.
pub fn problems(s: &Spa) -> Vec<String> {
    let mut out = Vec::new();
    let field = |k: &str| FIELDS.iter().find(|f| f.key == k).unwrap();
    let tex = s.textures.len() as i64;
    for (i, r) in s.resources.iter().enumerate() {
        let e = i + 1;
        let get = |k: &str| read(r, field(k));
        if get("interval") == Some(0) {
            out.push(format!(
                "Emitter {e}: the emission interval must be 1 or more."
            ));
        }
        if get("loopFrames") == Some(0) {
            out.push(format!("Emitter {e}: the loop length must be 1 or more."));
        }
        if get("texture").is_some_and(|t| t >= tex) {
            out.push(format!(
                "Emitter {e}: texture {} does not exist.",
                get("texture").unwrap()
            ));
        }
        if let Some(n) = get("frames") {
            if !(1..=8).contains(&n) {
                out.push(format!(
                    "Emitter {e}: a texture animation has 1 to 8 frames."
                ));
            }
            for k in 0..n.clamp(0, 8) {
                if get(&format!("frame{k}")).is_some_and(|t| t >= tex) {
                    out.push(format!(
                        "Emitter {e}: animation frame {} uses a missing texture.",
                        k + 1
                    ));
                }
            }
        }
        if get("childInterval") == Some(0) {
            out.push(format!(
                "Emitter {e}: the children's interval must be 1 or more."
            ));
        }
        if get("childLife") == Some(0) {
            out.push(format!(
                "Emitter {e}: the children's life must be 1 or more."
            ));
        }
        if get("childTexture").is_some_and(|t| t >= tex) {
            out.push(format!(
                "Emitter {e}: the children's texture does not exist."
            ));
        }
        if get("randomEvery") == Some(0) {
            out.push(format!(
                "Emitter {e}: the random push needs an interval of 1 or more."
            ));
        }
    }
    // The battle's work buffer (eidos research 5.1): 32 bytes per emitter,
    // 8 per behavior and 20 per texture, within 1012.
    let behaviors: usize = s
        .resources
        .iter()
        .map(|r| {
            r.blocks
                .iter()
                .filter(|(n, _)| BEHAVIORS.iter().any(|(b, _, _)| b == n))
                .count()
        })
        .sum();
    let work = 32 * s.resources.len() + 8 * behaviors + 20 * s.textures.len();
    if work > 1012 {
        out.push(format!(
            "Too many emitters, behaviors and textures for the battle ({work} of 1012 bytes)."
        ));
    }
    out
}

// ---- in a project ------------------------------------------------------

/// The particle archive (a/0/0/6).
pub const ARCHIVE: u16 = 6;

/// One particle file in the list.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Listed {
    pub index: usize,
    pub emitters: usize,
    pub textures: usize,
    pub changed: bool,
    /// Moves whose animation loads it (by LoadParticles), first few.
    pub moves: Vec<usize>,
}

/// Which moves' scripts load each particle file (command 6's key; the
/// ball-keyed ones count for their base key).
pub fn users(bridge: &crate::bridge::Bridge) -> std::collections::BTreeMap<usize, Vec<usize>> {
    let mut out: std::collections::BTreeMap<usize, Vec<usize>> = Default::default();
    let count = bridge
        .base_archive(crate::move_anims::SCRIPTS)
        .map_or(0, |a| a.len());
    let last = bridge
        .project()
        .and_then(|p| p.last_member(crate::move_anims::SCRIPTS))
        .map_or(count, |l| count.max(l + 1));
    for mv in 1..last.min(0x8000) {
        let Ok(b) = bridge.member(crate::bridge::Member {
            arc: crate::move_anims::SCRIPTS,
            index: mv,
        }) else {
            continue;
        };
        for key in crate::effscript::particle_keys(&b) {
            let list = out.entry(key as usize).or_default();
            if !list.contains(&mv) {
                list.push(mv);
            }
        }
    }
    out
}

/// The particle files: the game's, then the mod's past them.
pub fn list(bridge: &crate::bridge::Bridge) -> Result<Vec<Listed>, Error> {
    let base = bridge.base_archive(ARCHIVE)?.len();
    let project = bridge.project();
    let end = project
        .and_then(|p| p.last_member(ARCHIVE))
        .map_or(base, |l| base.max(l + 1));
    let users = users(bridge);
    let mut out = Vec::new();
    for index in 0..end {
        let Ok(b) = bridge.member(crate::bridge::Member {
            arc: ARCHIVE,
            index,
        }) else {
            continue;
        };
        let Ok(s) = parse(&b) else { continue };
        out.push(Listed {
            index,
            emitters: s.resources.len(),
            textures: s.textures.len(),
            changed: project.is_some_and(|p| p.member(ARCHIVE, index).is_some()),
            moves: users
                .get(&index)
                .map(|m| m.iter().take(4).copied().collect())
                .unwrap_or_default(),
        });
    }
    Ok(out)
}

/// A particle file of this mod.
pub fn load(bridge: &crate::bridge::Bridge, index: usize) -> Result<Spa, Error> {
    parse(&bridge.member(crate::bridge::Member {
        arc: ARCHIVE,
        index,
    })?)
}

/// Writes a particle file (the game's own drops the mod's copy).
pub fn store(bridge: &crate::bridge::Bridge, index: usize, s: &Spa) -> Result<(), Error> {
    crate::catalog::write_member(
        bridge,
        crate::bridge::Member {
            arc: ARCHIVE,
            index,
        },
        &build(s),
    )
}

/// A new particle file, a copy of `from`, after the last one. Returns its
/// number, which a script's LoadParticles names.
pub fn add(bridge: &crate::bridge::Bridge, from: usize) -> Result<usize, Error> {
    let project = bridge.project().ok_or_else(|| bad("no project is open"))?;
    let s = load(bridge, from)?;
    let base = bridge.base_archive(ARCHIVE)?.len();
    let index = project
        .last_member(ARCHIVE)
        .map_or(base, |l| base.max(l + 1));
    project
        .write_member(ARCHIVE, index, &build(&s))
        .map_err(|e| Error::Missing(e.to_string()))?;
    bridge.invalidate();
    Ok(index)
}

/// One emitter's values for the editor: every field it has, by key.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmitterView {
    pub values: std::collections::BTreeMap<String, i64>,
    /// The optional blocks it has.
    pub blocks: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileView {
    pub emitters: Vec<EmitterView>,
    pub textures: Vec<TexInfo>,
    pub problems: Vec<String>,
}

pub fn view(s: &Spa) -> FileView {
    FileView {
        emitters: s
            .resources
            .iter()
            .map(|r| EmitterView {
                values: FIELDS
                    .iter()
                    .filter_map(|f| Some((f.key.to_string(), read(r, f)?)))
                    .collect(),
                blocks: r.blocks.iter().map(|(n, _)| n.clone()).collect(),
            })
            .collect(),
        textures: s.textures.iter().map(|t| tex_info(t)).collect(),
        problems: problems(s),
    }
}

/// One edit from the editor: a field's value, or a block switched on or off.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Edit {
    pub emitter: usize,
    pub key: String,
    pub value: i64,
}

/// Applies edits (blocks first, so their fields can be set), then refuses
/// a file the game would break on.
pub fn apply(s: &mut Spa, blocks: &[(usize, String, bool)], edits: &[Edit]) -> Result<(), Error> {
    for (e, name, on) in blocks {
        let r = s
            .resources
            .get_mut(*e)
            .ok_or_else(|| bad(format!("no emitter {}", e + 1)))?;
        set_block(r, name, *on)?;
    }
    for ed in edits {
        let f = FIELDS
            .iter()
            .find(|f| f.key == ed.key)
            .ok_or_else(|| bad(format!("no field {}", ed.key)))?;
        let r = s
            .resources
            .get_mut(ed.emitter)
            .ok_or_else(|| bad(format!("no emitter {}", ed.emitter + 1)))?;
        write(r, f, ed.value)?;
    }
    let p = problems(s);
    if !p.is_empty() {
        return Err(Error::BadPath(p.join(" ")));
    }
    Ok(())
}

// ---- textures ----------------------------------------------------------

/// A texture's format and size, from its block's param word.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TexInfo {
    pub format: u32,
    pub width: usize,
    pub height: usize,
    pub colors: usize,
}

pub const FORMAT_NAMES: [&str; 8] = [
    "none",
    "A3I5",
    "4 colors",
    "16 colors",
    "256 colors",
    "4x4 compressed",
    "A5I3",
    "direct",
];

fn bpp(format: u32) -> usize {
    match format {
        1 | 4 | 6 => 8,
        2 => 2,
        3 => 4,
        7 => 16,
        _ => 0,
    }
}

pub fn tex_info(block: &[u8]) -> TexInfo {
    let p = rd32(block, 4).unwrap_or(0);
    TexInfo {
        format: p & 0xF,
        width: 8 << ((p >> 4) & 0xF),
        height: 8 << ((p >> 8) & 0xF),
        colors: rd32(block, 0x10).unwrap_or(0) as usize / 2,
    }
}

fn rgb(c: u16) -> [u8; 3] {
    let f = |v: u16| ((v & 31) * 255 / 31) as u8;
    [f(c), f(c >> 5), f(c >> 10)]
}

/// A texture as RGBA pixels, row by row.
pub fn decode_texture(block: &[u8]) -> Result<(TexInfo, Vec<u8>), Error> {
    let info = tex_info(block);
    let size = rd32(block, 8).unwrap_or(0) as usize;
    let texels = block
        .get(TEX_HEADER..TEX_HEADER + size)
        .ok_or_else(|| bad("texture is cut short"))?;
    let pal_at = rd32(block, 0x0C).unwrap_or(0) as usize;
    let pal: Vec<u16> = block
        .get(pal_at..pal_at + info.colors * 2)
        .unwrap_or(&[])
        .as_chunks::<2>()
        .0
        .iter()
        .map(|c| u16::from_le_bytes(*c))
        .collect();
    let color0_clear = rd32(block, 4).unwrap_or(0) & (1 << 16) != 0;
    let n = info.width * info.height;
    let mut out = Vec::with_capacity(n * 4);
    let px = |i: usize, a: u8, out: &mut Vec<u8>| {
        let [r, g, b] = rgb(pal.get(i).copied().unwrap_or(0));
        out.extend_from_slice(&[r, g, b, a]);
    };
    let bits = bpp(info.format);
    if bits == 0 {
        return Err(bad(format!(
            "texture format {} is not supported",
            FORMAT_NAMES[info.format as usize & 7]
        )));
    }
    for k in 0..n {
        match info.format {
            1 => {
                let t = texels.get(k).copied().unwrap_or(0);
                px(
                    (t & 31) as usize,
                    ((t >> 5) as u32 * 255 / 7) as u8,
                    &mut out,
                );
            }
            6 => {
                let t = texels.get(k).copied().unwrap_or(0);
                px(
                    (t & 7) as usize,
                    ((t >> 3) as u32 * 255 / 31) as u8,
                    &mut out,
                );
            }
            7 => {
                let c = rd16(texels, k * 2).unwrap_or(0);
                let [r, g, b] = rgb(c);
                out.extend_from_slice(&[r, g, b, if c & 0x8000 != 0 { 255 } else { 0 }]);
            }
            _ => {
                let byte = texels.get(k * bits / 8).copied().unwrap_or(0);
                let i = (byte >> ((k * bits) % 8)) & ((1u16 << bits) - 1) as u8;
                px(
                    i as usize,
                    if color0_clear && i == 0 { 0 } else { 255 },
                    &mut out,
                );
            }
        }
    }
    Ok((info, out))
}

/// Up to `max` colors for the opaque pixels (median cut on RGB555).
fn quantize(pixels: &[[u8; 3]], max: usize) -> Vec<u16> {
    let c555 =
        |p: &[u8; 3]| (p[0] as u16 >> 3) | (p[1] as u16 >> 3) << 5 | (p[2] as u16 >> 3) << 10;
    let mut boxes: Vec<Vec<[u8; 3]>> = vec![pixels.to_vec()];
    let mut uniq: Vec<u16> = pixels.iter().map(c555).collect();
    uniq.sort_unstable();
    uniq.dedup();
    if uniq.len() <= max {
        return uniq;
    }
    while boxes.len() < max {
        // Split the box with the widest channel.
        let (bi, ch, _) = boxes
            .iter()
            .enumerate()
            .filter(|(_, b)| b.len() > 1)
            .flat_map(|(i, b)| {
                (0..3).map(move |c| {
                    let lo = b.iter().map(|p| p[c]).min().unwrap();
                    let hi = b.iter().map(|p| p[c]).max().unwrap();
                    (i, c, hi - lo)
                })
            })
            .max_by_key(|t| t.2)
            .unwrap_or((0, 0, 0));
        if boxes[bi].len() < 2 {
            break;
        }
        let mut b = boxes.swap_remove(bi);
        b.sort_unstable_by_key(|p| p[ch]);
        let rest = b.split_off(b.len() / 2);
        boxes.push(b);
        boxes.push(rest);
    }
    let mut out: Vec<u16> = boxes
        .iter()
        .filter(|b| !b.is_empty())
        .map(|b| {
            let n = b.len() as u32;
            let avg = |c: usize| (b.iter().map(|p| p[c] as u32).sum::<u32>() / n) as u8;
            c555(&[avg(0), avg(1), avg(2)])
        })
        .collect();
    out.sort_unstable();
    out.dedup();
    out
}

fn nearest(pal: &[u16], p: [u8; 3]) -> usize {
    pal.iter()
        .enumerate()
        .min_by_key(|(_, &c)| {
            let q = rgb(c);
            (0..3)
                .map(|i| (q[i] as i32 - p[i] as i32).pow(2))
                .sum::<i32>()
        })
        .map_or(0, |(i, _)| i)
}

/// A texture block made from RGBA pixels, in the format of `old` (A3I5,
/// A5I3, 4, 16 or 256 colors, or direct) at the image's size, which must be
/// 8 << n wide and high (8 to 1024). The param word's other bits (repeat,
/// flip, color 0 clear) are kept.
pub fn encode_texture(
    old: &[u8],
    width: usize,
    height: usize,
    rgba: &[u8],
) -> Result<Vec<u8>, Error> {
    let shift = |v: usize| (0..8u32).find(|&s| 8 << s == v);
    let (Some(s), Some(t)) = (shift(width), shift(height)) else {
        return Err(bad(
            "a texture is 8, 16, 32, 64, 128... pixels wide and high",
        ));
    };
    let info = tex_info(old);
    let param = (rd32(old, 4).unwrap() & !0xFFF) | info.format | s << 4 | t << 8;
    let color0_clear = param & (1 << 16) != 0;
    let n = width * height;
    let px = |k: usize| [rgba[k * 4], rgba[k * 4 + 1], rgba[k * 4 + 2]];
    let alpha = |k: usize| rgba[k * 4 + 3];
    let (texels, pal): (Vec<u8>, Vec<u16>) = match info.format {
        1 | 6 => {
            let (cap, abits) = if info.format == 1 { (32, 3) } else { (8, 5) };
            let opaque: Vec<[u8; 3]> = (0..n).filter(|&k| alpha(k) > 0).map(px).collect();
            let pal = quantize(&opaque, cap);
            let pal = if pal.is_empty() { vec![0] } else { pal };
            let ibits = 8 - abits;
            let tx = (0..n)
                .map(|k| {
                    let a = (alpha(k) as u32 * ((1 << abits) - 1) + 127) / 255;
                    let i = if a == 0 { 0 } else { nearest(&pal, px(k)) };
                    (a << ibits | i as u32) as u8
                })
                .collect();
            (tx, pal)
        }
        2..=4 => {
            let bits = bpp(info.format);
            let cap = 1usize << bits;
            // Color 0 stays the transparent slot.
            let opaque: Vec<[u8; 3]> = (0..n).filter(|&k| alpha(k) >= 128).map(px).collect();
            let mut pal = quantize(&opaque, if color0_clear { cap - 1 } else { cap });
            if color0_clear {
                pal.insert(0, 0);
            }
            let mut tx = vec![0u8; n * bits / 8];
            for k in 0..n {
                let i = if color0_clear && alpha(k) < 128 {
                    0
                } else if color0_clear {
                    1 + nearest(&pal[1..], px(k))
                } else {
                    nearest(&pal, px(k))
                };
                tx[k * bits / 8] |= (i as u8) << ((k * bits) % 8);
            }
            (tx, pal)
        }
        7 => {
            let tx = (0..n)
                .flat_map(|k| {
                    let p = px(k);
                    let c = (p[0] as u16 >> 3) | (p[1] as u16 >> 3) << 5 | (p[2] as u16 >> 3) << 10;
                    (c | if alpha(k) >= 128 { 0x8000 } else { 0 }).to_le_bytes()
                })
                .collect();
            (tx, Vec::new())
        }
        f => {
            return Err(bad(format!(
                "texture format {} cannot be written",
                FORMAT_NAMES[f as usize & 7]
            )))
        }
    };
    let mut pal_bytes: Vec<u8> = pal.iter().flat_map(|c| c.to_le_bytes()).collect();
    while !pal_bytes.len().is_multiple_of(4) {
        pal_bytes.push(0);
    }
    let size = TEX_HEADER + texels.len() + pal_bytes.len();
    let mut out = Vec::with_capacity(size);
    out.extend_from_slice(b" TPS");
    out.extend_from_slice(&param.to_le_bytes());
    out.extend_from_slice(&(texels.len() as u32).to_le_bytes());
    out.extend_from_slice(&((TEX_HEADER + texels.len()) as u32).to_le_bytes());
    out.extend_from_slice(&(pal_bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(&(size as u32).to_le_bytes());
    out.extend_from_slice(&rd32(old, 0x18).unwrap_or(0).to_le_bytes());
    out.extend_from_slice(&(size as u32).to_le_bytes());
    out.extend(texels);
    out.extend(pal_bytes);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Vec<u8> {
        // One emitter with gravity, one 8x8 A5I3 texture of 2 colors.
        let mut res = vec![0u8; RES_HEADER];
        res[0..4].copy_from_slice(&(1u32 << 24 | 1 << 14).to_le_bytes());
        res[0x44] = 1;
        res[0x45] = 31;
        res[0x48] = 1;
        let grav = [0u8, 0, 0xC3, 0xFF, 0, 0, 0, 0];
        let mut tex = b" TPS".to_vec();
        tex.extend_from_slice(&(6u32 | 1 << 16).to_le_bytes());
        tex.extend_from_slice(&64u32.to_le_bytes());
        tex.extend_from_slice(&(0x20u32 + 64).to_le_bytes());
        tex.extend_from_slice(&4u32.to_le_bytes());
        tex.extend_from_slice(&(0x20u32 + 68).to_le_bytes());
        tex.extend_from_slice(&0u32.to_le_bytes());
        tex.extend_from_slice(&(0x20u32 + 68).to_le_bytes());
        tex.extend((0..64).map(|i| if i % 2 == 0 { 0xF9u8 } else { 0x00 }));
        tex.extend_from_slice(&[0xFF, 0x7F, 0x1F, 0x00]);
        let mut s = b" APS12_1".to_vec();
        s.extend_from_slice(&1u16.to_le_bytes());
        s.extend_from_slice(&1u16.to_le_bytes());
        s.extend_from_slice(&0u32.to_le_bytes());
        s.extend_from_slice(&((RES_HEADER + 8) as u32).to_le_bytes());
        s.extend_from_slice(&(tex.len() as u32).to_le_bytes());
        s.extend_from_slice(&((0x20 + RES_HEADER + 8) as u32).to_le_bytes());
        s.extend_from_slice(&0u32.to_le_bytes());
        s.extend(res);
        s.extend_from_slice(&grav);
        s.extend(tex);
        s
    }

    #[test]
    fn round_trip_and_fields() {
        let b = sample();
        let mut s = parse(&b).unwrap();
        assert_eq!(build(&s), b);
        let gy = FIELDS.iter().find(|f| f.key == "gravityY").unwrap();
        assert_eq!(read(&s.resources[0], gy), Some(-61));
        write(&mut s.resources[0], gy, -100).unwrap();
        assert_eq!(read(&s.resources[0], gy), Some(-100));
        let shape = FIELDS.iter().find(|f| f.key == "shape").unwrap();
        write(&mut s.resources[0], shape, 4).unwrap();
        assert_eq!(read(&s.resources[0], shape), Some(4));
        // Adding a scale animation puts it before gravity, with its flag.
        set_block(&mut s.resources[0], "scaleAnim", true).unwrap();
        assert_eq!(s.resources[0].blocks[0].0, "scaleAnim");
        let b2 = build(&s);
        let back = parse(&b2).unwrap();
        assert_eq!(back, s);
        assert!(problems(&s).is_empty());
    }

    #[test]
    fn texture_round_trip() {
        let b = sample();
        let s = parse(&b).unwrap();
        let (info, rgba) = decode_texture(&s.textures[0]).unwrap();
        assert_eq!((info.width, info.height, info.format), (8, 8, 6));
        let again = encode_texture(&s.textures[0], 8, 8, &rgba).unwrap();
        let (_, rgba2) = decode_texture(&again).unwrap();
        // Same pixels; a fully clear pixel's color does not matter.
        for (a, b) in rgba.chunks(4).zip(rgba2.chunks(4)) {
            assert_eq!(a[3], b[3]);
            if a[3] > 0 {
                assert_eq!(a, b);
            }
        }
    }
}

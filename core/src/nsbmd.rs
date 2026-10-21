//! NSBMD (BMD0) models: the MDL0 block with its nodes, materials and
//! shapes, and the GX display lists the shapes hold, decoded to triangles.
//! The overworld map (ground chunks and buildings) is made of these.
//!
//! Layout (checked against the map chunk models; eidos `engine/g3d/nsbmd.c`
//! and aletheia `docs/ASSETS.md` describe the same bytes):
//!
//! ```text
//! MDL0        "MDL0", u32 size, a dictionary of models whose u32 records
//!             are offsets from the MDL0 start
//! model       u32 size, u32 render commands, u32 materials, u32 shapes,
//!             u32 envelope; u8 counts at 0x18 (materials, shapes);
//!             fx32 position scale at 0x1C; node dictionary at 0x40
//! materials   u16 texture pairing offset, u16 palette pairing offset (both
//!             from here), then a dictionary of u32 offsets (from here) to
//!             44-byte records: u32 diffuse/ambient at 4, u32
//!             specular/emission at 8, u32 polygon attributes at 0x0C (light
//!             enable bits 0-3), u32 texture parameters at 0x14, u16 flags at
//!             0x1E. A pairing
//!             dictionary names textures (or palettes); each record is a u16
//!             offset to a list of the material ids that use it, and a u8
//!             count
//! shapes      a dictionary of u32 offsets (from its start) to 16-byte
//!             records; the u32 at 8 of a record is the display list
//!             offset from the record's own start, the u32 at 12 its size
//! nodes       a dictionary of u32 offsets (from its start) to records: u16
//!             flags (bit 0 no translation, 1 no rotation, 2 unit scale,
//!             3 pivot form), s16 first matrix element, then translation
//!             (3 fx32), rotation, scale (3 fx32 and 3 inverses)
//! commands    one byte opcode (low 5 bits) with flag bits above; NODEDESC
//!             0x06 (node, parent, flags), NODE 0x02, MAT 0x04, SHP 0x05,
//!             RET 0x01, BB 0x07 and BBY 0x08 (billboard the current node),
//!             POSSCALE 0x0B
//! ```

use crate::g3d::container_block;

fn rd16(b: &[u8], at: usize) -> Option<u16> {
    b.get(at..at + 2).map(|x| u16::from_le_bytes([x[0], x[1]]))
}

fn rd32(b: &[u8], at: usize) -> Option<u32> {
    b.get(at..at + 4)
        .map(|x| u32::from_le_bytes(x.try_into().unwrap()))
}

fn rds16(b: &[u8], at: usize) -> Option<i16> {
    rd16(b, at).map(|v| v as i16)
}

fn rds32(b: &[u8], at: usize) -> Option<i32> {
    rd32(b, at).map(|v| v as i32)
}

/// A decoded vertex. `uv` is in texels; `color` is the vertex colour.
#[derive(Debug, Clone, Copy, Default)]
pub struct Vertex {
    pub pos: [f32; 3],
    pub uv: [f32; 2],
    pub color: [f32; 3],
    /// The surface normal (unit length in model space), zero when the
    /// display list sets none.
    pub normal: [f32; 3],
}

pub type Triangle = [Vertex; 3];

/// Which faces a material draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cull {
    Both,
    Front,
    Back,
    Neither,
}

#[derive(Debug, Clone)]
pub struct Material {
    pub name: String,
    /// The texture it names in the TEX0 of the set that goes with the model.
    pub texture: Option<String>,
    /// Polygon alpha, 0 to 31 (31 is opaque).
    pub alpha: u8,
    pub cull: Cull,
    /// The vertex colours shade the surface.
    pub vertex_color: bool,
    /// Texture wrap per axis: repeat, and mirror on odd repeats.
    pub repeat: [bool; 2],
    pub flip: [bool; 2],
    /// Diffuse, ambient, specular and emission colours (0 to 1 per channel).
    pub diffuse: [f32; 3],
    pub ambient: [f32; 3],
    pub specular: [f32; 3],
    pub emission: [f32; 3],
    /// Polygon attribute bits 0 to 3: which of the four lights shine on it.
    pub light_mask: u8,
    /// The material's own flag word (which colours it sets itself).
    pub flag: u16,
}

/// A shape that turns to face the camera.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Billboard {
    /// Faces the camera fully.
    Full,
    /// Turns about the vertical axis only.
    Y,
}

/// One placed use of a shape: the shape with a material under a node.
#[derive(Debug, Clone, Copy)]
pub struct Draw {
    pub shape: usize,
    pub material: usize,
    pub node: usize,
    pub billboard: Option<Billboard>,
}

/// An affine transform, row vectors: `v' = v.x*r[0] + v.y*r[1] + v.z*r[2] + t`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Affine {
    pub r: [[f32; 3]; 3],
    pub t: [f32; 3],
}

impl Affine {
    pub const IDENTITY: Affine = Affine {
        r: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        t: [0.0; 3],
    };

    pub fn apply(&self, v: [f32; 3]) -> [f32; 3] {
        let mut o = self.t;
        for (i, &c) in v.iter().enumerate() {
            for (k, ok) in o.iter_mut().enumerate() {
                *ok += c * self.r[i][k];
            }
        }
        o
    }

    /// `self` first, then `next`.
    pub fn then(&self, next: &Affine) -> Affine {
        let mut r = [[0.0; 3]; 3];
        for (i, row) in r.iter_mut().enumerate() {
            for (k, cell) in row.iter_mut().enumerate() {
                *cell = (0..3).map(|j| self.r[i][j] * next.r[j][k]).sum();
            }
        }
        Affine {
            r,
            t: next.apply(self.t),
        }
    }

    /// Rotation about Y by `turns` of a full circle (the game's u16 angle
    /// over 65536), then the shift.
    pub fn yaw(turns: f32, t: [f32; 3]) -> Affine {
        let (s, c) = (turns * std::f32::consts::TAU).sin_cos();
        Affine {
            r: [[c, 0.0, -s], [0.0, 1.0, 0.0], [s, 0.0, c]],
            t,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Model {
    pub name: String,
    /// World units per model unit (the position scale).
    pub pos_scale: f32,
    /// World transform of each node.
    pub nodes: Vec<Affine>,
    pub materials: Vec<Material>,
    /// Triangles of each shape in model units (before the position scale).
    pub shapes: Vec<Vec<Triangle>>,
    /// Per shape: the matrix commands its display list holds (bit n is
    /// opcode 0x10 + n). They are not applied.
    pub matrix_ops: Vec<u32>,
    pub draws: Vec<Draw>,
}

struct Dict<'a> {
    base: usize,
    count: usize,
    records: Vec<&'a [u8]>,
    names: Vec<String>,
}

fn dict(b: &[u8], off: usize) -> Option<Dict<'_>> {
    let count = *b.get(off + 1)? as usize;
    let size = rd16(b, off + 2)? as usize;
    if *b.get(off)? != 0
        || rd16(b, off + 4)? != 8
        || rd16(b, off + 6)? as usize != 8 + (count + 1) * 4
    {
        return None;
    }
    let sub = off + 8 + (count + 1) * 4;
    let entry = rd16(b, sub)? as usize;
    let names_at = (off + size).checked_sub(count * 16)?;
    let mut records = Vec::new();
    let mut names = Vec::new();
    for i in 0..count {
        records.push(b.get(sub + 4 + i * entry..sub + 4 + (i + 1) * entry)?);
        let raw = b.get(names_at + i * 16..names_at + i * 16 + 16)?;
        let end = raw.iter().position(|&c| c == 0).unwrap_or(16);
        names.push(String::from_utf8_lossy(&raw[..end]).into_owned());
    }
    Some(Dict {
        base: off,
        count,
        records,
        names,
    })
}

impl Dict<'_> {
    fn offset(&self, i: usize) -> Option<usize> {
        let r = self.records.get(i)?;
        (r.len() >= 4).then(|| u32::from_le_bytes([r[0], r[1], r[2], r[3]]) as usize)
    }
}

/// Parses the first model of a BMD0 file.
pub fn parse(data: &[u8]) -> Option<Model> {
    if data.get(..4)? != b"BMD0" {
        return None;
    }
    let mdl = container_block(data, b"MDL0")?;
    let models = dict(mdl, 8)?;
    let m = mdl.get(models.offset(0)?..)?;
    let pos_scale = rds32(m, 0x1C)? as f32 / 4096.0;

    let nodes = parse_nodes(m)?;
    let materials = parse_materials(m)?;
    let (shapes, matrix_ops) = parse_shapes(m)?;
    let (draws, world) = run_commands(m, &nodes, materials.len(), shapes.len())?;
    Some(Model {
        name: models.names.first().cloned().unwrap_or_default(),
        pos_scale,
        nodes: world,
        materials,
        shapes,
        matrix_ops,
        draws,
    })
}

/// Local transform and parent of each node.
fn parse_nodes(m: &[u8]) -> Option<Vec<Affine>> {
    let d = dict(m, 0x40)?;
    let mut out = Vec::new();
    for i in 0..d.count {
        let a = d.base + d.offset(i)?;
        let flags = rd16(m, a)?;
        let first = rds16(m, a + 2)?;
        let mut o = a + 4;
        let mut t = [0.0f32; 3];
        let mut rot = Affine::IDENTITY.r;
        let mut scale = [1.0f32; 3];
        if flags & 1 == 0 {
            for (k, tk) in t.iter_mut().enumerate() {
                *tk = rds32(m, o + k * 4)? as f32 / 4096.0;
            }
            o += 12;
        }
        if flags & 2 == 0 {
            let f = |at: usize| rds16(m, at).map(|v| v as f32 / 4096.0);
            if flags & 8 == 0 {
                // First element in the header, then the other eight.
                let mut e = [0.0f32; 9];
                e[0] = first as f32 / 4096.0;
                for (k, ek) in e.iter_mut().enumerate().skip(1) {
                    *ek = f(o + (k - 1) * 2)?;
                }
                for (row, r) in rot.iter_mut().enumerate() {
                    r.copy_from_slice(&e[row * 3..row * 3 + 3]);
                }
                o += 16;
            } else {
                // Pivot form: one element is +-1 with its row and column
                // zero; the other four come from two stored values.
                let pivot = ((flags >> 4) & 15) as usize;
                let (a_v, b_v) = (f(o)?, f(o + 2)?);
                let mut e = [0.0f32; 9];
                if pivot < 9 {
                    let (pr, pc) = (pivot / 3, pivot % 3);
                    e[pivot] = if flags & 0x100 != 0 { -1.0 } else { 1.0 };
                    let rest: Vec<usize> = (0..9).filter(|k| k / 3 != pr && k % 3 != pc).collect();
                    let c_v = if flags & 0x200 != 0 { -b_v } else { b_v };
                    let d_v = if flags & 0x400 != 0 { -a_v } else { a_v };
                    for (k, v) in rest.iter().zip([a_v, b_v, c_v, d_v]) {
                        e[*k] = v;
                    }
                }
                for (row, r) in rot.iter_mut().enumerate() {
                    r.copy_from_slice(&e[row * 3..row * 3 + 3]);
                }
                o += 4;
            }
        }
        if flags & 4 == 0 {
            for (k, sk) in scale.iter_mut().enumerate() {
                *sk = rds32(m, o + k * 4)? as f32 / 4096.0;
            }
        }
        let mut r = rot;
        for (row, sc) in r.iter_mut().zip(scale) {
            for v in row.iter_mut() {
                *v *= sc;
            }
        }
        out.push(Affine { r, t });
    }
    Some(out)
}

/// A 5-bit-per-channel colour (red in the low bits) as 0 to 1.
fn rgb555(c: u32) -> [f32; 3] {
    [
        (c & 31) as f32 / 31.0,
        ((c >> 5) & 31) as f32 / 31.0,
        ((c >> 10) & 31) as f32 / 31.0,
    ]
}

fn parse_materials(m: &[u8]) -> Option<Vec<Material>> {
    let base = rd32(m, 0x08)? as usize;
    let b = m.get(base..)?;
    let tex_pairs = dict(b, rd16(b, 0)? as usize)?;
    let d = dict(b, 4)?;
    let mut texture = vec![None; d.count];
    for i in 0..tex_pairs.count {
        let r = tex_pairs.records.get(i)?;
        let (list, count) = (
            u16::from_le_bytes([r[0], r[1]]) as usize,
            *r.get(2)? as usize,
        );
        for k in 0..count {
            if let Some(&id) = b.get(list + k) {
                if let Some(slot) = texture.get_mut(id as usize) {
                    *slot = Some(tex_pairs.names[i].clone());
                }
            }
        }
    }
    let mut out = Vec::new();
    for (i, texture) in texture.into_iter().enumerate() {
        let a = d.offset(i)?;
        let diff_amb = rd32(b, a + 4)?;
        let spec_emi = rd32(b, a + 8)?;
        let poly = rd32(b, a + 0x0C)?;
        let tex = rd32(b, a + 0x14)?;
        let cull = match (poly >> 6) & 3 {
            0 => Cull::Neither,
            1 => Cull::Back,
            2 => Cull::Front,
            _ => Cull::Both,
        };
        out.push(Material {
            name: d.names[i].clone(),
            texture,
            alpha: ((poly >> 16) & 31) as u8,
            cull,
            vertex_color: diff_amb & 0x8000 != 0,
            repeat: [tex & 0x10000 != 0, tex & 0x20000 != 0],
            flip: [tex & 0x40000 != 0, tex & 0x80000 != 0],
            diffuse: rgb555(diff_amb),
            ambient: rgb555(diff_amb >> 16),
            specular: rgb555(spec_emi),
            emission: rgb555(spec_emi >> 16),
            light_mask: (poly & 15) as u8,
            flag: rd16(b, a + 0x1E)?,
        });
    }
    Some(out)
}

fn parse_shapes(m: &[u8]) -> Option<(Vec<Vec<Triangle>>, Vec<u32>)> {
    let base = rd32(m, 0x0C)? as usize;
    let d = dict(m, base)?;
    let mut out = Vec::new();
    let mut ops = Vec::new();
    for i in 0..d.count {
        let a = base + d.offset(i)?;
        let off = rd32(m, a + 8)? as usize;
        let size = rd32(m, a + 12)? as usize;
        let dl = m.get(a + off..a + off + size)?;
        let (tris, seen) = decode_list_ops(dl);
        out.push(tris);
        ops.push(seen);
    }
    Some((out, ops))
}

/// Walks the render commands: which shape is drawn with which material
/// under which node, and the world transform of every node.
fn run_commands(
    m: &[u8],
    local: &[Affine],
    materials: usize,
    shapes: usize,
) -> Option<(Vec<Draw>, Vec<Affine>)> {
    let mut at = rd32(m, 0x04)? as usize;
    let mut parent: Vec<Option<usize>> = vec![None; local.len()];
    let (mut node, mut material) = (0usize, 0usize);
    let mut visible = true;
    let mut billboard = None;
    let mut draws = Vec::new();
    while let Some(&cmd) = m.get(at) {
        let op = cmd & 0x1F;
        let p = |k: usize| m.get(at + 1 + k).copied();
        let len = match op {
            0x00 | 0x0B => 0,
            0x01 => break,
            0x02 => {
                node = p(0)? as usize;
                visible = p(1)? != 0;
                billboard = None;
                2
            }
            0x07 | 0x08 => {
                billboard = Some(if op == 0x07 {
                    Billboard::Full
                } else {
                    Billboard::Y
                });
                1
            }
            0x03 => 1,
            0x04 => {
                material = p(0)? as usize;
                1
            }
            0x05 => {
                let shape = p(0)? as usize;
                if visible && shape < shapes && material < materials && node < local.len() {
                    draws.push(Draw {
                        shape,
                        material,
                        node,
                        billboard,
                    });
                }
                1
            }
            0x06 => {
                let (n, par) = (p(0)? as usize, p(1)? as usize);
                if n < parent.len() && par < parent.len() && n != par {
                    parent[n] = Some(par);
                }
                3 + (cmd & 0x20 != 0) as usize + (cmd & 0x40 != 0) as usize
            }
            0x09 => 2 + 3 * p(1)? as usize,
            0x0A => 12,
            0x0C | 0x0D => 2,
            _ => break,
        };
        at += 1 + len;
    }
    let mut world = local.to_vec();
    // Parents come before children in every model; a few passes settle
    // any other order.
    for _ in 0..local.len().min(8) {
        for i in 0..local.len() {
            if let Some(par) = parent[i] {
                world[i] = local[i].then(&world[par]);
            }
        }
    }
    Some((draws, world))
}

/// Arguments (words) of each GX command.
fn gx_words(op: u8) -> Option<usize> {
    Some(match op {
        0x00 | 0x11 | 0x15 | 0x41 => 0,
        0x10 | 0x12 | 0x13 | 0x14 => 1,
        0x16 | 0x18 => 16,
        0x17 | 0x19 => 12,
        0x1A => 9,
        0x1B | 0x1C => 3,
        0x20..=0x22 | 0x24..=0x2B => 1,
        0x23 => 2,
        0x30..=0x33 => 1,
        0x34 => 32,
        0x40 | 0x50 | 0x60 | 0x72 => 1,
        0x70 => 3,
        0x71 => 2,
        _ => return None,
    })
}

fn sext10(v: u32) -> f32 {
    (((v & 0x3FF) << 22) as i32 >> 22) as f32
}

/// Decodes a display list to triangles: model units, texture coordinates
/// in texels, colour defaulting to white.
pub fn decode_list(dl: &[u8]) -> Vec<Triangle> {
    decode_list_ops(dl).0
}

/// `decode_list`, and a bit set of the matrix commands (0x10 to 0x1C, bit
/// n for opcode 0x10 + n) the list holds.
pub fn decode_list_ops(dl: &[u8]) -> (Vec<Triangle>, u32) {
    let mut seen = 0u32;
    let mut out = Vec::new();
    let mut at = 0;
    let mut cur = Vertex {
        color: [1.0; 3],
        ..Vertex::default()
    };
    let mut mode = 0u32;
    let mut run: Vec<Vertex> = Vec::new();
    let word = |i: usize| rd32(dl, i);
    'list: while let Some(cmds) = word(at) {
        at += 4;
        for k in 0..4 {
            let op = (cmds >> (k * 8)) as u8;
            let Some(n) = gx_words(op) else {
                break 'list;
            };
            let Some(args) = (0..n)
                .map(|i| word(at + i * 4))
                .collect::<Option<Vec<u32>>>()
            else {
                break 'list;
            };
            at += n * 4;
            if (0x10..=0x1C).contains(&op) {
                seen |= 1 << (op - 0x10);
            }
            let s16 = |v: u32, hi: bool| ((if hi { v >> 16 } else { v }) as u16 as i16) as f32;
            let mut vertex = false;
            match op {
                0x20 => {
                    let c = args[0];
                    cur.color = [
                        (c & 31) as f32 / 31.0,
                        ((c >> 5) & 31) as f32 / 31.0,
                        ((c >> 10) & 31) as f32 / 31.0,
                    ];
                }
                0x21 => {
                    cur.normal = [
                        sext10(args[0]) / 512.0,
                        sext10(args[0] >> 10) / 512.0,
                        sext10(args[0] >> 20) / 512.0,
                    ];
                }
                0x22 => cur.uv = [s16(args[0], false) / 16.0, s16(args[0], true) / 16.0],
                0x23 => {
                    cur.pos = [
                        s16(args[0], false) / 4096.0,
                        s16(args[0], true) / 4096.0,
                        s16(args[1], false) / 4096.0,
                    ];
                    vertex = true;
                }
                0x24 => {
                    cur.pos = [
                        sext10(args[0]) / 64.0,
                        sext10(args[0] >> 10) / 64.0,
                        sext10(args[0] >> 20) / 64.0,
                    ];
                    vertex = true;
                }
                0x25 => {
                    cur.pos[0] = s16(args[0], false) / 4096.0;
                    cur.pos[1] = s16(args[0], true) / 4096.0;
                    vertex = true;
                }
                0x26 => {
                    cur.pos[0] = s16(args[0], false) / 4096.0;
                    cur.pos[2] = s16(args[0], true) / 4096.0;
                    vertex = true;
                }
                0x27 => {
                    cur.pos[1] = s16(args[0], false) / 4096.0;
                    cur.pos[2] = s16(args[0], true) / 4096.0;
                    vertex = true;
                }
                0x28 => {
                    cur.pos[0] += sext10(args[0]) / 4096.0;
                    cur.pos[1] += sext10(args[0] >> 10) / 4096.0;
                    cur.pos[2] += sext10(args[0] >> 20) / 4096.0;
                    vertex = true;
                }
                0x40 => {
                    mode = args[0] & 3;
                    run.clear();
                }
                0x41 => run.clear(),
                _ => {}
            }
            if vertex {
                run.push(cur);
                emit(&mut out, mode, &run);
            }
        }
    }
    (out, seen)
}

/// Adds the triangles the newest vertex of a run completes.
fn emit(out: &mut Vec<Triangle>, mode: u32, run: &[Vertex]) {
    let n = run.len();
    match mode {
        0 if n.is_multiple_of(3) => out.push([run[n - 3], run[n - 2], run[n - 1]]),
        1 if n.is_multiple_of(4) => {
            out.push([run[n - 4], run[n - 3], run[n - 2]]);
            out.push([run[n - 4], run[n - 2], run[n - 1]]);
        }
        2 if n >= 3 => {
            if n % 2 == 1 {
                out.push([run[n - 3], run[n - 2], run[n - 1]]);
            } else {
                out.push([run[n - 2], run[n - 3], run[n - 1]]);
            }
        }
        3 if n >= 4 && n.is_multiple_of(2) => {
            // Quad strip: vertices pair up across the strip.
            out.push([run[n - 4], run[n - 3], run[n - 1]]);
            out.push([run[n - 4], run[n - 1], run[n - 2]]);
        }
        _ => {}
    }
}

/// One texture matrix input: the five tracks of a material in a texture SRT
/// animation, sampled at a frame. Scale is a factor, rotation is a sine and
/// cosine, translation is a fraction of the texture size.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TexSrt {
    pub scale: [f32; 2],
    pub sin: f32,
    pub cos: f32,
    pub trans: [f32; 2],
}

impl TexSrt {
    pub const IDENTITY: TexSrt = TexSrt {
        scale: [1.0; 2],
        sin: 0.0,
        cos: 1.0,
        trans: [0.0; 2],
    };

    /// The map on texel coordinates `[a, b, c, d, e, f]`: `u' = a*u + c*v +
    /// e`, `v' = b*u + d*v + f`, for a texture `w` by `h` texels (the
    /// "Maya" texture matrix of `NNSi_G3dTexMtxScaleRotTrans4x4`: rotation
    /// and scale about the middle of the texture, translation flipped in v).
    pub fn matrix(&self, w: f32, h: f32) -> [f32; 6] {
        let [s, t] = self.scale;
        let (sin, cos) = (self.sin, self.cos);
        [
            s * cos,
            -t * sin * h / w,
            s * sin * w / h,
            t * cos,
            w * (s - s * sin - s * cos) / 2.0 - w * s * self.trans[0],
            h * (t * self.trans[1] + 1.0 + (t * sin - t * cos - t) / 2.0),
        ]
    }
}

/// A key track: `info` flags and the inline value or key offset.
#[derive(Debug, Clone, Copy)]
struct Track {
    info: u32,
    offset: u32,
}

const TRACK_CONST: u32 = 0x2000_0000;
const TRACK_S16: u32 = 0x1000_0000;
const TRACK_STEP: u32 = 0xC000_0000;
const TRACK_STEP2: u32 = 0x4000_0000;

/// A texture SRT animation (BTA0): per material name, five tracks over a
/// number of 60 Hz frames. From aletheia `nns_g3d_anm_calc.c`
/// (`NNSi_G3dAnmGetTexSRTData`).
///
/// ```text
/// BTA0        header, then an "SRT0" block: a dictionary of animation
///             offsets (from the block start)
/// animation   4 bytes kind, u16 frame count, u16, then a dictionary at 8 of
///             40-byte records: tracks scaleS, scaleT, rotation, transS,
///             transT, each u32 info + u32 offset (from the animation start)
/// info        bit 29 constant (offset is the value; for rotation, sine in
///             the low half and cosine in the high half, 4.12), bit 28 keys
///             are s16 not s32, bits 31:30 key step (1, 2 or 4 frames per
///             key), bits 15:0 the last frame that interpolates
/// ```
#[derive(Debug, Clone)]
pub struct SrtAnim {
    /// Length of the loop in frames.
    pub frames: u32,
    res: Vec<u8>,
    materials: Vec<(String, [Track; 5])>,
}

impl SrtAnim {
    /// Animation `index` of a BTA0 file.
    pub fn parse(data: &[u8], index: usize) -> Option<SrtAnim> {
        if data.get(..4)? != b"BTA0" {
            return None;
        }
        let block = container_block(data, b"SRT0")?;
        let list = dict(block, 8)?;
        let res = block.get(list.offset(index)?..)?;
        let frames = rd16(res, 4)? as u32;
        let d = dict(res, 8)?;
        let mut materials = Vec::new();
        for i in 0..d.count {
            let r = d.records.get(i)?;
            if r.len() < 40 {
                return None;
            }
            let word = |k: usize| u32::from_le_bytes([r[k], r[k + 1], r[k + 2], r[k + 3]]);
            let t = |n: usize| Track {
                info: word(n * 8),
                offset: word(n * 8 + 4),
            };
            materials.push((d.names[i].clone(), [t(0), t(1), t(2), t(3), t(4)]));
        }
        Some(SrtAnim {
            frames,
            res: res.to_vec(),
            materials,
        })
    }

    /// Names of the animated materials.
    pub fn names(&self) -> Vec<&str> {
        self.materials.iter().map(|(n, _)| n.as_str()).collect()
    }

    /// The track set of the material called `name`.
    pub fn material(&self, name: &str) -> Option<usize> {
        self.materials.iter().position(|(n, _)| n == name)
    }

    /// The five tracks of material `m` at a frame (wrapped to the loop).
    pub fn at(&self, m: usize, frame: u32) -> TexSrt {
        let frame = frame % self.frames.max(1);
        let tracks = &self.materials[m].1;
        let scalar = |t: &Track| self.scalar(t, frame) as f32 / 4096.0;
        let rot = self.rotation(&tracks[2], frame);
        TexSrt {
            scale: [scalar(&tracks[0]), scalar(&tracks[1])],
            sin: (rot as u16 as i16) as f32 / 4096.0,
            cos: ((rot >> 16) as u16 as i16) as f32 / 4096.0,
            trans: [scalar(&tracks[3]), scalar(&tracks[4])],
        }
    }

    /// Where a track reads its keys for a frame: the index of the key, and
    /// how to blend it (`NNSi_G3dAnmGetTexSRTScalarData`).
    fn key_plan(info: u32, frame: u32) -> KeyPlan {
        if info & TRACK_STEP == 0 {
            return KeyPlan::Key(frame);
        }
        let last = info & 0xFFFF;
        if info & TRACK_STEP2 != 0 {
            if frame & 1 == 0 {
                return KeyPlan::Key(frame >> 1);
            }
            if frame > last {
                return KeyPlan::Key((last >> 1) + 1);
            }
            return KeyPlan::Mid(frame >> 1);
        }
        let rem = frame & 3;
        if rem == 0 {
            return KeyPlan::Key(frame >> 2);
        }
        if frame > last {
            return KeyPlan::Key(rem + (last >> 2));
        }
        if frame & 1 == 0 {
            return KeyPlan::Mid(frame >> 2);
        }
        let base = frame >> 2;
        if frame & 2 != 0 {
            KeyPlan::Quad(base + 1, base)
        } else {
            KeyPlan::Quad(base, base + 1)
        }
    }

    fn scalar(&self, t: &Track, frame: u32) -> i32 {
        if t.info & TRACK_CONST != 0 {
            return t.offset as i32;
        }
        let key = |i: u32| -> i32 {
            let at = t.offset as usize;
            if t.info & TRACK_S16 != 0 {
                rds16(&self.res, at + i as usize * 2).unwrap_or(0) as i32
            } else {
                rds32(&self.res, at + i as usize * 4).unwrap_or(0)
            }
        };
        match Self::key_plan(t.info, frame) {
            KeyPlan::Key(i) => key(i),
            KeyPlan::Mid(i) => (key(i) + key(i + 1)) >> 1,
            KeyPlan::Quad(a, b) => (key(a) * 3 + key(b)) >> 2,
        }
    }

    /// The rotation track: each key is a (sine, cosine) pair of s16, returned
    /// packed (sine low, cosine high).
    fn rotation(&self, t: &Track, frame: u32) -> u32 {
        if t.info & TRACK_CONST != 0 {
            return t.offset;
        }
        let pair = |i: u32| -> (i32, i32) {
            let at = t.offset as usize + i as usize * 4;
            (
                rds16(&self.res, at).unwrap_or(0) as i32,
                rds16(&self.res, at + 2).unwrap_or(0) as i32,
            )
        };
        let pack = |s: i32, c: i32| (s as u16 as u32) | ((c as u32) << 16);
        match Self::key_plan(t.info, frame) {
            KeyPlan::Key(i) => {
                let (s, c) = pair(i);
                pack(s, c)
            }
            KeyPlan::Mid(i) => {
                let ((s0, c0), (s1, c1)) = (pair(i), pair(i + 1));
                pack((s0 + s1) >> 1, (c0 + c1) >> 1)
            }
            KeyPlan::Quad(a, b) => {
                let ((s0, c0), (s1, c1)) = (pair(a), pair(b));
                pack((s0 * 3 + s1) >> 2, (c0 * 3 + c1) >> 2)
            }
        }
    }
}

enum KeyPlan {
    /// This key.
    Key(u32),
    /// The mean of this key and the next.
    Mid(u32),
    /// Three parts the first key, one part the second.
    Quad(u32, u32),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list(cmds: [u8; 4], args: &[u32]) -> Vec<u8> {
        let mut v = u32::from_le_bytes(cmds).to_le_bytes().to_vec();
        for a in args {
            v.extend_from_slice(&a.to_le_bytes());
        }
        v
    }

    #[test]
    fn quad_makes_two_triangles() {
        let vtx = |x: i16, y: i16| (y as u16 as u32) << 16 | x as u16 as u32;
        let mut dl = list(
            [0x40, 0x23, 0x23, 0x23],
            &[1, vtx(0, 0), 0, vtx(4096, 0), 0, vtx(4096, 4096), 0],
        );
        dl.extend(list([0x23, 0x41, 0, 0], &[vtx(0, 4096), 0]));
        let t = decode_list(&dl);
        assert_eq!(t.len(), 2);
        assert_eq!(t[0][1].pos, [1.0, 0.0, 0.0]);
        assert_eq!(t[1][2].pos, [0.0, 1.0, 0.0]);
    }

    #[test]
    fn identity_tex_matrix() {
        let m = TexSrt::IDENTITY.matrix(32.0, 16.0);
        assert_eq!(m, [1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
        // A translation of a quarter texture moves u backwards, v forwards.
        let t = TexSrt {
            trans: [0.25, 0.25],
            ..TexSrt::IDENTITY
        };
        let m = t.matrix(32.0, 16.0);
        assert_eq!((m[4], m[5]), (-8.0, 4.0));
    }

    #[test]
    fn affine_order() {
        let a = Affine {
            t: [1.0, 0.0, 0.0],
            ..Affine::IDENTITY
        };
        let b = Affine {
            r: [[2.0, 0.0, 0.0], [0.0, 2.0, 0.0], [0.0, 0.0, 2.0]],
            t: [0.0; 3],
        };
        assert_eq!(a.then(&b).apply([0.0; 3]), [2.0, 0.0, 0.0]);
    }
}

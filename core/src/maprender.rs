//! A zone's map drawn the way the game shows it: the 3D map models with
//! their textures, seen from above and lined up with the tile grid, so the
//! map editor can draw them under the grid. A software rasteriser
//! (orthographic, nearest texel, depth buffer, alpha).
//!
//! Where the pieces are (from aletheia `src/game/ov021/field_g3d_map_block.c`,
//! `field_buildmodel.c`, and a look at the data):
//!
//! ```text
//! zone header   a/0/1/2: u16 areaId at 0x02, u16 matrixId at 0x04
//! area record   a/0/1/3 (flat file, 10 bytes each): u16 modelSetId, u16
//!               texSetId, ..., u8 innerOuter at 6 (1 = indoors)
//! ground        a/0/0/8 chunk: tag, u16, u32 BMD0 offset at 4; the model's
//!               textures are member texSetId of a/0/1/4 (a BTX0)
//! objects       the chunk's last section: u32 count, then 16 bytes each:
//!               fx32 x, y, z (z negated), u16 rotation, u16 model id.
//!               Section start: u32 at 12 (WB, RD), 16 (GC), 8 (NG)
//! buildings     member m of a/2/3/0 (outdoors) or a/2/2/9 (indoors): u8 at
//!               2 is twice the model count n, u32 offsets at 4: n model
//!               definitions (u16 model id first), then n BMD0 files;
//!               textures: the same member of a/1/7/7 (outdoors) or a/1/7/6.
//!               m = areaId - 0xD2 when areaId >= 0xD2, (areaId - 2) / 4 when
//!               2 <= areaId, else 0 (0 again when past the last member)
//! ```
//!
//! A tile is 16 world units. A chunk model is centred on the chunk.

use std::collections::HashMap;
use std::rc::Rc;

use serde::Serialize;

use crate::bridge::{Bridge, Error, Member};
use crate::g3d;
use crate::image::Rgba;
use crate::nsbmd::{self, Affine, Billboard, Cull, Model};

const MATRICES: u16 = 9;
const CHUNKS: u16 = 8;
const ZONE_HEADERS: u16 = 12;
const GROUND_TEXTURES: u16 = 14;
const BUILDINGS_OUTDOOR: u16 = 230;
const BUILDINGS_INDOOR: u16 = 229;
const BUILDING_TEXTURES_OUTDOOR: u16 = 177;
const BUILDING_TEXTURES_INDOOR: u16 = 176;
const CHUNK_TILES: usize = 32;
/// World units per tile.
const TILE: f32 = 16.0;

fn rd16(b: &[u8], at: usize) -> Option<u16> {
    b.get(at..at + 2).map(|x| u16::from_le_bytes([x[0], x[1]]))
}

fn rd32(b: &[u8], at: usize) -> Option<u32> {
    b.get(at..at + 4)
        .map(|x| u32::from_le_bytes(x.try_into().unwrap()))
}

/// How the image lines up with the zone's tiles.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Layout {
    pub zone: usize,
    /// Pixels per tile edge: tile (x, z) is the rectangle
    /// [x*px, (x+1)*px) by [z*px, (z+1)*px).
    pub px: usize,
    /// The map size in tiles (as `zonemap::zone_map`) and its origin in
    /// matrix tiles.
    pub width: usize,
    pub height: usize,
    pub origin_x: i32,
    pub origin_z: i32,
    pub image_width: usize,
    pub image_height: usize,
}

/// What to draw.
#[derive(Debug, Clone)]
pub struct Options {
    pub px: usize,
    /// Draw the buildings and props on the ground.
    pub buildings: bool,
    /// Samples per pixel edge (1 to 4); 0 picks 2 for small tiles.
    pub supersample: usize,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            px: 16,
            buildings: true,
            supersample: 0,
        }
    }
}

pub struct Render {
    pub layout: Layout,
    pub image: Rgba,
}

struct Cells {
    cx0: usize,
    cz0: usize,
    wc: usize,
    hc: usize,
    /// (cell x, cell z, chunk id) in the matrix.
    chunks: Vec<(usize, usize, usize)>,
    area: u16,
}

/// The chunks a zone owns, as `zonemap::zone_map` picks them.
fn cells(bridge: &Bridge, zone_id: usize) -> Result<Cells, Error> {
    let headers = bridge.member(Member {
        arc: ZONE_HEADERS,
        index: 0,
    })?;
    let rec = headers
        .get(zone_id * 48..zone_id * 48 + 48)
        .ok_or_else(|| Error::Missing(format!("zone {zone_id}")))?;
    let area = rd16(rec, 2).unwrap_or(0);
    let matrix = rd16(rec, 4).unwrap_or(0);
    let b = bridge.member(Member {
        arc: MATRICES,
        index: matrix as usize,
    })?;
    let bad = || Error::Format(format!("matrix {matrix}"));
    let mode = rd16(&b, 0).ok_or_else(bad)?;
    let (w, h) = (
        rd16(&b, 4).ok_or_else(bad)? as usize,
        rd16(&b, 6).ok_or_else(bad)? as usize,
    );
    let read = |start: usize, i: usize| rd32(&b, start + i * 4).unwrap_or(u32::MAX);
    let mut chunks = Vec::new();
    for i in 0..w * h {
        let chunk = read(8, i);
        if chunk == u32::MAX {
            continue;
        }
        if mode == 1 && read(8 + 4 * w * h, i) != zone_id as u32 {
            continue;
        }
        chunks.push((i % w, i / w, chunk as usize));
    }
    let min = |f: fn(&(usize, usize, usize)) -> usize| chunks.iter().map(f).min().unwrap_or(0);
    let max = |f: fn(&(usize, usize, usize)) -> usize| chunks.iter().map(f).max().unwrap_or(0);
    let (cx0, cz0) = (min(|c| c.0), min(|c| c.1));
    let (wc, hc) = (max(|c| c.0) + 1 - cx0, max(|c| c.1) + 1 - cz0);
    Ok(Cells {
        cx0,
        cz0,
        wc,
        hc,
        chunks,
        area,
    })
}

fn layout(c: &Cells, zone: usize, px: usize) -> Layout {
    let (width, height) = (c.wc * CHUNK_TILES, c.hc * CHUNK_TILES);
    Layout {
        zone,
        px,
        width,
        height,
        origin_x: (c.cx0 * CHUNK_TILES) as i32,
        origin_z: (c.cz0 * CHUNK_TILES) as i32,
        image_width: width * px,
        image_height: height * px,
    }
}

/// The image size and tile mapping of a zone's render, without drawing it.
pub fn info(bridge: &Bridge, zone: usize, px: usize) -> Result<Layout, Error> {
    Ok(layout(&cells(bridge, zone)?, zone, px))
}

/// A texture set: the BTX0 (or BMD0) bytes of a member, decoded by name.
struct Textures {
    bytes: Vec<u8>,
    cache: HashMap<String, Option<Rc<Rgba>>>,
}

impl Textures {
    fn new(bytes: Vec<u8>) -> Self {
        Textures {
            bytes,
            cache: HashMap::new(),
        }
    }

    fn get(&mut self, name: &str) -> Option<Rc<Rgba>> {
        if let Some(t) = self.cache.get(name) {
            return t.clone();
        }
        let found = g3d::find_tex0(&self.bytes).into_iter().find_map(|block| {
            let tex = g3d::Tex0::parse(block)?;
            let i = tex.textures.iter().position(|t| t.name == name)?;
            tex.decode(i)
        });
        let found = found.map(Rc::new);
        self.cache.insert(name.to_string(), found.clone());
        found
    }
}

#[derive(Clone, Copy, Default)]
struct V {
    p: [f32; 3],
    uv: [f32; 2],
    c: [f32; 3],
}

struct Poly {
    v: [V; 3],
    tex: Option<Rc<Rgba>>,
    alpha: u8,
    cull: Cull,
    vertex_color: bool,
    repeat: [bool; 2],
    flip: [bool; 2],
    /// Translucent by polygon alpha or by the texture's alpha format.
    translucent: bool,
    /// A billboard turns to face the camera about this point.
    billboard: Option<(Billboard, [f32; 3])>,
}

fn add_model(
    out: &mut Vec<Poly>,
    model: &Model,
    textures: &mut Textures,
    base: &Affine,
    cull_flip: bool,
) {
    for draw in &model.draws {
        let mat = &model.materials[draw.material];
        let tex = mat.texture.as_deref().and_then(|n| textures.get(n));
        let translucent = mat.alpha < 31
            || tex
                .as_ref()
                .is_some_and(|t| t.pixels.chunks(4).any(|p| p[3] != 0 && p[3] < 255));
        if mat.alpha == 0 {
            continue;
        }
        let node = model.nodes[draw.node].then(base);
        let to_world = |v: &nsbmd::Vertex| {
            let s = model.pos_scale;
            V {
                p: node.apply([v.pos[0] * s, v.pos[1] * s, v.pos[2] * s]),
                uv: v.uv,
                c: v.color,
            }
        };
        for tri in &model.shapes[draw.shape] {
            let mut v = [to_world(&tri[0]), to_world(&tri[1]), to_world(&tri[2])];
            let mut cull = mat.cull;
            if cull_flip {
                v.swap(1, 2);
                // The swap reverses the winding the cull test sees.
                cull = match cull {
                    Cull::Front => Cull::Back,
                    Cull::Back => Cull::Front,
                    c => c,
                };
            }
            out.push(Poly {
                v,
                tex: tex.clone(),
                alpha: mat.alpha,
                cull,
                vertex_color: mat.vertex_color,
                repeat: mat.repeat,
                flip: mat.flip,
                translucent,
                billboard: draw.billboard.map(|b| (b, node.apply([0.0; 3]))),
            });
        }
    }
}

/// The BMD0 inside a chunk file, and the object section.
struct Chunk<'a> {
    model: &'a [u8],
    objects: Option<(usize, usize)>,
}

fn chunk(bytes: &[u8]) -> Option<Chunk<'_>> {
    let tag = bytes.get(..2)?;
    let bmd = rd32(bytes, 4)? as usize;
    let (from, to) = match tag {
        b"NG" => (8, 12),
        b"GC" => (16, 20),
        _ => (12, 16),
    };
    let (a, b) = (rd32(bytes, from)? as usize, rd32(bytes, to)? as usize);
    Some(Chunk {
        model: bytes.get(bmd..)?,
        objects: (b > a && b <= bytes.len()).then_some((a, b)),
    })
}

/// The building archive member for an area.
fn building_member(area: u16, count: usize) -> usize {
    let m = if area >= 0xD2 {
        area as usize - 0xD2
    } else if area >= 2 {
        (area as usize - 2) / 4
    } else {
        0
    };
    if m < count {
        m
    } else {
        0
    }
}

/// Building models of an area: table id -> model, and their textures.
struct Buildings {
    models: HashMap<u16, Rc<Model>>,
    textures: Textures,
}

fn load_buildings(bridge: &Bridge, area: u16, indoor: bool) -> Option<Buildings> {
    let (arc, tex_arc) = if indoor {
        (BUILDINGS_INDOOR, BUILDING_TEXTURES_INDOOR)
    } else {
        (BUILDINGS_OUTDOOR, BUILDING_TEXTURES_OUTDOOR)
    };
    let count = bridge.base_archive(arc).ok()?.len();
    let index = building_member(area, count);
    let m = bridge.member(Member { arc, index }).ok()?;
    let n = *m.get(2)? as usize / 2;
    let mut models = HashMap::new();
    for i in 0..n {
        let def = rd32(&m, 4 + i * 4)? as usize;
        let file = rd32(&m, 4 + (n + i) * 4)? as usize;
        let id = rd16(&m, def)?;
        if let Some(model) = m.get(file..).and_then(nsbmd::parse) {
            models.entry(id).or_insert(Rc::new(model));
        }
    }
    let textures = bridge
        .member(Member {
            arc: tex_arc,
            index,
        })
        .unwrap_or_default();
    Some(Buildings {
        models,
        textures: Textures::new(textures),
    })
}

/// All triangles of a zone in world units, x east and z south from the
/// zone's origin corner (tile (0, 0) starts at the origin).
fn build_scene(bridge: &Bridge, c: &Cells, opts: &Options) -> Result<Vec<Poly>, Error> {
    let area_file = bridge
        .source()
        .read("a/0/1/3")
        .map_err(|e| Error::Missing(format!("a/0/1/3: {e}")))?;
    let rec = area_file
        .get(c.area as usize * 10..c.area as usize * 10 + 10)
        .ok_or_else(|| Error::Missing(format!("area {}", c.area)))?;
    let tex_set = rd16(rec, 2).unwrap_or(0) as usize;
    let indoor = rec[6] != 0;
    let mut ground = Textures::new(
        bridge
            .member(Member {
                arc: GROUND_TEXTURES,
                index: tex_set,
            })
            .unwrap_or_default(),
    );
    let mut buildings = if opts.buildings {
        load_buildings(bridge, c.area, indoor)
    } else {
        None
    };
    let mut models: HashMap<usize, Option<Rc<Model>>> = HashMap::new();
    let mut polys = Vec::new();
    let half = (CHUNK_TILES as f32 * TILE) / 2.0;
    for &(cx, cz, id) in &c.chunks {
        let Ok(bytes) = bridge.member(Member {
            arc: CHUNKS,
            index: id,
        }) else {
            continue;
        };
        let Some(ch) = chunk(&bytes) else { continue };
        let center = [
            ((cx - c.cx0) * CHUNK_TILES) as f32 * TILE + half,
            0.0,
            ((cz - c.cz0) * CHUNK_TILES) as f32 * TILE + half,
        ];
        let model = models
            .entry(id)
            .or_insert_with(|| nsbmd::parse(ch.model).map(Rc::new))
            .clone();
        let base = Affine {
            t: center,
            ..Affine::IDENTITY
        };
        if let Some(model) = &model {
            add_model(&mut polys, model, &mut ground, &base, false);
        }
        let (Some((a, b)), Some(bs)) = (ch.objects, buildings.as_mut()) else {
            continue;
        };
        let count = rd32(&bytes, a).unwrap_or(0) as usize;
        for k in 0..count.min((b - a) / 16) {
            let at = a + 4 + k * 16;
            let (Some(x), Some(y), Some(z), Some(rot), Some(id)) = (
                rd32(&bytes, at),
                rd32(&bytes, at + 4),
                rd32(&bytes, at + 8),
                rd16(&bytes, at + 12),
                rd16(&bytes, at + 14),
            ) else {
                continue;
            };
            // The id is stored high byte first.
            let id = id.swap_bytes();
            let Some(model) = bs.models.get(&id).cloned() else {
                continue;
            };
            let f = |v: u32| v as i32 as f32 / 4096.0;
            let place = Affine::yaw(
                rot as f32 / 65536.0,
                [center[0] + f(x), f(y), center[2] - f(z)],
            );
            add_model(&mut polys, &model, &mut bs.textures, &place, false);
        }
    }
    Ok(polys)
}

struct Screen {
    w: usize,
    h: usize,
    color: Vec<[u8; 4]>,
    /// Larger is nearer.
    depth: Vec<f32>,
}

impl Screen {
    fn new(w: usize, h: usize) -> Self {
        Screen {
            w,
            h,
            color: vec![[0; 4]; w * h],
            depth: vec![f32::MIN; w * h],
        }
    }
}

fn sample(tex: &Rgba, p: &Poly, u: f32, v: f32) -> [u8; 4] {
    let wrap = |t: f32, size: usize, repeat: bool, flip: bool| -> usize {
        let n = size as i32;
        let i = t.floor() as i32;
        let i = if repeat {
            if flip {
                let m = i.rem_euclid(2 * n);
                if m >= n {
                    2 * n - 1 - m
                } else {
                    m
                }
            } else {
                i.rem_euclid(n)
            }
        } else {
            i.clamp(0, n - 1)
        };
        i as usize
    };
    let x = wrap(u, tex.width, p.repeat[0], p.flip[0]);
    let y = wrap(v, tex.height, p.repeat[1], p.flip[1]);
    tex.get(x, y)
}

/// A vertex on the screen. `depth` is linear on the screen and larger is
/// nearer; `iw` is 1 over the view depth (1 when there is no perspective).
#[derive(Clone, Copy)]
struct SV {
    x: f32,
    y: f32,
    depth: f32,
    iw: f32,
    uv: [f32; 2],
    c: [f32; 3],
}

/// Draws one screen triangle of a polygon.
fn raster(screen: &mut Screen, t: &[SV; 3], p: &Poly, translucent_pass: bool) {
    let (a, b, c) = (&t[0], &t[1], &t[2]);
    let area = (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x);
    if area == 0.0 {
        return;
    }
    // Front faces wind this way on screen (checked against the ground).
    let front = area < 0.0;
    match p.cull {
        Cull::Front if !front => return,
        Cull::Back if front => return,
        Cull::Neither => return,
        _ => {}
    }
    let min_x = a.x.min(b.x).min(c.x).floor().max(0.0) as usize;
    let max_x = (a.x.max(b.x).max(c.x).ceil().max(0.0) as usize).min(screen.w);
    let min_y = a.y.min(b.y).min(c.y).floor().max(0.0) as usize;
    let max_y = (a.y.max(b.y).max(c.y).ceil().max(0.0) as usize).min(screen.h);
    let inv = 1.0 / area;
    let eps = -1e-4;
    for y in min_y..max_y {
        for x in min_x..max_x {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            let w0 = ((b.x - px) * (c.y - py) - (b.y - py) * (c.x - px)) * inv;
            let w1 = ((c.x - px) * (a.y - py) - (c.y - py) * (a.x - px)) * inv;
            let w2 = 1.0 - w0 - w1;
            if w0 < eps || w1 < eps || w2 < eps {
                continue;
            }
            let depth = w0 * a.depth + w1 * b.depth + w2 * c.depth;
            let i = y * screen.w + x;
            if depth < screen.depth[i] {
                continue;
            }
            // Perspective-correct weights.
            let (q0, q1, q2) = (w0 * a.iw, w1 * b.iw, w2 * c.iw);
            let norm = 1.0 / (q0 + q1 + q2);
            let (q0, q1, q2) = (q0 * norm, q1 * norm, q2 * norm);
            let mut rgba = match &p.tex {
                Some(tex) => {
                    let u = q0 * a.uv[0] + q1 * b.uv[0] + q2 * c.uv[0];
                    let v = q0 * a.uv[1] + q1 * b.uv[1] + q2 * c.uv[1];
                    sample(tex, p, u, v)
                }
                None => [255, 255, 255, 255],
            };
            if rgba[3] == 0 {
                continue;
            }
            if p.vertex_color {
                for (k, ch) in rgba.iter_mut().take(3).enumerate() {
                    let shade = q0 * a.c[k] + q1 * b.c[k] + q2 * c.c[k];
                    *ch = (*ch as f32 * shade.clamp(0.0, 1.0)) as u8;
                }
            }
            let alpha = rgba[3] as f32 / 255.0 * (p.alpha as f32 / 31.0);
            if translucent_pass {
                let under = screen.color[i];
                let mix = |f: u8, b: u8| (f as f32 * alpha + b as f32 * (1.0 - alpha)) as u8;
                let ua = under[3] as f32 / 255.0;
                screen.color[i] = [
                    mix(rgba[0], under[0]),
                    mix(rgba[1], under[1]),
                    mix(rgba[2], under[2]),
                    ((alpha + ua * (1.0 - alpha)) * 255.0) as u8,
                ];
            } else {
                screen.color[i] = [rgba[0], rgba[1], rgba[2], 255];
                screen.depth[i] = depth;
            }
        }
    }
}

/// How world points reach the screen.
enum Projection {
    /// Seen from above: screen x from world x, screen y from world z,
    /// higher is nearer.
    TopDown {
        scale: f32,
    },
    Camera(Camera),
}

struct Camera {
    eye: [f32; 3],
    right: [f32; 3],
    up: [f32; 3],
    forward: [f32; 3],
    /// Focal length in pixels.
    focal: f32,
    cx: f32,
    cy: f32,
    near: f32,
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn norm(a: [f32; 3]) -> [f32; 3] {
    let l = dot(a, a).sqrt().max(1e-9);
    [a[0] / l, a[1] / l, a[2] / l]
}

/// A vertex in camera space (x right, y up, z forward).
#[derive(Clone, Copy)]
struct ViewVertex {
    q: [f32; 3],
    uv: [f32; 2],
    c: [f32; 3],
}

impl Camera {
    /// The polygon turned to face the camera about its pivot.
    fn face(&self, p: &Poly, kind: Billboard, pivot: [f32; 3]) -> Poly {
        let back = [-self.forward[0], -self.forward[1], -self.forward[2]];
        let (x, y, z) = match kind {
            Billboard::Full => (self.right, self.up, back),
            Billboard::Y => {
                let z = norm([back[0], 0.0, back[2]]);
                (cross([0.0, 1.0, 0.0], z), [0.0, 1.0, 0.0], z)
            }
        };
        let mut out = Poly {
            v: p.v,
            tex: p.tex.clone(),
            billboard: None,
            ..*p
        };
        for v in &mut out.v {
            let d = [v.p[0] - pivot[0], v.p[1] - pivot[1], v.p[2] - pivot[2]];
            for k in 0..3 {
                v.p[k] = pivot[k] + d[0] * x[k] + d[1] * y[k] + d[2] * z[k];
            }
        }
        out
    }
}

impl Projection {
    /// The screen triangles of a polygon (none when it is behind the camera).
    fn project(&self, p: &Poly) -> Vec<[SV; 3]> {
        match self {
            Projection::TopDown { scale } => {
                // A billboard faces the camera: nothing to see from above.
                if p.billboard.is_some() {
                    return Vec::new();
                }
                let sv = |v: &V| SV {
                    x: v.p[0] * scale,
                    y: v.p[2] * scale,
                    depth: v.p[1],
                    iw: 1.0,
                    uv: v.uv,
                    c: v.c,
                };
                vec![[sv(&p.v[0]), sv(&p.v[1]), sv(&p.v[2])]]
            }
            Projection::Camera(cam) => {
                let view = |v: &V| {
                    let d = [
                        v.p[0] - cam.eye[0],
                        v.p[1] - cam.eye[1],
                        v.p[2] - cam.eye[2],
                    ];
                    ViewVertex {
                        q: [dot(d, cam.right), dot(d, cam.up), dot(d, cam.forward)],
                        uv: v.uv,
                        c: v.c,
                    }
                };
                let turned;
                let p = match p.billboard {
                    Some((kind, pivot)) => {
                        turned = cam.face(p, kind, pivot);
                        &turned
                    }
                    None => p,
                };
                let poly: Vec<ViewVertex> = p.v.iter().map(view).collect();
                // Clip to the near plane.
                let mut out: Vec<ViewVertex> = Vec::new();
                for i in 0..3 {
                    let (a, b) = (poly[i], poly[(i + 1) % 3]);
                    let (ain, bin) = (a.q[2] >= cam.near, b.q[2] >= cam.near);
                    if ain {
                        out.push(a);
                    }
                    if ain != bin {
                        let t = (cam.near - a.q[2]) / (b.q[2] - a.q[2]);
                        let l = |x: f32, y: f32| x + (y - x) * t;
                        out.push(ViewVertex {
                            q: [l(a.q[0], b.q[0]), l(a.q[1], b.q[1]), cam.near],
                            uv: [l(a.uv[0], b.uv[0]), l(a.uv[1], b.uv[1])],
                            c: [l(a.c[0], b.c[0]), l(a.c[1], b.c[1]), l(a.c[2], b.c[2])],
                        });
                    }
                }
                let sv: Vec<SV> = out
                    .iter()
                    .map(|v| {
                        let iw = 1.0 / v.q[2];
                        SV {
                            x: cam.cx + cam.focal * v.q[0] * iw,
                            y: cam.cy - cam.focal * v.q[1] * iw,
                            depth: iw,
                            iw,
                            uv: v.uv,
                            c: v.c,
                        }
                    })
                    .collect();
                (1..sv.len().saturating_sub(1))
                    .map(|i| [sv[0], sv[i], sv[i + 1]])
                    .collect()
            }
        }
    }
}

fn draw(screen: &mut Screen, polys: &[Poly], proj: &Projection) {
    for translucent in [false, true] {
        for p in polys.iter().filter(|p| p.translucent == translucent) {
            for t in proj.project(p) {
                raster(screen, &t, p, translucent);
            }
        }
    }
}

/// Averages `ss` by `ss` samples into one pixel (colour weighted by alpha).
fn downsample(screen: &Screen, ss: usize) -> Rgba {
    let (w, h) = (screen.w / ss, screen.h / ss);
    let mut image = Rgba::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let (mut sum, mut alpha) = ([0u32; 3], 0u32);
            for dy in 0..ss {
                for dx in 0..ss {
                    let p = screen.color[(y * ss + dy) * screen.w + x * ss + dx];
                    for (acc, &c) in sum.iter_mut().zip(&p) {
                        *acc += c as u32 * p[3] as u32;
                    }
                    alpha += p[3] as u32;
                }
            }
            let n = (ss * ss) as u32;
            let o = (y * w + x) * 4;
            if let Some(div) = std::num::NonZeroU32::new(alpha) {
                for (k, acc) in sum.iter().enumerate() {
                    image.pixels[o + k] = (*acc / div) as u8;
                }
                image.pixels[o + 3] = (alpha / n) as u8;
            }
        }
    }
    image
}

fn finish(screen: &Screen, ss: usize) -> Rgba {
    if ss == 1 {
        let mut image = Rgba::new(screen.w, screen.h);
        for (i, px) in screen.color.iter().enumerate() {
            image.pixels[i * 4..i * 4 + 4].copy_from_slice(px);
        }
        image
    } else {
        downsample(screen, ss)
    }
}

/// Most pixels an image may have.
const MAX_PIXELS: usize = 40_000_000;
/// Most samples a render may hold before it stops supersampling.
const MAX_SAMPLES: usize = 24_000_000;

fn choose_supersample(requested: usize, w: usize, h: usize, px: usize) -> usize {
    let want = if requested > 0 {
        requested
    } else if px < 24 {
        2
    } else {
        1
    };
    let mut ss = want.clamp(1, 4);
    while ss > 1 && w * h * ss * ss > MAX_SAMPLES {
        ss -= 1;
    }
    ss
}

/// Draws a zone's map from above.
pub fn render(bridge: &Bridge, zone: usize, opts: &Options) -> Result<Render, Error> {
    let c = cells(bridge, zone)?;
    let layout = layout(&c, zone, opts.px.max(1));
    if layout.image_width * layout.image_height > MAX_PIXELS {
        return Err(Error::BadPath(format!(
            "map render of {}x{} pixels is too large; use a smaller px",
            layout.image_width, layout.image_height
        )));
    }
    let polys = build_scene(bridge, &c, opts)?;
    let ss = choose_supersample(
        opts.supersample,
        layout.image_width,
        layout.image_height,
        layout.px,
    );
    let mut screen = Screen::new(layout.image_width * ss, layout.image_height * ss);
    let proj = Projection::TopDown {
        scale: (layout.px * ss) as f32 / TILE,
    };
    draw(&mut screen, &polys, &proj);
    Ok(Render {
        layout,
        image: finish(&screen, ss),
    })
}

/// The game camera preview: where to look and how big.
#[derive(Debug, Clone)]
pub struct CameraOptions {
    /// The tile the camera follows (zone tiles, as `Layout`).
    pub x: f32,
    pub z: f32,
    pub width: usize,
    pub height: usize,
    /// A camera setting (a/0/6/0 record); the zone's own when `None`.
    pub setting: Option<usize>,
    pub buildings: bool,
    pub supersample: usize,
}

impl Default for CameraOptions {
    fn default() -> Self {
        CameraOptions {
            x: 0.0,
            z: 0.0,
            width: 512,
            height: 384,
            setting: None,
            buildings: true,
            supersample: 0,
        }
    }
}

const CAMERA_SETTINGS: u16 = 60;
const CAMERA_RECORD: usize = 0x2C;

/// The highest ground at a world point (x, z), 0 when nothing is there.
fn ground_height(polys: &[Poly], x: f32, z: f32) -> f32 {
    let mut best: Option<f32> = None;
    for p in polys {
        let (a, b, c) = (&p.v[0].p, &p.v[1].p, &p.v[2].p);
        let area = (b[0] - a[0]) * (c[2] - a[2]) - (b[2] - a[2]) * (c[0] - a[0]);
        if area == 0.0 {
            continue;
        }
        let w0 = ((b[0] - x) * (c[2] - z) - (b[2] - z) * (c[0] - x)) / area;
        let w1 = ((c[0] - x) * (a[2] - z) - (c[2] - z) * (a[0] - x)) / area;
        let w2 = 1.0 - w0 - w1;
        if w0 >= 0.0 && w1 >= 0.0 && w2 >= 0.0 {
            let h = w0 * a[1] + w1 * b[1] + w2 * c[1];
            best = Some(best.map_or(h, |o: f32| o.max(h)));
        }
    }
    best.unwrap_or(0.0)
}

/// Draws what the game's field camera shows around a tile: a perspective
/// view tilted by the zone's camera setting (a/0/6/0, read by
/// `FIELD_CAMERA_ReadSetting`: distance, pitch, yaw, half field of view,
/// near plane, and a target shift or a fixed target).
pub fn render_camera(bridge: &Bridge, zone: usize, opts: &CameraOptions) -> Result<Rgba, Error> {
    let c = cells(bridge, zone)?;
    let (w, h) = (opts.width.clamp(16, 4096), opts.height.clamp(16, 4096));
    let setting = match opts.setting {
        Some(s) => s,
        None => {
            let headers = bridge.member(Member {
                arc: ZONE_HEADERS,
                index: 0,
            })?;
            (rd16(&headers, zone * 48 + 0x1C).unwrap_or(0) >> 9) as usize
        }
    };
    let file = bridge.member(Member {
        arc: CAMERA_SETTINGS,
        index: 0,
    })?;
    let rec = file
        .get(setting * CAMERA_RECORD..(setting + 1) * CAMERA_RECORD)
        .ok_or_else(|| Error::Missing(format!("camera setting {setting}")))?;
    let s32 = |at: usize| rd32(rec, at).unwrap_or(0) as i32;
    let angle = |v: i32| (v as u16 as f32) / 65536.0 * std::f32::consts::TAU;
    let (distance, pitch, yaw) = (s32(0) as f32, angle(s32(4)), angle(s32(8)));
    let fov_half = angle(rd16(rec, 0x12).unwrap_or(0) as i32);
    let near = (s32(0x14) as f32 / 4096.0).max(0.5);
    let vec = [s32(0x20), s32(0x24), s32(0x28)].map(|v| v as f32 / 4096.0);
    let polys = build_scene(
        bridge,
        &c,
        &Options {
            px: 16,
            buildings: opts.buildings,
            supersample: 0,
        },
    )?;
    let (wx, wz) = ((opts.x + 0.5) * TILE, (opts.z + 0.5) * TILE);
    let target = if s32(0x1C) != 0 {
        let ground = ground_height(&polys, wx, wz);
        [wx + vec[0], ground + vec[1], wz + vec[2]]
    } else {
        vec
    };
    // The eye sits behind the target (FIELD_CAMERA_CalcOffsetPoint).
    let (ps, pc) = (pitch.sin(), pitch.cos().abs().max(0.05));
    let dir = norm([yaw.sin() * pc, ps, yaw.cos() * pc]);
    let eye = [
        target[0] + dir[0] * distance,
        target[1] + dir[1] * distance,
        target[2] + dir[2] * distance,
    ];
    let forward = norm([target[0] - eye[0], target[1] - eye[1], target[2] - eye[2]]);
    let right = norm(cross(forward, [0.0, 1.0, 0.0]));
    let up = cross(right, forward);
    let ss = choose_supersample(opts.supersample, w, h, 16);
    let cam = Camera {
        eye,
        right,
        up,
        forward,
        focal: (h * ss) as f32 / 2.0 / fov_half.tan().max(0.01),
        cx: (w * ss) as f32 / 2.0,
        cy: (h * ss) as f32 / 2.0,
        near,
    };
    let mut screen = Screen::new(w * ss, h * ss);
    draw(&mut screen, &polys, &Projection::Camera(cam));
    Ok(finish(&screen, ss))
}

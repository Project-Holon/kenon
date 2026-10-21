//! A zone's map drawn the way the game shows it: the 3D map models with
//! their textures, seen from above and lined up with the tile grid, so the
//! map editor can draw them under the grid, or through the game's field
//! camera. A software rasteriser (nearest texel, depth buffer, alpha).
//!
//! Where the pieces are (from aletheia `src/game/ov021/field_g3d_map_block.c`,
//! `field_buildmodel.c`, `field_light.c`, `zone_light.c`, and a look at the
//! data):
//!
//! ```text
//! zone header   a/0/1/2: u16 areaId at 0x02, u16 matrixId at 0x04
//! area record   a/0/1/3 (flat file, 10 bytes each): u16 modelSetId, u16
//!               texSetId, u8 itaAnime, u8 itpAnime, u8 innerOuter at 6 (1 =
//!               indoors), u8 lightType at 7, u8 edgeMark, u8 billboardColor.
//!               Record = areaId + season when 2 <= areaId < 0xD2 (four
//!               seasonal variants per area, `ZONEDATA_CreateAreaData`)
//! ground        a/0/0/8 chunk: tag, u16, u32 BMD0 offset at 4; the model's
//!               textures are member texSetId of a/0/1/4 (a BTX0), so the
//!               season picks the texture set
//! objects       the chunk's last section: u32 count, then 16 bytes each:
//!               fx32 x, y, z (z negated), u16 rotation, u16 model id.
//!               Section start: u32 at 12 (WB, RD), 16 (GC), 8 (NG)
//! buildings     member m of a/2/3/0 (outdoors) or a/2/2/9 (indoors): u8 at
//!               2 is twice the model count n, u32 offsets at 4: n model
//!               definitions (u16 model id first), then n BMD0 files;
//!               textures: the same member of a/1/7/7 (outdoors) or a/1/7/6.
//!               m = areaId - 0xD2 when areaId >= 0xD2, (areaId - 2) / 4 when
//!               2 <= areaId, else 0 (0 again when past the last member)
//! light         a/0/6/1 (arc 61) member lightType: 0x34-byte keyframes
//!               (`FieldLightKeyframe`): u16 timezone, s16 minute offset, u8
//!               slotUsed[4], u16 colour[4], s16 direction[4][3] (4.12),
//!               then material diffuse, ambient, specular, emission, fog and
//!               clear colours. A keyframe holds until its time; the time is
//!               hour(season, timezone) * 3600 + minute * 60, with the hour
//!               from `_TimeZoneLightTable` (arm9 0x0209DF10, 5 bytes per
//!               season). The lights are world space; a lit vertex is
//!               emission + sum over the material's enabled lights of
//!               ambient * light + diffuse * light * max(0, -(dir . normal))
//! ```
//!
//! A tile is 16 world units. A chunk model is centred on the chunk.
//!
//! The built scene (decoded models and textures, lit) is kept per zone and
//! per option set through `Bridge::cached`, so a second view of a zone, or
//! a camera that moves, only rasterises.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, PoisonError};

use serde::Serialize;

use crate::bridge::{Bridge, Error, Member};
use crate::code::Image;
use crate::g3d;
use crate::image::Rgba;
use crate::nsbmd::{self, Affine, Billboard, Cull, Material, Model};
use crate::zonemap;

const MATRICES: u16 = 9;
const CHUNKS: u16 = 8;
const ZONE_HEADERS: u16 = 12;
const GROUND_TEXTURES: u16 = 14;
const BUILDINGS_OUTDOOR: u16 = 230;
const BUILDINGS_INDOOR: u16 = 229;
const BUILDING_TEXTURES_OUTDOOR: u16 = 177;
const BUILDING_TEXTURES_INDOOR: u16 = 176;
const LIGHT_TABLES: u16 = 61;
const PERSON_TEXTURES: u16 = 49;
const AREA_RECORDS: usize = 282;
/// `_TimeZoneLightTable` in the ARM9 image: 5 hours per season.
const TIMEZONE_LIGHT_TABLE: u32 = 0x0209_DF10;
const CHUNK_TILES: usize = 32;
/// World units per tile.
const TILE: f32 = 16.0;
/// A person's picture is 32 by 32 texels, drawn 32 by 32 world units (the
/// editor draws it two tiles across).
const PERSON_SIZE: f32 = 32.0;

fn rd16(b: &[u8], at: usize) -> Option<u16> {
    b.get(at..at + 2).map(|x| u16::from_le_bytes([x[0], x[1]]))
}

fn rd32(b: &[u8], at: usize) -> Option<u32> {
    b.get(at..at + 4)
        .map(|x| u32::from_le_bytes(x.try_into().unwrap()))
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
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

/// What the top-down view does with tall buildings.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Roofs {
    /// Cut where the zone has buildings much taller than a few tiles.
    Auto,
    /// Draw everything.
    All,
    /// Drop opaque polygons that start this far (world units) above the
    /// ground beneath them: roofs, bridges between towers.
    Cut(f32),
}

/// What to draw.
#[derive(Debug, Clone)]
pub struct Options {
    pub px: usize,
    /// Draw the buildings and props on the ground.
    pub buildings: bool,
    /// Samples per pixel edge (1 to 4); 0 picks 2 for small tiles.
    pub supersample: usize,
    /// Draw the zone's people (off by default from above: the editor draws
    /// them itself).
    pub people: bool,
    /// 0 to 3: spring, summer, autumn, winter.
    pub season: u8,
    /// Hour of the day (0 to 24) for the field light.
    pub hour: f32,
    /// Light the surfaces with the field light.
    pub light: bool,
    pub roofs: Roofs,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            px: 16,
            buildings: true,
            supersample: 0,
            people: false,
            season: 0,
            hour: DAY_HOUR,
            light: true,
            roofs: Roofs::Auto,
        }
    }
}

/// The hour `time=day` means.
pub const DAY_HOUR: f32 = 12.0;

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

/// A decoded texture and what its alpha holds.
struct Tex {
    img: Rgba,
    /// Some pixels are neither clear nor solid.
    partial: bool,
    /// Some pixels are clear (cut-outs such as leaves).
    cutout: bool,
}

impl Tex {
    fn new(img: Rgba) -> Self {
        let (mut partial, mut cutout) = (false, false);
        for p in img.pixels.chunks(4) {
            match p[3] {
                255 => {}
                0 => cutout = true,
                _ => partial = true,
            }
        }
        Tex {
            img,
            partial,
            cutout,
        }
    }
}

/// A texture set: the BTX0 (or BMD0) bytes of a member, decoded by name.
struct Textures {
    bytes: Vec<u8>,
    cache: HashMap<String, Option<Arc<Tex>>>,
}

impl Textures {
    fn new(bytes: Vec<u8>) -> Self {
        Textures {
            bytes,
            cache: HashMap::new(),
        }
    }

    fn get(&mut self, name: &str) -> Option<Arc<Tex>> {
        if let Some(t) = self.cache.get(name) {
            return t.clone();
        }
        let found = g3d::find_tex0(&self.bytes).into_iter().find_map(|block| {
            let tex = g3d::Tex0::parse(block)?;
            let i = tex.textures.iter().position(|t| t.name == name)?;
            tex.decode(i)
        });
        let found = found.map(|img| Arc::new(Tex::new(img)));
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
    tex: Option<Arc<Tex>>,
    alpha: u8,
    cull: Cull,
    vertex_color: bool,
    repeat: [bool; 2],
    flip: [bool; 2],
    /// Translucent by polygon alpha or by the texture's alpha format.
    translucent: bool,
    /// A billboard turns to face the camera about this point.
    billboard: Option<(Billboard, [f32; 3])>,
    /// Part of a chunk's ground model (not a building or prop).
    ground: bool,
    /// How far its lowest point is above the ground beneath (opaque
    /// polygons only; 0 for the rest).
    above: f32,
}

impl Poly {
    fn min_y(&self) -> f32 {
        self.v[0].p[1].min(self.v[1].p[1]).min(self.v[2].p[1])
    }
}

/// The four light slots of the field light.
#[derive(Clone, Copy)]
struct Lights {
    dir: [[f32; 3]; 4],
    color: [[f32; 3]; 4],
    /// The background colour (the keyframe's clear colour).
    clear: [u8; 3],
}

impl Lights {
    /// What the game's daylight comes to when no table can be read.
    fn fixed() -> Self {
        Lights {
            dir: [
                norm([-0.47, -0.87, -0.07]),
                [0.0, 0.0, 1.0],
                [0.0; 3],
                [0.0; 3],
            ],
            color: [[0.75, 0.75, 0.65], [0.2, 0.2, 0.2], [0.0; 3], [0.0; 3]],
            clear: [150, 190, 235],
        }
    }

    /// The lit colour of a vertex with this normal (world space).
    fn shade(&self, m: &Material, n: [f32; 3]) -> [f32; 3] {
        let mut c = m.emission;
        for i in 0..4 {
            if m.light_mask >> i & 1 == 0 {
                continue;
            }
            let d = (-dot(self.dir[i], n)).max(0.0);
            for (k, ck) in c.iter_mut().enumerate() {
                *ck += m.diffuse[k] * self.color[i][k] * d + m.ambient[k] * self.color[i][k];
            }
        }
        c.map(|v| v.clamp(0.0, 1.0))
    }
}

fn rgb5(c: u16) -> [f32; 3] {
    [
        (c & 31) as f32 / 31.0,
        ((c >> 5) & 31) as f32 / 31.0,
        ((c >> 10) & 31) as f32 / 31.0,
    ]
}

/// The hour of each time zone per season (`_TimeZoneLightTable`).
fn light_hours(bridge: &Bridge) -> Option<[[u8; 5]; 4]> {
    let raw = bridge.code()?.read(Image::Arm9, TIMEZONE_LIGHT_TABLE, 20)?;
    let mut out = [[0u8; 5]; 4];
    for (s, row) in out.iter_mut().enumerate() {
        row.copy_from_slice(&raw[s * 5..s * 5 + 5]);
    }
    Some(out)
}

/// The light table of an area at an hour: the keyframe `FIELD_LIGHT_Create`
/// starts from (the first whose time is after now).
fn load_lights(bridge: &Bridge, light_type: u8, season: u8, hour: f32) -> Lights {
    let fixed = Lights::fixed();
    let Ok(table) = bridge.member(Member {
        arc: LIGHT_TABLES,
        index: light_type as usize,
    }) else {
        return fixed;
    };
    let Some(hours) = light_hours(bridge) else {
        return fixed;
    };
    let keys = table.len() / 0x34;
    let seconds = (hour.clamp(0.0, 24.0) * 3600.0) as i32;
    let time = |k: usize| -> Option<i32> {
        let r = &table[k * 0x34..k * 0x34 + 0x34];
        let zone = rd16(r, 0)? as usize;
        let minute = rd16(r, 2)? as i16 as i32;
        let h = *hours.get(season as usize % 4)?.get(zone)? as i32;
        Some(h * 3600 + minute * 60)
    };
    let pick = (0..keys)
        .find(|&k| time(k).is_some_and(|t| t > seconds))
        .unwrap_or(0);
    if pick >= keys {
        return fixed;
    }
    let r = &table[pick * 0x34..pick * 0x34 + 0x34];
    let mut lights = Lights {
        dir: [[0.0; 3]; 4],
        color: [[0.0; 3]; 4],
        clear: rgb5(rd16(r, 0x32).unwrap_or(0)).map(|v| (v * 255.0) as u8),
    };
    for i in 0..4 {
        if r[4 + i] == 0 {
            continue;
        }
        let d = |k: usize| rd16(r, 16 + i * 6 + k * 2).unwrap_or(0) as i16 as f32 / 4096.0;
        lights.dir[i] = norm([d(0), d(1), d(2)]);
        lights.color[i] = rgb5(rd16(r, 8 + i * 2).unwrap_or(0));
    }
    lights
}

fn add_model(
    out: &mut Vec<Poly>,
    model: &Model,
    textures: &mut Textures,
    base: &Affine,
    lights: Option<&Lights>,
    ground: bool,
) {
    for draw in &model.draws {
        let mat = &model.materials[draw.material];
        let tex = mat.texture.as_deref().and_then(|n| textures.get(n));
        let translucent = mat.alpha < 31 || tex.as_ref().is_some_and(|t| t.partial);
        if mat.alpha == 0 {
            continue;
        }
        let node = model.nodes[draw.node].then(base);
        // A lit surface takes its colour from the lights; a billboard has
        // no fixed normal.
        let lit = lights.filter(|_| mat.light_mask != 0 && draw.billboard.is_none());
        let to_world = |v: &nsbmd::Vertex| {
            let s = model.pos_scale;
            let mut c = v.color;
            let mut shaded = false;
            if let Some(l) = lit {
                if v.normal != [0.0; 3] {
                    let r = &node.r;
                    let n = norm([
                        v.normal[0] * r[0][0] + v.normal[1] * r[1][0] + v.normal[2] * r[2][0],
                        v.normal[0] * r[0][1] + v.normal[1] * r[1][1] + v.normal[2] * r[2][1],
                        v.normal[0] * r[0][2] + v.normal[1] * r[1][2] + v.normal[2] * r[2][2],
                    ]);
                    c = l.shade(mat, n);
                    shaded = true;
                }
            }
            (
                V {
                    p: node.apply([v.pos[0] * s, v.pos[1] * s, v.pos[2] * s]),
                    uv: v.uv,
                    c,
                },
                shaded,
            )
        };
        for tri in &model.shapes[draw.shape] {
            let a = to_world(&tri[0]);
            let b = to_world(&tri[1]);
            let c = to_world(&tri[2]);
            out.push(Poly {
                v: [a.0, b.0, c.0],
                tex: tex.clone(),
                alpha: mat.alpha,
                cull: mat.cull,
                vertex_color: mat.vertex_color || (a.1 && b.1 && c.1),
                repeat: mat.repeat,
                flip: mat.flip,
                translucent,
                billboard: draw.billboard.map(|b| (b, node.apply([0.0; 3]))),
                ground,
                above: 0.0,
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
    models: HashMap<u16, Arc<Model>>,
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
            models.entry(id).or_insert(Arc::new(model));
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

/// The scene's ground, by tile, for height lookups.
struct GroundIndex {
    w: usize,
    h: usize,
    cells: Vec<Vec<u32>>,
}

impl GroundIndex {
    fn new(polys: &[Poly], w: usize, h: usize) -> Self {
        let mut cells = vec![Vec::new(); w * h];
        for (i, p) in polys.iter().enumerate() {
            if !p.ground || p.billboard.is_some() || p.tex.as_ref().is_some_and(|t| t.cutout) {
                continue;
            }
            let (a, b, c) = (&p.v[0].p, &p.v[1].p, &p.v[2].p);
            let n = cross(
                [b[0] - a[0], b[1] - a[1], b[2] - a[2]],
                [c[0] - a[0], c[1] - a[1], c[2] - a[2]],
            );
            // Roughly level only.
            if n[1].abs() < 0.5 * dot(n, n).sqrt() {
                continue;
            }
            let lo = |f: fn(&[f32; 3]) -> f32| f(a).min(f(b)).min(f(c));
            let hi = |f: fn(&[f32; 3]) -> f32| f(a).max(f(b)).max(f(c));
            let cell = |v: f32, n: usize| ((v / TILE).floor().max(0.0) as usize).min(n - 1);
            let (x0, x1) = (cell(lo(|v| v[0]), w), cell(hi(|v| v[0]), w));
            let (z0, z1) = (cell(lo(|v| v[2]), h), cell(hi(|v| v[2]), h));
            for z in z0..=z1 {
                for x in x0..=x1 {
                    cells[z * w + x].push(i as u32);
                }
            }
        }
        GroundIndex { w, h, cells }
    }

    /// The ground at a world point (x, z): the highest level polygon that
    /// is within `MAX_STEP` of the lowest one (a bridge or a pier counts,
    /// a skyscraper roof does not), `None` when nothing is there.
    fn height(&self, polys: &[Poly], x: f32, z: f32) -> Option<f32> {
        let (cx, cz) = ((x / TILE).floor(), (z / TILE).floor());
        if cx < 0.0 || cz < 0.0 || cx as usize >= self.w || cz as usize >= self.h {
            return None;
        }
        let mut found: Vec<f32> = Vec::new();
        for &i in &self.cells[cz as usize * self.w + cx as usize] {
            let p = &polys[i as usize];
            let (a, b, c) = (&p.v[0].p, &p.v[1].p, &p.v[2].p);
            let area = (b[0] - a[0]) * (c[2] - a[2]) - (b[2] - a[2]) * (c[0] - a[0]);
            if area == 0.0 {
                continue;
            }
            let w0 = ((b[0] - x) * (c[2] - z) - (b[2] - z) * (c[0] - x)) / area;
            let w1 = ((c[0] - x) * (a[2] - z) - (c[2] - z) * (a[0] - x)) / area;
            let w2 = 1.0 - w0 - w1;
            if w0 >= -1e-4 && w1 >= -1e-4 && w2 >= -1e-4 {
                found.push(w0 * a[1] + w1 * b[1] + w2 * c[1]);
            }
        }
        let lowest = found.iter().copied().fold(f32::MAX, f32::min);
        found
            .into_iter()
            .filter(|&h| h <= lowest + MAX_STEP)
            .fold(None, |best: Option<f32>, h| {
                Some(best.map_or(h, |o| o.max(h)))
            })
    }
}

/// Ground this much above the lowest ground at a point is a roof or
/// overhead structure, not something to stand on.
const MAX_STEP: f32 = 64.0;

/// Everything a zone draws except the people: built once per option set.
struct Scene {
    polys: Vec<Poly>,
    ground: GroundIndex,
    /// The height below which 95 of 100 building polygons start: a tall
    /// zone has far more above than a few tiles.
    tall_roofs: bool,
    lights: Option<Lights>,
}

/// Buildings taller than this many tiles make `Roofs::Auto` cut.
const TALL_TILES: f32 = 8.0;
/// Where `Roofs::Auto` cuts, in tiles.
const AUTO_CUT_TILES: f32 = 6.0;

/// All triangles of a zone in world units, x east and z south from the
/// zone's origin corner (tile (0, 0) starts at the origin).
fn build_scene(bridge: &Bridge, c: &Cells, key: &SceneKey) -> Result<Scene, Error> {
    let area_file = bridge
        .source()
        .read("a/0/1/3")
        .map_err(|e| Error::Missing(format!("a/0/1/3: {e}")))?;
    // Four seasonal variants of each area.
    let index = if (2..0xD2).contains(&c.area) {
        c.area as usize + key.season as usize % 4
    } else {
        c.area as usize
    };
    let index = if index >= AREA_RECORDS { 0 } else { index };
    let rec = area_file
        .get(index * 10..index * 10 + 10)
        .ok_or_else(|| Error::Missing(format!("area {index}")))?;
    let tex_set = rd16(rec, 2).unwrap_or(0) as usize;
    let indoor = rec[6] != 0;
    let lights = key
        .light
        .then(|| load_lights(bridge, rec[7], key.season, key.hour()));
    let mut ground = Textures::new(
        bridge
            .member(Member {
                arc: GROUND_TEXTURES,
                index: tex_set,
            })
            .unwrap_or_default(),
    );
    let mut buildings = if key.buildings {
        load_buildings(bridge, c.area, indoor)
    } else {
        None
    };
    let mut models: HashMap<usize, Option<Arc<Model>>> = HashMap::new();
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
            .or_insert_with(|| nsbmd::parse(ch.model).map(Arc::new))
            .clone();
        let base = Affine {
            t: center,
            ..Affine::IDENTITY
        };
        if let Some(model) = &model {
            add_model(&mut polys, model, &mut ground, &base, lights.as_ref(), true);
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
            add_model(
                &mut polys,
                &model,
                &mut bs.textures,
                &place,
                lights.as_ref(),
                false,
            );
        }
    }
    let (w, h) = (c.wc * CHUNK_TILES, c.hc * CHUNK_TILES);
    let ground = GroundIndex::new(&polys, w, h);
    // How far each opaque polygon hangs above the ground beneath it.
    let above: Vec<f32> = polys
        .iter()
        .map(|p| {
            let y = p.min_y();
            if y < TILE
                || p.billboard.is_some()
                || p.tex.as_ref().is_some_and(|t| t.cutout || t.partial)
            {
                return 0.0;
            }
            let at = |k: usize| (p.v[0].p[k] + p.v[1].p[k] + p.v[2].p[k]) / 3.0;
            y - ground.height(&polys, at(0), at(2)).unwrap_or(0.0)
        })
        .collect();
    for (p, a) in polys.iter_mut().zip(above) {
        p.above = a;
    }
    let tall_roofs = polys.iter().any(|p| p.above > TALL_TILES * TILE);
    Ok(Scene {
        polys,
        ground,
        tall_roofs,
        lights,
    })
}

/// What changes a built scene.
#[derive(Clone, Copy)]
struct SceneKey {
    buildings: bool,
    season: u8,
    /// The hour in minutes.
    minutes: u32,
    light: bool,
}

impl SceneKey {
    fn hour(&self) -> f32 {
        self.minutes as f32 / 60.0
    }

    fn name(&self, zone: usize) -> String {
        format!(
            "maprender/scene/{zone}/{}/{}/{}/{}",
            self.buildings as u8,
            self.season % 4,
            self.minutes,
            self.light as u8
        )
    }
}

fn minutes(hour: f32) -> u32 {
    (hour.clamp(0.0, 24.0) * 60.0).round() as u32
}

/// A scene held by `Bridge::cached`. It can be emptied to free memory
/// while the cache entry stays.
#[derive(Default)]
struct Slot(Mutex<Option<Arc<Scene>>>);

/// The slots that hold a scene, oldest first.
static RESIDENT: Mutex<VecDeque<Arc<Slot>>> = Mutex::new(VecDeque::new());
/// Most scenes kept at once (a big zone is tens of megabytes).
const RESIDENT_MAX: usize = 4;

/// The scene of a zone, built on the first call and kept after.
fn scene(bridge: &Bridge, zone: usize, key: &SceneKey) -> Result<Arc<Scene>, Error> {
    let slot = bridge.cached(&key.name(zone), || Ok(Slot::default()))?;
    let mut held = lock(&slot.0);
    if let Some(s) = held.as_ref() {
        return Ok(s.clone());
    }
    let c = cells(bridge, zone)?;
    let built = Arc::new(build_scene(bridge, &c, key)?);
    *held = Some(built.clone());
    drop(held);
    let mut resident = lock(&RESIDENT);
    resident.retain(|s| !Arc::ptr_eq(s, &slot));
    resident.push_back(slot);
    while resident.len() > RESIDENT_MAX {
        if let Some(old) = resident.pop_front() {
            *lock(&old.0) = None;
        }
    }
    Ok(built)
}

/// A person standing on a tile.
struct Person {
    tex: Arc<Tex>,
    /// Tile (zone tiles).
    x: i32,
    z: i32,
}

/// The zone's people with their facing pictures.
fn people(bridge: &Bridge, zone: usize) -> Vec<Person> {
    let (Ok(map), Ok(sprites)) = (
        zonemap::zone_map(bridge, zone),
        zonemap::object_sprites(bridge),
    ) else {
        return Vec::new();
    };
    let mut cache: HashMap<(u16, usize, bool), Option<Arc<Tex>>> = HashMap::new();
    let mut out = Vec::new();
    for ev in map.events.iter().filter(|e| e.kind == "npc") {
        if ev.x < 0 || ev.z < 0 {
            continue;
        }
        let value = |key: &str| ev.values.iter().find(|v| v.key == key).map(|v| v.value);
        let code = value("objCode").unwrap_or(0).max(0) as usize;
        let Some(Some(sprite)) = sprites.get(code).copied() else {
            continue;
        };
        let (frame, mirror) = zonemap::sprite_frame(sprite.frames, value("dirHeader").unwrap_or(1));
        let member = sprite.tex;
        let tex = cache
            .entry((member, frame, mirror))
            .or_insert_with(|| {
                let bytes = bridge
                    .member(Member {
                        arc: PERSON_TEXTURES,
                        index: member as usize,
                    })
                    .ok()?;
                let block = g3d::find_tex0(&bytes).into_iter().next()?;
                let mut img = g3d::Tex0::parse(block)?.decode(frame)?;
                if mirror {
                    let w = img.width;
                    for row in img.pixels.chunks_mut(w * 4) {
                        for x in 0..w / 2 {
                            for c in 0..4 {
                                row.swap(x * 4 + c, (w - 1 - x) * 4 + c);
                            }
                        }
                    }
                }
                Some(Arc::new(Tex::new(img)))
            })
            .clone();
        if let Some(tex) = tex {
            out.push(Person {
                tex,
                x: ev.x,
                z: ev.z,
            });
        }
    }
    out
}

struct Screen {
    w: usize,
    h: usize,
    color: Vec<[u8; 4]>,
    /// Larger is nearer.
    depth: Vec<f32>,
}

/// A run of whole rows of a screen, drawn by one thread.
struct Band<'a> {
    y0: usize,
    w: usize,
    color: &'a mut [[u8; 4]],
    depth: &'a mut [f32],
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

/// The attributes `raster` needs of a surface.
struct Surface<'a> {
    tex: Option<&'a Tex>,
    alpha: u8,
    cull: Cull,
    vertex_color: bool,
    repeat: [bool; 2],
    flip: [bool; 2],
}

impl<'a> Surface<'a> {
    fn of(p: &'a Poly) -> Self {
        Surface {
            tex: p.tex.as_deref(),
            alpha: p.alpha,
            cull: p.cull,
            vertex_color: p.vertex_color,
            repeat: p.repeat,
            flip: p.flip,
        }
    }
}

fn sample(tex: &Rgba, s: &Surface, u: f32, v: f32) -> [u8; 4] {
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
    let x = wrap(u, tex.width, s.repeat[0], s.flip[0]);
    let y = wrap(v, tex.height, s.repeat[1], s.flip[1]);
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

/// Draws one screen triangle of a surface.
fn raster(band: &mut Band, t: &[SV; 3], p: &Surface, translucent_pass: bool) {
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
    let rows = band.color.len() / band.w;
    let min_x = a.x.min(b.x).min(c.x).floor().max(0.0) as usize;
    let max_x = (a.x.max(b.x).max(c.x).ceil().max(0.0) as usize).min(band.w);
    let min_y = (a.y.min(b.y).min(c.y).floor().max(band.y0 as f32) as usize).max(band.y0);
    let max_y = (a.y.max(b.y).max(c.y).ceil().max(0.0) as usize).min(band.y0 + rows);
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
            let i = (y - band.y0) * band.w + x;
            if depth < band.depth[i] {
                continue;
            }
            // Perspective-correct weights.
            let (q0, q1, q2) = (w0 * a.iw, w1 * b.iw, w2 * c.iw);
            let norm = 1.0 / (q0 + q1 + q2);
            let (q0, q1, q2) = (q0 * norm, q1 * norm, q2 * norm);
            let mut rgba = match p.tex {
                Some(tex) => {
                    let u = q0 * a.uv[0] + q1 * b.uv[0] + q2 * c.uv[0];
                    let v = q0 * a.uv[1] + q1 * b.uv[1] + q2 * c.uv[1];
                    sample(&tex.img, p, u, v)
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
                let under = band.color[i];
                let mix = |f: u8, b: u8| (f as f32 * alpha + b as f32 * (1.0 - alpha)) as u8;
                let ua = under[3] as f32 / 255.0;
                band.color[i] = [
                    mix(rgba[0], under[0]),
                    mix(rgba[1], under[1]),
                    mix(rgba[2], under[2]),
                    ((alpha + ua * (1.0 - alpha)) * 255.0) as u8,
                ];
            } else {
                band.color[i] = [rgba[0], rgba[1], rgba[2], 255];
                band.depth[i] = depth;
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
        /// Opaque polygons hanging further than this above the ground
        /// beneath them are not drawn.
        roof_limit: Option<f32>,
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
    fn face(&self, p: &Poly, kind: Billboard, pivot: [f32; 3]) -> [V; 3] {
        let (x, y, z) = self.basis(kind);
        let mut v = p.v;
        for vk in &mut v {
            let d = [vk.p[0] - pivot[0], vk.p[1] - pivot[1], vk.p[2] - pivot[2]];
            for k in 0..3 {
                vk.p[k] = pivot[k] + d[0] * x[k] + d[1] * y[k] + d[2] * z[k];
            }
        }
        v
    }

    /// The axes a billboard takes: its x, y and z in world space.
    fn basis(&self, kind: Billboard) -> ([f32; 3], [f32; 3], [f32; 3]) {
        let back = [-self.forward[0], -self.forward[1], -self.forward[2]];
        match kind {
            Billboard::Full => (self.right, self.up, back),
            Billboard::Y => {
                let z = norm([back[0], 0.0, back[2]]);
                (cross([0.0, 1.0, 0.0], z), [0.0, 1.0, 0.0], z)
            }
        }
    }

    fn view(&self, v: &V) -> ViewVertex {
        let d = [
            v.p[0] - self.eye[0],
            v.p[1] - self.eye[1],
            v.p[2] - self.eye[2],
        ];
        ViewVertex {
            q: [dot(d, self.right), dot(d, self.up), dot(d, self.forward)],
            uv: v.uv,
            c: v.c,
        }
    }

    /// True when the triangle is wholly outside one side of the view.
    fn outside(&self, t: &[ViewVertex; 3]) -> bool {
        let all = |f: &dyn Fn(&ViewVertex) -> bool| t.iter().all(f);
        let (w, h) = (self.cx * 2.0, self.cy * 2.0);
        all(&|v| v.q[2] < self.near)
            || all(&|v| self.focal * v.q[0] + self.cx * v.q[2] < 0.0)
            || all(&|v| self.focal * v.q[0] - (w - self.cx) * v.q[2] > 0.0)
            || all(&|v| self.focal * -v.q[1] + self.cy * v.q[2] < 0.0)
            || all(&|v| self.focal * -v.q[1] - (h - self.cy) * v.q[2] > 0.0)
    }

    /// Clips to the near plane and projects; pushes up to two triangles.
    fn screen(&self, poly: &[ViewVertex; 3], out: &mut Vec<[SV; 3]>) {
        let mut clipped = [poly[0]; 4];
        let mut n = 0;
        for i in 0..3 {
            let (a, b) = (poly[i], poly[(i + 1) % 3]);
            let (ain, bin) = (a.q[2] >= self.near, b.q[2] >= self.near);
            if ain {
                clipped[n] = a;
                n += 1;
            }
            if ain != bin {
                let t = (self.near - a.q[2]) / (b.q[2] - a.q[2]);
                let l = |x: f32, y: f32| x + (y - x) * t;
                clipped[n] = ViewVertex {
                    q: [l(a.q[0], b.q[0]), l(a.q[1], b.q[1]), self.near],
                    uv: [l(a.uv[0], b.uv[0]), l(a.uv[1], b.uv[1])],
                    c: [l(a.c[0], b.c[0]), l(a.c[1], b.c[1]), l(a.c[2], b.c[2])],
                };
                n += 1;
            }
        }
        if n < 3 {
            return;
        }
        let sv = |v: &ViewVertex| {
            let iw = 1.0 / v.q[2];
            SV {
                x: self.cx + self.focal * v.q[0] * iw,
                y: self.cy - self.focal * v.q[1] * iw,
                depth: iw,
                iw,
                uv: v.uv,
                c: v.c,
            }
        };
        let s: Vec<SV> = clipped[..n].iter().map(sv).collect();
        for i in 1..n - 1 {
            out.push([s[0], s[i], s[i + 1]]);
        }
    }
}

impl Projection {
    /// The screen triangles of a polygon (none when it is behind the
    /// camera or off the screen), pushed to `out`.
    fn project(&self, p: &Poly, out: &mut Vec<[SV; 3]>) {
        match self {
            Projection::TopDown { scale, roof_limit } => {
                // A billboard faces the camera: nothing to see from above.
                if p.billboard.is_some() {
                    return;
                }
                if roof_limit.is_some_and(|limit| p.above > limit) {
                    return;
                }
                let sv = |v: &V| SV {
                    x: v.p[0] * scale,
                    y: v.p[2] * scale,
                    depth: v.p[1],
                    iw: 1.0,
                    uv: v.uv,
                    c: v.c,
                };
                out.push([sv(&p.v[0]), sv(&p.v[1]), sv(&p.v[2])]);
            }
            Projection::Camera(cam) => {
                let verts = match p.billboard {
                    Some((kind, pivot)) => cam.face(p, kind, pivot),
                    None => p.v,
                };
                let view = [
                    cam.view(&verts[0]),
                    cam.view(&verts[1]),
                    cam.view(&verts[2]),
                ];
                if cam.outside(&view) {
                    return;
                }
                cam.screen(&view, out);
            }
        }
    }

    /// A person's picture as screen triangles. From above it is an upright
    /// picture on top of everything, its bottom edge on the tile's bottom
    /// edge as the editor draws it; in the camera it stands on the ground
    /// and turns about the vertical axis to face the camera.
    /// A person's round shadow on the ground (camera view only, as the
    /// field draws one under every character).
    fn project_shadow(&self, x: f32, ground: f32, z: f32, out: &mut Vec<[SV; 3]>) {
        let Projection::Camera(cam) = self else {
            return;
        };
        const SEGMENTS: usize = 16;
        let (rx, rz) = (9.0, 5.0);
        let at = |a: f32| {
            cam.view(&V {
                p: [x + rx * a.cos(), ground + 0.4, z + rz * a.sin()],
                uv: [0.0; 2],
                c: [0.0; 3],
            })
        };
        let centre = cam.view(&V {
            p: [x, ground + 0.4, z],
            uv: [0.0; 2],
            c: [0.0; 3],
        });
        for i in 0..SEGMENTS {
            let a0 = i as f32 / SEGMENTS as f32 * std::f32::consts::TAU;
            let a1 = (i + 1) as f32 / SEGMENTS as f32 * std::f32::consts::TAU;
            let t = [centre, at(a0), at(a1)];
            if !cam.outside(&t) {
                cam.screen(&t, out);
            }
        }
    }

    fn project_person(
        &self,
        x: f32,
        ground: f32,
        z: f32,
        tex: &Tex,
        tint: [f32; 3],
        out: &mut Vec<[SV; 3]>,
    ) {
        let half = PERSON_SIZE / 2.0;
        let (tw, th) = (tex.img.width as f32, tex.img.height as f32);
        match self {
            Projection::TopDown { scale, .. } => {
                let bottom = z + TILE / 2.0;
                let corner = |dx: f32, dy: f32, u: f32, v: f32| SV {
                    x: (x + dx) * scale,
                    y: (bottom - dy) * scale,
                    depth: 1.0e6,
                    iw: 1.0,
                    uv: [u, v],
                    c: tint,
                };
                let a = corner(-half, PERSON_SIZE, 0.0, 0.0);
                let b = corner(half, PERSON_SIZE, tw, 0.0);
                let c = corner(half, 0.0, tw, th);
                let d = corner(-half, 0.0, 0.0, th);
                out.push([a, b, c]);
                out.push([a, c, d]);
            }
            Projection::Camera(cam) => {
                let (bx, by, _) = cam.basis(Billboard::Y);
                let pivot = [x, ground, z];
                let corner = |dx: f32, dy: f32, u: f32, v: f32| {
                    let mut p = pivot;
                    for k in 0..3 {
                        p[k] += dx * bx[k] + dy * by[k];
                    }
                    cam.view(&V {
                        p,
                        uv: [u, v],
                        c: tint,
                    })
                };
                let a = corner(-half, PERSON_SIZE, 0.0, 0.0);
                let b = corner(half, PERSON_SIZE, tw, 0.0);
                let c = corner(half, 0.0, tw, th);
                let d = corner(-half, 0.0, 0.0, th);
                for t in [[a, b, c], [a, c, d]] {
                    if !cam.outside(&t) {
                        cam.screen(&t, out);
                    }
                }
            }
        }
    }
}

/// Samples below which one thread draws the whole screen.
const THREAD_MIN_SAMPLES: usize = 60_000;

fn draw(screen: &mut Screen, scene: &Scene, people: &[Person], proj: &Projection) {
    let mut tris: Vec<[SV; 3]> = Vec::with_capacity(4);
    // People take the time of day's light: full at noon, dimmer and bluer
    // at night (the sum of the field lights against plain daylight).
    let tint = scene.lights.as_ref().map_or([1.0; 3], |l| {
        let day = Lights::fixed();
        let sum = |x: &Lights, k: usize| x.color.iter().map(|c| c[k]).sum::<f32>();
        [0, 1, 2].map(|k| (sum(l, k) / sum(&day, k).max(0.01)).clamp(0.35, 1.0))
    });
    let spots: Vec<(f32, f32, f32)> = people
        .iter()
        .map(|p| {
            let (x, z) = ((p.x as f32 + 0.5) * TILE, (p.z as f32 + 0.5) * TILE);
            (x, scene.ground.height(&scene.polys, x, z).unwrap_or(0.0), z)
        })
        .collect();
    // Project everything once: the triangles of each pass (opaque, then
    // translucent) with the surface each one is drawn with.
    let mut surfaces: Vec<Surface> = Vec::new();
    let mut passes: [Vec<(u32, [SV; 3])>; 2] = [Vec::new(), Vec::new()];
    for (pass, jobs) in passes.iter_mut().enumerate() {
        let translucent = pass == 1;
        for p in scene.polys.iter().filter(|p| p.translucent == translucent) {
            tris.clear();
            proj.project(p, &mut tris);
            if tris.is_empty() {
                continue;
            }
            surfaces.push(Surface::of(p));
            jobs.extend(tris.iter().map(|t| (surfaces.len() as u32 - 1, *t)));
        }
        // Shadows are see-through: drawn with the translucent pass, under
        // the people that follow.
        if translucent && !people.is_empty() {
            tris.clear();
            for &(x, y, z) in &spots {
                proj.project_shadow(x, y, z, &mut tris);
            }
            surfaces.push(Surface {
                tex: None,
                alpha: 15,
                cull: Cull::Both,
                vertex_color: true,
                repeat: [false; 2],
                flip: [false; 2],
            });
            jobs.extend(tris.iter().map(|t| (surfaces.len() as u32 - 1, *t)));
        }
        for (person, &(x, y, z)) in people.iter().zip(&spots) {
            if person.tex.partial != translucent {
                continue;
            }
            tris.clear();
            proj.project_person(x, y, z, &person.tex, tint, &mut tris);
            surfaces.push(Surface {
                tex: Some(&person.tex),
                alpha: 31,
                cull: Cull::Both,
                vertex_color: true,
                repeat: [false; 2],
                flip: [false; 2],
            });
            jobs.extend(tris.iter().map(|t| (surfaces.len() as u32 - 1, *t)));
        }
    }
    let threads = if screen.w * screen.h < THREAD_MIN_SAMPLES {
        1
    } else {
        std::thread::available_parallelism()
            .map_or(1, |n| n.get())
            .clamp(1, 8)
    };
    let rows = screen.h.div_ceil(threads).max(1);
    let w = screen.w;
    let bands = screen
        .color
        .chunks_mut(rows * w)
        .zip(screen.depth.chunks_mut(rows * w))
        .enumerate()
        .map(|(i, (color, depth))| Band {
            y0: i * rows,
            w,
            color,
            depth,
        });
    let run = |mut band: Band| {
        for (pass, jobs) in passes.iter().enumerate() {
            for (s, t) in jobs {
                raster(&mut band, t, &surfaces[*s as usize], pass == 1);
            }
        }
    };
    if threads == 1 {
        bands.for_each(run);
    } else {
        std::thread::scope(|sc| {
            for band in bands {
                sc.spawn(|| run(band));
            }
        });
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

/// PNG bytes, quickly: a preview is read once, so it trades size for
/// speed. An image with no transparency is written as RGB.
pub fn encode_png_fast(img: &Rgba) -> Vec<u8> {
    let opaque = img.pixels.chunks(4).all(|p| p[3] == 255);
    let mut out = Vec::new();
    {
        let mut encoder =
            png::Encoder::new(&mut out, img.width.max(1) as u32, img.height.max(1) as u32);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_compression(png::Compression::Fast);
        encoder.set_filter(png::Filter::Sub);
        let data: Vec<u8> = if opaque {
            encoder.set_color(png::ColorType::Rgb);
            img.pixels
                .chunks(4)
                .flat_map(|p| [p[0], p[1], p[2]])
                .collect()
        } else {
            encoder.set_color(png::ColorType::Rgba);
            img.pixels.clone()
        };
        let mut writer = encoder.write_header().expect("PNG header to memory");
        let empty = img.width == 0 || img.height == 0;
        writer
            .write_image_data(if empty { &[0; 4] } else { &data })
            .expect("PNG data to memory");
    }
    out
}

fn scene_key(opts: &Options) -> SceneKey {
    SceneKey {
        buildings: opts.buildings,
        season: opts.season % 4,
        minutes: minutes(opts.hour),
        light: opts.light,
    }
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
    let scene = scene(bridge, zone, &scene_key(opts))?;
    let ss = choose_supersample(
        opts.supersample,
        layout.image_width,
        layout.image_height,
        layout.px,
    );
    let roof_limit = match opts.roofs {
        Roofs::All => None,
        Roofs::Cut(h) => Some(h),
        Roofs::Auto => scene.tall_roofs.then_some(AUTO_CUT_TILES * TILE),
    };
    let mut screen = Screen::new(layout.image_width * ss, layout.image_height * ss);
    let proj = Projection::TopDown {
        scale: (layout.px * ss) as f32 / TILE,
        roof_limit,
    };
    let crowd = if opts.people {
        people(bridge, zone)
    } else {
        Vec::new()
    };
    draw(&mut screen, &scene, &crowd, &proj);
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
    pub people: bool,
    pub season: u8,
    pub hour: f32,
    pub light: bool,
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
            people: true,
            season: 0,
            hour: DAY_HOUR,
            light: true,
        }
    }
}

const CAMERA_SETTINGS: u16 = 60;
const CAMERA_RECORD: usize = 0x2C;

/// Draws what the game's field camera shows around a tile: a perspective
/// view tilted by the zone's camera setting (a/0/6/0, read by
/// `FIELD_CAMERA_ReadSetting`: distance, pitch, yaw, half field of view,
/// near plane, and a target shift or a fixed target).
pub fn render_camera(bridge: &Bridge, zone: usize, opts: &CameraOptions) -> Result<Rgba, Error> {
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
    let scene = scene(
        bridge,
        zone,
        &SceneKey {
            buildings: opts.buildings,
            season: opts.season % 4,
            minutes: minutes(opts.hour),
            light: opts.light,
        },
    )?;
    let (wx, wz) = ((opts.x + 0.5) * TILE, (opts.z + 0.5) * TILE);
    let target = if s32(0x1C) != 0 {
        let ground = scene.ground.height(&scene.polys, wx, wz).unwrap_or(0.0);
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
    let crowd = if opts.people {
        people(bridge, zone)
    } else {
        Vec::new()
    };
    let mut screen = Screen::new(w * ss, h * ss);
    draw(&mut screen, &scene, &crowd, &Projection::Camera(cam));
    let mut image = finish(&screen, ss);
    // The sky: what the game clears the screen to.
    if let Some(l) = &scene.lights {
        for px in image.pixels.chunks_mut(4) {
            let a = px[3] as u32;
            for (k, c) in px.iter_mut().take(3).enumerate() {
                *c = ((*c as u32 * a + l.clear[k] as u32 * (255 - a)) / 255) as u8;
            }
            px[3] = 255;
        }
    }
    Ok(image)
}

/// A building or prop model of a zone's area, posed for a thumbnail.
struct PropScene {
    polys: Vec<Poly>,
    lights: Lights,
    /// World bounds: min and max per axis.
    lo: [f32; 3],
    hi: [f32; 3],
}

/// The model `model` (an id from `props::prop_models`) of the zone's area,
/// as untransformed world polygons lit like day.
fn prop_scene(bridge: &Bridge, zone: usize, model: u16) -> Result<PropScene, Error> {
    let headers = bridge.member(Member {
        arc: ZONE_HEADERS,
        index: 0,
    })?;
    let area =
        rd16(&headers, zone * 48 + 2).ok_or_else(|| Error::Missing(format!("zone {zone}")))?;
    let area_file = bridge
        .source()
        .read("a/0/1/3")
        .map_err(|e| Error::Missing(format!("a/0/1/3: {e}")))?;
    let rec = area_file
        .get(area as usize * 10..area as usize * 10 + 10)
        .ok_or_else(|| Error::Missing(format!("area {area}")))?;
    let mut buildings = load_buildings(bridge, area, rec[6] != 0)
        .ok_or_else(|| Error::Missing(format!("models of zone {zone}")))?;
    let m = buildings
        .models
        .get(&model)
        .cloned()
        .ok_or_else(|| Error::Missing(format!("model {model} in zone {zone}")))?;
    let lights = load_lights(bridge, rec[7], 0, DAY_HOUR);
    let mut polys = Vec::new();
    add_model(
        &mut polys,
        &m,
        &mut buildings.textures,
        &Affine::IDENTITY,
        Some(&lights),
        false,
    );
    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    // Solid polygons give the bounds; a model of billboards only uses those.
    for solid in [true, false] {
        for p in polys.iter().filter(|p| p.billboard.is_none() == solid) {
            for v in &p.v {
                for k in 0..3 {
                    lo[k] = lo[k].min(v.p[k]);
                    hi[k] = hi[k].max(v.p[k]);
                }
            }
        }
        if lo[0] <= hi[0] {
            break;
        }
    }
    if lo[0] > hi[0] {
        // Nothing to draw: an empty model.
        (lo, hi) = ([0.0; 3], [0.0; 3]);
    }
    Ok(PropScene {
        polys,
        lights,
        lo,
        hi,
    })
}

/// How big a prop model stands on the map.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PropInfo {
    pub zone: usize,
    pub model: u16,
    /// East-west size, north-south size and height, in tiles.
    pub width: f32,
    pub depth: f32,
    pub height: f32,
    /// The same bounds in world units (16 per tile), per axis.
    pub min: [f32; 3],
    pub max: [f32; 3],
}

/// The footprint and height of a zone's prop model.
pub fn prop_info(bridge: &Bridge, zone: usize, model: u16) -> Result<PropInfo, Error> {
    let s = prop_scene(bridge, zone, model)?;
    Ok(PropInfo {
        zone,
        model,
        width: (s.hi[0] - s.lo[0]) / TILE,
        depth: (s.hi[2] - s.lo[2]) / TILE,
        height: (s.hi[1] - s.lo[1]) / TILE,
        min: s.lo,
        max: s.hi,
    })
}

/// A square thumbnail of a zone's prop model from the front and a bit
/// above, lit like day, fitted with a margin, background transparent.
pub fn render_prop(bridge: &Bridge, zone: usize, model: u16, px: usize) -> Result<Rgba, Error> {
    let s = prop_scene(bridge, zone, model)?;
    let px = px.clamp(8, 1024);
    if s.polys.is_empty() {
        return Ok(Rgba::new(px, px));
    }
    let ss = if px <= 256 { 2 } else { 1 };
    let size = px * ss;
    let center = [
        (s.lo[0] + s.hi[0]) / 2.0,
        (s.lo[1] + s.hi[1]) / 2.0,
        (s.lo[2] + s.hi[2]) / 2.0,
    ];
    let radius = (0..3)
        .map(|k| (s.hi[k] - s.lo[k]).powi(2))
        .sum::<f32>()
        .sqrt()
        .max(1.0)
        / 2.0;
    // The game looks from the south and above.
    let pitch = 32f32.to_radians();
    let dir = [0.0, pitch.sin(), pitch.cos()];
    let dist = radius * 6.0;
    let eye = [
        center[0] + dir[0] * dist,
        center[1] + dir[1] * dist,
        center[2] + dir[2] * dist,
    ];
    let forward = norm([center[0] - eye[0], center[1] - eye[1], center[2] - eye[2]]);
    let right = norm(cross(forward, [0.0, 1.0, 0.0]));
    let up = cross(right, forward);
    let mut cam = Camera {
        eye,
        right,
        up,
        forward,
        focal: 1.0,
        cx: 0.0,
        cy: 0.0,
        near: 1.0,
    };
    // Fit: the bounds of the points on the image plane at focal 1.
    let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]);
    for p in &s.polys {
        for v in &p.v {
            let q = cam.view(v).q;
            let (x, y) = (q[0] / q[2], -q[1] / q[2]);
            lo = [lo[0].min(x), lo[1].min(y)];
            hi = [hi[0].max(x), hi[1].max(y)];
        }
    }
    let span = (hi[0] - lo[0]).max(hi[1] - lo[1]).max(1e-6);
    let margin = 0.92;
    cam.focal = size as f32 * margin / span;
    cam.cx = size as f32 / 2.0 - cam.focal * (lo[0] + hi[0]) / 2.0;
    cam.cy = size as f32 / 2.0 - cam.focal * (lo[1] + hi[1]) / 2.0;
    let scene = Scene {
        polys: s.polys,
        ground: GroundIndex::new(&[], 1, 1),
        tall_roofs: false,
        lights: Some(s.lights),
    };
    let mut screen = Screen::new(size, size);
    draw(&mut screen, &scene, &[], &Projection::Camera(cam));
    Ok(finish(&screen, ss))
}

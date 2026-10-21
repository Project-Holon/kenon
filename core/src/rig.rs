//! Battle sprite rigs: the multi-cell resources (NMCR) that pose and
//! animate the large battle pictures of Pokemon and trainers.
//!
//! The game's composer (aletheia `tools/assets/rig.py`, from
//! func_020643CC, func_0206419C; eidos `g2d_nmcr_parse`):
//!
//! ```text
//! NMCR (on disk "RCMN"), block "KBCM": u16 group count, u16 0xBEEF,
//!   u32 group array offset, u32 node base (body-relative);
//!   group, 8 bytes: u16 node count, u16 cell-animation count,
//!   u32 offset of its nodes from the node base;
//!   node, 8 bytes: u16 animation sequence, s16 x, s16 y, u8 attribute,
//!   u8 cell-animation slot
//! ```
//!
//! Each node plays its NANR sequence; the frame active at time t names a
//! cell of the NCER, drawn at the node's position plus the frame's
//! translation, rotated and scaled about that point when the frame says
//! so. Nodes and their OBJs are emitted in order and an earlier one draws
//! in front (priority first).

use crate::image::{cell_map, CellMap, Rgba};
use crate::nanr::Nanr;
use crate::ncer::Ncer;
use crate::ncgr::Ncgr;
use crate::nclr::Palette;
use crate::nitro::{self, read_u16, read_u32, Error};

pub const MAGIC: &[u8; 4] = b"RCMN";
const MCBK: &[u8; 4] = b"KBCM";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Node {
    pub sequence: u16,
    pub x: i16,
    pub y: i16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Nmcr {
    /// Groups of nodes: alternate assemblies of the same sprite.
    pub groups: Vec<Vec<Node>>,
}

impl Nmcr {
    pub fn parse(bytes: &[u8]) -> Result<Self, Error> {
        let body = nitro::block(bytes, MAGIC, MCBK)?;
        let count = read_u16(body, 0)? as usize;
        let groups_at = read_u32(body, 4)? as usize;
        let nodes_at = read_u32(body, 8)? as usize;
        let groups = (0..count)
            .map(|g| {
                let d = groups_at + g * 8;
                let n = read_u16(body, d)? as usize;
                let first = nodes_at + read_u32(body, d + 4)? as usize;
                // A node run past the body is cut short, as the tools do.
                Ok((0..n)
                    .map_while(|j| {
                        let p = first + j * 8;
                        Some(Node {
                            sequence: read_u16(body, p).ok()?,
                            x: read_u16(body, p + 2).ok()? as i16,
                            y: read_u16(body, p + 4).ok()? as i16,
                        })
                    })
                    .collect())
            })
            .collect::<Result<_, Error>>()?;
        Ok(Nmcr { groups })
    }
}

/// Where a pixel of a pose comes from: `tile * 64 + pixel` of the sheet,
/// and the palette bank of its OBJ.
pub type Source = Option<(usize, u8)>;

/// One placed piece of a pose: a cell's pixel map, relative to the rig
/// origin.
struct Piece {
    /// Draw order key: the cell's front-most OBJ priority (lower is in
    /// front), then emission order (earlier is in front).
    priority: u8,
    order: usize,
    x: i32,
    y: i32,
    w: usize,
    h: usize,
    map: Vec<Source>,
}

/// The pieces at time `t` (60ths of a second), back to front.
fn pose(sheet: &Ncgr, cells: &Ncer, anims: &Nanr, nodes: &[Node], t: u32) -> Vec<Piece> {
    let mut pieces = Vec::new();
    for node in nodes {
        let Some(seq) = anims.sequences.get(node.sequence as usize) else {
            continue;
        };
        let Some(frame) = seq.frames.get(seq.frame_at(t)) else {
            continue;
        };
        let Some(cell) = cells.cells.get(frame.cell as usize) else {
            continue;
        };
        let m = cell_map(cells, cell, sheet);
        let (ax, ay) = (
            node.x as i32 + frame.offset.0 as i32,
            node.y as i32 + frame.offset.1 as i32,
        );
        let identity = frame.rotation == 0 && frame.scale == (4096, 4096);
        let priority = cell.objs.iter().map(|o| o.priority).min().unwrap_or(0);
        let order = pieces.len();
        pieces.push(if identity {
            Piece {
                priority,
                order,
                x: ax - m.origin.0,
                y: ay - m.origin.1,
                w: m.width,
                h: m.height,
                map: m.pixels,
            }
        } else {
            Piece {
                priority,
                order,
                ..transform(&m, frame.rotation, frame.scale, ax, ay)
            }
        });
    }
    // Painted back to front: priority outranks order, and an earlier node
    // draws in front (rig.py's composer, func_020643CC's emission order).
    pieces.sort_by_key(|p| std::cmp::Reverse((p.priority, p.order)));
    pieces
}

/// A cell's pixel map rotated and scaled about its origin, placed at
/// (ax, ay); each target pixel takes the source pixel under its centre.
fn transform(m: &CellMap, rot: u16, scale: (i32, i32), ax: i32, ay: i32) -> Piece {
    let angle = rot as f64 / 65536.0 * std::f64::consts::TAU;
    let (sx, sy) = (scale.0 as f64 / 4096.0, scale.1 as f64 / 4096.0);
    let (c, s) = (angle.cos(), angle.sin());
    let (ox, oy) = (m.origin.0 as f64, m.origin.1 as f64);
    // Forward: p' = R * S * (p - origin).
    let fwd = |x: f64, y: f64| (c * sx * x - s * sy * y, s * sx * x + c * sy * y);
    let (wf, hf) = (m.width as f64, m.height as f64);
    let corners = [(0.0, 0.0), (wf, 0.0), (0.0, hf), (wf, hf)].map(|(x, y)| fwd(x - ox, y - oy));
    let x0 = corners.iter().map(|p| p.0).fold(f64::MAX, f64::min).floor() as i32;
    let y0 = corners.iter().map(|p| p.1).fold(f64::MAX, f64::min).floor() as i32;
    let x1 = corners.iter().map(|p| p.0).fold(f64::MIN, f64::max).ceil() as i32;
    let y1 = corners.iter().map(|p| p.1).fold(f64::MIN, f64::max).ceil() as i32;
    let (w, h) = ((x1 - x0).max(1) as usize, (y1 - y0).max(1) as usize);
    let mut map = vec![None; w * h];
    if (sx * sy).abs() > 1e-6 {
        for py in 0..h {
            for px in 0..w {
                let (qx, qy) = ((x0 + px as i32) as f64 + 0.5, (y0 + py as i32) as f64 + 0.5);
                let (rx, ry) = (c * qx + s * qy, -s * qx + c * qy);
                let (ux, uy) = (rx / sx + ox, ry / sy + oy);
                if ux >= 0.0 && uy >= 0.0 && ux < wf && uy < hf {
                    map[py * w + px] = m.pixels[uy as usize * m.width + ux as usize];
                }
            }
        }
    }
    Piece {
        priority: 0,
        order: 0,
        x: ax + x0,
        y: ay + y0,
        w,
        h,
        map,
    }
}

/// An animated rig on one canvas: each frame's picture and pixel map, and
/// how long each frame shows.
pub struct Animation {
    pub width: usize,
    pub height: usize,
    pub frames: Vec<Rgba>,
    /// Per frame, where each canvas pixel comes from in the sheet.
    pub maps: Vec<Vec<Source>>,
    /// Durations in 60ths of a second.
    pub durations: Vec<u32>,
}

/// Plays the first moving group of a rig over one loop of its longest node sequence
/// (capped at 10 seconds), keeping only the moments something changes, all
/// on one canvas so the frames line up.
pub fn animate(
    sheet: &Ncgr,
    palette: &Palette,
    cells: &Ncer,
    anims: &Nanr,
    rig: &Nmcr,
) -> Option<Animation> {
    // The first group that moves (trainers keep a still pose in group 0).
    let moves = |nodes: &&Vec<Node>| {
        nodes.iter().any(|n| {
            anims
                .sequences
                .get(n.sequence as usize)
                .is_some_and(|s| s.frames.len() > 1)
        })
    };
    let nodes = rig.groups.iter().find(moves).or(rig.groups.first())?;
    let total = |seq: &crate::nanr::Sequence| -> u32 {
        seq.frames.iter().map(|f| (f.duration as u32).max(1)).sum()
    };
    let period = nodes
        .iter()
        .filter_map(|n| anims.sequences.get(n.sequence as usize))
        .map(total)
        .max()
        .unwrap_or(1)
        .clamp(1, 600);
    // The times a node's frame changes within one period.
    let mut times = vec![0u32];
    for n in nodes {
        let Some(seq) = anims.sequences.get(n.sequence as usize) else {
            continue;
        };
        if seq.frames.len() < 2 {
            continue;
        }
        let mut t = 0;
        while t < period {
            for f in &seq.frames {
                t += (f.duration as u32).max(1);
                if t < period {
                    times.push(t);
                }
            }
        }
    }
    times.sort_unstable();
    times.dedup();
    times.truncate(240);

    let poses: Vec<Vec<Piece>> = times
        .iter()
        .map(|&t| pose(sheet, cells, anims, nodes, t))
        .collect();
    let durations = times
        .iter()
        .enumerate()
        .map(|(i, &t)| {
            times
                .get(i + 1)
                .copied()
                .unwrap_or(period)
                .saturating_sub(t)
                .max(1)
        })
        .collect();
    finish(sheet, palette, poses, durations)
}

/// Plays a multi-cell animation (an NMAR, like `nanr` but each frame names
/// a rig group): sequence `seq`, each frame showing its group with the
/// group's own node animations running from the frame's start. Back
/// sprites throw this way, one group per pose.
pub fn animate_multi(
    sheet: &Ncgr,
    palette: &Palette,
    cells: &Ncer,
    anims: &Nanr,
    rig: &Nmcr,
    multi: &Nanr,
    seq: usize,
) -> Option<Animation> {
    let seq = multi.sequences.get(seq)?;
    let mut samples: Vec<(usize, u32)> = Vec::new();
    let mut durations = Vec::new();
    for f in &seq.frames {
        let group = f.cell as usize;
        let nodes = rig.groups.get(group)?;
        let length = (f.duration as u32).max(1);
        // The moments the group's nodes change within this frame.
        let mut times = vec![0u32];
        for n in nodes {
            let Some(s) = anims.sequences.get(n.sequence as usize) else {
                continue;
            };
            let mut t = 0;
            'walk: while s.frames.len() > 1 {
                for fr in &s.frames {
                    t += (fr.duration as u32).max(1);
                    if t >= length {
                        break 'walk;
                    }
                    times.push(t);
                }
            }
        }
        times.sort_unstable();
        times.dedup();
        for (i, &t) in times.iter().enumerate() {
            samples.push((group, t));
            durations.push(times.get(i + 1).copied().unwrap_or(length) - t);
        }
        if samples.len() >= 240 {
            break;
        }
    }
    let poses = samples
        .iter()
        .map(|&(g, t)| pose(sheet, cells, anims, &rig.groups[g], t))
        .collect();
    finish(sheet, palette, poses, durations)
}

/// Lays poses out on one canvas and draws them.
fn finish(
    sheet: &Ncgr,
    palette: &Palette,
    poses: Vec<Vec<Piece>>,
    durations: Vec<u32>,
) -> Option<Animation> {
    let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
    for p in poses.iter().flatten() {
        x0 = x0.min(p.x);
        y0 = y0.min(p.y);
        x1 = x1.max(p.x + p.w as i32);
        y1 = y1.max(p.y + p.h as i32);
    }
    if x0 > x1 {
        return None;
    }
    let (w, h) = ((x1 - x0) as usize, (y1 - y0) as usize);
    let tiles: Vec<[u8; 64]> = (0..sheet.tile_count())
        .map(|t| sheet.tile(t, false, false))
        .collect();
    let value = |src: usize| tiles.get(src / 64).map_or(0, |t| t[src % 64]);
    let maps: Vec<Vec<Source>> = poses
        .iter()
        .map(|pieces| {
            // A drawn pixel wins over what is behind it; a clear pixel of a
            // piece still maps there when nothing is drawn, so painting can
            // fill it.
            let mut canvas: Vec<Source> = vec![None; w * h];
            for p in pieces {
                for yy in 0..p.h {
                    for xx in 0..p.w {
                        let Some(src) = p.map[yy * p.w + xx] else {
                            continue;
                        };
                        let i = ((p.y - y0) as usize + yy) * w + (p.x - x0) as usize + xx;
                        if value(src.0) != 0 || canvas[i].is_none() {
                            canvas[i] = Some(src);
                        }
                    }
                }
            }
            canvas
        })
        .collect();
    let frames = maps
        .iter()
        .map(|map| {
            let mut img = Rgba::new(w, h);
            for (i, src) in map.iter().enumerate() {
                let Some((s, bank)) = *src else { continue };
                let v = value(s) as usize;
                if v == 0 {
                    continue;
                }
                let base = match sheet.depth {
                    crate::ncgr::Depth::Bpp4 => bank as usize * 16,
                    crate::ncgr::Depth::Bpp8 => 0,
                };
                img.pixels[i * 4..i * 4 + 4].copy_from_slice(&palette.rgba(base + v));
            }
            img
        })
        .collect();
    Some(Animation {
        width: w,
        height: h,
        frames,
        maps,
        durations,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_groups_and_nodes() {
        let mut body = Vec::new();
        body.extend_from_slice(&1u16.to_le_bytes());
        body.extend_from_slice(&0xBEEFu16.to_le_bytes());
        body.extend_from_slice(&16u32.to_le_bytes()); // groups
        body.extend_from_slice(&24u32.to_le_bytes()); // node base
        body.extend_from_slice(&0u32.to_le_bytes());
        // Group: 2 nodes at offset 0.
        body.extend_from_slice(&2u16.to_le_bytes());
        body.extend_from_slice(&2u16.to_le_bytes());
        body.extend_from_slice(&0u32.to_le_bytes());
        for (seq, x, y) in [(0u16, -8i16, 4i16), (1, 10, -2)] {
            body.extend_from_slice(&seq.to_le_bytes());
            body.extend_from_slice(&x.to_le_bytes());
            body.extend_from_slice(&y.to_le_bytes());
            body.extend_from_slice(&[0, 0]);
        }
        let file = nitro::build(MAGIC, &[(MCBK, &body)]);
        let rig = Nmcr::parse(&file).unwrap();
        assert_eq!(rig.groups.len(), 1);
        assert_eq!(
            rig.groups[0][1],
            Node {
                sequence: 1,
                x: 10,
                y: -2
            }
        );
    }
}

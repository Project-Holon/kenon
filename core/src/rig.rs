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

use crate::image::{render_cell, Rgba};
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

/// One drawn piece of a pose: a cell image and where its origin goes.
struct Piece {
    image: Rgba,
    /// Canvas position of the image's top-left corner, relative to the rig
    /// origin.
    x: i32,
    y: i32,
}

/// The pieces of group `group` at time `t` (60ths of a second), back to
/// front.
fn pose(
    sheet: &Ncgr,
    palette: &Palette,
    cells: &Ncer,
    anims: &Nanr,
    nodes: &[Node],
    t: u32,
) -> Vec<Piece> {
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
        let drawn = render_cell(cells, cell, sheet, palette, 0);
        let (ax, ay) = (
            node.x as i32 + frame.offset.0 as i32,
            node.y as i32 + frame.offset.1 as i32,
        );
        let identity = frame.rotation == 0 && frame.scale == (4096, 4096);
        let piece = if identity {
            Piece {
                x: ax - drawn.origin.0,
                y: ay - drawn.origin.1,
                image: drawn.image,
            }
        } else {
            transform(
                &drawn.image,
                drawn.origin,
                frame.rotation,
                frame.scale,
                ax,
                ay,
            )
        };
        pieces.push(piece);
    }
    // Earlier nodes draw in front: paint them last.
    pieces.reverse();
    pieces
}

/// A cell image rotated and scaled about its origin, placed at (ax, ay).
fn transform(
    img: &Rgba,
    origin: (i32, i32),
    rot: u16,
    scale: (i32, i32),
    ax: i32,
    ay: i32,
) -> Piece {
    let angle = rot as f64 / 65536.0 * std::f64::consts::TAU;
    let (sx, sy) = (scale.0 as f64 / 4096.0, scale.1 as f64 / 4096.0);
    let (c, s) = (angle.cos(), angle.sin());
    // Forward: p' = R * S * (p - origin).
    let fwd = |x: f64, y: f64| (c * sx * x - s * sy * y, s * sx * x + c * sy * y);
    let corners = [
        (0.0, 0.0),
        (img.width as f64, 0.0),
        (0.0, img.height as f64),
        (img.width as f64, img.height as f64),
    ]
    .map(|(x, y)| fwd(x - origin.0 as f64, y - origin.1 as f64));
    let x0 = corners.iter().map(|p| p.0).fold(f64::MAX, f64::min).floor() as i32;
    let y0 = corners.iter().map(|p| p.1).fold(f64::MAX, f64::min).floor() as i32;
    let x1 = corners.iter().map(|p| p.0).fold(f64::MIN, f64::max).ceil() as i32;
    let y1 = corners.iter().map(|p| p.1).fold(f64::MIN, f64::max).ceil() as i32;
    let (w, h) = ((x1 - x0).max(1) as usize, (y1 - y0).max(1) as usize);
    let mut out = Rgba::new(w, h);
    let det = sx * sy;
    if det.abs() > 1e-6 {
        for py in 0..h {
            for px in 0..w {
                // Inverse: p = S^-1 * R^-1 * p' + origin, sampled at centres.
                let (qx, qy) = ((x0 + px as i32) as f64 + 0.5, (y0 + py as i32) as f64 + 0.5);
                let (rx, ry) = (c * qx + s * qy, -s * qx + c * qy);
                let (ux, uy) = (rx / sx + origin.0 as f64, ry / sy + origin.1 as f64);
                if ux < 0.0 || uy < 0.0 || ux >= img.width as f64 || uy >= img.height as f64 {
                    continue;
                }
                let px_src = img.get(ux as usize, uy as usize);
                if px_src[3] != 0 {
                    let i = (py * w + px) * 4;
                    out.pixels[i..i + 4].copy_from_slice(&px_src);
                }
            }
        }
    }
    Piece {
        image: out,
        x: ax + x0,
        y: ay + y0,
    }
}

/// An animated rig: its frames on one canvas and how long each shows.
pub struct Animation {
    pub frames: Vec<Rgba>,
    /// Durations in 60ths of a second.
    pub durations: Vec<u32>,
}

/// Plays group 0 of a rig over one loop of its longest node sequence
/// (capped at 10 seconds), keeping only the moments something changes, all
/// on one canvas so the frames line up.
pub fn animate(
    sheet: &Ncgr,
    palette: &Palette,
    cells: &Ncer,
    anims: &Nanr,
    rig: &Nmcr,
) -> Option<Animation> {
    let nodes = rig.groups.first()?;
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
            if total(seq) == 0 {
                break;
            }
        }
    }
    times.sort_unstable();
    times.dedup();
    times.truncate(240);

    let poses: Vec<Vec<Piece>> = times
        .iter()
        .map(|&t| pose(sheet, palette, cells, anims, nodes, t))
        .collect();
    let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
    for p in poses.iter().flatten() {
        x0 = x0.min(p.x);
        y0 = y0.min(p.y);
        x1 = x1.max(p.x + p.image.width as i32);
        y1 = y1.max(p.y + p.image.height as i32);
    }
    if x0 > x1 {
        return None;
    }
    let (w, h) = ((x1 - x0) as usize, (y1 - y0) as usize);
    let frames = poses
        .iter()
        .map(|pieces| {
            let mut canvas = Rgba::new(w, h);
            for p in pieces {
                for yy in 0..p.image.height {
                    for xx in 0..p.image.width {
                        let c = p.image.get(xx, yy);
                        if c[3] == 0 {
                            continue;
                        }
                        let (cx, cy) = ((p.x - x0) as usize + xx, (p.y - y0) as usize + yy);
                        let i = (cy * w + cx) * 4;
                        canvas.pixels[i..i + 4].copy_from_slice(&c);
                    }
                }
            }
            canvas
        })
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
    Some(Animation { frames, durations })
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

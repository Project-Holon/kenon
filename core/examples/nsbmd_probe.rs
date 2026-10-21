//! Prints model bounds of map chunks: `nsbmd_probe <rom> <first> <count>`.

use kenon_core::bridge::{Bridge, Member};
use kenon_core::nsbmd;
use kenon_core::rom::Rom;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let bridge = Bridge::new(Rom::open(a[1].as_ref()).unwrap());
    let first: usize = a[2].parse().unwrap();
    let count: usize = a[3].parse().unwrap();
    let verbose = std::env::var_os("VERBOSE").is_some();
    for i in first..first + count {
        let bytes = bridge.member(Member { arc: 8, index: i }).unwrap();
        let off = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
        let Some(m) = nsbmd::parse(&bytes[off..]) else {
            println!("{i}: no model");
            continue;
        };
        let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
        for d in &m.draws {
            for t in &m.shapes[d.shape] {
                for v in t {
                    let p = m.nodes[d.node].apply(v.pos.map(|c| c * m.pos_scale));
                    for k in 0..3 {
                        lo[k] = lo[k].min(p[k]);
                        hi[k] = hi[k].max(p[k]);
                    }
                }
            }
        }
        if verbose {
            for d in &m.draws {
                let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
                for t in &m.shapes[d.shape] {
                    for v in t {
                        let p = m.nodes[d.node].apply(v.pos.map(|c| c * m.pos_scale));
                        for k in 0..3 {
                            lo[k] = lo[k].min(p[k]);
                            hi[k] = hi[k].max(p[k]);
                        }
                    }
                }
                println!(
                    "   shape {} mat {} ({:?}) node {} tris {} lo {:?} hi {:?} node {:?}",
                    d.shape,
                    d.material,
                    m.materials[d.material].texture,
                    d.node,
                    m.shapes[d.shape].len(),
                    lo,
                    hi,
                    m.nodes[d.node]
                );
            }
        }
        if std::env::var_os("BIG").is_some() {
            let mut worst: Vec<(f32, usize, usize)> = Vec::new();
            for d in &m.draws {
                for (ti, t) in m.shapes[d.shape].iter().enumerate() {
                    let q: Vec<[f32; 3]> = t
                        .iter()
                        .map(|v| m.nodes[d.node].apply(v.pos.map(|c| c * m.pos_scale)))
                        .collect();
                    let a = ((q[1][0] - q[0][0]) * (q[2][2] - q[0][2])
                        - (q[1][2] - q[0][2]) * (q[2][0] - q[0][0]))
                        .abs();
                    worst.push((a, d.shape, ti));
                }
            }
            worst.sort_by(|a, b| b.0.total_cmp(&a.0));
            for (a, sh, ti) in worst.iter().take(6) {
                let t = &m.shapes[*sh][*ti];
                println!(
                    "   big tri area {a} shape {sh} #{ti}: {:?} {:?} {:?}",
                    t[0].pos, t[1].pos, t[2].pos
                );
            }
        }
        let ops = m.matrix_ops.iter().fold(0, |a, b| a | b);
        println!(
            "{i}: ops {ops:#x} {} scale {} nodes {} mats {} shapes {} draws {} lo {:?} hi {:?}",
            m.name,
            m.pos_scale,
            m.nodes.len(),
            m.materials.len(),
            m.shapes.len(),
            m.draws.len(),
            lo,
            hi
        );
    }
}

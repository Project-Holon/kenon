//! Bringing a PNG into a picture: the PNG is laid over the picture the way
//! the pixel editor shows it (`catalog::sheet_data`), and every pixel is
//! written into the tile pixel the editor would paint.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::bridge::{Bridge, Error, Member};
use crate::catalog::{self, Layout, SheetData};
use crate::image::Rgba;
use crate::ncgr::Ncgr;
use crate::nclr::{bgr555_to_rgba, rgb_to_bgr555, Palette};

/// What happens to the palette.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PaletteMode {
    /// Each pixel takes the nearest colour of the palette bank it lands on.
    Keep,
    /// The PNG's colours go into the palette bank (reduced to fit).
    Replace,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportSummary {
    /// Picture pixels whose colour index changed.
    pub pixels: usize,
    /// Distinct palette colours the picture uses afterwards (not counting
    /// transparent).
    pub colors_used: usize,
    /// PNG colours that had no palette colour of their own.
    pub colors_merged: usize,
    /// The PNG was larger by a whole factor and was scaled down.
    pub scaled: bool,
    pub palette_changed: bool,
    pub width: usize,
    pub height: usize,
}

/// The size of a PNG without reading its pixels.
pub fn png_size(bytes: &[u8]) -> Result<(usize, usize), Error> {
    let reader = png::Decoder::new(std::io::Cursor::new(bytes))
        .read_info()
        .map_err(|e| Error::Format(format!("not a PNG picture: {e}")))?;
    let info = reader.info();
    Ok((info.width as usize, info.height as usize))
}

/// A PNG as 8-bit RGBA.
pub fn decode_png(bytes: &[u8]) -> Result<Rgba, Error> {
    let bad = |e: png::DecodingError| Error::Format(format!("not a PNG picture: {e}"));
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().map_err(bad)?;
    let mut buf = vec![
        0;
        reader
            .output_buffer_size()
            .ok_or_else(|| Error::Format("PNG too large".into()))?
    ];
    let frame = reader.next_frame(&mut buf).map_err(bad)?;
    let (w, h) = (frame.width as usize, frame.height as usize);
    let data = &buf[..frame.buffer_size()];
    let mut out = Rgba::new(w, h);
    let step = match frame.color_type {
        png::ColorType::Grayscale => 1,
        png::ColorType::GrayscaleAlpha => 2,
        png::ColorType::Rgb => 3,
        png::ColorType::Rgba => 4,
        png::ColorType::Indexed => {
            return Err(Error::Format("PNG palette was not expanded".into()))
        }
    };
    for i in 0..w * h {
        let p = &data[i * step..(i + 1) * step];
        out.pixels[i * 4..i * 4 + 4].copy_from_slice(&match step {
            1 => [p[0], p[0], p[0], 255],
            2 => [p[0], p[0], p[0], p[1]],
            3 => [p[0], p[1], p[2], 255],
            _ => [p[0], p[1], p[2], p[3]],
        });
    }
    Ok(out)
}

/// The PNG at the picture's size: as is, or scaled down by a whole factor
/// (nearest neighbour). Returns whether it was scaled.
fn fit(img: Rgba, width: usize, height: usize) -> Result<(Rgba, bool), Error> {
    if (img.width, img.height) == (width, height) {
        return Ok((img, false));
    }
    let k = img.width / width.max(1);
    if k > 1 && img.width == width * k && img.height == height * k {
        let mut out = Rgba::new(width, height);
        for y in 0..height {
            for x in 0..width {
                let at = ((y * k + k / 2) * img.width + x * k + k / 2) * 4;
                out.pixels[(y * width + x) * 4..][..4].copy_from_slice(&img.pixels[at..at + 4]);
            }
        }
        return Ok((out, true));
    }
    Err(Error::Format(format!(
        "the PNG is {}x{} pixels and the picture is {}x{}",
        img.width, img.height, width, height
    )))
}

/// A colour as the DS keeps it, or None for a transparent pixel.
fn pixel_color(p: &[u8]) -> Option<u16> {
    (p[3] >= 128).then(|| rgb_to_bgr555(p[0], p[1], p[2]))
}

fn dist(a: u16, b: u16) -> i32 {
    let (a, b) = (bgr555_to_rgba(a), bgr555_to_rgba(b));
    (0..3).map(|i| (a[i] as i32 - b[i] as i32).pow(2)).sum()
}

/// Reduces weighted colours to at most `n`: median cut, then a few rounds
/// of k-means. Nothing is random, so the same input gives the same palette.
pub fn quantize(hist: &[(u16, u32)], n: usize) -> Vec<u16> {
    if hist.len() <= n {
        return hist.iter().map(|&(c, _)| c).collect();
    }
    type Entry = ([i64; 3], i64);
    let ch = |c: u16| {
        [
            (c & 31) as i64,
            ((c >> 5) & 31) as i64,
            ((c >> 10) & 31) as i64,
        ]
    };
    let pack = |v: [i64; 3]| (v[0] | v[1] << 5 | v[2] << 10) as u16;
    let all: Vec<Entry> = hist.iter().map(|&(c, w)| (ch(c), w as i64)).collect();
    let mean = |b: &[Entry]| -> [i64; 3] {
        let total: i64 = b.iter().map(|e| e.1).sum();
        let mut m = [0; 3];
        for (k, v) in m.iter_mut().enumerate() {
            let s: i64 = b.iter().map(|e| e.0[k] * e.1).sum();
            *v = (s + total / 2) / total.max(1);
        }
        m
    };
    let spread = |b: &[Entry]| -> i64 {
        let m = mean(b);
        b.iter()
            .map(|e| (0..3).map(|k| (e.0[k] - m[k]).pow(2)).sum::<i64>() * e.1)
            .sum()
    };
    let mut boxes = vec![all.clone()];
    while boxes.len() < n {
        // The box that varies most, if it can still split.
        let Some((at, _)) = boxes
            .iter()
            .enumerate()
            .filter(|(_, b)| b.len() > 1)
            .map(|(i, b)| (i, spread(b)))
            .max_by_key(|&(i, s)| (s, std::cmp::Reverse(i)))
        else {
            break;
        };
        let mut b = boxes.swap_remove(at);
        let axis = (0..3)
            .max_by_key(|&k| {
                let lo = b.iter().map(|e| e.0[k]).min().unwrap();
                let hi = b.iter().map(|e| e.0[k]).max().unwrap();
                (hi - lo, std::cmp::Reverse(k))
            })
            .unwrap();
        b.sort_by_key(|e| (e.0[axis], e.0));
        let half = b.iter().map(|e| e.1).sum::<i64>() / 2;
        let (mut run, mut cut) = (0, 1);
        for (i, e) in b.iter().enumerate() {
            run += e.1;
            if run >= half {
                cut = (i + 1).clamp(1, b.len() - 1);
                break;
            }
        }
        let right = b.split_off(cut);
        boxes.push(b);
        boxes.push(right);
    }
    let mut reps: Vec<[i64; 3]> = boxes.iter().map(|b| mean(b)).collect();
    for _ in 0..6 {
        let mut sums = vec![([0i64; 3], 0i64); reps.len()];
        for e in &all {
            let best = (0..reps.len())
                .min_by_key(|&r| (0..3).map(|k| (e.0[k] - reps[r][k]).pow(2)).sum::<i64>())
                .unwrap();
            for k in 0..3 {
                sums[best].0[k] += e.0[k] * e.1;
            }
            sums[best].1 += e.1;
        }
        for (r, (s, w)) in reps.iter_mut().zip(sums) {
            if w > 0 {
                *r = [(s[0] + w / 2) / w, (s[1] + w / 2) / w, (s[2] + w / 2) / w];
            }
        }
    }
    // Light to dark order, no repeats.
    let mut out: Vec<u16> = reps.into_iter().map(pack).collect();
    out.sort_by_key(|&c| {
        let [r, g, b, _] = bgr555_to_rgba(c);
        (
            std::cmp::Reverse(r as u32 * 299 + g as u32 * 587 + b as u32 * 114),
            c,
        )
    });
    out.dedup();
    out
}

/// What the import decided, before anything is written.
struct Plan {
    /// The new value of every tile pixel the picture covers.
    tiles: Vec<Vec<u8>>,
    colors: Vec<u16>,
    summary: ImportSummary,
}

/// The tile pixel changes and palette for a PNG laid over `data`. `colors`
/// is the palette as stored.
fn plan(data: &SheetData, colors: &[u16], img: &Rgba, mode: PaletteMode) -> Result<Plan, Error> {
    let four = data.bpp == 4;
    let base_of = |i: usize| if four { data.banks[i] as usize * 16 } else { 0 };
    // The usable colours of a bank: 1..len (0 is transparent).
    let span = if four { 16 } else { 256 };
    let room = |base: usize| span.min(colors.len().saturating_sub(base));
    let covered: Vec<usize> = (0..data.map.len()).filter(|&i| data.map[i] >= 0).collect();
    if covered.is_empty() {
        return Err(Error::Format("the picture has no pixels to write".into()));
    }
    for &i in &covered {
        if room(base_of(i)) < 2 {
            return Err(Error::Format(format!(
                "palette bank {} is not in the palette",
                base_of(i) / 16
            )));
        }
    }
    let color_at = |i: usize| pixel_color(&img.pixels[i * 4..i * 4 + 4]);
    let mut new_colors = colors.to_vec();
    let mut target = vec![0u8; data.map.len()];
    // PNG colours that found no colour of their own, per bank.
    let mut merged: BTreeSet<(usize, u16)> = BTreeSet::new();

    match mode {
        PaletteMode::Keep => {
            for &i in &covered {
                let Some(c) = color_at(i) else { continue };
                let base = base_of(i);
                let (src, usable) = (data.map[i] as usize, room(base));
                let current = data.tiles[src / 64][src % 64] as usize;
                let mut best = (i32::MAX, 0usize);
                for idx in 1..usable {
                    let d = dist(c, colors[base + idx]);
                    // A tie keeps the index the pixel has, so a picture that
                    // did not change is written back as it was.
                    if d < best.0 || (d == best.0 && idx == current) {
                        best = (d, idx);
                    }
                }
                target[i] = best.1 as u8;
                if colors[base + best.1] != c {
                    merged.insert((base, c));
                }
            }
        }
        PaletteMode::Replace => {
            if four {
                // A tile pixel seen through two banks has no single index.
                let mut seen: BTreeMap<i32, u8> = BTreeMap::new();
                for &i in &covered {
                    if *seen.entry(data.map[i]).or_insert(data.banks[i]) != data.banks[i] {
                        return Err(Error::Format(
                            "this picture shows the same tile pixels with different palette banks; \
                             keep the palette colours"
                                .into(),
                        ));
                    }
                }
            }
            let mut groups: BTreeMap<usize, BTreeMap<u16, u32>> = BTreeMap::new();
            for &i in &covered {
                let base = base_of(i);
                let group = groups.entry(base).or_default();
                if let Some(c) = color_at(i) {
                    *group.entry(c).or_default() += 1;
                }
            }
            let mut reps: BTreeMap<usize, Vec<u16>> = BTreeMap::new();
            for (base, hist) in &groups {
                if hist.is_empty() {
                    continue;
                }
                let list: Vec<(u16, u32)> = hist.iter().map(|(&c, &w)| (c, w)).collect();
                let chosen = quantize(&list, room(*base) - 1);
                for (k, &c) in chosen.iter().enumerate() {
                    new_colors[base + 1 + k] = c;
                }
                for &(c, _) in &list {
                    if !chosen.contains(&c) {
                        merged.insert((*base, c));
                    }
                }
                reps.insert(*base, chosen);
            }
            for &i in &covered {
                let Some(c) = color_at(i) else { continue };
                let chosen = &reps[&base_of(i)];
                let best = (0..chosen.len())
                    .min_by_key(|&k| (dist(c, chosen[k]), k))
                    .unwrap();
                target[i] = best as u8 + 1;
            }
        }
    }

    let mut tiles = data.tiles.clone();
    let mut pixels = 0;
    let mut used = BTreeSet::new();
    for &i in &covered {
        let src = data.map[i] as usize;
        let slot = &mut tiles[src / 64][src % 64];
        if *slot != target[i] {
            pixels += 1;
        }
        *slot = target[i];
        if target[i] != 0 {
            used.insert(base_of(i) + target[i] as usize);
        }
    }
    Ok(Plan {
        tiles,
        summary: ImportSummary {
            pixels,
            colors_used: used.len(),
            colors_merged: merged.len(),
            scaled: false,
            palette_changed: new_colors != colors,
            width: data.width,
            height: data.height,
        },
        colors: new_colors,
    })
}

/// Lays a PNG over the picture `layout` describes and writes the result
/// into the project: the sheet, and the palette when it changed.
/// `bank_offset` is added to every pixel's palette bank, as the editor does
/// for the picture it shows.
pub fn import_picture(
    bridge: &Bridge,
    chr: Member,
    pal: Member,
    layout: &Layout,
    png_bytes: &[u8],
    mode: PaletteMode,
    bank_offset: u8,
) -> Result<ImportSummary, Error> {
    let mut data = catalog::sheet_data(bridge, chr, pal, layout)?;
    data.banks
        .iter_mut()
        .for_each(|b| *b = b.saturating_add(bank_offset));
    let (img, scaled) = fit(decode_png(png_bytes)?, data.width, data.height)?;
    let palette_file = bridge.member(pal)?;
    let palette = Palette::parse(&palette_file).map_err(|e| Error::Format(e.to_string()))?;
    let mut plan = plan(&data, &palette.colors, &img, mode)?;
    plan.summary.scaled = scaled;

    let file = bridge.member(chr)?;
    let mut ncgr = Ncgr::parse(&file).map_err(|e| Error::Format(e.to_string()))?;
    for (t, (new, old)) in plan.tiles.iter().zip(&data.tiles).enumerate() {
        if new != old {
            for (p, &v) in new.iter().enumerate() {
                ncgr.set_pixel(t, p % 8, p / 8, v);
            }
        }
    }
    let sheet = Ncgr::with_data(&file, &ncgr.data).map_err(|e| Error::Format(e.to_string()))?;
    catalog::write_member(bridge, chr, &sheet)?;
    if plan.summary.palette_changed {
        let out = Palette::with_colors(&palette_file, &plan.colors)
            .map_err(|e| Error::Format(e.to_string()))?;
        catalog::write_member(bridge, pal, &out)?;
    }
    Ok(plan.summary)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data(bpp: u8, tiles: Vec<Vec<u8>>, w: usize, h: usize, banks: Vec<u8>) -> SheetData {
        let mut map = vec![-1; w * h];
        for (t, _) in tiles.iter().enumerate() {
            for p in 0..64 {
                map[(p / 8) * w + t * 8 + p % 8] = (t * 64 + p) as i32;
            }
        }
        SheetData {
            bpp,
            tiles,
            colors: vec![],
            width: w,
            height: h,
            map,
            banks,
            tiles_wide: None,
        }
    }

    fn solid(c: [u8; 4], w: usize, h: usize) -> Rgba {
        let mut img = Rgba::new(w, h);
        img.pixels.chunks_mut(4).for_each(|p| p.copy_from_slice(&c));
        img
    }

    #[test]
    fn quantize_is_exact_when_it_fits_and_stable_when_not() {
        let few = [(0x001F, 3), (0x03E0, 1)];
        assert_eq!(quantize(&few, 4).len(), 2);
        let many: Vec<(u16, u32)> = (0..200u16)
            .map(|c| (c * 37 % 32768, 1 + c as u32 % 5))
            .collect();
        let a = quantize(&many, 15);
        assert!(a.len() <= 15 && a.len() > 8);
        assert_eq!(a, quantize(&many, 15));
    }

    #[test]
    fn keep_takes_the_nearest_colour_and_transparent_is_zero() {
        let colors = [0, 0x001F, 0x03E0, 0x7C00];
        let d = data(4, vec![vec![1; 64]], 8, 8, vec![0; 64]);
        let mut padded = colors.to_vec();
        padded.resize(16, 0x0421);
        // Nearly green.
        let img = solid([10, 240, 8, 255], 8, 8);
        let p = plan(&d, &padded, &img, PaletteMode::Keep).unwrap();
        assert!(p.tiles[0].iter().all(|&v| v == 2));
        assert_eq!(p.summary.colors_used, 1);
        assert_eq!(p.summary.colors_merged, 1);
        assert!(!p.summary.palette_changed);
        let clear = solid([9, 9, 9, 0], 8, 8);
        let p = plan(&d, &padded, &clear, PaletteMode::Keep).unwrap();
        assert!(p.tiles[0].iter().all(|&v| v == 0));
        assert_eq!(p.summary.pixels, 64);
    }

    #[test]
    fn replace_writes_the_png_colours_into_the_bank() {
        let d = data(4, vec![vec![0; 64]], 8, 8, vec![1; 64]);
        let mut img = solid([255, 0, 0, 255], 8, 8);
        img.pixels[..4].copy_from_slice(&[0, 0, 255, 255]);
        let colors = vec![0x7FFF; 32];
        let p = plan(&d, &colors, &img, PaletteMode::Replace).unwrap();
        assert!(p.summary.palette_changed);
        assert_eq!(p.summary.colors_used, 2);
        assert_eq!(p.summary.colors_merged, 0);
        // Bank 1 holds the two colours; bank 0 is untouched.
        assert_eq!(&p.colors[..16], &colors[..16]);
        let red = rgb_to_bgr555(255, 0, 0);
        assert!(p.colors[17..19].contains(&red));
        assert_ne!(p.tiles[0][0], p.tiles[0][1]);
    }

    #[test]
    fn sizes() {
        let img = solid([1, 2, 3, 255], 16, 16);
        assert!(fit(img.clone(), 8, 8).unwrap().1);
        let err = fit(img, 8, 12).unwrap_err().to_string();
        assert!(err.contains("16x16") && err.contains("8x12"), "{err}");
    }
}

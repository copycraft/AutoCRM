//! Thumbnails for drawings, so a design file shows what it is before anyone downloads it.
//!
//! DXF is text: its lines, polylines, circles, arcs, splines and block inserts are drawn
//! here onto a small canvas. DWG is a closed binary format, but every DWG since R13 carries
//! a preview picture its CAD program saved with it; that picture is extracted. Neither
//! needs a CAD library or anything installed on the server. CPU-bound and synchronous —
//! call from `spawn_blocking`.

use std::collections::HashMap;
use std::io::Cursor;

use image::{DynamicImage, ImageFormat, Rgb, RgbImage};

pub const THUMB_WIDTH: u32 = 640;
pub const THUMB_HEIGHT: u32 = 480;
/// Insert nesting deeper than this is either a cycle or not worth drawing.
const MAX_BLOCK_DEPTH: usize = 8;
/// A drawing with more segments than this is drawn from its first segments only.
const MAX_SEGMENTS: usize = 400_000;

#[derive(Debug, thiserror::Error)]
pub enum CadError {
    #[error("not a text DXF (binary DXF is not supported)")]
    NotTextDxf,
    #[error("the drawing has nothing to draw")]
    Empty,
    #[error("the DWG carries no preview picture")]
    NoDwgPreview,
    #[error("the preview picture could not be read: {0}")]
    Image(String),
}

type Point = (f64, f64);
type Segment = (Point, Point);

/// The DXF as (group code, value) pairs.
fn pairs(text: &str) -> Vec<(i32, &str)> {
    let mut lines = text.lines();
    let mut out = Vec::new();
    while let (Some(code), Some(value)) = (lines.next(), lines.next()) {
        if let Ok(code) = code.trim().parse::<i32>() {
            out.push((code, value.trim()));
        }
    }
    out
}

/// One entity: its type and its group codes in order.
#[derive(Debug, Clone)]
struct Entity<'a> {
    kind: &'a str,
    codes: Vec<(i32, &'a str)>,
}

impl Entity<'_> {
    fn num(&self, code: i32) -> Option<f64> {
        self.codes
            .iter()
            .find(|(c, _)| *c == code)
            .and_then(|(_, v)| v.parse().ok())
    }
    fn text(&self, code: i32) -> Option<&str> {
        self.codes.iter().find(|(c, _)| *c == code).map(|(_, v)| *v)
    }
    /// Every (10, 20) pair, in order: polyline vertices, spline control points.
    fn points(&self) -> Vec<Point> {
        let mut out = Vec::new();
        let mut x = None;
        for (code, value) in &self.codes {
            match code {
                10 => x = value.parse::<f64>().ok(),
                20 => {
                    if let (Some(px), Ok(py)) = (x.take(), value.parse::<f64>()) {
                        out.push((px, py));
                    }
                }
                _ => {}
            }
        }
        out
    }
}

/// Splits a section's pairs into entities at every code 0.
fn entities<'a>(pairs: &[(i32, &'a str)]) -> Vec<Entity<'a>> {
    let mut out: Vec<Entity<'a>> = Vec::new();
    for &(code, value) in pairs {
        if code == 0 {
            out.push(Entity {
                kind: value,
                codes: Vec::new(),
            });
        } else if let Some(last) = out.last_mut() {
            last.codes.push((code, value));
        }
    }
    out
}

/// The pairs of the named section (`ENTITIES`, `BLOCKS`).
fn section<'a>(all: &[(i32, &'a str)], name: &str) -> Vec<(i32, &'a str)> {
    let mut out = Vec::new();
    let mut inside = false;
    for (i, &(code, value)) in all.iter().enumerate() {
        if code == 0 && value == "SECTION" {
            inside = all.get(i + 1) == Some(&(2, name));
            continue;
        }
        if code == 0 && value == "ENDSEC" {
            inside = false;
            continue;
        }
        if inside {
            out.push((code, value));
        }
    }
    // Skip the section's own name pair.
    if out.first().is_some_and(|(c, _)| *c == 2) {
        out.remove(0);
    }
    out
}

/// Block name → its entities.
fn blocks<'a>(all: &[(i32, &'a str)]) -> HashMap<String, Vec<Entity<'a>>> {
    let mut out = HashMap::new();
    let mut current: Option<(String, Vec<Entity<'a>>)> = None;
    for entity in entities(&section(all, "BLOCKS")) {
        match entity.kind {
            "BLOCK" => current = Some((entity.text(2).unwrap_or_default().to_string(), Vec::new())),
            "ENDBLK" => {
                if let Some((name, list)) = current.take() {
                    out.insert(name, list);
                }
            }
            _ => {
                if let Some((_, list)) = current.as_mut() {
                    list.push(entity);
                }
            }
        }
    }
    out
}

/// A 2D transform: scale, rotate, translate (what an INSERT does to its block).
#[derive(Debug, Clone, Copy)]
struct Transform {
    sx: f64,
    sy: f64,
    cos: f64,
    sin: f64,
    tx: f64,
    ty: f64,
}

impl Transform {
    const IDENTITY: Transform = Transform {
        sx: 1.0,
        sy: 1.0,
        cos: 1.0,
        sin: 0.0,
        tx: 0.0,
        ty: 0.0,
    };

    fn apply(&self, (x, y): Point) -> Point {
        let (x, y) = (x * self.sx, y * self.sy);
        (
            x * self.cos - y * self.sin + self.tx,
            x * self.sin + y * self.cos + self.ty,
        )
    }

    /// `self` applied after `inner`.
    fn then(&self, inner: &Transform) -> Transform {
        // Composition is approximated for non-uniform scales under rotation; thumbnails
        // forgive that, and real drawings rarely nest skewed inserts.
        let origin = self.apply((inner.tx, inner.ty));
        Transform {
            sx: self.sx * inner.sx,
            sy: self.sy * inner.sy,
            cos: self.cos * inner.cos - self.sin * inner.sin,
            sin: self.sin * inner.cos + self.cos * inner.sin,
            tx: origin.0,
            ty: origin.1,
        }
    }
}

fn arc_points(cx: f64, cy: f64, r: f64, start_deg: f64, end_deg: f64) -> Vec<Point> {
    let mut end = end_deg;
    while end <= start_deg {
        end += 360.0;
    }
    let steps = (((end - start_deg) / 6.0).ceil() as usize).clamp(4, 120);
    (0..=steps)
        .map(|i| {
            let a = (start_deg + (end - start_deg) * i as f64 / steps as f64).to_radians();
            (cx + r * a.cos(), cy + r * a.sin())
        })
        .collect()
}

fn polyline(points: &[Point], closed: bool, t: &Transform, out: &mut Vec<Segment>) {
    for w in points.windows(2) {
        out.push((t.apply(w[0]), t.apply(w[1])));
    }
    if closed && points.len() > 2 {
        out.push((t.apply(points[points.len() - 1]), t.apply(points[0])));
    }
}

fn collect(
    list: &[Entity<'_>],
    blocks: &HashMap<String, Vec<Entity<'_>>>,
    t: &Transform,
    depth: usize,
    out: &mut Vec<Segment>,
) {
    let mut pending_polyline: Option<(Vec<Point>, bool)> = None;
    for e in list {
        if out.len() > MAX_SEGMENTS {
            return;
        }
        // Old-style POLYLINE: VERTEX entities follow until SEQEND.
        if let Some((points, closed)) = pending_polyline.as_mut() {
            match e.kind {
                "VERTEX" => {
                    if let (Some(x), Some(y)) = (e.num(10), e.num(20)) {
                        points.push((x, y));
                    }
                    continue;
                }
                _ => {
                    polyline(points, *closed, t, out);
                    pending_polyline = None;
                    if e.kind == "SEQEND" {
                        continue;
                    }
                }
            }
        }
        match e.kind {
            "LINE" => {
                if let (Some(x1), Some(y1), Some(x2), Some(y2)) =
                    (e.num(10), e.num(20), e.num(11), e.num(21))
                {
                    out.push((t.apply((x1, y1)), t.apply((x2, y2))));
                }
            }
            "LWPOLYLINE" => {
                let closed = e.num(70).is_some_and(|f| (f as i64) & 1 == 1);
                polyline(&e.points(), closed, t, out);
            }
            "POLYLINE" => {
                let closed = e.num(70).is_some_and(|f| (f as i64) & 1 == 1);
                pending_polyline = Some((Vec::new(), closed));
            }
            // Control points are close enough to the curve for a thumbnail.
            "SPLINE" => polyline(&e.points(), false, t, out),
            "CIRCLE" => {
                if let (Some(cx), Some(cy), Some(r)) = (e.num(10), e.num(20), e.num(40)) {
                    polyline(&arc_points(cx, cy, r, 0.0, 360.0), false, t, out);
                }
            }
            "ARC" => {
                if let (Some(cx), Some(cy), Some(r), Some(a), Some(b)) =
                    (e.num(10), e.num(20), e.num(40), e.num(50), e.num(51))
                {
                    polyline(&arc_points(cx, cy, r, a, b), false, t, out);
                }
            }
            "INSERT" if depth < MAX_BLOCK_DEPTH => {
                let Some(name) = e.text(2) else { continue };
                let Some(block) = blocks.get(name) else {
                    continue;
                };
                let angle = e.num(50).unwrap_or(0.0).to_radians();
                let insert = Transform {
                    sx: e.num(41).unwrap_or(1.0),
                    sy: e.num(42).unwrap_or(1.0),
                    cos: angle.cos(),
                    sin: angle.sin(),
                    tx: e.num(10).unwrap_or(0.0),
                    ty: e.num(20).unwrap_or(0.0),
                };
                collect(block, blocks, &t.then(&insert), depth + 1, out);
            }
            _ => {}
        }
    }
    if let Some((points, closed)) = pending_polyline {
        polyline(&points, closed, t, out);
    }
}

/// Every segment the drawing's model space draws.
fn segments(text: &str) -> Vec<Segment> {
    let all = pairs(text);
    let blocks = blocks(&all);
    let mut out = Vec::new();
    collect(
        &entities(&section(&all, "ENTITIES")),
        &blocks,
        &Transform::IDENTITY,
        0,
        &mut out,
    );
    out.retain(|((x1, y1), (x2, y2))| {
        x1.is_finite() && y1.is_finite() && x2.is_finite() && y2.is_finite()
    });
    out
}

fn draw_line(img: &mut RgbImage, (x0, y0): (i64, i64), (x1, y1): (i64, i64), color: Rgb<u8>) {
    let (w, h) = (img.width() as i64, img.height() as i64);
    let (dx, dy) = ((x1 - x0).abs(), -(y1 - y0).abs());
    let (sx, sy) = (if x0 < x1 { 1 } else { -1 }, if y0 < y1 { 1 } else { -1 });
    let (mut x, mut y, mut err) = (x0, y0, dx + dy);
    for _ in 0..(w + h) * 4 {
        if (0..w).contains(&x) && (0..h).contains(&y) {
            img.put_pixel(x as u32, y as u32, color);
        }
        if x == x1 && y == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
}

fn png(img: &DynamicImage) -> Result<Vec<u8>, CadError> {
    let mut out = Cursor::new(Vec::new());
    img.write_to(&mut out, ImageFormat::Png)
        .map_err(|e| CadError::Image(e.to_string()))?;
    Ok(out.into_inner())
}

/// A PNG of the drawing, fitted into the thumbnail with a margin, dark lines on white.
pub fn dxf_thumbnail(bytes: &[u8]) -> Result<Vec<u8>, CadError> {
    if bytes.starts_with(b"AutoCAD Binary DXF") {
        return Err(CadError::NotTextDxf);
    }
    let text = String::from_utf8_lossy(bytes);
    let segs = segments(&text);
    if segs.is_empty() {
        return Err(CadError::Empty);
    }
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for ((x1, y1), (x2, y2)) in &segs {
        min_x = min_x.min(*x1).min(*x2);
        max_x = max_x.max(*x1).max(*x2);
        min_y = min_y.min(*y1).min(*y2);
        max_y = max_y.max(*y1).max(*y2);
    }
    let margin = 16.0;
    let (w, h) = (
        THUMB_WIDTH as f64 - 2.0 * margin,
        THUMB_HEIGHT as f64 - 2.0 * margin,
    );
    let span_x = (max_x - min_x).max(1e-9);
    let span_y = (max_y - min_y).max(1e-9);
    let scale = (w / span_x).min(h / span_y);
    let off_x = margin + (w - span_x * scale) / 2.0;
    let off_y = margin + (h - span_y * scale) / 2.0;
    let to_px = |(x, y): Point| -> (i64, i64) {
        let px = off_x + (x - min_x) * scale;
        // DXF y grows upwards, image y downwards.
        let py = THUMB_HEIGHT as f64 - (off_y + (y - min_y) * scale);
        (px.round() as i64, py.round() as i64)
    };
    let mut img = RgbImage::from_pixel(THUMB_WIDTH, THUMB_HEIGHT, Rgb([255, 255, 255]));
    let ink = Rgb([31, 34, 40]);
    for (a, b) in segs {
        draw_line(&mut img, to_px(a), to_px(b), ink);
    }
    png(&DynamicImage::ImageRgb8(img))
}

/// Marks the start of a DWG's preview section (R13 and later).
const DWG_IMAGE_SENTINEL: [u8; 16] = [
    0x1F, 0x25, 0x6D, 0x07, 0xD4, 0x36, 0x28, 0x28, 0x9D, 0x57, 0xCA, 0x3F, 0x9D, 0x44, 0x10, 0x2B,
];

fn le32(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

/// The preview picture a CAD program stored inside a DWG, as a PNG thumbnail.
///
/// The file header points at the preview (offset 0x0D); where that pointer is unreadable the
/// section is found by its sentinel. The section lists pictures by type: 2 is a bitmap
/// without its file header, 6 a PNG; a Windows metafile (3) is not drawn.
pub fn dwg_thumbnail(bytes: &[u8]) -> Result<Vec<u8>, CadError> {
    if !bytes.starts_with(b"AC10") {
        return Err(CadError::NoDwgPreview);
    }
    let pointed = le32(bytes, 0x0D).map(|p| p as usize);
    let start = match pointed {
        Some(p) if bytes.get(p..p + 16) == Some(&DWG_IMAGE_SENTINEL[..]) => p,
        _ => bytes
            .windows(16)
            .position(|w| w == DWG_IMAGE_SENTINEL)
            .ok_or(CadError::NoDwgPreview)?,
    };
    // sentinel (16), overall size (4), picture count (1), then 9 bytes per picture.
    let count = *bytes.get(start + 20).ok_or(CadError::NoDwgPreview)? as usize;
    let mut found: Option<(u8, usize, usize)> = None;
    for i in 0..count.min(8) {
        let at = start + 21 + i * 9;
        let code = *bytes.get(at).ok_or(CadError::NoDwgPreview)?;
        let offset = le32(bytes, at + 1).ok_or(CadError::NoDwgPreview)? as usize;
        let size = le32(bytes, at + 5).ok_or(CadError::NoDwgPreview)? as usize;
        if (code == 6 || code == 2) && size > 0 {
            // Prefer the PNG when both are there.
            if found.is_none_or(|(c, _, _)| c != 6) {
                found = Some((code, offset, size));
            }
        }
    }
    let (code, offset, size) = found.ok_or(CadError::NoDwgPreview)?;
    let data = bytes
        .get(offset..offset + size)
        .ok_or(CadError::NoDwgPreview)?;
    let decoded = if code == 6 {
        image::load_from_memory_with_format(data, ImageFormat::Png)
    } else {
        // A DIB: put the 14-byte BMP file header in front of it.
        let header_len = le32(data, 0).unwrap_or(40) as usize;
        let bit_count = data
            .get(14..16)
            .map(|b| u16::from_le_bytes([b[0], b[1]]))
            .unwrap_or(8);
        let colors_used = le32(data, 32).unwrap_or(0) as usize;
        let palette = if bit_count <= 8 {
            4 * if colors_used == 0 {
                1usize << bit_count
            } else {
                colors_used
            }
        } else {
            0
        };
        let pixel_offset = 14 + header_len + palette;
        let mut bmp = Vec::with_capacity(14 + data.len());
        bmp.extend_from_slice(b"BM");
        bmp.extend_from_slice(&((14 + data.len()) as u32).to_le_bytes());
        bmp.extend_from_slice(&[0, 0, 0, 0]);
        bmp.extend_from_slice(&(pixel_offset as u32).to_le_bytes());
        bmp.extend_from_slice(data);
        image::load_from_memory_with_format(&bmp, ImageFormat::Bmp)
    }
    .map_err(|e| CadError::Image(e.to_string()))?;
    // Down to fit, never up: `thumbnail` would blow a 160 px preview up into a blur.
    if decoded.width() > THUMB_WIDTH || decoded.height() > THUMB_HEIGHT {
        png(&decoded.thumbnail(THUMB_WIDTH, THUMB_HEIGHT))
    } else {
        png(&decoded)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dxf(body: &str) -> String {
        format!("0\nSECTION\n2\nENTITIES\n{body}0\nENDSEC\n0\nEOF\n")
    }

    #[test]
    fn lines_circles_and_polylines_are_drawn() {
        let text = dxf("0\nLINE\n8\n0\n10\n0\n20\n0\n11\n100\n21\n50\n\
             0\nCIRCLE\n10\n50\n20\n25\n40\n10\n\
             0\nLWPOLYLINE\n90\n3\n70\n1\n10\n0\n20\n0\n10\n100\n20\n0\n10\n100\n20\n50\n");
        let segs = segments(&text);
        // 1 line, a 60-step circle, a closed triangle (3 sides).
        assert_eq!(segs.len(), 1 + 60 + 3);
        let png = dxf_thumbnail(text.as_bytes()).unwrap();
        let img = image::load_from_memory(&png).unwrap();
        assert_eq!((img.width(), img.height()), (THUMB_WIDTH, THUMB_HEIGHT));
        // Something dark was drawn.
        assert!(img.to_rgb8().pixels().any(|p| p.0[0] < 100));
    }

    #[test]
    fn inserts_place_their_block() {
        let text = format!(
            "0\nSECTION\n2\nBLOCKS\n0\nBLOCK\n2\nBOX\n0\nLINE\n10\n0\n20\n0\n11\n1\n21\n0\n0\nENDBLK\n0\nENDSEC\n{}",
            dxf("0\nINSERT\n2\nBOX\n10\n10\n20\n5\n41\n2\n42\n2\n"),
        );
        let segs = segments(&text);
        assert_eq!(segs.len(), 1);
        let ((x1, y1), (x2, y2)) = segs[0];
        assert!((x1 - 10.0).abs() < 1e-9 && (y1 - 5.0).abs() < 1e-9);
        assert!((x2 - 12.0).abs() < 1e-9 && (y2 - 5.0).abs() < 1e-9);
    }

    #[test]
    fn old_style_polylines_collect_their_vertices() {
        let text = dxf(
            "0\nPOLYLINE\n70\n0\n0\nVERTEX\n10\n0\n20\n0\n0\nVERTEX\n10\n1\n20\n1\n0\nVERTEX\n10\n2\n20\n0\n0\nSEQEND\n",
        );
        assert_eq!(segments(&text).len(), 2);
    }

    #[test]
    fn an_empty_drawing_or_binary_dxf_is_refused() {
        assert!(matches!(
            dxf_thumbnail(dxf("").as_bytes()),
            Err(CadError::Empty)
        ));
        assert!(matches!(
            dxf_thumbnail(b"AutoCAD Binary DXF\r\n\x1a\0"),
            Err(CadError::NotTextDxf)
        ));
    }

    #[test]
    fn a_dwg_preview_png_is_found_by_its_sentinel() {
        let pic = {
            let mut out = Cursor::new(Vec::new());
            DynamicImage::ImageRgb8(RgbImage::from_pixel(20, 10, Rgb([200, 0, 0])))
                .write_to(&mut out, ImageFormat::Png)
                .unwrap();
            out.into_inner()
        };
        let mut file = b"AC1032".to_vec();
        file.resize(0x80, 0);
        let section = file.len();
        file.extend_from_slice(&DWG_IMAGE_SENTINEL);
        file.extend_from_slice(&0u32.to_le_bytes());
        file.push(1);
        let data_at = section + 16 + 4 + 1 + 9;
        file.push(6);
        file.extend_from_slice(&(data_at as u32).to_le_bytes());
        file.extend_from_slice(&(pic.len() as u32).to_le_bytes());
        file.extend_from_slice(&pic);
        // The header pointer is left at zero: the sentinel search finds the section.
        let thumb = dwg_thumbnail(&file).unwrap();
        let img = image::load_from_memory(&thumb).unwrap();
        assert_eq!((img.width(), img.height()), (20, 10));
    }

    #[test]
    fn a_dwg_without_a_preview_says_so() {
        let mut file = b"AC1018".to_vec();
        file.resize(200, 0);
        assert!(matches!(dwg_thumbnail(&file), Err(CadError::NoDwgPreview)));
        assert!(matches!(
            dwg_thumbnail(b"not a dwg"),
            Err(CadError::NoDwgPreview)
        ));
    }
}

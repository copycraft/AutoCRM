//! Image processing: EXIF read, orientation, derived copies. CPU-bound and synchronous —
//! call it from `spawn_blocking`. The original bytes are never modified; derived copies are
//! re-encoded, which strips all EXIF (including GPS) by construction.

use std::io::Cursor;

use chrono::{DateTime, FixedOffset, NaiveDateTime, TimeZone, Utc};
use chrono_tz::Tz;
use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::{DynamicImage, ImageReader, Limits};

pub const DISPLAY_MAX_EDGE: u32 = 2560;
pub const THUMB_MAX_EDGE: u32 = 480;
const JPEG_QUALITY: u8 = 85;

#[derive(Debug, thiserror::Error)]
pub enum PipelineError {
    #[error("unsupported or corrupt image: {0}")]
    Decode(String),
    #[error("encoding derived image failed: {0}")]
    Encode(String),
}

#[derive(Debug)]
pub struct Processed {
    /// Dimensions after applying EXIF orientation (as a human sees the photo).
    pub width: u32,
    pub height: u32,
    pub captured_at: Option<DateTime<Utc>>,
    pub display_jpeg: Vec<u8>,
    pub thumb_jpeg: Vec<u8>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ExifInfo {
    pub orientation: Option<u32>,
    pub date_time_original: Option<String>,
    pub offset_time_original: Option<String>,
}

pub fn read_exif(bytes: &[u8]) -> ExifInfo {
    let Ok(exif) = exif::Reader::new().read_from_container(&mut Cursor::new(bytes)) else {
        return ExifInfo::default();
    };
    let ascii = |tag| {
        exif.get_field(tag, exif::In::PRIMARY)
            .and_then(|f| match &f.value {
                exif::Value::Ascii(parts) => parts
                    .first()
                    .map(|p| String::from_utf8_lossy(p).trim().to_string()),
                _ => None,
            })
    };
    ExifInfo {
        orientation: exif
            .get_field(exif::Tag::Orientation, exif::In::PRIMARY)
            .and_then(|f| f.value.get_uint(0)),
        date_time_original: ascii(exif::Tag::DateTimeOriginal)
            .or_else(|| ascii(exif::Tag::DateTime)),
        offset_time_original: ascii(exif::Tag::OffsetTimeOriginal),
    }
}

/// EXIF dates are "YYYY:MM:DD HH:MM:SS" in camera-local time. With an offset tag we know the
/// instant exactly; without one, assume the phone was in the business time zone.
pub fn parse_exif_datetime(value: &str, offset: Option<&str>, tz: Tz) -> Option<DateTime<Utc>> {
    let naive = NaiveDateTime::parse_from_str(value.trim(), "%Y:%m:%d %H:%M:%S").ok()?;
    if let Some(offset) = offset.and_then(parse_offset) {
        return offset
            .from_local_datetime(&naive)
            .single()
            .map(|dt| dt.with_timezone(&Utc));
    }
    tz.from_local_datetime(&naive)
        .earliest()
        .map(|dt| dt.with_timezone(&Utc))
}

fn parse_offset(s: &str) -> Option<FixedOffset> {
    let s = s.trim();
    let sign = match s.chars().next()? {
        '+' => 1,
        '-' => -1,
        _ => return None,
    };
    let (h, m) = s[1..].split_once(':')?;
    let secs = h.parse::<i32>().ok()? * 3600 + m.parse::<i32>().ok()? * 60;
    FixedOffset::east_opt(sign * secs)
}

fn apply_orientation(img: DynamicImage, orientation: Option<u32>) -> DynamicImage {
    match orientation {
        Some(2) => img.fliph(),
        Some(3) => img.rotate180(),
        Some(4) => img.flipv(),
        Some(5) => img.rotate90().fliph(),
        Some(6) => img.rotate90(),
        Some(7) => img.rotate270().fliph(),
        Some(8) => img.rotate270(),
        _ => img,
    }
}

fn encode_jpeg(img: &DynamicImage) -> Result<Vec<u8>, PipelineError> {
    let mut out = Vec::new();
    let rgb = DynamicImage::ImageRgb8(img.to_rgb8());
    rgb.write_with_encoder(JpegEncoder::new_with_quality(&mut out, JPEG_QUALITY))
        .map_err(|e| PipelineError::Encode(e.to_string()))?;
    Ok(out)
}

pub const AVATAR_EDGE: u32 = 512;

/// A profile picture: orientation applied, cropped to the centre square, scaled to
/// `AVATAR_EDGE`, re-encoded as JPEG (which drops all EXIF, GPS included).
pub fn process_avatar(bytes: &[u8]) -> Result<Vec<u8>, PipelineError> {
    let exif = read_exif(bytes);
    let mut limits = Limits::default();
    limits.max_image_width = Some(20_000);
    limits.max_image_height = Some(20_000);
    limits.max_alloc = Some(1024 * 1024 * 1024);
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| PipelineError::Decode(e.to_string()))?;
    reader.limits(limits);
    let decoded = reader
        .decode()
        .map_err(|e| PipelineError::Decode(e.to_string()))?;
    let img = apply_orientation(decoded, exif.orientation);
    encode_jpeg(&img.resize_to_fill(AVATAR_EDGE, AVATAR_EDGE, FilterType::Lanczos3))
}

pub fn process(bytes: &[u8], tz: Tz) -> Result<Processed, PipelineError> {
    let exif = read_exif(bytes);

    // Limits guard against decompression bombs in uploaded files.
    let mut limits = Limits::default();
    limits.max_image_width = Some(20_000);
    limits.max_image_height = Some(20_000);
    limits.max_alloc = Some(1024 * 1024 * 1024);
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| PipelineError::Decode(e.to_string()))?;
    reader.limits(limits);
    let decoded = reader
        .decode()
        .map_err(|e| PipelineError::Decode(e.to_string()))?;

    let img = apply_orientation(decoded, exif.orientation);
    let (width, height) = (img.width(), img.height());

    let display = if width.max(height) > DISPLAY_MAX_EDGE {
        img.resize(DISPLAY_MAX_EDGE, DISPLAY_MAX_EDGE, FilterType::Lanczos3)
    } else {
        img.clone()
    };
    let thumb = img.thumbnail(THUMB_MAX_EDGE, THUMB_MAX_EDGE);

    Ok(Processed {
        width,
        height,
        captured_at: exif
            .date_time_original
            .as_deref()
            .and_then(|dt| parse_exif_datetime(dt, exif.offset_time_original.as_deref(), tz)),
        display_jpeg: encode_jpeg(&display)?,
        thumb_jpeg: encode_jpeg(&thumb)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageFormat, Rgb, RgbImage};

    fn sample_jpeg(w: u32, h: u32) -> Vec<u8> {
        let img = RgbImage::from_fn(w, h, |x, y| Rgb([(x % 256) as u8, (y % 256) as u8, 128]));
        let mut out = Cursor::new(Vec::new());
        DynamicImage::ImageRgb8(img)
            .write_to(&mut out, ImageFormat::Jpeg)
            .unwrap();
        out.into_inner()
    }

    #[test]
    fn derives_display_and_thumbnail_within_bounds() {
        let processed = process(&sample_jpeg(3000, 2000), chrono_tz::Europe::Budapest).unwrap();
        assert_eq!((processed.width, processed.height), (3000, 2000));
        let display = image::load_from_memory(&processed.display_jpeg).unwrap();
        assert_eq!(display.width(), DISPLAY_MAX_EDGE);
        let thumb = image::load_from_memory(&processed.thumb_jpeg).unwrap();
        assert!(thumb.width() <= THUMB_MAX_EDGE && thumb.height() <= THUMB_MAX_EDGE);
        assert_eq!(processed.captured_at, None);
    }

    #[test]
    fn derived_copies_carry_no_exif() {
        let processed = process(&sample_jpeg(100, 50), chrono_tz::Europe::Budapest).unwrap();
        assert_eq!(read_exif(&processed.display_jpeg), ExifInfo::default());
    }

    #[test]
    fn small_images_are_not_upscaled() {
        let processed = process(&sample_jpeg(800, 600), chrono_tz::Europe::Budapest).unwrap();
        assert_eq!(
            image::load_from_memory(&processed.display_jpeg)
                .unwrap()
                .width(),
            800
        );
    }

    #[test]
    fn avatars_are_square_and_bounded() {
        for (w, h) in [(3000, 2000), (200, 900), (64, 64)] {
            let out = process_avatar(&sample_jpeg(w, h)).unwrap();
            let img = image::load_from_memory(&out).unwrap();
            assert_eq!((img.width(), img.height()), (AVATAR_EDGE, AVATAR_EDGE));
            assert_eq!(read_exif(&out), ExifInfo::default());
        }
        assert!(process_avatar(b"not an image").is_err());
    }

    #[test]
    fn garbage_is_a_decode_error() {
        assert!(matches!(
            process(b"definitely not an image", chrono_tz::Europe::Budapest),
            Err(PipelineError::Decode(_))
        ));
    }

    #[test]
    fn exif_datetime_with_and_without_offset() {
        let tz = chrono_tz::Europe::Budapest;
        assert_eq!(
            parse_exif_datetime("2026:09:10 14:30:00", Some("+02:00"), tz)
                .unwrap()
                .to_rfc3339(),
            "2026-09-10T12:30:00+00:00"
        );
        // No offset: Budapest summer time (+02:00) assumed.
        assert_eq!(
            parse_exif_datetime("2026:09:10 14:30:00", None, tz)
                .unwrap()
                .to_rfc3339(),
            "2026-09-10T12:30:00+00:00"
        );
        // Winter: +01:00.
        assert_eq!(
            parse_exif_datetime("2026:01:10 14:30:00", None, tz)
                .unwrap()
                .to_rfc3339(),
            "2026-01-10T13:30:00+00:00"
        );
        assert_eq!(parse_exif_datetime("0000:00:00 00:00:00", None, tz), None);
    }

    #[test]
    fn orientation_rotates_dimensions() {
        let img = DynamicImage::ImageRgb8(RgbImage::new(40, 10));
        let rotated = apply_orientation(img, Some(6));
        assert_eq!((rotated.width(), rotated.height()), (10, 40));
    }
}

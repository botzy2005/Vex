//! Pixel conversions: Windows DIB <-> RGBA, PNG <-> RGBA, and thumbnails. Pure, so they are tested here.

use std::io::Cursor;

/// Images larger than this many pixels are not kept (a decoded copy would exceed ~64 MB).
pub const MAX_PIXELS: usize = 16_000_000;
pub const THUMB_EDGE: u32 = 120;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawImage {
    pub width: u32,
    pub height: u32,
    /// 8-bit RGBA, row-major, top row first. `width * height * 4` bytes.
    pub rgba: Vec<u8>,
}

impl RawImage {
    pub fn new(width: u32, height: u32, rgba: Vec<u8>) -> Option<Self> {
        Self::with_limit(width, height, rgba, MAX_PIXELS)
    }

    /// Like `new` with a caller-chosen pixel limit (screenshots of large displays exceed the clipboard's).
    pub fn with_limit(width: u32, height: u32, rgba: Vec<u8>, max_pixels: usize) -> Option<Self> {
        let n = (width as usize).checked_mul(height as usize)?;
        (width > 0 && height > 0 && n <= max_pixels && rgba.len() == n * 4).then_some(Self { width, height, rgba })
    }
}

fn le32(b: &[u8], at: usize) -> Option<u32> {
    b.get(at..at + 4).map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}
fn le16(b: &[u8], at: usize) -> Option<u16> {
    b.get(at..at + 2).map(|s| u16::from_le_bytes([s[0], s[1]]))
}

/// Parse a packed DIB (`BITMAPINFOHEADER`/V4/V5 followed by pixels, as found on the clipboard).
/// Supports 24- and 32-bit images in the standard channel layout; anything else returns `None`
/// (those formats are rare, and the clipboard usually also offers PNG).
pub fn decode_dib(b: &[u8]) -> Option<RawImage> {
    let header = le32(b, 0)? as usize;
    if !matches!(header, 40 | 52 | 56 | 108 | 124) {
        return None;
    }
    let width = le32(b, 4)? as i32;
    let height = le32(b, 8)? as i32;
    let planes = le16(b, 12)?;
    let bpp = le16(b, 14)?;
    let compression = le32(b, 16)?;
    if planes != 1 || width <= 0 || height == 0 || !matches!(bpp, 24 | 32) || !matches!(compression, 0 | 3) {
        return None;
    }
    let top_down = height < 0;
    let (w, h) = (width as usize, height.unsigned_abs() as usize);
    if w.checked_mul(h)? > MAX_PIXELS {
        return None;
    }
    let mut offset = header;
    if compression == 3 {
        // Channel masks: after a 40-byte header they follow it; larger headers hold them inside.
        let (r, g, bl) = (le32(b, 40)?, le32(b, 44)?, le32(b, 48)?);
        if bpp != 32 || (r, g, bl) != (0x00FF_0000, 0x0000_FF00, 0x0000_00FF) {
            return None;
        }
        if header == 40 {
            offset += 12;
        }
    }
    let stride = (w * bpp as usize).div_ceil(32) * 4;
    let need = offset.checked_add(stride.checked_mul(h)?)?;
    if b.len() < need {
        return None;
    }
    let px = &b[offset..need];
    let mut rgba = vec![0u8; w * h * 4];
    let mut any_alpha = false;
    for y in 0..h {
        let src_y = if top_down { y } else { h - 1 - y };
        let row = &px[src_y * stride..src_y * stride + stride];
        for x in 0..w {
            let o = (y * w + x) * 4;
            if bpp == 32 {
                let p = &row[x * 4..x * 4 + 4];
                rgba[o..o + 4].copy_from_slice(&[p[2], p[1], p[0], p[3]]);
                any_alpha |= p[3] != 0;
            } else {
                let p = &row[x * 3..x * 3 + 3];
                rgba[o..o + 4].copy_from_slice(&[p[2], p[1], p[0], 255]);
            }
        }
    }
    // A 32-bit DIB without real alpha (the common CF_DIB case) leaves the fourth byte at 0: opaque.
    if bpp == 32 && !any_alpha {
        rgba.chunks_exact_mut(4).for_each(|p| p[3] = 255);
    }
    RawImage::new(w as u32, h as u32, rgba)
}

/// A packed 32-bit bottom-up DIB for `SetClipboardData(CF_DIB)`.
pub fn encode_dib(img: &RawImage) -> Vec<u8> {
    let (w, h) = (img.width as usize, img.height as usize);
    let mut out = Vec::with_capacity(40 + w * h * 4);
    out.extend_from_slice(&40u32.to_le_bytes());
    out.extend_from_slice(&(img.width as i32).to_le_bytes());
    out.extend_from_slice(&(img.height as i32).to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&32u16.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes()); // BI_RGB
    out.extend_from_slice(&((w * h * 4) as u32).to_le_bytes());
    out.extend_from_slice(&[0u8; 16]); // resolution and palette counts
    for y in (0..h).rev() {
        for x in 0..w {
            let p = &img.rgba[(y * w + x) * 4..(y * w + x) * 4 + 4];
            out.extend_from_slice(&[p[2], p[1], p[0], p[3]]);
        }
    }
    out
}

pub fn encode_png(img: &RawImage) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, img.width, img.height);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        // Lossless either way; "Fast" keeps a big screenshot from stalling the clipboard thread.
        enc.set_compression(png::Compression::Fast);
        let mut w = enc.write_header().map_err(|e| e.to_string())?;
        w.write_image_data(&img.rgba).map_err(|e| e.to_string())?;
    }
    Ok(out)
}

pub fn decode_png(bytes: &[u8]) -> Result<RawImage, String> {
    decode_png_limit(bytes, MAX_PIXELS)
}

pub fn decode_png_limit(bytes: &[u8], max_pixels: usize) -> Result<RawImage, String> {
    let mut dec = png::Decoder::new(Cursor::new(bytes));
    dec.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = dec.read_info().map_err(|e| e.to_string())?;
    let (iw, ih) = (reader.info().width as usize, reader.info().height as usize);
    if iw.checked_mul(ih).is_none_or(|n| n > max_pixels) {
        return Err("image is too large".into());
    }
    let mut buf = vec![0u8; reader.output_buffer_size().ok_or("image is too large")?];
    let info = reader.next_frame(&mut buf).map_err(|e| e.to_string())?;
    let (w, h) = (info.width as usize, info.height as usize);
    let src = &buf[..info.line_size * h];
    let mut rgba = Vec::with_capacity(w * h * 4);
    for y in 0..h {
        let row = &src[y * info.line_size..y * info.line_size + info.line_size];
        match info.color_type {
            png::ColorType::Rgba => rgba.extend_from_slice(&row[..w * 4]),
            png::ColorType::Rgb => row[..w * 3].chunks_exact(3).for_each(|p| rgba.extend_from_slice(&[p[0], p[1], p[2], 255])),
            png::ColorType::Grayscale => row[..w].iter().for_each(|&g| rgba.extend_from_slice(&[g, g, g, 255])),
            png::ColorType::GrayscaleAlpha => row[..w * 2].chunks_exact(2).for_each(|p| rgba.extend_from_slice(&[p[0], p[0], p[0], p[1]])),
            png::ColorType::Indexed => return Err("unsupported palette image".into()),
        }
    }
    RawImage::with_limit(w as u32, h as u32, rgba, max_pixels).ok_or_else(|| "invalid image".into())
}

/// Box-filter downscale so the longest edge is at most `edge` (never upscales).
pub fn thumbnail(img: &RawImage, edge: u32) -> RawImage {
    let longest = img.width.max(img.height);
    if longest <= edge {
        return img.clone();
    }
    let scale = f64::from(edge) / f64::from(longest);
    let tw = ((f64::from(img.width) * scale).round() as u32).max(1);
    let th = ((f64::from(img.height) * scale).round() as u32).max(1);
    let (w, h) = (img.width as usize, img.height as usize);
    let mut out = vec![0u8; (tw * th * 4) as usize];
    for ty in 0..th as usize {
        let y0 = ty * h / th as usize;
        let y1 = (((ty + 1) * h).div_ceil(th as usize)).clamp(y0 + 1, h);
        for tx in 0..tw as usize {
            let x0 = tx * w / tw as usize;
            let x1 = (((tx + 1) * w).div_ceil(tw as usize)).clamp(x0 + 1, w);
            let mut sum = [0u32; 4];
            for y in y0..y1 {
                for x in x0..x1 {
                    let p = &img.rgba[(y * w + x) * 4..(y * w + x) * 4 + 4];
                    for c in 0..4 {
                        sum[c] += u32::from(p[c]);
                    }
                }
            }
            let n = ((y1 - y0) * (x1 - x0)) as u32;
            let o = (ty * tw as usize + tx) * 4;
            for c in 0..4 {
                out[o + c] = (sum[c] / n) as u8;
            }
        }
    }
    RawImage { width: tw, height: th, rgba: out }
}

/// FNV-1a over arbitrary bytes (stable across runs and Rust versions, unlike `DefaultHasher`).
pub fn fnv64(parts: &[&[u8]]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for p in parts {
        for &b in *p {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    h
}

pub fn hash_image(img: &RawImage) -> u64 {
    fnv64(&[b"I", &img.width.to_le_bytes(), &img.height.to_le_bytes(), &img.rgba])
}

pub fn hash_text(t: &str) -> u64 {
    fnv64(&[b"T", t.as_bytes()])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gradient(w: u32, h: u32) -> RawImage {
        let mut v = Vec::new();
        for y in 0..h {
            for x in 0..w {
                v.extend_from_slice(&[(x * 255 / w.max(1)) as u8, (y * 255 / h.max(1)) as u8, ((x + y) % 256) as u8, 255]);
            }
        }
        RawImage::new(w, h, v).unwrap()
    }

    #[test]
    fn dib_roundtrip_is_lossless_including_alpha_and_orientation() {
        let mut img = gradient(7, 5); // 7 px wide: rows are not 4-byte aligned in a 24-bit image
        img.rgba[3] = 100; // a translucent pixel in the top-left corner
        let dib = encode_dib(&img);
        let back = decode_dib(&dib).unwrap();
        assert_eq!(back, img);
        // Bottom-up on the wire: the first stored row is the image's last row.
        assert_eq!(&dib[40..44], &[img.rgba[(4 * 7) * 4 + 2], img.rgba[(4 * 7) * 4 + 1], img.rgba[(4 * 7) * 4], 255]);
    }

    #[test]
    fn dib_24_bit_pads_rows_and_top_down_heights_work() {
        // 3x2, 24 bpp, stride 12 (9 bytes + 3 padding). Height -2 means top-down.
        let mut b = Vec::new();
        b.extend_from_slice(&40u32.to_le_bytes());
        b.extend_from_slice(&3i32.to_le_bytes());
        b.extend_from_slice(&(-2i32).to_le_bytes());
        b.extend_from_slice(&1u16.to_le_bytes());
        b.extend_from_slice(&24u16.to_le_bytes());
        b.extend_from_slice(&[0u8; 24]);
        for row in 0..2u8 {
            for x in 0..3u8 {
                b.extend_from_slice(&[row * 10 + x, 1, 2]); // B, G, R
            }
            b.extend_from_slice(&[0, 0, 0]);
        }
        let img = decode_dib(&b).unwrap();
        assert_eq!((img.width, img.height), (3, 2));
        assert_eq!(&img.rgba[0..4], &[2, 1, 0, 255]);
        assert_eq!(&img.rgba[12..16], &[2, 1, 10, 255]); // second row = second stored row
    }

    #[test]
    fn a_32_bit_dib_without_alpha_becomes_opaque() {
        let mut dib = encode_dib(&gradient(2, 2));
        for px in dib[40..].chunks_exact_mut(4) {
            px[3] = 0;
        }
        assert!(decode_dib(&dib).unwrap().rgba.chunks_exact(4).all(|p| p[3] == 255));
    }

    #[test]
    fn malformed_or_unsupported_dibs_are_rejected_not_panicked_on() {
        assert!(decode_dib(&[]).is_none());
        assert!(decode_dib(&[0u8; 39]).is_none());
        let good = encode_dib(&gradient(4, 4));
        assert!(decode_dib(&good[..good.len() - 1]).is_none()); // truncated pixels
        let mut huge = good.clone();
        huge[4..8].copy_from_slice(&i32::MAX.to_le_bytes());
        assert!(decode_dib(&huge).is_none());
        let mut bad_bpp = good.clone();
        bad_bpp[14] = 8;
        assert!(decode_dib(&bad_bpp).is_none());
        let mut zero = good;
        zero[4..8].copy_from_slice(&0i32.to_le_bytes());
        assert!(decode_dib(&zero).is_none());
    }

    #[test]
    fn png_roundtrip_preserves_every_pixel() {
        let mut img = gradient(31, 17);
        img.rgba[7] = 0; // transparent pixel
        let png = encode_png(&img).unwrap();
        assert_eq!(&png[..8], &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]);
        assert_eq!(decode_png(&png).unwrap(), img);
    }

    #[test]
    fn png_decode_handles_rgb_grayscale_and_rejects_junk() {
        let mut out = Vec::new();
        {
            let mut e = png::Encoder::new(&mut out, 2, 1);
            e.set_color(png::ColorType::Rgb);
            e.set_depth(png::BitDepth::Eight);
            e.write_header().unwrap().write_image_data(&[1, 2, 3, 4, 5, 6]).unwrap();
        }
        assert_eq!(decode_png(&out).unwrap().rgba, vec![1, 2, 3, 255, 4, 5, 6, 255]);
        let mut gray = Vec::new();
        {
            let mut e = png::Encoder::new(&mut gray, 2, 1);
            e.set_color(png::ColorType::Grayscale);
            e.set_depth(png::BitDepth::Eight);
            e.write_header().unwrap().write_image_data(&[10, 200]).unwrap();
        }
        assert_eq!(decode_png(&gray).unwrap().rgba, vec![10, 10, 10, 255, 200, 200, 200, 255]);
        assert!(decode_png(b"not a png").is_err());
        assert!(decode_png(&[]).is_err());
    }

    #[test]
    fn thumbnails_keep_aspect_never_upscale_and_average_colour() {
        let small = gradient(50, 30);
        assert_eq!(thumbnail(&small, 120), small);
        let wide = RawImage::new(400, 100, vec![200; 400 * 100 * 4]).unwrap();
        let t = thumbnail(&wide, 120);
        assert_eq!((t.width, t.height), (120, 30));
        assert!(t.rgba.iter().all(|&c| c == 200)); // a flat image stays flat
        let tall = RawImage::new(10, 1000, vec![9; 10 * 1000 * 4]).unwrap();
        let t = thumbnail(&tall, 120);
        assert_eq!((t.width, t.height), (1, 120));
    }

    #[test]
    fn raw_image_rejects_bad_sizes() {
        assert!(RawImage::new(0, 5, vec![]).is_none());
        assert!(RawImage::new(2, 2, vec![0; 15]).is_none());
        assert!(RawImage::new(5000, 5000, vec![]).is_none());
    }

    #[test]
    fn hashes_distinguish_content_and_kind() {
        let a = gradient(4, 4);
        let mut b = a.clone();
        b.rgba[0] ^= 1;
        assert_ne!(hash_image(&a), hash_image(&b));
        assert_eq!(hash_image(&a), hash_image(&a.clone()));
        assert_ne!(hash_text("x"), hash_text("y"));
        assert_ne!(hash_text("I"), fnv64(&[b"I"]));
    }
}

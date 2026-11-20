//! Saving formats. PNG is lossless (re-exported from the clipboard module); JPEG is encoded here.

use crate::clipboard::image::encode_png;
use crate::clipboard::RawImage;
use crate::settings::ImageFormat;

/// High enough that text and UI edges stay clean; JPEG is a choice the user makes, PNG is the default.
pub const JPEG_QUALITY: u8 = 92;

/// JPEG has no transparency: any see-through pixels are composited onto white, as image editors do.
pub fn encode_jpeg(img: &RawImage, quality: u8) -> Result<Vec<u8>, String> {
    let (w, h) = (u16::try_from(img.width), u16::try_from(img.height));
    let (Ok(w), Ok(h)) = (w, h) else {
        return Err("This image is too large for JPEG (the limit is 65,535 pixels on a side). Save it as PNG instead.".into());
    };
    let mut rgb = Vec::with_capacity(img.rgba.len() / 4 * 3);
    for p in img.rgba.chunks_exact(4) {
        let a = u32::from(p[3]);
        for &c in &p[..3] {
            rgb.push(((u32::from(c) * a + 255 * (255 - a) + 127) / 255) as u8);
        }
    }
    let mut out = Vec::new();
    jpeg_encoder::Encoder::new(&mut out, quality.clamp(1, 100))
        .encode(&rgb, w, h, jpeg_encoder::ColorType::Rgb)
        .map_err(|e| e.to_string())?;
    Ok(out)
}

pub fn encode_as(img: &RawImage, format: ImageFormat) -> Result<Vec<u8>, String> {
    match format {
        ImageFormat::Png => encode_png(img),
        ImageFormat::Jpeg => encode_jpeg(img, JPEG_QUALITY),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(w: u32, h: u32, px: [u8; 4]) -> RawImage {
        RawImage::new(w, h, px.iter().copied().cycle().take((w * h * 4) as usize).collect()).unwrap()
    }

    /// Width and height from the JPEG's start-of-frame marker.
    fn jpeg_size(b: &[u8]) -> Option<(u16, u16)> {
        let mut i = 2;
        while i + 9 < b.len() {
            if b[i] != 0xFF {
                return None;
            }
            let marker = b[i + 1];
            let len = u16::from_be_bytes([b[i + 2], b[i + 3]]) as usize;
            if (0xC0..=0xC3).contains(&marker) {
                return Some((u16::from_be_bytes([b[i + 7], b[i + 8]]), u16::from_be_bytes([b[i + 5], b[i + 6]])));
            }
            i += 2 + len;
        }
        None
    }

    #[test]
    fn jpeg_is_valid_and_keeps_its_size() {
        let out = encode_jpeg(&solid(37, 21, [200, 30, 60, 255]), JPEG_QUALITY).unwrap();
        assert_eq!(&out[..2], &[0xFF, 0xD8]);
        assert_eq!(&out[out.len() - 2..], &[0xFF, 0xD9]);
        assert_eq!(jpeg_size(&out), Some((37, 21)));
    }

    #[test]
    fn transparent_pixels_become_white_not_black() {
        let transparent = encode_jpeg(&solid(16, 16, [0, 0, 0, 0]), 100).unwrap();
        let white = encode_jpeg(&solid(16, 16, [255, 255, 255, 255]), 100).unwrap();
        let black = encode_jpeg(&solid(16, 16, [0, 0, 0, 255]), 100).unwrap();
        assert_eq!(transparent, white);
        assert_ne!(transparent, black);
    }

    #[test]
    fn png_round_trips_losslessly_through_encode_as() {
        let img = solid(9, 5, [1, 2, 3, 200]);
        let png = encode_as(&img, ImageFormat::Png).unwrap();
        assert_eq!(crate::clipboard::image::decode_png(&png).unwrap(), img);
        assert_eq!(&encode_as(&img, ImageFormat::Jpeg).unwrap()[..2], &[0xFF, 0xD8]);
    }

    #[test]
    fn oversized_jpeg_is_a_clear_error() {
        let img = RawImage::with_limit(70_000, 1, vec![0; 70_000 * 4], 100_000).unwrap();
        assert!(encode_jpeg(&img, 90).unwrap_err().contains("PNG"));
    }
}

//! Rectangles in physical (device) pixels, and the mapping from what the user drags on screen to them.

use crate::clipboard::RawImage;
use serde::{Deserialize, Serialize};

/// A capture may be larger than a clipboard image: an 8K display is about 33 million pixels.
pub const MAX_CAPTURE_PIXELS: usize = 40_000_000;
/// Smaller selections are treated as an accidental click.
pub const MIN_REGION: u32 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

impl Rect {
    pub fn new(x: i32, y: i32, w: u32, h: u32) -> Self {
        Self { x, y, w, h }
    }
    pub fn right(&self) -> i64 {
        i64::from(self.x) + i64::from(self.w)
    }
    pub fn bottom(&self) -> i64 {
        i64::from(self.y) + i64::from(self.h)
    }
    pub fn is_empty(&self) -> bool {
        self.w == 0 || self.h == 0
    }
    pub fn pixels(&self) -> u64 {
        u64::from(self.w) * u64::from(self.h)
    }
    pub fn contains(&self, px: i32, py: i32) -> bool {
        i64::from(px) >= i64::from(self.x) && i64::from(px) < self.right() && i64::from(py) >= i64::from(self.y) && i64::from(py) < self.bottom()
    }
    pub fn intersect(&self, o: &Rect) -> Option<Rect> {
        let x = i64::from(self.x).max(i64::from(o.x));
        let y = i64::from(self.y).max(i64::from(o.y));
        let r = self.right().min(o.right());
        let b = self.bottom().min(o.bottom());
        (r > x && b > y).then(|| Rect { x: x as i32, y: y as i32, w: (r - x) as u32, h: (b - y) as u32 })
    }
    /// The smallest rectangle holding both.
    pub fn union(&self, o: &Rect) -> Rect {
        let x = i64::from(self.x).min(i64::from(o.x));
        let y = i64::from(self.y).min(i64::from(o.y));
        let r = self.right().max(o.right());
        let b = self.bottom().max(o.bottom());
        Rect { x: x as i32, y: y as i32, w: (r - x) as u32, h: (b - y) as u32 }
    }
}

/// Index of the monitor holding the point; a point outside every monitor (pointer on a gap between
/// differently sized displays) goes to the nearest one. `None` only when there are no monitors.
pub fn monitor_at(monitors: &[Rect], x: i32, y: i32) -> Option<usize> {
    if let Some(i) = monitors.iter().position(|m| m.contains(x, y)) {
        return Some(i);
    }
    monitors
        .iter()
        .enumerate()
        .min_by_key(|(_, m)| {
            let dx = (i64::from(m.x) - i64::from(x)).max(0).max(i64::from(x) - (m.right() - 1));
            let dy = (i64::from(m.y) - i64::from(y)).max(0).max(i64::from(y) - (m.bottom() - 1));
            dx * dx + dy * dy
        })
        .map(|(i, _)| i)
}

/// A drag on one monitor's overlay window → a rectangle in desktop pixels.
///
/// `x0..y1` are page (CSS) pixels measured from the overlay's top-left corner, `scale` is physical
/// pixels per CSS pixel on that monitor (its display scaling), and `monitor` is the monitor's physical
/// rectangle. The selection is normalised (any drag direction), rounded outwards so the edge pixels the
/// user saw are included, and clamped to the monitor. A selection under `MIN_REGION` pixels is rejected.
pub fn from_drag(monitor: &Rect, scale: f64, x0: f64, y0: f64, x1: f64, y1: f64) -> Option<Rect> {
    if !(scale.is_finite() && scale > 0.0) || ![x0, y0, x1, y1].iter().all(|v| v.is_finite()) {
        return None;
    }
    let (w, h) = (f64::from(monitor.w), f64::from(monitor.h));
    let left = (x0.min(x1) * scale).floor().clamp(0.0, w);
    let top = (y0.min(y1) * scale).floor().clamp(0.0, h);
    let right = (x0.max(x1) * scale).ceil().clamp(0.0, w);
    let bottom = (y0.max(y1) * scale).ceil().clamp(0.0, h);
    let (rw, rh) = ((right - left) as u32, (bottom - top) as u32);
    (rw >= MIN_REGION && rh >= MIN_REGION).then(|| Rect { x: monitor.x + left as i32, y: monitor.y + top as i32, w: rw, h: rh })
}

/// The part of a window that can be captured: its rectangle limited to the desktop. A window that is
/// entirely off-screen or collapsed has nothing to capture.
pub fn plan_window_capture(window: &Rect, desktop: &Rect) -> Option<Rect> {
    window.intersect(desktop).filter(|r| r.w >= MIN_REGION && r.h >= MIN_REGION && r.pixels() <= MAX_CAPTURE_PIXELS as u64)
}

/// Cut `r` (desktop pixels) out of `src`, whose top-left corner sits at `origin` on the desktop.
/// The request is limited to what `src` covers; `None` when nothing is left.
pub fn crop(src: &RawImage, origin: (i32, i32), r: &Rect) -> Option<RawImage> {
    let bounds = Rect { x: origin.0, y: origin.1, w: src.width, h: src.height };
    let c = r.intersect(&bounds)?;
    let (sx, sy) = ((i64::from(c.x) - i64::from(origin.0)) as usize, (i64::from(c.y) - i64::from(origin.1)) as usize);
    let (w, h, sw) = (c.w as usize, c.h as usize, src.width as usize);
    let mut out = Vec::with_capacity(w * h * 4);
    for row in 0..h {
        let start = ((sy + row) * sw + sx) * 4;
        out.extend_from_slice(&src.rgba[start..start + w * 4]);
    }
    RawImage::with_limit(c.w, c.h, out, MAX_CAPTURE_PIXELS)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mon(x: i32, y: i32, w: u32, h: u32) -> Rect {
        Rect::new(x, y, w, h)
    }

    #[test]
    fn rect_basics() {
        let a = mon(0, 0, 100, 50);
        assert!(a.contains(0, 0) && a.contains(99, 49) && !a.contains(100, 0) && !a.contains(0, 50) && !a.contains(-1, 0));
        assert_eq!(a.intersect(&mon(50, 25, 100, 100)), Some(mon(50, 25, 50, 25)));
        assert_eq!(a.intersect(&mon(100, 0, 10, 10)), None, "touching edges share no pixels");
        assert_eq!(a.union(&mon(-10, 20, 20, 100)), mon(-10, 0, 110, 120));
        assert_eq!(mon(0, 0, 0, 5).pixels(), 0);
    }

    #[test]
    fn monitor_lookup_handles_negative_positions_and_gaps() {
        // A second display to the left of and above the primary, as Windows allows.
        let ms = [mon(0, 0, 1920, 1080), mon(-2560, -360, 2560, 1440)];
        assert_eq!(monitor_at(&ms, 10, 10), Some(0));
        assert_eq!(monitor_at(&ms, -1, 10), Some(1));
        assert_eq!(monitor_at(&ms, -2560, -360), Some(1));
        // Below the left display but left of the primary: nearest is the left one.
        assert_eq!(monitor_at(&ms, -100, 1200), Some(1));
        // Far right of everything: the primary.
        assert_eq!(monitor_at(&ms, 5000, 500), Some(0));
        assert_eq!(monitor_at(&[], 0, 0), None);
    }

    #[test]
    fn drag_maps_css_pixels_to_physical_with_scaling_in_any_direction() {
        let m = mon(0, 0, 3840, 2160); // a 4K display at 200 %
        let r = from_drag(&m, 2.0, 100.0, 50.0, 300.0, 150.0).unwrap();
        assert_eq!(r, mon(200, 100, 400, 200));
        // Dragging up and to the left gives the same rectangle.
        assert_eq!(from_drag(&m, 2.0, 300.0, 150.0, 100.0, 50.0), Some(r));
    }

    #[test]
    fn drag_on_a_second_monitor_is_offset_into_desktop_coordinates() {
        let m = mon(1920, -200, 2560, 1440); // 125 % display placed to the right and higher
        let r = from_drag(&m, 1.25, 10.0, 20.0, 110.0, 120.0).unwrap();
        assert_eq!((r.x, r.y), (1920 + 12, -200 + 25)); // 12.5 floors to 12, 25 exact
        assert_eq!((r.w, r.h), (138 - 12, 150 - 25)); // 137.5 ceils to 138
    }

    #[test]
    fn drag_is_clamped_to_the_monitor_and_tiny_or_bad_input_is_rejected() {
        let m = mon(0, 0, 1000, 800);
        assert_eq!(from_drag(&m, 1.0, -50.0, -50.0, 2000.0, 2000.0), Some(mon(0, 0, 1000, 800)));
        assert_eq!(from_drag(&m, 1.0, 10.0, 10.0, 12.0, 300.0), None, "2 px wide is a stray click");
        assert_eq!(from_drag(&m, 1.0, 5.0, 5.0, 5.0, 5.0), None);
        assert_eq!(from_drag(&m, 0.0, 0.0, 0.0, 100.0, 100.0), None);
        assert_eq!(from_drag(&m, f64::NAN, 0.0, 0.0, 100.0, 100.0), None);
        assert_eq!(from_drag(&m, 1.0, 0.0, f64::INFINITY, 100.0, 100.0), None);
        // Entirely outside the monitor collapses to nothing.
        assert_eq!(from_drag(&m, 1.0, 1500.0, 100.0, 1800.0, 400.0), None);
    }

    #[test]
    fn window_capture_is_limited_to_the_desktop() {
        let desktop = mon(-1920, 0, 3840, 1080);
        // A maximised window extends a few pixels past the screen edge.
        let w = mon(-1928, -8, 1936, 1096);
        assert_eq!(plan_window_capture(&w, &desktop), Some(mon(-1920, 0, 1928, 1080)));
        assert_eq!(plan_window_capture(&mon(5000, 0, 400, 400), &desktop), None);
        assert_eq!(plan_window_capture(&mon(0, 0, 2, 400), &desktop), None);
    }

    #[test]
    fn crop_takes_the_right_pixels_and_respects_the_origin() {
        // 4x3 image whose pixel value encodes its position; its top-left is at (-100, 50) on the desktop.
        let rgba: Vec<u8> = (0..12u8).flat_map(|i| [i, 0, 0, 255]).collect();
        let img = RawImage::new(4, 3, rgba).unwrap();
        let c = crop(&img, (-100, 50), &mon(-99, 51, 2, 2)).unwrap();
        assert_eq!((c.width, c.height), (2, 2));
        let reds: Vec<u8> = c.rgba.chunks(4).map(|p| p[0]).collect();
        assert_eq!(reds, vec![5, 6, 9, 10]);
        // A request that hangs over the edge is trimmed to what exists.
        let c = crop(&img, (-100, 50), &mon(-101, 49, 3, 3)).unwrap();
        assert_eq!((c.width, c.height), (2, 2));
        assert_eq!(c.rgba[0], 0);
        assert!(crop(&img, (-100, 50), &mon(0, 0, 10, 10)).is_none());
    }
}

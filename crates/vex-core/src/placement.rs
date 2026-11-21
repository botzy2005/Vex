//! Where the island window goes. Pure math over physical pixels so DPI
//! scaling and multi-monitor layouts (including monitors at negative
//! coordinates) can be unit-tested without a display.

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MonitorRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    /// OS scale factor (1.0 = 100 %, 1.5 = 150 %).
    pub scale: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// Choose a monitor: `wanted < 0` or out of range falls back to the primary.
pub fn pick_monitor(
    monitors: &[MonitorRect],
    primary_index: usize,
    wanted: i32,
) -> Option<(usize, MonitorRect)> {
    if monitors.is_empty() {
        return None;
    }
    let idx = if wanted >= 0 && (wanted as usize) < monitors.len() {
        wanted as usize
    } else if primary_index < monitors.len() {
        primary_index
    } else {
        0
    };
    Some((idx, monitors[idx]))
}

/// Top-centre placement of a window whose size is given in logical pixels.
/// `offset_x` and `margin_top` are logical pixels too.
pub fn island_rect(
    monitor: &MonitorRect,
    logical_w: f64,
    logical_h: f64,
    offset_x: i32,
    margin_top: i32,
) -> WindowRect {
    let scale = if monitor.scale.is_finite() && monitor.scale > 0.0 {
        monitor.scale
    } else {
        1.0
    };
    let width = ((logical_w * scale).round().max(1.0) as u32).min(monitor.width.max(1));
    let height = ((logical_h * scale).round().max(1.0) as u32).min(monitor.height.max(1));

    let centre = monitor.x as f64 + monitor.width as f64 / 2.0 + offset_x as f64 * scale;
    let mut x = (centre - width as f64 / 2.0).round() as i32;
    let min_x = monitor.x;
    let max_x = monitor.x + (monitor.width - width) as i32;
    x = x.clamp(min_x, max_x.max(min_x));

    let y = monitor.y + (margin_top as f64 * scale).round() as i32;
    WindowRect {
        x,
        y,
        width,
        height,
    }
}

fn sane_scale(monitor: &MonitorRect) -> f64 {
    if monitor.scale.is_finite() && monitor.scale > 0.0 {
        monitor.scale
    } else {
        1.0
    }
}

/// The furthest the island's centre may sit from the monitor's centre (logical px) while a window of
/// `logical_w` still fits on the monitor. Rounded down so the placed window never crosses an edge.
pub fn max_offset(monitor: &MonitorRect, logical_w: f64) -> i32 {
    let scale = sane_scale(monitor);
    let width = ((logical_w * scale).round().max(1.0) as u32).min(monitor.width.max(1));
    let spare = monitor.width.saturating_sub(width) as f64;
    (spare / 2.0 / scale).floor() as i32
}

/// Keep a requested offset inside the monitor (used while dragging and before saving a position).
pub fn clamp_offset(monitor: &MonitorRect, logical_w: f64, offset: i32) -> i32 {
    let m = max_offset(monitor, logical_w);
    offset.clamp(-m, m)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mon(x: i32, y: i32, w: u32, h: u32, scale: f64) -> MonitorRect {
        MonitorRect {
            x,
            y,
            width: w,
            height: h,
            scale,
        }
    }

    #[test]
    fn centres_at_100_percent() {
        let r = island_rect(&mon(0, 0, 1920, 1080, 1.0), 300.0, 80.0, 0, 8);
        assert_eq!(
            r,
            WindowRect {
                x: 810,
                y: 8,
                width: 300,
                height: 80
            }
        );
    }

    #[test]
    fn scales_size_margin_and_offset_at_150_percent() {
        let r = island_rect(&mon(0, 0, 2880, 1620, 1.5), 300.0, 80.0, 20, 8);
        assert_eq!(r.width, 450);
        assert_eq!(r.height, 120);
        assert_eq!(r.y, 12);
        // centre 1440 + 20*1.5 = 1470, minus half width 225
        assert_eq!(r.x, 1245);
    }

    #[test]
    fn handles_monitor_at_negative_coordinates() {
        let r = island_rect(&mon(-1920, -200, 1920, 1080, 1.0), 300.0, 80.0, 0, 10);
        assert_eq!(r.x, -1920 + 810);
        assert_eq!(r.y, -190);
    }

    #[test]
    fn offset_cannot_push_window_off_the_monitor() {
        let m = mon(0, 0, 1920, 1080, 1.0);
        let right = island_rect(&m, 300.0, 80.0, 2000, 0);
        assert_eq!(right.x, 1920 - 300);
        let left = island_rect(&m, 300.0, 80.0, -2000, 0);
        assert_eq!(left.x, 0);
    }

    #[test]
    fn window_is_never_larger_than_the_monitor() {
        let r = island_rect(&mon(0, 0, 800, 600, 1.0), 5000.0, 5000.0, 0, 0);
        assert_eq!((r.width, r.height), (800, 600));
    }

    #[test]
    fn bad_scale_falls_back_to_one() {
        let r = island_rect(&mon(0, 0, 1920, 1080, 0.0), 300.0, 80.0, 0, 0);
        assert_eq!(r.width, 300);
    }

    #[test]
    fn monitor_choice_falls_back_to_primary() {
        let ms = [mon(0, 0, 1920, 1080, 1.0), mon(1920, 0, 2560, 1440, 1.25)];
        assert_eq!(pick_monitor(&ms, 1, -1).unwrap().0, 1);
        assert_eq!(pick_monitor(&ms, 1, 0).unwrap().0, 0);
        assert_eq!(pick_monitor(&ms, 1, 9).unwrap().0, 1);
        assert_eq!(pick_monitor(&ms, 7, -1).unwrap().0, 0);
        assert!(pick_monitor(&[], 0, -1).is_none());
    }

    #[test]
    fn max_offset_is_half_the_spare_width() {
        // 1920 wide, 300 wide window: 810 either side of centre.
        assert_eq!(max_offset(&mon(0, 0, 1920, 1080, 1.0), 300.0), 810);
        // 150 %: window is 450 physical, spare 2430, half 1215 physical = 810 logical.
        assert_eq!(max_offset(&mon(0, 0, 2880, 1620, 1.5), 300.0), 810);
        // 200 %: window 1080 physical, spare 2760, half 1380 physical = 690 logical.
        assert_eq!(max_offset(&mon(0, 0, 3840, 2160, 2.0), 540.0), 1380 / 2);
        // A window as wide as the monitor cannot move at all.
        assert_eq!(max_offset(&mon(0, 0, 800, 600, 1.0), 800.0), 0);
        assert_eq!(max_offset(&mon(0, 0, 800, 600, 1.0), 5000.0), 0);
    }

    #[test]
    fn clamp_offset_is_symmetric_and_keeps_the_window_on_the_monitor() {
        let m = mon(0, 0, 1920, 1080, 1.0);
        assert_eq!(clamp_offset(&m, 300.0, 5000), 810);
        assert_eq!(clamp_offset(&m, 300.0, -5000), -810);
        assert_eq!(clamp_offset(&m, 300.0, 123), 123);
        assert_eq!(clamp_offset(&m, 300.0, 0), 0);
    }

    #[test]
    fn a_clamped_offset_never_places_the_window_past_an_edge() {
        // Many monitor sizes, scales, window widths and positions (including monitors at negative coordinates).
        for (mx, mw, scale) in [(0, 1920u32, 1.0), (0, 1366, 1.0), (-1920, 1920, 1.0), (1920, 2560, 1.25), (0, 2880, 1.5), (0, 3840, 2.0), (-2560, 2560, 1.75), (0, 5120, 1.0), (0, 1001, 1.0)] {
            let m = mon(mx, 0, mw, 1080, scale);
            for w in [176.0, 248.0, 300.0, 400.0, 568.0, 1000.0] {
                for off in [-5000, -1000, -333, -1, 0, 1, 77, 500, 5000] {
                    let o = clamp_offset(&m, w, off);
                    let r = island_rect(&m, w, 80.0, o, 0);
                    assert!(r.x >= mx, "left edge: monitor {mx}/{mw}@{scale} w {w} off {off}");
                    assert!(r.x + r.width as i32 <= mx + mw as i32, "right edge: monitor {mx}/{mw}@{scale} w {w} off {off} -> {r:?}");
                    assert_eq!(clamp_offset(&m, w, o), o, "clamping twice changes nothing");
                }
            }
        }
    }

    #[test]
    fn an_ultra_wide_monitor_can_be_dragged_to_both_ends() {
        let m = mon(0, 0, 5120, 1440, 1.0);
        assert_eq!(max_offset(&m, 300.0), 2410);
        let r = island_rect(&m, 300.0, 80.0, 2410, 0);
        assert_eq!(r.x + r.width as i32, 5120);
    }
}

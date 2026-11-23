//! Pure System Monitor rules: turn two raw readings into CPU %, memory %, and network speed.
//! The Windows side (`sysmon_windows.rs` in the shell) only reads the raw counters.

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Battery {
    /// 0..=100
    pub percent: u8,
    pub charging: bool,
    /// On mains power (may be true while not charging, e.g. full).
    pub plugged: bool,
    pub minutes_left: Option<u32>,
}

/// One raw reading. CPU times are cumulative (100 ns units, `kernel` includes `idle`); network bytes are cumulative.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Raw {
    pub at_ms: u64,
    pub idle: u64,
    pub kernel: u64,
    pub user: u64,
    pub mem_total: u64,
    pub mem_available: u64,
    pub net_in: u64,
    pub net_out: u64,
    pub battery: Option<Battery>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub available: bool,
    /// None until two readings exist.
    pub cpu_percent: Option<u8>,
    pub mem_percent: u8,
    pub mem_used_bytes: u64,
    pub mem_total_bytes: u64,
    pub down_bps: Option<u64>,
    pub up_bps: Option<u64>,
    pub battery: Option<Battery>,
    pub error: Option<String>,
}

impl Snapshot {
    pub fn unavailable(msg: &str) -> Self {
        Self {
            available: false,
            cpu_percent: None,
            mem_percent: 0,
            mem_used_bytes: 0,
            mem_total_bytes: 0,
            down_bps: None,
            up_bps: None,
            battery: None,
            error: Some(msg.to_string()),
        }
    }
}

fn pct(part: u64, whole: u64) -> u8 {
    if whole == 0 {
        return 0;
    }
    (((part as f64 / whole as f64) * 100.0).round() as i64).clamp(0, 100) as u8
}

pub fn compute(prev: Option<&Raw>, cur: &Raw) -> Snapshot {
    let used = cur.mem_total.saturating_sub(cur.mem_available);
    let mut s = Snapshot {
        available: true,
        cpu_percent: None,
        mem_percent: pct(used, cur.mem_total),
        mem_used_bytes: used,
        mem_total_bytes: cur.mem_total,
        down_bps: None,
        up_bps: None,
        battery: cur.battery.map(|b| Battery { percent: b.percent.min(100), ..b }),
        error: None,
    };
    if let Some(p) = prev {
        let total = cur.kernel.wrapping_add(cur.user).wrapping_sub(p.kernel.wrapping_add(p.user));
        let idle = cur.idle.wrapping_sub(p.idle);
        if total > 0 && total < u64::MAX / 2 {
            s.cpu_percent = Some(pct(total.saturating_sub(idle), total));
        }
        let dt = cur.at_ms.saturating_sub(p.at_ms);
        if dt > 0 {
            // wrapping_sub covers a counter that rolled over; a counter that went backwards (adapter reset) reads as 0 below.
            let rate = |c: u64, o: u64| if c >= o { (c - o).saturating_mul(1000) / dt } else { 0 };
            s.down_bps = Some(rate(cur.net_in, p.net_in));
            s.up_bps = Some(rate(cur.net_out, p.net_out));
        }
    }
    s
}

/// "1.2 MB/s", "340 KB/s", "0 B/s" (decimal units, like most network tools).
pub fn format_rate(bps: u64) -> String {
    let b = bps as f64;
    if bps >= 1_000_000_000 {
        format!("{:.1} GB/s", b / 1e9)
    } else if bps >= 1_000_000 {
        format!("{:.1} MB/s", b / 1e6)
    } else if bps >= 1_000 {
        format!("{:.0} KB/s", b / 1e3)
    } else {
        format!("{bps} B/s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(at: u64, idle: u64, kernel: u64, user: u64, ni: u64, no: u64) -> Raw {
        Raw { at_ms: at, idle, kernel, user, mem_total: 16_000, mem_available: 4_000, net_in: ni, net_out: no, battery: None }
    }

    #[test]
    fn first_reading_has_memory_but_no_rates() {
        let s = compute(None, &raw(0, 0, 0, 0, 0, 0));
        assert!(s.available && s.cpu_percent.is_none() && s.down_bps.is_none());
        assert_eq!((s.mem_percent, s.mem_used_bytes), (75, 12_000));
    }

    #[test]
    fn cpu_is_busy_share_of_the_interval() {
        // interval: kernel 800 (of which idle 600) + user 200 = 1000 total, 400 busy
        let a = raw(0, 100, 200, 100, 0, 0);
        let b = raw(2000, 700, 1000, 300, 0, 0);
        assert_eq!(compute(Some(&a), &b).cpu_percent, Some(40));
    }

    #[test]
    fn idle_machine_and_no_elapsed_time() {
        let a = raw(0, 0, 0, 0, 0, 0);
        assert_eq!(compute(Some(&a), &raw(1, 500, 500, 0, 0, 0)).cpu_percent, Some(0));
        assert_eq!(compute(Some(&a), &raw(1, 0, 0, 0, 0, 0)).cpu_percent, None);
    }

    #[test]
    fn network_rates_use_elapsed_time() {
        let a = raw(1000, 0, 0, 0, 1_000, 500);
        let b = raw(3000, 0, 0, 0, 3_001_000, 100_500);
        let s = compute(Some(&a), &b);
        assert_eq!((s.down_bps, s.up_bps), (Some(1_500_000), Some(50_000)));
    }

    #[test]
    fn a_counter_that_goes_backwards_reads_as_zero_not_huge() {
        let a = raw(0, 0, 0, 0, 9_000, 9_000);
        let s = compute(Some(&a), &raw(1000, 0, 0, 0, 10, 10));
        assert_eq!((s.down_bps, s.up_bps), (Some(0), Some(0)));
    }

    #[test]
    fn percent_is_clamped_and_empty_total_is_safe() {
        let mut r = raw(0, 0, 0, 0, 0, 0);
        r.mem_total = 0;
        assert_eq!(compute(None, &r).mem_percent, 0);
        r.mem_total = 100;
        r.mem_available = 500;
        assert_eq!(compute(None, &r).mem_percent, 0);
        r.battery = Some(Battery { percent: 200, charging: false, plugged: false, minutes_left: None });
        assert_eq!(compute(None, &r).battery.unwrap().percent, 100);
    }

    #[test]
    fn rate_formatting() {
        assert_eq!(format_rate(0), "0 B/s");
        assert_eq!(format_rate(999), "999 B/s");
        assert_eq!(format_rate(340_000), "340 KB/s");
        assert_eq!(format_rate(1_200_000), "1.2 MB/s");
        assert_eq!(format_rate(2_500_000_000), "2.5 GB/s");
    }
}

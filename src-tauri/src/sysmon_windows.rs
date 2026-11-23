//! Raw Windows readings for System Monitor: CPU times, memory, battery and network byte counters.

use std::time::{SystemTime, UNIX_EPOCH};
use vex_core::sysmon::{Battery, Raw};
use windows::Win32::Foundation::FILETIME;
use windows::Win32::NetworkManagement::IpHelper::{FreeMibTable, GetIfTable2, MIB_IF_TABLE2};
use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
use windows::Win32::System::Threading::GetSystemTimes;

fn ft(f: FILETIME) -> u64 {
    ((f.dwHighDateTime as u64) << 32) | f.dwLowDateTime as u64
}

fn network() -> (u64, u64) {
    // Hardware adapters that are up; software ones (loopback, VPN, virtual switches) would count the same bytes twice.
    let mut table: *mut MIB_IF_TABLE2 = std::ptr::null_mut();
    let (mut rx, mut tx) = (0u64, 0u64);
    unsafe {
        if GetIfTable2(&mut table).0 != 0 || table.is_null() {
            return (0, 0);
        }
        let n = (*table).NumEntries as usize;
        let rows = std::slice::from_raw_parts((*table).Table.as_ptr(), n);
        for r in rows {
            let up = r.OperStatus.0 == 1;
            let hardware = r.InterfaceAndOperStatusFlags._bitfield & 1 == 1;
            if up && hardware {
                rx = rx.wrapping_add(r.InOctets);
                tx = tx.wrapping_add(r.OutOctets);
            }
        }
        FreeMibTable(table as *const _);
    }
    (rx, tx)
}

pub fn read() -> Result<Raw, String> {
    let at_ms = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0);
    let (mut idle, mut kernel, mut user) = (FILETIME::default(), FILETIME::default(), FILETIME::default());
    unsafe { GetSystemTimes(Some(&mut idle), Some(&mut kernel), Some(&mut user)) }.map_err(|e| format!("CPU times: {e}"))?;
    let mut mem = MEMORYSTATUSEX { dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32, ..Default::default() };
    unsafe { GlobalMemoryStatusEx(&mut mem) }.map_err(|e| format!("Memory: {e}"))?;

    let mut ps = SYSTEM_POWER_STATUS::default();
    let battery = if unsafe { GetSystemPowerStatus(&mut ps) }.is_ok() && ps.BatteryLifePercent <= 100 && ps.BatteryFlag != 128 && ps.BatteryFlag != 255 {
        Some(Battery {
            percent: ps.BatteryLifePercent,
            charging: ps.BatteryFlag & 8 != 0,
            plugged: ps.ACLineStatus == 1,
            minutes_left: if ps.BatteryLifeTime != u32::MAX { Some(ps.BatteryLifeTime / 60) } else { None },
        })
    } else {
        None
    };

    let (net_in, net_out) = network();
    Ok(Raw {
        at_ms,
        idle: ft(idle),
        kernel: ft(kernel),
        user: ft(user),
        mem_total: mem.ullTotalPhys,
        mem_available: mem.ullAvailPhys,
        net_in,
        net_out,
        battery,
    })
}

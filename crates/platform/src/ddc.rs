//! Talking to a monitor over DDC/CI, the same channel its own buttons use.
//!
//! Every call here crosses the display cable and can take from 40 ms to a few
//! seconds, so the tray app only calls these from its worker thread.

use std::ptr::null_mut;

use windows_sys::Win32::Devices::Display::{
    CapabilitiesRequestAndCapabilitiesReply, DestroyPhysicalMonitor, GetCapabilitiesStringLength,
    GetNumberOfPhysicalMonitorsFromHMONITOR, GetPhysicalMonitorsFromHMONITOR, GetVCPFeatureAndVCPFeatureReply,
    PHYSICAL_MONITOR, SetVCPFeature,
};
use windows_sys::Win32::Foundation::HANDLE;

/// VCP code for the monitor's power mode (MCCS "Power mode").
pub const VCP_POWER: u8 = 0xD6;
pub const POWER_ON: u32 = 0x01;
/// DPM off: the panel sleeps like when the PC sends no signal, but the
/// monitor keeps listening for the command that wakes it.
pub const POWER_OFF: u32 = 0x04;

/// One physical monitor's DDC/CI handle; closed on drop.
pub struct Physical(HANDLE);

// The handle is a plain kernel handle, used from one thread at a time.
unsafe impl Send for Physical {}

impl Drop for Physical {
    fn drop(&mut self) {
        unsafe { DestroyPhysicalMonitor(self.0) };
    }
}

impl Physical {
    /// The physical monitor behind a Windows monitor handle.
    pub fn open(hmonitor: usize) -> Option<Self> {
        let hmon = hmonitor as _;
        let mut n = 0u32;
        if unsafe { GetNumberOfPhysicalMonitorsFromHMONITOR(hmon, &mut n) } == 0 || n == 0 {
            return None;
        }
        let mut list: Vec<PHYSICAL_MONITOR> = vec![unsafe { std::mem::zeroed() }; n as usize];
        if unsafe { GetPhysicalMonitorsFromHMONITOR(hmon, n, list.as_mut_ptr()) } == 0 {
            return None;
        }
        // Mirrored screens share a monitor handle; the first one is the one we want.
        let mut it = list.into_iter().map(|p| Self(p.hPhysicalMonitor));
        let first = it.next()?;
        drop(it.collect::<Vec<_>>());
        (!first.0.is_null()).then_some(first)
    }

    /// The monitor's self-description, e.g. `(prot(monitor)type(lcd)vcp(10 12 D6(01 04 05)))`.
    pub fn capabilities(&self) -> Option<String> {
        let mut len = 0u32;
        if unsafe { GetCapabilitiesStringLength(self.0, &mut len) } == 0 || len == 0 {
            return None;
        }
        let mut buf = vec![0u8; len as usize];
        if unsafe { CapabilitiesRequestAndCapabilitiesReply(self.0, buf.as_mut_ptr(), len) } == 0 {
            return None;
        }
        let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        Some(String::from_utf8_lossy(&buf[..end]).into_owned())
    }

    /// Current value of a VCP code.
    pub fn get(&self, code: u8) -> Option<u32> {
        let (mut cur, mut max) = (0u32, 0u32);
        let ok = unsafe { GetVCPFeatureAndVCPFeatureReply(self.0, code, null_mut(), &mut cur, &mut max) };
        (ok != 0).then_some(cur)
    }

    pub fn set(&self, code: u8, value: u32) -> bool {
        unsafe { SetVCPFeature(self.0, code, value) != 0 }
    }
}

/// Does a capabilities string list `code` among its VCP features?
pub fn supports(caps: &str, code: u8) -> bool {
    vcp_list(caps).iter().any(|(c, _)| *c == code)
}

/// The values a capabilities string allows for `code`, e.g. `[01, 04, 05]`
/// for `D6(01 04 05)`. Empty when it lists the code without values.
pub fn allowed_values(caps: &str, code: u8) -> Vec<u8> {
    vcp_list(caps).into_iter().find(|(c, _)| *c == code).map(|(_, v)| v).unwrap_or_default()
}

/// Which power mode to use for "off". Standby (04) is preferred: monitors
/// keep listening in it, so they wake when asked. Some only offer 05 ("off",
/// like their power button).
pub fn off_value(caps: Option<&str>) -> u32 {
    let allowed = caps.map(|c| allowed_values(c, VCP_POWER)).unwrap_or_default();
    if allowed.is_empty() || allowed.contains(&0x04) {
        POWER_OFF
    } else if allowed.contains(&0x05) {
        0x05
    } else {
        allowed.iter().copied().find(|&v| v != 0x01).map_or(POWER_OFF, u32::from)
    }
}

/// The VCP codes in a capabilities string.
pub fn vcp_codes(caps: &str) -> Vec<u8> {
    vcp_list(caps).into_iter().map(|(c, _)| c).collect()
}

/// The VCP codes in a capabilities string, each with the values listed in its
/// brackets (the `01 04 05` in `D6(01 04 05)`).
fn vcp_list(caps: &str) -> Vec<(u8, Vec<u8>)> {
    let lower = caps.to_ascii_lowercase();
    let bytes = lower.as_bytes();
    // "vcp(" but not "vcpname(".
    let Some(start) =
        lower.match_indices("vcp(").map(|(i, _)| i).find(|&i| i == 0 || !bytes[i - 1].is_ascii_alphabetic())
    else {
        return Vec::new();
    };
    let mut list: Vec<(u8, Vec<u8>)> = Vec::new();
    let mut depth = 0;
    let mut token = String::new();
    // Codes sit at depth 0; their values at depth 1.
    let flush = |token: &mut String, list: &mut Vec<(u8, Vec<u8>)>, depth: i32| {
        if let Ok(v) = u8::from_str_radix(token, 16) {
            match depth {
                0 => list.push((v, Vec::new())),
                1 => {
                    if let Some((_, values)) = list.last_mut() {
                        values.push(v);
                    }
                }
                _ => {}
            }
        }
        token.clear();
    };
    for &c in &bytes[start + 4..] {
        match c {
            b'(' => {
                flush(&mut token, &mut list, depth);
                depth += 1;
            }
            b')' => {
                flush(&mut token, &mut list, depth);
                if depth == 0 {
                    break;
                }
                depth -= 1;
            }
            c if c.is_ascii_hexdigit() => {
                token.push(c as char);
                // Some monitors run codes together: "vcp(0210D6)".
                if token.len() == 2 {
                    flush(&mut token, &mut list, depth);
                }
            }
            _ => flush(&mut token, &mut list, depth),
        }
    }
    list
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_power_mode() {
        let caps = "(prot(monitor)type(LCD)model(U2723QE)cmds(01 02 03 07 0C E3 F3)vcp(02 04 05 08 10 12 14(01 04 05 06 08 0B) 16 18 1A 52 60(0F 11 1B) AA(01 02) D6(01 04 05) DC(00 03 05) DF E0 E1 E2(00 1D 02 04 0E 12 14 23 24) F0(00 0C) F1 F2 FD)mccs_ver(2.1)mswhql(1))";
        assert!(supports(caps, VCP_POWER));
        assert!(supports(caps, 0x10));
        // A value inside brackets isn't a code.
        assert!(!supports(caps, 0x0B));
    }

    #[test]
    fn missing_power_mode() {
        assert!(!supports("(prot(monitor)vcp(10 12 60(0F 11)))", VCP_POWER));
        assert!(!supports("(prot(monitor)type(lcd))", VCP_POWER));
        assert!(!supports("", VCP_POWER));
    }

    #[test]
    fn ignores_vcpname() {
        assert!(!supports("(vcpname(D6(Power)) vcp(10 12))", VCP_POWER));
        assert!(supports("(vcpname(10(Brightness)) vcp(10 D6))", VCP_POWER));
    }

    #[test]
    fn picks_the_off_value_each_monitor_offers() {
        let msi = "(prot(monitor)type(lcd)G274QPXcmds(01 02 03 07 0C E3 F3)vcp(02 04 05 08 10 12 14(05 08 0B 0C) 16 18 1A 52 60( 11 12 0F 10) AA(01 02) AC AE B2 B6 C6 C8 C9 D6(05) DC(00 02 03 05 ) DF FD)mccs_ver(2.1)mswhql(1))";
        assert_eq!(allowed_values(msi, VCP_POWER), vec![0x05]);
        assert_eq!(off_value(Some(msi)), 0x05);
        assert_eq!(off_value(Some("vcp(10 D6(01 04 05))")), 0x04);
        assert_eq!(off_value(Some("vcp(10 D6)")), 0x04);
        assert_eq!(off_value(None), 0x04);
    }

    #[test]
    fn codes_run_together() {
        assert_eq!(vcp_codes("vcp(0210D6)"), vec![0x02, 0x10, 0xD6]);
    }
}

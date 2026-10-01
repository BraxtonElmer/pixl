//! Which monitors are connected, where Windows puts them, and how to tell them
//! apart from one boot to the next.

use std::ptr::{null, null_mut};

use serde::{Deserialize, Serialize};
use windows_sys::Win32::Devices::Display::{
    DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME, DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME, DISPLAYCONFIG_MODE_INFO,
    DISPLAYCONFIG_OUTPUT_TECHNOLOGY_DISPLAYPORT_EMBEDDED, DISPLAYCONFIG_OUTPUT_TECHNOLOGY_DISPLAYPORT_EXTERNAL,
    DISPLAYCONFIG_OUTPUT_TECHNOLOGY_DISPLAYPORT_USB_TUNNEL, DISPLAYCONFIG_OUTPUT_TECHNOLOGY_DVI,
    DISPLAYCONFIG_OUTPUT_TECHNOLOGY_HD15, DISPLAYCONFIG_OUTPUT_TECHNOLOGY_HDMI,
    DISPLAYCONFIG_OUTPUT_TECHNOLOGY_INDIRECT_VIRTUAL, DISPLAYCONFIG_OUTPUT_TECHNOLOGY_INDIRECT_WIRED,
    DISPLAYCONFIG_OUTPUT_TECHNOLOGY_INTERNAL, DISPLAYCONFIG_OUTPUT_TECHNOLOGY_LVDS,
    DISPLAYCONFIG_OUTPUT_TECHNOLOGY_MIRACAST, DISPLAYCONFIG_OUTPUT_TECHNOLOGY_UDI_EMBEDDED, DISPLAYCONFIG_PATH_INFO,
    DISPLAYCONFIG_SOURCE_DEVICE_NAME, DISPLAYCONFIG_TARGET_DEVICE_NAME, DISPLAYCONFIG_VIDEO_OUTPUT_TECHNOLOGY,
    DisplayConfigGetDeviceInfo, GetDisplayConfigBufferSizes, QDC_ONLY_ACTIVE_PATHS, QueryDisplayConfig,
};
use windows_sys::Win32::Foundation::{BOOL, LPARAM, RECT};
use windows_sys::Win32::Graphics::Gdi::{EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFOEXW};
use windows_sys::Win32::System::Registry::{
    HKEY, HKEY_LOCAL_MACHINE, KEY_READ, RegCloseKey, RegOpenKeyExW, RegQueryValueExW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::MONITORINFOF_PRIMARY;

use crate::edid;
use crate::wide::{from_wide, to_wide};

/// A rectangle in Windows' virtual-desktop pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.w && y < self.y + self.h
    }
}

#[derive(Clone, Debug)]
pub struct Display {
    /// Stable identity across reboots and ports: EDID maker + model + serial.
    pub id: String,
    pub name: String,
    pub px: Rect,
    pub primary: bool,
    /// The number Windows shows for this display (from `\\.\DISPLAYn`).
    pub number: u32,
    /// Diagonal in inches, when the monitor reports its size.
    pub inches: Option<f64>,
    /// "HDMI", "DisplayPort", "Built-in"...
    pub connection: String,
    /// Windows' handle for this monitor; valid until the next display change.
    pub hmonitor: usize,
}

struct Gdi {
    device: String,
    px: Rect,
    primary: bool,
    hmonitor: usize,
}

struct Target {
    gdi_device: String,
    friendly: String,
    device_path: String,
    tech: DISPLAYCONFIG_VIDEO_OUTPUT_TECHNOLOGY,
}

pub fn detect() -> Vec<Display> {
    let targets = targets();
    let mut out: Vec<Display> = gdi_monitors()
        .into_iter()
        .map(|g| {
            let t = targets.iter().find(|t| t.gdi_device.eq_ignore_ascii_case(&g.device));
            let e = t.and_then(|t| read_edid(&t.device_path)).and_then(|b| edid::parse(&b));
            let name = e
                .as_ref()
                .and_then(|e| e.name.clone())
                .or_else(|| t.map(|t| t.friendly.clone()).filter(|s| !s.is_empty()))
                .unwrap_or_else(|| g.device.trim_start_matches(r"\\.\").to_string());
            let id = match (&e, t) {
                (Some(e), Some(t)) => format!(
                    "{}{:04X}-{}",
                    e.manufacturer,
                    e.product,
                    e.serial.clone().unwrap_or_else(|| instance_of(&t.device_path))
                ),
                (None, Some(t)) => instance_of(&t.device_path),
                _ => g.device.clone(),
            };
            let inches = e.as_ref().and_then(|e| e.size_mm).map(|(w, h)| w.hypot(h) / 25.4);
            let tech = t.map_or(-1, |t| t.tech);
            Display {
                id,
                name,
                px: g.px,
                primary: g.primary,
                number: g.device.trim_start_matches(r"\\.\DISPLAY").parse().unwrap_or(0),
                inches,
                connection: connection_name(tech).to_string(),
                hmonitor: g.hmonitor,
            }
        })
        .collect();

    // Two identical monitors without serials would share an id; keep them apart.
    for i in 1..out.len() {
        let mut n = 2;
        while out[..i].iter().any(|d| d.id == out[i].id) {
            out[i].id = format!("{}#{n}", out[i].id.split('#').next().unwrap_or_default());
            n += 1;
        }
    }
    out.sort_by_key(|d| d.number);
    out
}

fn connection_name(tech: DISPLAYCONFIG_VIDEO_OUTPUT_TECHNOLOGY) -> &'static str {
    match tech {
        DISPLAYCONFIG_OUTPUT_TECHNOLOGY_HDMI => "HDMI",
        DISPLAYCONFIG_OUTPUT_TECHNOLOGY_DISPLAYPORT_EXTERNAL => "DisplayPort",
        DISPLAYCONFIG_OUTPUT_TECHNOLOGY_DISPLAYPORT_USB_TUNNEL => "USB-C",
        DISPLAYCONFIG_OUTPUT_TECHNOLOGY_DVI => "DVI",
        DISPLAYCONFIG_OUTPUT_TECHNOLOGY_HD15 => "VGA",
        DISPLAYCONFIG_OUTPUT_TECHNOLOGY_MIRACAST => "Wireless",
        DISPLAYCONFIG_OUTPUT_TECHNOLOGY_INDIRECT_WIRED => "USB display",
        DISPLAYCONFIG_OUTPUT_TECHNOLOGY_INDIRECT_VIRTUAL => "Virtual",
        t if is_internal(t) => "Built-in",
        _ => "",
    }
}

fn is_internal(tech: DISPLAYCONFIG_VIDEO_OUTPUT_TECHNOLOGY) -> bool {
    matches!(
        tech,
        DISPLAYCONFIG_OUTPUT_TECHNOLOGY_INTERNAL
            | DISPLAYCONFIG_OUTPUT_TECHNOLOGY_DISPLAYPORT_EMBEDDED
            | DISPLAYCONFIG_OUTPUT_TECHNOLOGY_UDI_EMBEDDED
            | DISPLAYCONFIG_OUTPUT_TECHNOLOGY_LVDS
    )
}

/// `\\?\DISPLAY#GSM5B09#5&2a1b&0&UID4352#{guid}` → `GSM5B09-5&2a1b&0&UID4352`
fn instance_of(device_path: &str) -> String {
    let parts: Vec<&str> = device_path.split('#').collect();
    match parts.as_slice() {
        [_, model, instance, ..] => format!("{model}-{instance}"),
        _ => device_path.to_string(),
    }
}

fn gdi_monitors() -> Vec<Gdi> {
    unsafe extern "system" fn cb(hmon: HMONITOR, _: HDC, _: *mut RECT, data: LPARAM) -> BOOL {
        let list = unsafe { &mut *(data as *mut Vec<Gdi>) };
        let mut info: MONITORINFOEXW = unsafe { std::mem::zeroed() };
        info.monitorInfo.cbSize = size_of::<MONITORINFOEXW>() as u32;
        if unsafe { GetMonitorInfoW(hmon, (&raw mut info).cast()) } != 0 {
            let r = info.monitorInfo.rcMonitor;
            list.push(Gdi {
                device: from_wide(&info.szDevice),
                px: Rect { x: r.left, y: r.top, w: r.right - r.left, h: r.bottom - r.top },
                primary: info.monitorInfo.dwFlags & MONITORINFOF_PRIMARY != 0,
                hmonitor: hmon as usize,
            });
        }
        1
    }
    let mut list: Vec<Gdi> = Vec::new();
    unsafe { EnumDisplayMonitors(null_mut(), null(), Some(cb), (&raw mut list) as LPARAM) };
    list
}

fn targets() -> Vec<Target> {
    let (mut np, mut nm) = (0u32, 0u32);
    if unsafe { GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &mut np, &mut nm) } != 0 {
        return Vec::new();
    }
    let mut paths: Vec<DISPLAYCONFIG_PATH_INFO> = vec![unsafe { std::mem::zeroed() }; np as usize];
    let mut modes: Vec<DISPLAYCONFIG_MODE_INFO> = vec![unsafe { std::mem::zeroed() }; nm as usize];
    let rc = unsafe {
        QueryDisplayConfig(QDC_ONLY_ACTIVE_PATHS, &mut np, paths.as_mut_ptr(), &mut nm, modes.as_mut_ptr(), null_mut())
    };
    if rc != 0 {
        return Vec::new();
    }
    paths.truncate(np as usize);

    paths
        .iter()
        .filter_map(|p| {
            let mut src: DISPLAYCONFIG_SOURCE_DEVICE_NAME = unsafe { std::mem::zeroed() };
            src.header.r#type = DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME;
            src.header.size = size_of::<DISPLAYCONFIG_SOURCE_DEVICE_NAME>() as u32;
            src.header.adapterId = p.sourceInfo.adapterId;
            src.header.id = p.sourceInfo.id;
            if unsafe { DisplayConfigGetDeviceInfo(&mut src.header) } != 0 {
                return None;
            }
            let mut tgt: DISPLAYCONFIG_TARGET_DEVICE_NAME = unsafe { std::mem::zeroed() };
            tgt.header.r#type = DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME;
            tgt.header.size = size_of::<DISPLAYCONFIG_TARGET_DEVICE_NAME>() as u32;
            tgt.header.adapterId = p.targetInfo.adapterId;
            tgt.header.id = p.targetInfo.id;
            if unsafe { DisplayConfigGetDeviceInfo(&mut tgt.header) } != 0 {
                return None;
            }
            Some(Target {
                gdi_device: from_wide(&src.viewGdiDeviceName),
                friendly: from_wide(&tgt.monitorFriendlyDeviceName),
                device_path: from_wide(&tgt.monitorDevicePath),
                tech: p.targetInfo.outputTechnology,
            })
        })
        .collect()
}

/// The EDID blob Windows cached for this monitor in the registry.
fn read_edid(device_path: &str) -> Option<Vec<u8>> {
    let parts: Vec<&str> = device_path.trim_start_matches(r"\\?\").split('#').collect();
    let [kind, model, instance, ..] = parts.as_slice() else { return None };
    let key = to_wide(&format!(r"SYSTEM\CurrentControlSet\Enum\{kind}\{model}\{instance}\Device Parameters"));
    let mut hkey: HKEY = null_mut();
    if unsafe { RegOpenKeyExW(HKEY_LOCAL_MACHINE, key.as_ptr(), 0, KEY_READ, &mut hkey) } != 0 {
        return None;
    }
    let name = to_wide("EDID");
    let mut buf = vec![0u8; 512];
    let mut len = buf.len() as u32;
    let rc = unsafe { RegQueryValueExW(hkey, name.as_ptr(), null(), null_mut(), buf.as_mut_ptr(), &mut len) };
    unsafe { RegCloseKey(hkey) };
    (rc == 0).then(|| {
        buf.truncate(len as usize);
        buf
    })
}

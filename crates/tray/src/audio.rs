//! Which apps are making sound right now, from Windows' per-app volume meters
//! (what the volume mixer shows). Lets "Stay on while something is playing"
//! notice protected streaming video, which screen capture shows as black.

use std::collections::HashSet;

use pixl_platform::apps;
use windows::Win32::Media::Audio::Endpoints::IAudioMeterInformation;
use windows::Win32::Media::Audio::{
    AudioSessionStateActive, DEVICE_STATE_ACTIVE, IAudioSessionControl2, IAudioSessionManager2, IMMDeviceEnumerator,
    MMDeviceEnumerator, eRender,
};
use windows::Win32::System::Com::{CLSCTX_ALL, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx};
use windows::core::Interface;

/// Quieter than this counts as silence (a paused player keeps its stream open).
const SILENCE: f32 = 0.001;

/// Call once on the thread that will ask.
pub fn init() {
    let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
}

/// Lower-case exe names of apps playing audible sound on any output device.
pub fn sounding() -> HashSet<String> {
    let mut names = HashSet::new();
    let _ = collect(&mut names);
    names
}

fn collect(names: &mut HashSet<String>) -> windows::core::Result<()> {
    unsafe {
        let devices: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        let list = devices.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE)?;
        for d in 0..list.GetCount()? {
            let Ok(manager) = list.Item(d).and_then(|dev| dev.Activate::<IAudioSessionManager2>(CLSCTX_ALL, None))
            else {
                continue;
            };
            let sessions = manager.GetSessionEnumerator()?;
            for s in 0..sessions.GetCount()? {
                let Ok(session) = sessions.GetSession(s) else { continue };
                if session.GetState().ok() != Some(AudioSessionStateActive) {
                    continue;
                }
                let Ok(control) = session.cast::<IAudioSessionControl2>() else { continue };
                // S_OK means it is the system sounds session (dings, not media).
                if control.IsSystemSoundsSession().0 == 0 {
                    continue;
                }
                let loud = session.cast::<IAudioMeterInformation>().and_then(|m| m.GetPeakValue()).unwrap_or(0.0);
                if loud > SILENCE
                    && let Some(name) = control.GetProcessId().ok().and_then(apps::exe_name)
                {
                    names.insert(name.to_lowercase());
                }
            }
        }
    }
    Ok(())
}

/// Every audio session with its state and loudness, for `--watch`.
pub fn describe() -> String {
    let mut lines = Vec::new();
    let result: windows::core::Result<()> = (|| unsafe {
        let devices: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        let list = devices.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE)?;
        lines.push(format!("{} output devices", list.GetCount()?));
        for d in 0..list.GetCount()? {
            let manager = list.Item(d)?.Activate::<IAudioSessionManager2>(CLSCTX_ALL, None)?;
            let sessions = manager.GetSessionEnumerator()?;
            for s in 0..sessions.GetCount()? {
                let session = sessions.GetSession(s)?;
                let control = session.cast::<IAudioSessionControl2>()?;
                let pid = control.GetProcessId().unwrap_or(0);
                let peak = session.cast::<IAudioMeterInformation>().and_then(|m| m.GetPeakValue()).unwrap_or(-1.0);
                lines.push(format!(
                    "  device {d}: {} state {:?} peak {peak:.4}",
                    apps::exe_name(pid).unwrap_or_else(|| format!("pid {pid}")),
                    session.GetState().map(|s| s.0).unwrap_or(-1),
                ));
            }
        }
        Ok(())
    })();
    if let Err(e) = result {
        lines.push(format!("error: {e}"));
    }
    lines.join("\n")
}

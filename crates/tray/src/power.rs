//! A worker thread for DDC/CI. Commands to a monitor can take seconds to
//! answer, so they never run on the thread that watches the mouse.
//! Finished work is posted back to the tray window as `MSG_POWER_DONE`.

use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

use pixl_platform::ddc::{self, POWER_OFF, POWER_ON, Physical, VCP_POWER};
use windows_sys::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_APP};

pub const MSG_POWER_DONE: u32 = WM_APP + 40;

pub enum Job {
    /// Ask whether the monitor can be powered off over DDC/CI.
    Probe { id: String, hmonitor: usize },
    /// Send the off or on command.
    Set { id: String, hmonitor: usize, value: u32 },
    /// Read the power mode back.
    Read { id: String, hmonitor: usize },
}

pub enum Done {
    Probed { id: String, supported: bool, note: &'static str, off_value: u32 },
    Set { id: String, on: bool, ok: bool },
    Read { id: String, on: Option<bool> },
}

pub struct Worker {
    jobs: Sender<Job>,
    done: Receiver<Done>,
}

impl Worker {
    pub fn start(hwnd: usize) -> Self {
        let (jobs, rx) = mpsc::channel::<Job>();
        let (tx, done) = mpsc::channel::<Done>();
        std::thread::spawn(move || {
            for job in rx {
                let result = run(job);
                if tx.send(result).is_err() {
                    break;
                }
                unsafe { PostMessageW(hwnd as _, MSG_POWER_DONE, 0, 0) };
            }
        });
        Self { jobs, done }
    }

    pub fn send(&self, job: Job) {
        let _ = self.jobs.send(job);
    }

    /// Everything finished since the last call.
    pub fn finished(&self) -> Vec<Done> {
        self.done.try_iter().collect()
    }
}

fn run(job: Job) -> Done {
    match job {
        Job::Probe { id, hmonitor } => {
            let (supported, note, off_value) = probe(hmonitor);
            Done::Probed { id, supported, note, off_value }
        }
        Job::Set { id, hmonitor, value } => {
            let on = value == POWER_ON;
            let ok = Physical::open(hmonitor).is_some_and(|m| {
                // A busy monitor sometimes drops a command; one retry covers it.
                m.set(VCP_POWER, value) || {
                    std::thread::sleep(Duration::from_millis(150));
                    m.set(VCP_POWER, value)
                }
            });
            Done::Set { id, on, ok }
        }
        Job::Read { id, hmonitor } => {
            let on = Physical::open(hmonitor).and_then(|m| m.get(VCP_POWER)).map(|v| v == POWER_ON);
            Done::Read { id, on }
        }
    }
}

/// Can this monitor be powered off, why not, and which value means "off" for it.
fn probe(hmonitor: usize) -> (bool, &'static str, u32) {
    let Some(m) = Physical::open(hmonitor) else {
        return (
            false,
            "Windows can't reach this screen's controls. Docks, adapters and TVs often block them.",
            POWER_OFF,
        );
    };
    // Reading the power mode is quick and the surest sign. Monitors need a
    // moment between commands, and some drop the first one after waking.
    let answers = (0..3).any(|attempt| {
        if attempt > 0 {
            std::thread::sleep(Duration::from_millis(200));
        }
        m.get(VCP_POWER).is_some()
    });
    std::thread::sleep(Duration::from_millis(100));
    let caps = m.capabilities();
    let off = ddc::off_value(caps.as_deref());
    if answers || caps.as_deref().is_some_and(|c| ddc::supports(c, VCP_POWER)) {
        // 05 is the monitor's own power button: it drops off the cable and only
        // the button brings it back, so it's no use for turning off when idle.
        if off == 0x05 {
            let note = "This screen can only switch itself fully off, like its power button, and then only the \
                        button turns it back on. Pixl covers it with black instead.";
            return (false, note, off);
        }
        return (true, "", off);
    }
    let note = if caps.is_some() {
        "This screen doesn't offer a power command to the PC."
    } else {
        "This screen doesn't answer the PC. Turning on DDC/CI in its own menu may help, then press Check again."
    };
    (false, note, off)
}

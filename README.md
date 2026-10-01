# Pixl

OLED screens burn in when they show the same thing for hours. Windows can
only turn every display off at once, after the whole PC has been idle. Pixl
turns each screen black on its own schedule: the one you stepped away from,
the one you haven't looked at in half an hour. It comes back the moment you
return. Your PC, your apps and your other screens keep running.

- **A rule per screen.** Turn a screen off when the PC is idle, or when you
  haven't used *that* screen, even while you're busy on another one. Pick
  the time with a slider or type anything from 10 seconds to 24 hours.
- **Black, not powered off.** On OLED, black pixels are switched off, so a
  black screen protects the panel like turning it off, and it can't leave a
  monitor stuck dark or shuffle your windows. The cursor is hidden too.
- **Knows when you're watching.** A playing video, a fullscreen game or an
  app on your keep-on list keeps its screen on, even without touching the
  mouse.
- **Gentle.** Screens fade out over 5 seconds first; touch anything to
  cancel. Any mouse movement or key brings them back, or only moving the
  cursor onto that screen, if you prefer.
- **Light.** The part that runs in the tray is under 500 KB, uses about
  1.5 MB of memory and no measurable CPU. The settings window closes
  completely when you close it.

Shortcuts work anywhere and can be changed in the settings:
**Ctrl+Alt+O** turns every screen off now, **Ctrl+Alt+W** wakes them all,
**Ctrl+Alt+P** pauses or resumes Pixl.

## Install

Download the installer from the
[latest release](https://github.com/BraxtonElmer/pixl/releases/latest) and
run it. No admin rights are needed.

## Build

Needs Rust and Node.js.

```powershell
./scripts/build.ps1              # ready-to-run copy in dist/
./scripts/build.ps1 -Installer   # also builds the Windows installer
```

Then run `dist/Pixl.exe`, or the installer from `target/release/bundle/nsis/`.

| Folder | What's in it |
| --- | --- |
| `crates/core` | The idle rules: when each screen fades, turns off and wakes. No Windows code, fully unit tested. |
| `crates/platform` | Monitor detection, settings file, running apps, start with Windows. |
| `crates/tray` | `Pixl.exe`: tray icon, watching for input, the black screens. |
| `settings` | The settings window (Tauri + Svelte). |

## License

Pixl is free software, licensed under the [GNU General Public License v3.0](LICENSE).

# quor-presence

Small Rust CLI that sets a custom Discord Rich Presence: any activity name, text, images and a timer that starts "N hours ago".

Talks to the local Discord client over its IPC pipe. No network access of its own, nothing is sent anywhere except to your own Discord app.

## Window version

`quor-presence-gui.exe` is the same thing with a small window (egui): pick a profile, edit texts and the hours field (`+1`, `+10` buttons), press **Start**. **Apply** restarts the timer from the entered number, **Save profile** writes `profiles.toml` next to the exe. Closing the window removes the status.

## Setup (CLI)

1. Create an application at <https://discord.com/developers/applications>. Its name is what friends will see ("Playing ...").
2. (Optional) upload images in *Rich Presence → Art Assets* and use their keys as `large_image` / `small_image`.
3. Copy the Application ID.

```powershell
quor-presence init          # creates profiles.toml from the example
# put your Application ID into profiles.toml
quor-presence list
quor-presence start vscode
quor-presence start vscode --hours 20 --details "Debugging for hours"
```

The status stays while the program runs. `Ctrl+C` removes it. If Discord is not running yet or restarts, the tool waits and reconnects by itself.

## Profiles

```toml
[vscode]
app_id = "your application id"
name = "Visual Studio Code"      # optional, see note below
details = "Editing main.rs"
state = "Workspace: quor-presence"
hours = 20                       # timer shows 20:00:00 and keeps counting
large_image = "vscode"
large_text = "Visual Studio Code"
```

`hours` and `--hours` accept decimals (`0.5`), max one year. Text fields are 2..128 characters (Discord limit).

## Notes

- Only the timer inside the status is customised. Discord's own per-game playtime counters are not touched.
- `name` overrides the activity name when the Discord client honours it; otherwise the name of your Discord application is shown.
- `profiles.toml` is git-ignored, keep your own IDs out of the repo.

## Build

```powershell
cargo build --release
cargo test
```

## Roadmap

- tray icon and autostart
- keep the running timer when only the texts are changed
- start automatically when a chosen process appears (`sysinfo`)

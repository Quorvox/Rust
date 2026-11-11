# winlite-tuner

Small Windows CLI that turns off telemetry the **reversible** way: every change is backed up, can be previewed with `--dry-run` and rolled back with one command.

Written in Rust. No network access, no telemetry of its own.

## What it does

| id | what | how |
|---|---|---|
| `telemetry-hosts` | blocks Microsoft telemetry domains | adds a marked block to `hosts` (`0.0.0.0 domain`) |
| `telemetry-diagtrack` | disables the DiagTrack service | sets `Start=4` in the registry and stops the service |
| `telemetry-policy` | sets `AllowTelemetry=0` policy | HKLM policy key; Home/Pro treat it as the minimum level |

Update domains (`windowsupdate`, `delivery`) are deliberately **not** blocked, so Windows Update keeps working.

## Usage

```powershell
winlite-tuner list
winlite-tuner status
winlite-tuner apply --dry-run      # show what would change, touch nothing
winlite-tuner apply                # needs an elevated terminal
winlite-tuner apply --only telemetry-hosts
winlite-tuner revert               # puts everything back
```

## Safety

- Previous values are saved to `%ProgramData%\winlite-tuner\state.json`, the original `hosts` to `hosts.original`.
- The `hosts` block is wrapped in markers. Revert removes only that block, so your own lines stay untouched. Apply followed by revert gives back the exact same bytes (CRLF and a missing final newline included).
- A broken block (begin without end) stops the tool instead of guessing.
- `apply` / `revert` ask for admin rights up front instead of failing halfway.

## Build

```powershell
cargo build --release
cargo test
```

Tests run the hosts logic against temp files, never against the real system.

## Notes

- Windows only.
- On Home/Pro editions Windows still sends the minimum required diagnostic data, whatever the policy says. This tool reduces telemetry, it does not remove it.

## Roadmap

- more modules (autostart audit, gaming-related registry tweaks, honestly benchmarked)
- tweak definitions in TOML instead of code
- small GUI

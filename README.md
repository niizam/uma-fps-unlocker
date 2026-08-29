# uma-fps-unlocker
### **Overview**
- Standalone FPS unlocker for Umamusume (Windows). Supports both JP and Global releases.
- Includes a DLL that detours Unity’s `Application.set_targetFrameRate` and `QualitySettings.set_vSyncCount`, plus GUI and CLI injectors.
- No existing game files are modified; the injector adds two small configuration files and loads the DLL into the running game process.
<img width="346" height="193" alt="{E5624AEB-E00C-4FCE-9CE3-15A31CDF85AD}" src="https://github.com/user-attachments/assets/f5218897-f44b-477f-9dca-c7d5a48734bc" />

### **Quick Start (GUI)**
- Download `uma-fps-unlocker.zip` from [Releases](https://github.com/niizam/uma-fps-unlocker/releases)
- Extract the `uma-fps-unlocker.zip` and open the folder extracted.
- Start the game first.
- Run `uma_unlock_gui.exe` and choose:
  - Server: JP or Global
  - FPS: e.g., 60, 90, 120, 144, 165, 240
  - VSync: on or off
- Click Inject. A message box confirms success.
- You can quit the injector after.

### **Build**
- Requirements:
  - Rust (MSVC toolchain) on Windows
- Commands:
  - `cargo build --release`
  - Built artifacts appear in `target/release`.

### **Publishing a Release**
- Push a version tag matching `v*`, for example:
  ```sh
  git tag v0.1.0
  git push origin v0.1.0
  ```
- GitHub Actions builds the x64 Windows DLL and injectors, packages them with
  `fps.sh`, `README.md`, and `LICENSE`, then creates the matching GitHub Release.
- Re-running the workflow for an existing tag replaces the ZIP asset when the
  repository does not enforce immutable releases.

### **Linux / Steam Proton**
The Windows injectors can be driven from Linux through Steam Proton. A POSIX
`fps.sh` wrapper launches the game with the original Proton command and then
injects into the running game through the **same prefix** using Proton's
`runinprefix` verb. No proxy DLL and no `LD_PRELOAD` are used.

**Placement**
- You need the prebuilt **x64 Windows** artifacts: `uma_unlocker.dll` plus the
  injector for your server (`uma_unlock_jp.exe` or `uma_unlock_global.exe`).
  These are produced by a Windows `cargo build --release` (see Build above).
- Put `fps.sh`, `uma_unlocker.dll`, and the selected injector `.exe` in the
  game's install directory, next to the game executable. `fps.sh` resolves the
  artifacts relative to its own location, not the current working directory.

**Steam launch options**
```
sh ./fps.sh %command%
```
`%command%` expands to the full Proton command; `fps.sh` preserves every
argument and passes them to the game unchanged.

**Configuration (environment variables)**

| Variable | Default | Description |
|---|---|---|
| `FPS` | `120` | Target FPS (positive integer). |
| `VSYNC` | `off` | `on` or `off`. |
| `SERVER` | `global` | `jp` or `global`; selects the injector `.exe`. |
| `WAIT_SECONDS` | `60` | How long the injector waits for the game process to appear (positive integer). |
| `PROTON_PATH` | *(auto)* | Absolute path to the Proton script. When unset, `fps.sh` scans the `%command%` argv for an argument whose basename is `proton`. |

Example (set these in the game's launch options or environment):
```
FPS=144 VSYNC=off SERVER=jp WAIT_SECONDS=90 sh ./fps.sh %command%
```

**How it works**
- `fps.sh` finds the Proton script (from `PROTON_PATH` or by scanning the
  `%command%` argv for a `proton` basename) and validates it is executable.
- It starts the original full command in the background and remembers its PID.
- It runs `"$proton" runinprefix "$injector" --fps "$FPS" --vsync "$VSYNC" --wait "$WAIT_SECONDS"`,
  inheriting Steam's `STEAM_COMPAT_*` environment so the injector runs in the
  same prefix as the game.
- The injector polls for the game process (up to `WAIT_SECONDS`) and injects
  `uma_unlocker.dll` with `CreateRemoteThread(LoadLibraryW)`.
- `INT`/`TERM` are forwarded to the Proton game and injector wrappers. If injection
  fails, `fps.sh` reports it but does **not** terminate a playable game; it
  returns the game command's exit status after the game exits.

**Troubleshooting (Linux)**
- `fps.sh: could not find the Proton script...`
  - Set `PROTON_PATH` to the absolute path of the Proton script (e.g.
    `.../steamapps/common/Proton 9.0 (Beta)/proton`).
- `fps.sh: uma_unlocker.dll not found...` / `uma_unlock_jp.exe not found...`
  - Ensure the prebuilt x64 Windows artifacts sit next to `fps.sh`.
- Injection fails but the game runs
  - The game may not have reached the process name yet; raise `WAIT_SECONDS`.
  - Confirm the selected `SERVER` matches the game you launched.
- `cargo test` on Linux
  - Native host tests are not supported: the `minhook` 0.2 dependency rejects
    non-Windows targets at build time. Build and test on Windows, or run the
    shell test suite (`sh tests/fps_test.sh`) which exercises `fps.sh` with
    deterministic fakes and needs no Proton or game.

### **How It Works**
- The injector writes the FPS/VSync files into the game folder and loads the DLL into the game process using `LoadLibraryW`.
- The DLL resolves and hooks Unity’s `Application.set_targetFrameRate(Int32)` and optionally `QualitySettings.set_vSyncCount(Int32)` via `il2cpp_resolve_icall` and applies the configured values.

### **Troubleshooting**
- Game not found
  - Ensure the game is running. JP names: `umamusume.exe` or `umamusumeprettyderby_jpn.exe`. Global name: `umamusumeprettyderby.exe`.
- Injection failed
  - Try running the injector as Administrator.
  - Ensure `uma_unlocker.dll` is in the same directory as the injector EXE.
  - Antivirus/EDR may block process injection; add an allow‑rule for the tools if necessary.
- FPS still capped
  - Set `--vsync off` (or uncheck VSync in GUI). Your display driver may also enforce sync.
  - Some in‑game scenes may limit FPS internally.

### **Notes**
- These tools interact with a running process and may be flagged by security software; use at your own discretion.
- Close and relaunch the game to revert to default behavior.

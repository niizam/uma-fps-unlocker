# uma-fps-unlocker
### **Overview**
- Standalone FPS unlocker for Umamusume (Windows). Supports both JP and Global releases.
- Includes a DLL that detours Unity’s `Application.set_targetFrameRate` and `QualitySettings.set_vSyncCount`, plus GUI and CLI injectors.
- No game files are modified; the injector loads the DLL into the running game process and applies the FPS and VSync settings.
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


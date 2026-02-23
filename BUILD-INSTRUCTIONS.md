# LCARS OS — Build Instructions for macOS (Apple Silicon)

## Prerequisites

You need three things installed. If you don't have them yet, each one is a single command.

### 1. Xcode Command Line Tools
Open Terminal and run:
```
xcode-select --install
```
Click "Install" in the dialog that appears. This gives you the C/C++ compiler that Rust and Tauri need.

### 2. Rust
```
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```
When prompted, choose the default installation (just press Enter). Then restart your terminal or run:
```
source $HOME/.cargo/env
```

### 3. Node.js
If you don't have it, the easiest way on Mac:
```
brew install node
```
Or download from https://nodejs.org (LTS version).

If you don't have Homebrew: https://brew.sh

---

## Building LCARS OS

### Step 1: Open Terminal and navigate to the project
```
cd /path/to/lcars-os-app
```
(Replace with wherever you saved the `lcars-os-app` folder.)

### Step 2: Install JavaScript dependencies
```
npm install
```

### Step 3: Run in development mode (to test it)
```
npm run dev
```
This compiles the Rust backend (takes ~1 minute the first time), then opens LCARS OS in a native window. You'll see real CPU, memory, disk, and network data from your Mac displayed in the LCARS panels.

Press `Cmd+Q` to quit.

### Step 4: Build the final .app
```
npm run build
```
This produces a native macOS `.app` bundle in:
```
src-tauri/target/release/bundle/macos/LCARS OS.app
```

### Step 5: Install it
Drag `LCARS OS.app` to your Applications folder. You can now launch it from Spotlight or your Dock like any other Mac app.

---

## What You'll See

When you launch the app, it opens as a **borderless fullscreen-ready window** (no browser chrome, no address bar — just pure LCARS). The Operations panel will show your Mac's actual:

- **CPU usage** (real-time, per-core data from your M-series chip)
- **Memory** (actual GB used / total, not simulated)
- **Network** (real bandwidth in Mb/s)
- **Disk** (mapped to the "Warp Core" power readout — because free disk space is power)
- **Uptime** (your Mac's real system uptime)

All other panels (Tactical, Science, Comms, etc.) work exactly as they do in the browser version.

---

## Making It Full Screen

- Press `Cmd+Ctrl+F` to toggle macOS native fullscreen
- Or in the `tauri.conf.json` file, change `"fullscreen": false` to `"fullscreen": true` to always launch in fullscreen

---

## Generating App Icons

The project expects icons in `src-tauri/icons/`. To generate them from a single image:

1. Create or find a 1024x1024 PNG of your LCARS logo
2. Install the Tauri icon generator:
   ```
   cargo install tauri-cli
   ```
3. Run:
   ```
   cargo tauri icon /path/to/your-icon.png
   ```
This creates all the required icon sizes automatically.

---

## Troubleshooting

**"Rust not found" error:**
Make sure you restarted your terminal after installing Rust, or run `source $HOME/.cargo/env`.

**First build is slow (~2-5 minutes):**
This is normal — Rust is compiling the sysinfo library and Tauri framework. Subsequent builds are much faster.

**Window appears but panels are blank:**
The boot animation takes ~4 seconds. Wait for it to complete.

**"Permission denied" when accessing system info:**
On macOS, Tauri apps may need to be approved in System Settings > Privacy & Security the first time you run them. Just click "Open Anyway."

---

## Project Structure

```
lcars-os-app/
├── package.json              # Node.js config + build scripts
├── BUILD-INSTRUCTIONS.md     # This file
├── src/
│   └── index.html            # The LCARS interface (HTML/CSS/JS)
└── src-tauri/
    ├── Cargo.toml             # Rust dependencies
    ├── tauri.conf.json        # Tauri app configuration
    ├── build.rs               # Tauri build script
    └── src/
        └── main.rs            # Rust backend (system metrics)
```

## Customizing

- **Change colors/layout:** Edit `src/index.html` — all CSS is in the `<style>` block at the top
- **Add new panels:** Search for `panel-container` in the HTML and add a new section following the same pattern
- **Change window size:** Edit `width` and `height` in `src-tauri/tauri.conf.json`
- **Add new system metrics:** Add a new `#[tauri::command]` function in `src-tauri/src/main.rs` and call it from JS with `window.__TAURI__.core.invoke('your_function_name')`

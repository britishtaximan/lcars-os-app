// LCARS OS — Tauri v2 Backend
// lcars-os-tauri/src/main.rs

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::Serialize;
use std::process::Command;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use sysinfo::System;
use tauri::State;

struct AppState {
    sys: Mutex<System>,
    comms_cache: Mutex<Option<(Instant, CommsStatus)>>,
}

#[derive(Serialize)]
struct SystemMetrics {
    cpu_usage: f32,
    cpu_brand: String,
    memory_total: f64,
    memory_used: f64,
    memory_usage_percent: f64,
    disk_total: u64,
    disk_used: u64,
    disk_usage_percent: f64,
    network_rx_bytes: u64,
    network_tx_bytes: u64,
    uptime_seconds: u64,
    battery_percent: f64,
    battery_charging: bool,
    thermal_pressure: String,
}

#[derive(Serialize, Clone)]
struct CommsStatus {
    wifi: String,
    bluetooth_enabled: bool,
    bluetooth_devices: Vec<String>,
    volume_percent: i32,
    brightness_percent: i32,
}

#[derive(Serialize)]
struct FileEntry {
    name: String,
    path: String,
    is_dir: bool,
    size: u64,
}

#[tauri::command]
fn get_system_metrics(state: State<AppState>) -> SystemMetrics {
    let mut sys = state.sys.lock().unwrap();
    sys.refresh_cpu_usage();
    sys.refresh_memory();
    let cpu_usage = sys.cpus().iter().map(|c| c.cpu_usage()).sum::<f32>() / sys.cpus().len().max(1) as f32;
    let cpu_brand = sys.cpus().first().map(|c| c.brand().to_string()).unwrap_or_else(|| "Unknown".to_string());
    let memory_total = sys.total_memory() as f64 / 1_073_741_824.0;
    let memory_used = sys.used_memory() as f64 / 1_073_741_824.0;
    let memory_usage_percent = if memory_total > 0.0 { (memory_used / memory_total) * 100.0 } else { 0.0 };
    let uptime_seconds = System::uptime();
    drop(sys);

    // Disk
    let mut disk_total: u64 = 0;
    let mut disk_used: u64 = 0;
    let disks = sysinfo::Disks::new_with_refreshed_list();
    for disk in disks.list() {
        if disk.mount_point() == std::path::Path::new("/") {
            disk_total = disk.total_space();
            disk_used = disk_total - disk.available_space();
            break;
        }
    }
    let disk_usage_percent = if disk_total > 0 { (disk_used as f64 / disk_total as f64) * 100.0 } else { 0.0 };

    // Network
    let mut network_rx_bytes: u64 = 0;
    let mut network_tx_bytes: u64 = 0;
    let nets = sysinfo::Networks::new_with_refreshed_list();
    for (_name, net) in nets.iter() {
        network_rx_bytes += net.total_received();
        network_tx_bytes += net.total_transmitted();
    }

    let (battery_percent, battery_charging) = get_battery_info();
    let thermal_pressure = get_thermal_pressure();

    SystemMetrics {
        cpu_usage, cpu_brand, memory_total, memory_used, memory_usage_percent,
        disk_total, disk_used, disk_usage_percent, network_rx_bytes, network_tx_bytes,
        uptime_seconds, battery_percent, battery_charging, thermal_pressure,
    }
}

fn get_battery_info() -> (f64, bool) {
    let output = Command::new("pmset").arg("-g").arg("batt").output();
    if let Ok(out) = output {
        let text = String::from_utf8_lossy(&out.stdout);
        for line in text.lines() {
            if line.contains('%') {
                let parts: Vec<&str> = line.split('\t').collect();
                if parts.len() >= 2 {
                    let info = parts[1];
                    let pct_str: String = info.chars().take_while(|c| c.is_ascii_digit()).collect();
                    if let Ok(pct) = pct_str.parse::<f64>() {
                        let charging = info.contains("charging") && !info.contains("discharging");
                        return (pct, charging);
                    }
                }
            }
        }
    }
    (-1.0, false)
}

fn get_thermal_pressure() -> String {
    let output = Command::new("pmset").arg("-g").arg("therm").output();
    if let Ok(out) = output {
        let text = String::from_utf8_lossy(&out.stdout);
        if text.contains("Normal") { return "NOMINAL".to_string(); }
        if text.contains("Moderate") { return "MODERATE".to_string(); }
        if text.contains("Heavy") { return "HEAVY".to_string(); }
        if text.contains("Critical") { return "CRITICAL".to_string(); }
    }
    "NOMINAL".to_string()
}

#[tauri::command]
async fn get_comms_status(state: State<'_, AppState>) -> Result<CommsStatus, String> {
    // Check cache (30s TTL)
    {
        let cache = state.comms_cache.lock().unwrap();
        if let Some((timestamp, ref cached)) = *cache {
            if timestamp.elapsed() < Duration::from_secs(30) {
                return Ok(cached.clone());
            }
        }
    }

    // Fetch on background thread (system_profiler is slow)
    let status = tauri::async_runtime::spawn_blocking(|| {
        let wifi = get_wifi_info();
        let (bluetooth_enabled, bluetooth_devices) = get_bluetooth_info();
        let volume_percent = get_volume();
        let brightness_percent = get_brightness();
        CommsStatus { wifi, bluetooth_devices, bluetooth_enabled, volume_percent, brightness_percent }
    }).await.map_err(|e| format!("Comms thread error: {}", e))?;

    // Update cache
    {
        let mut cache = state.comms_cache.lock().unwrap();
        *cache = Some((Instant::now(), status.clone()));
    }
    Ok(status)
}

fn get_wifi_info() -> String {
    let output = Command::new("system_profiler").arg("SPAirPortDataType").output();
    if let Ok(out) = output {
        let text = String::from_utf8_lossy(&out.stdout);
        let mut in_current_network = false;
        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed == "Current Network Information:" { in_current_network = true; continue; }
            if in_current_network {
                if trimmed.ends_with(':') && !trimmed.contains("Current Network") {
                    return trimmed.trim_end_matches(':').to_string();
                }
            }
        }
    }
    "Not Connected".to_string()
}

fn get_bluetooth_info() -> (bool, Vec<String>) {
    let output = Command::new("system_profiler").arg("SPBluetoothDataType").output();
    if let Ok(out) = output {
        let text = String::from_utf8_lossy(&out.stdout);
        let mut enabled = false;
        let mut devices: Vec<String> = Vec::new();
        let mut in_connected = false;
        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.contains("State:") && trimmed.contains("On") { enabled = true; }
            if trimmed.contains("Bluetooth:") && trimmed.contains("On") { enabled = true; }
            if trimmed == "Connected:" || trimmed.starts_with("Connected:") { in_connected = true; continue; }
            if in_connected {
                if trimmed.is_empty() || trimmed.starts_with("Not Connected:") { in_connected = false; continue; }
                if trimmed.ends_with(':') && !trimmed.contains("Address") && !trimmed.contains("Services") {
                    let name = trimmed.trim_end_matches(':').to_string();
                    if !name.is_empty() && name != "Yes" && name != "No" { devices.push(name); }
                }
            }
        }
        return (enabled, devices);
    }
    (false, Vec::new())
}

fn get_volume() -> i32 {
    let output = Command::new("osascript").arg("-e").arg("output volume of (get volume settings)").output();
    if let Ok(out) = output {
        let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if let Ok(vol) = text.parse::<i32>() { return vol; }
    }
    -1
}

fn get_brightness() -> i32 {
    let output = Command::new("bash").arg("-c").arg("ioreg -c AppleBacklightDisplay -r | grep -i brightness | head -1").output();
    if let Ok(out) = output {
        let text = String::from_utf8_lossy(&out.stdout);
        for part in text.split('=') {
            let trimmed = part.trim().trim_end_matches('}').trim();
            if let Ok(val) = trimmed.parse::<f64>() {
                if val <= 1.0 { return (val * 100.0) as i32; }
                else if val <= 1024.0 { return ((val / 1024.0) * 100.0) as i32; }
            }
        }
    }
    -1
}

#[tauri::command]
fn list_directory(path: String) -> Result<Vec<FileEntry>, String> {
    let dir = if path.is_empty() { dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("/")) } else { std::path::PathBuf::from(&path) };
    let entries = std::fs::read_dir(&dir).map_err(|e| format!("Cannot read directory: {}", e))?;
    let mut result: Vec<FileEntry> = Vec::new();
    for entry in entries {
        if let Ok(entry) = entry {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') { continue; }
            let metadata = entry.metadata();
            let (is_dir, size) = if let Ok(m) = metadata { (m.is_dir(), if m.is_dir() { 0 } else { m.len() }) } else { (false, 0) };
            let full_path = entry.path().to_string_lossy().to_string();
            result.push(FileEntry { name, path: full_path, is_dir, size });
        }
    }
    Ok(result)
}

#[tauri::command]
fn open_file(path: String) -> Result<(), String> {
    Command::new("open").arg(&path).spawn().map_err(|e| format!("Cannot open file: {}", e))?;
    Ok(())
}

#[tauri::command]
fn get_home_dir() -> Result<String, String> {
    dirs::home_dir().map(|p| p.to_string_lossy().to_string()).ok_or_else(|| "Cannot determine home directory".to_string())
}

#[tauri::command]
fn launch_app(name: String) -> Result<(), String> {
    Command::new("open").arg("-a").arg(&name).spawn().map_err(|e| format!("Cannot launch {}: {}", name, e))?;
    Ok(())
}

#[tauri::command]
fn save_tasks(data: String) -> Result<(), String> {
    let home = dirs::home_dir().ok_or("No home directory")?;
    let path = home.join(".lcars-os-tasks.json");
    std::fs::write(&path, &data).map_err(|e| format!("Cannot save tasks: {}", e))?;
    Ok(())
}

#[tauri::command]
fn load_tasks() -> Result<String, String> {
    let home = dirs::home_dir().ok_or("No home directory")?;
    let path = home.join(".lcars-os-tasks.json");
    if path.exists() { std::fs::read_to_string(&path).map_err(|e| format!("Cannot load tasks: {}", e)) } else { Ok("[]".to_string()) }
}

#[tauri::command]
fn save_log(data: String) -> Result<(), String> {
    let home = dirs::home_dir().ok_or("No home directory")?;
    let path = home.join(".lcars-os-captains-log.json");
    std::fs::write(&path, &data).map_err(|e| format!("Cannot save log: {}", e))?;
    Ok(())
}

#[tauri::command]
fn load_log() -> Result<String, String> {
    let home = dirs::home_dir().ok_or("No home directory")?;
    let path = home.join(".lcars-os-captains-log.json");
    if path.exists() { std::fs::read_to_string(&path).map_err(|e| format!("Cannot load log: {}", e)) } else { Ok("[]".to_string()) }
}

#[tauri::command]
fn purge_memory() -> Result<String, String> {
    // Use osascript to request admin privileges and run purge
    let output = std::process::Command::new("osascript")
        .arg("-e")
        .arg("do shell script \"purge\" with administrator privileges")
        .output()
        .map_err(|e| format!("Failed to execute: {}", e))?;

    if output.status.success() {
        Ok("Memory purged successfully".to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("Purge failed: {}", stderr))
    }
}

#[tauri::command]
async fn start_dictation(duration_secs: u64, session_id: String) -> Result<String, String> {
    let duration = duration_secs;
    let sid = session_id;

    let result = tauri::async_runtime::spawn_blocking(move || {
        let home = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("/tmp"));
        let helper_dir = home.join(".lcars-os");
        let app_bundle = helper_dir.join("LCARSDictation.app");
        let contents_dir = app_bundle.join("Contents");
        let macos_dir = contents_dir.join("MacOS");
        let binary_path = macos_dir.join("LCARSDictation");
        let plist_path = contents_dir.join("Info.plist");
        let source_path = helper_dir.join("dictation_helper.swift");

        // Session-specific file paths — each dictation gets its own files
        // Swift helper appends .err/.partial/.stop to the output path
        let result_path = helper_dir.join(format!("dictation_{}.txt", sid));
        let error_path = helper_dir.join(format!("dictation_{}.txt.err", sid));
        let partial_path = helper_dir.join(format!("dictation_{}.txt.partial", sid));
        let stop_path = helper_dir.join(format!("dictation_{}.txt.stop", sid));

        // Clean this session's files
        let _ = std::fs::remove_file(&result_path);
        let _ = std::fs::remove_file(&error_path);
        let _ = std::fs::remove_file(&partial_path);
        let _ = std::fs::remove_file(&stop_path);

        // Ensure all directories exist
        let _ = std::fs::create_dir_all(&macos_dir);

        // Write the Swift source (always update to latest version)
        let swift_source = include_str!("../helpers/dictation_helper.swift");
        let _ = std::fs::write(&source_path, swift_source);

        // Write the Info.plist
        let info_plist = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleIdentifier</key>
    <string>com.lcars.os.dictation</string>
    <key>CFBundleName</key>
    <string>LCARS Dictation</string>
    <key>CFBundleExecutable</key>
    <string>LCARSDictation</string>
    <key>CFBundleVersion</key>
    <string>1.0</string>
    <key>CFBundleShortVersionString</key>
    <string>1.0</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>LSUIElement</key>
    <true/>
    <key>NSMicrophoneUsageDescription</key>
    <string>LCARS OS needs microphone access for Captain's Log voice dictation.</string>
    <key>NSSpeechRecognitionUsageDescription</key>
    <string>LCARS OS uses on-device speech recognition to transcribe your Captain's Log entries.</string>
</dict>
</plist>"#;
        let _ = std::fs::write(&plist_path, info_plist);

        // Compile if binary doesn't exist or source is newer
        let needs_compile = if binary_path.exists() {
            let src_mod = std::fs::metadata(&source_path).and_then(|m| m.modified()).unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            let bin_mod = std::fs::metadata(&binary_path).and_then(|m| m.modified()).unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            src_mod > bin_mod
        } else {
            true
        };

        if needs_compile {
            let _ = std::fs::remove_file(&binary_path);
            let compile = std::process::Command::new("swiftc")
                .arg(&source_path)
                .arg("-o")
                .arg(&binary_path)
                .arg("-framework").arg("Cocoa")
                .arg("-framework").arg("Speech")
                .arg("-framework").arg("AVFoundation")
                .output()
                .map_err(|e| format!("Failed to compile dictation helper: {}", e))?;

            if !compile.status.success() {
                let err = String::from_utf8_lossy(&compile.stderr);
                return Err(format!("Compile error: {}", err));
            }
        }

        // Register the .app bundle with LaunchServices
        let _ = std::process::Command::new("/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister")
            .arg("-f")
            .arg(&app_bundle)
            .output();

        // Kill any stale LCARSDictation process from a previous session
        let _ = std::process::Command::new("pkill").arg("-f").arg("LCARSDictation").output();
        std::thread::sleep(std::time::Duration::from_millis(200));

        // Write config file for Swift helper (more reliable than --args with 'open')
        let config_path = helper_dir.join("dictation_config.txt");
        let config = format!("{}\n{}", duration, result_path.to_string_lossy());
        std::fs::write(&config_path, &config).map_err(|e| format!("Failed to write config: {}", e))?;

        // Launch via 'open -n -W' (-n forces new instance)
        let output = std::process::Command::new("open")
            .arg("-n")
            .arg("-W")
            .arg(&app_bundle)
            .output()
            .map_err(|e| format!("Failed to launch dictation app: {}", e))?;

        std::thread::sleep(std::time::Duration::from_millis(500));

        if result_path.exists() {
            let text = std::fs::read_to_string(&result_path)
                .map_err(|e| format!("Failed to read result: {}", e))?;
            let _ = std::fs::remove_file(&result_path);
            let _ = std::fs::remove_file(&partial_path);
            let _ = std::fs::remove_file(&stop_path);
            Ok(text.trim().to_string())
        } else if error_path.exists() {
            let err = std::fs::read_to_string(&error_path).unwrap_or_else(|_| "Unknown error".to_string());
            let _ = std::fs::remove_file(&error_path);
            Err(err)
        } else {
            let stderr_text = String::from_utf8_lossy(&output.stderr).to_string();
            if !stderr_text.is_empty() {
                Err(format!("Dictation failed: {}", stderr_text))
            } else {
                Err("Dictation timed out — no speech detected or permissions denied".to_string())
            }
        }
    }).await.map_err(|e| format!("Thread error: {}", e))?;

    result
}

#[tauri::command]
fn poll_dictation(session_id: String) -> Result<String, String> {
    let home = dirs::home_dir().ok_or("No home directory")?;
    let helper_dir = home.join(".lcars-os");
    let result_path = helper_dir.join(format!("dictation_{}.txt", session_id));
    let partial_path = helper_dir.join(format!("dictation_{}.txt.partial", session_id));
    let error_path = helper_dir.join(format!("dictation_{}.txt.err", session_id));

    if result_path.exists() {
        let text = std::fs::read_to_string(&result_path).unwrap_or_default();
        return Ok(format!("FINAL:{}", text.trim()));
    }
    if error_path.exists() {
        let err = std::fs::read_to_string(&error_path).unwrap_or_default();
        return Err(err);
    }
    if partial_path.exists() {
        let text = std::fs::read_to_string(&partial_path).unwrap_or_default();
        return Ok(format!("PARTIAL:{}", text.trim()));
    }
    Ok("WAITING".to_string())
}

#[tauri::command]
fn stop_dictation(session_id: String) -> Result<(), String> {
    let home = dirs::home_dir().ok_or("No home directory")?;
    let stop_path = home.join(".lcars-os").join(format!("dictation_{}.txt.stop", session_id));
    std::fs::write(&stop_path, "stop").map_err(|e| format!("Failed to signal stop: {}", e))?;
    Ok(())
}

fn cleanup_dictation_files() {
    if let Some(home) = dirs::home_dir() {
        let helper_dir = home.join(".lcars-os");
        if let Ok(entries) = std::fs::read_dir(&helper_dir) {
            for entry in entries.flatten() {
                if let Some(name) = entry.file_name().to_str() {
                    // Only delete session data files (dictation_<hex>.txt*), not helper/config
                    if name.starts_with("dictation_") && name != "dictation_helper.swift" && name != "dictation_config.txt" {
                        let _ = std::fs::remove_file(entry.path());
                    }
                }
            }
        }
    }
}

fn main() {
    // Clean up stale dictation files from previous sessions/crashes
    cleanup_dictation_files();

    let sys = System::new_all();
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(AppState { sys: Mutex::new(sys), comms_cache: Mutex::new(None) })
        .invoke_handler(tauri::generate_handler![
            get_system_metrics, list_directory, open_file, get_home_dir,
            get_comms_status, launch_app, save_tasks, load_tasks, save_log, load_log, purge_memory,
            start_dictation, poll_dictation, stop_dictation
        ])
        .run(tauri::generate_context!())
        .expect("error while running LCARS OS");
}

// LCARS OS — Shared Metrics Helper
// Same sysinfo crate and logic as the Tauri backend
// Outputs JSON to stdout for use by Electron (or any other frontend)
//
// Usage:
//   lcars-metrics metrics    → system metrics JSON
//   lcars-metrics comms      → comms status JSON

use serde::Serialize;
use std::process::Command;
use sysinfo::System;

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

#[derive(Serialize)]
struct CommsStatus {
    wifi: String,
    bluetooth_enabled: bool,
    bluetooth_devices: Vec<String>,
    volume_percent: i32,
    brightness_percent: i32,
}

fn get_system_metrics() -> SystemMetrics {
    let mut sys = System::new_all();
    std::thread::sleep(std::time::Duration::from_millis(250));
    sys.refresh_cpu_usage();
    sys.refresh_memory();

    let cpu_usage = sys.cpus().iter().map(|c| c.cpu_usage()).sum::<f32>() / sys.cpus().len().max(1) as f32;
    let cpu_brand = sys.cpus().first().map(|c| c.brand().to_string()).unwrap_or_else(|| "Unknown".to_string());
    let memory_total = sys.total_memory() as f64 / 1_073_741_824.0;
    let memory_used = sys.used_memory() as f64 / 1_073_741_824.0;
    let memory_usage_percent = if memory_total > 0.0 { (memory_used / memory_total) * 100.0 } else { 0.0 };
    let uptime_seconds = System::uptime();

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

fn get_comms_status() -> CommsStatus {
    let wifi = get_wifi_info();
    let (bluetooth_enabled, bluetooth_devices) = get_bluetooth_info();
    let volume_percent = get_volume();
    let brightness_percent = get_brightness();
    CommsStatus { wifi, bluetooth_devices, bluetooth_enabled, volume_percent, brightness_percent }
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

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let command = if args.len() > 1 { args[1].as_str() } else { "metrics" };

    match command {
        "metrics" => {
            let metrics = get_system_metrics();
            println!("{}", serde_json::to_string(&metrics).unwrap());
        }
        "comms" => {
            let comms = get_comms_status();
            println!("{}", serde_json::to_string(&comms).unwrap());
        }
        _ => {
            eprintln!("Usage: lcars-metrics [metrics|comms]");
            std::process::exit(1);
        }
    }
}

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::process::Command;

#[tauri::command]
fn list_devices() -> Result<Vec<String>, String> {
    let output = Command::new("adb")
        .arg("devices")
        .output()
        .map_err(|e| format!("failed to execute adb: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("adb devices failed: {stderr}"));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let devices = stdout
        .lines()
        .skip(1)
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                return None;
            }

            let mut parts = trimmed.split_whitespace();
            let serial = parts.next()?;
            let state = parts.next()?;

            if state == "device" {
                Some(serial.to_string())
            } else {
                None
            }
        })
        .collect();

    Ok(devices)
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![list_devices])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

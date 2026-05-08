#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod input;

use std::{
    process::{Child, Command},
    sync::Mutex,
};

struct StreamState {
    child: Mutex<Option<Child>>,
}

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

#[tauri::command]
fn start_stream(serial: String, state: tauri::State<'_, StreamState>) -> Result<(), String> {
    let mut stream = state
        .child
        .lock()
        .map_err(|_| "failed to lock stream state".to_string())?;

    if stream.is_some() {
        return Err("stream is already running".to_string());
    }

    let child = Command::new("/usr/local/bin/scrcpy")
        .arg("--serial")
        .arg(serial)
        .arg("--turn-screen-off")
        .arg("--max-fps")
        .arg("60")
        .arg("--video-bit-rate")
        .arg("8M")
        .arg("--no-audio")
        .spawn()
        .map_err(|e| format!("failed to start scrcpy: {e}"))?;

    *stream = Some(child);

    Ok(())
}

#[tauri::command]
fn start_input(serial: String) -> Result<(), String> {
    input::start_input_listener(serial)
}

#[tauri::command]
fn stop_input() -> Result<(), String> {
    input::stop_input_listener();
    Ok(())
}

#[tauri::command]
fn stop_stream(state: tauri::State<'_, StreamState>) -> Result<(), String> {
    let child = state
        .child
        .lock()
        .map_err(|_| "failed to lock stream state".to_string())?
        .take();

    if let Some(mut child) = child {
        child
            .kill()
            .map_err(|e| format!("failed to stop scrcpy: {e}"))?;
        let _ = child.wait();
    }

    Ok(())
}

fn main() {
    tauri::Builder::default()
        .manage(StreamState {
            child: Mutex::new(None),
        })
        .invoke_handler(tauri::generate_handler![
            list_devices,
            start_stream,
            stop_stream,
            start_input,
            stop_input
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

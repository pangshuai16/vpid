// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use hidapi::HidApi;
use serde::Serialize;
use specta::*;
use tauri_specta::*;

#[derive(Serialize, Type)]
pub struct DeviceInfo {
    product_name: String,
    path: String,
    vendor_id: u16,
    product_id: u16,
    vendor_name: String,
}

#[tauri::command]
#[specta::specta]
fn get_hid_devices() -> Result<Vec<DeviceInfo>, String> {
    let api = HidApi::new().map_err(|err| format!("HID API error: {}", err))?;
    let devices = api.device_list();

    let devices: Vec<DeviceInfo> = devices
        .map(|device| DeviceInfo {
            product_name: device.product_string().unwrap().to_string(),
            path: device.path().to_str().unwrap().to_string(),
            vendor_id: device.vendor_id(),
            product_id: device.product_id(),
            vendor_name: device.manufacturer_string().unwrap_or_default().to_string(),
        })
        .collect();

    Ok(devices)
}

fn main() {
    #[cfg(debug_assertions)]
    tauri_specta::ts::builder()
        .commands(collect_commands![get_hid_devices])
        .path("../src/bindings.ts")
        .export()
        .unwrap();

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![get_hid_devices])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

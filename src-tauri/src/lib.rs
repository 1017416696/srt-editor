mod srt_parser;

use srt_parser::{read_srt_file, write_srt_file, SRTFile, SubtitleEntry};
use std::fs;
use tauri::menu::{MenuBuilder, SubmenuBuilder};
use tauri::{Emitter, Manager};

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    println!("Backend was called with an argument: {}", name);
    format!("Hello, {}! You've been greeted from Rust!", name)
}

/// Read and parse an SRT file
#[tauri::command]
fn read_srt(file_path: String) -> Result<SRTFile, String> {
    read_srt_file(&file_path)
}

/// Write SRT file
#[tauri::command]
fn write_srt(file_path: String, entries: Vec<SubtitleEntry>) -> Result<(), String> {
    write_srt_file(&file_path, &entries)
}

/// Read audio file and return as base64
#[tauri::command]
fn read_audio_file(file_path: String) -> Result<String, String> {
    let file_data = fs::read(&file_path)
        .map_err(|e| format!("Failed to read audio file: {}", e))?;

    // Convert to base64
    let base64_data = base64_encode(&file_data);
    Ok(base64_data)
}

/// 触发前端打开文件事件
#[tauri::command]
fn trigger_open_file(window: tauri::Window) -> Result<(), String> {
    window.emit("menu:open-file", ()).map_err(|e| e.to_string())
}

/// Simple base64 encoding function
fn base64_encode(data: &[u8]) -> String {
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::new();

    for chunk in data.chunks(3) {
        let b1 = chunk[0];
        let b2 = if chunk.len() > 1 { chunk[1] } else { 0 };
        let b3 = if chunk.len() > 2 { chunk[2] } else { 0 };

        let n = ((b1 as u32) << 16) | ((b2 as u32) << 8) | (b3 as u32);

        result.push(CHARSET[((n >> 18) & 63) as usize] as char);
        result.push(CHARSET[((n >> 12) & 63) as usize] as char);

        if chunk.len() > 1 {
            result.push(CHARSET[((n >> 6) & 63) as usize] as char);
        } else {
            result.push('=');
        }

        if chunk.len() > 2 {
            result.push(CHARSET[(n & 63) as usize] as char);
        } else {
            result.push('=');
        }
    }

    result
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            #[cfg(target_os = "macos")]
            {
                // 在 macOS 上，第一个子菜单会自动成为应用菜单
                let app_menu = SubmenuBuilder::new(app, "SRT Editor")
                    .text("about", "About SRT Editor")
                    .separator()
                    .text("quit", "Quit SRT Editor")
                    .build()?;

                // 创建 File 菜单
                let file_menu = SubmenuBuilder::new(app, "File")
                    .text("open", "Open")
                    .text("save", "Save")
                    .separator()
                    .text("close", "Close Window")
                    .build()?;

                // 创建 Edit 菜单
                let edit_menu = SubmenuBuilder::new(app, "Edit")
                    .text("undo", "Undo")
                    .text("redo", "Redo")
                    .separator()
                    .text("cut", "Cut")
                    .text("copy", "Copy")
                    .text("paste", "Paste")
                    .build()?;

                // 创建菜单：应用菜单 -> File -> Edit
                let menu = MenuBuilder::new(app)
                    .item(&app_menu)
                    .item(&file_menu)
                    .item(&edit_menu)
                    .build()?;

                app.set_menu(menu)?;
            }

            // 处理菜单事件（在 macOS 条件之外，确保总是被注册）
            app.on_menu_event(|app_handle, event| {
                println!("Menu event triggered: {}", event.id().0);
                match event.id().0.as_str() {
                    "open" => {
                        println!("Open menu clicked - executing open file via JavaScript");
                        if let Some(window) = app_handle.get_webview_window("main") {
                            // 使用 evaluate_script 直接在前端执行 JavaScript
                            let js_code = r#"
                                (async () => {
                                    console.log('✓ JavaScript executed: Opening file dialog...');
                                    if (window.__handleMenuOpenFile && typeof window.__handleMenuOpenFile === 'function') {
                                        await window.__handleMenuOpenFile();
                                    } else {
                                        console.warn('Handler not available');
                                    }
                                })();
                            "#;
                            match window.eval(js_code) {
                                Ok(_) => {
                                    println!("✓ Successfully executed JavaScript to open file");
                                }
                                Err(e) => {
                                    println!("✗ Failed to execute JavaScript: {:?}", e);
                                }
                            }
                        } else {
                            println!("✗ Main window not found!");
                        }
                    }
                    "save" => {
                        println!("Save menu clicked - executing save via JavaScript");
                        if let Some(window) = app_handle.get_webview_window("main") {
                            let js_code = r#"
                                (async () => {
                                    console.log('✓ JavaScript executed: Saving file...');
                                    if (window.__handleMenuSave && typeof window.__handleMenuSave === 'function') {
                                        await window.__handleMenuSave();
                                    } else {
                                        console.warn('Handler not available');
                                    }
                                })();
                            "#;
                            match window.eval(js_code) {
                                Ok(_) => {
                                    println!("✓ Successfully executed JavaScript to save file");
                                }
                                Err(e) => {
                                    println!("✗ Failed to execute JavaScript: {:?}", e);
                                }
                            }
                        } else {
                            println!("✗ Main window not found!");
                        }
                    }
                    _ => {
                        println!("Other menu event: {}", event.id().0);
                    }
                }
            });

            #[cfg(target_os = "macos")]
            {
                // macOS 特定配置已完成
            }

            #[cfg(debug_assertions)]
            {
                let window = tauri::Manager::get_webview_window(app, "main").unwrap();
                window.open_devtools();
            }
            Ok(())
        })
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_prevent_default::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![greet, read_srt, write_srt, read_audio_file, trigger_open_file])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

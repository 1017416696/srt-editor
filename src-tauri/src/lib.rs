mod srt_parser;
mod waveform_generator;

use srt_parser::{read_srt_file, write_srt_file, SRTFile, SubtitleEntry};
use waveform_generator::{generate_waveform_with_progress, ProgressCallback};
use std::fs;
use tauri::menu::{MenuBuilder, PredefinedMenuItem, SubmenuBuilder};
use tauri::{Emitter, Manager};
use tauri_plugin_prevent_default::Flags;

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

/// Generate waveform data from an audio file
/// Returns a vector of normalized amplitude values (0.0 to 1.0)
/// target_samples: number of data points to generate (default: 2000)
#[tauri::command]
async fn generate_audio_waveform(
    window: tauri::Window,
    file_path: String,
    target_samples: Option<usize>,
) -> Result<Vec<f32>, String> {
    let samples = target_samples.unwrap_or(2000);
    
    let (tx, rx) = std::sync::mpsc::channel();
    let window_clone = window.clone();
    let file_path_clone = file_path.clone();
    
    // 在后台线程执行波形生成
    std::thread::spawn(move || {
        let window_for_callback = window_clone.clone();
        let callback: ProgressCallback = Box::new(move |progress| {
            let _ = window_for_callback.emit("waveform-progress", progress);
        });
        
        let result = generate_waveform_with_progress(&file_path_clone, samples, Some(callback));
        let _ = tx.send(result);
    });
    
    // 异步等待结果
    let result = tauri::async_runtime::spawn_blocking(move || {
        rx.recv().map_err(|e| format!("Channel error: {:?}", e))?
    })
    .await
    .map_err(|e| format!("Task error: {:?}", e))??;
    
    Ok(result)
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
                    .text("about", "关于 SRT Editor")
                    .separator()
                    .text("quit", "退出 SRT Editor")
                    .build()?;

                // 创建 文件 菜单（macOS 使用 Cmd）
                let file_menu = SubmenuBuilder::new(app, "文件")
                    .text("open", "打开\t⌘ O")
                    .text("save", "保存\t⌘ S")
                    .separator()
                    .text("close", "关闭窗口")
                    .build()?;

                // 创建 编辑 菜单（macOS 使用 Cmd）- 使用预定义菜单项以支持系统快捷键
                let edit_menu = SubmenuBuilder::new(app, "编辑")
                    .item(&PredefinedMenuItem::undo(app, Some("撤销"))?)
                    .item(&PredefinedMenuItem::redo(app, Some("重做"))?)
                    .separator()
                    .item(&PredefinedMenuItem::cut(app, Some("剪切"))?)
                    .item(&PredefinedMenuItem::copy(app, Some("复制"))?)
                    .item(&PredefinedMenuItem::paste(app, Some("粘贴"))?)
                    .item(&PredefinedMenuItem::select_all(app, Some("全选"))?)
                    .separator()
                    .text("batch-add-cjk-spaces", "批量添加中英文空格")
                    .text("batch-remove-html", "批量移除HTML标签")
                    .text("batch-remove-punctuation", "批量删除标点符号")
                    .build()?;

                // 创建菜单：应用菜单 -> File -> Edit
                let menu = MenuBuilder::new(app)
                    .item(&app_menu)
                    .item(&file_menu)
                    .item(&edit_menu)
                    .build()?;

                app.set_menu(menu)?;
            }

            // Windows 配置
            #[cfg(target_os = "windows")]
            {
                // 创建 文件 菜单（Windows 使用 Ctrl）
                let file_menu = SubmenuBuilder::new(app, "文件")
                    .text("open", "打开\tCtrl+O")
                    .text("save", "保存\tCtrl+S")
                    .separator()
                    .text("close", "关闭窗口")
                    .build()?;

                // 创建 编辑 菜单（Windows 使用 Ctrl）- 使用预定义菜单项以支持系统快捷键
                let edit_menu = SubmenuBuilder::new(app, "编辑")
                    .item(&PredefinedMenuItem::undo(app, Some("撤销"))?)
                    .item(&PredefinedMenuItem::redo(app, Some("重做"))?)
                    .separator()
                    .item(&PredefinedMenuItem::cut(app, Some("剪切"))?)
                    .item(&PredefinedMenuItem::copy(app, Some("复制"))?)
                    .item(&PredefinedMenuItem::paste(app, Some("粘贴"))?)
                    .item(&PredefinedMenuItem::select_all(app, Some("全选"))?)
                    .separator()
                    .text("batch-add-cjk-spaces", "批量添加中英文空格")
                    .text("batch-remove-html", "批量移除HTML标签")
                    .text("batch-remove-punctuation", "批量删除标点符号")
                    .build()?;

                // 创建菜单：File -> Edit
                let menu = MenuBuilder::new(app)
                    .item(&file_menu)
                    .item(&edit_menu)
                    .build()?;

                app.set_menu(menu)?;
            }

            // 处理菜单事件
            app.on_menu_event(|app_handle, event| {
                match event.id().0.as_str() {
                    "open" => {
                        if let Some(window) = app_handle.get_webview_window("main") {
                            let js_code = r#"
                                (async () => {
                                    if (window.__globalOpenFile && typeof window.__globalOpenFile === 'function') {
                                        await window.__globalOpenFile();
                                    }
                                })();
                            "#;
                            let _ = window.eval(js_code);
                        }
                    }
                    "save" => {
                        if let Some(window) = app_handle.get_webview_window("main") {
                            let js_code = r#"
                                (async () => {
                                    if (window.__globalSaveFile && typeof window.__globalSaveFile === 'function') {
                                        await window.__globalSaveFile();
                                    }
                                })();
                            "#;
                            let _ = window.eval(js_code);
                        }
                    }
                    "batch-add-cjk-spaces" => {
                        if let Some(window) = app_handle.get_webview_window("main") {
                            let js_code = r#"
                                (async () => {
                                    if (window.__globalBatchAddCJKSpaces && typeof window.__globalBatchAddCJKSpaces === 'function') {
                                        await window.__globalBatchAddCJKSpaces();
                                    }
                                })();
                            "#;
                            let _ = window.eval(js_code);
                        }
                    }
                    "batch-remove-html" => {
                        if let Some(window) = app_handle.get_webview_window("main") {
                            let js_code = r#"
                                (async () => {
                                    if (window.__globalBatchRemoveHTML && typeof window.__globalBatchRemoveHTML === 'function') {
                                        await window.__globalBatchRemoveHTML();
                                    }
                                })();
                            "#;
                            let _ = window.eval(js_code);
                        }
                    }
                    "batch-remove-punctuation" => {
                        if let Some(window) = app_handle.get_webview_window("main") {
                            let js_code = r#"
                                (async () => {
                                    if (window.__globalBatchRemovePunctuation && typeof window.__globalBatchRemovePunctuation === 'function') {
                                        await window.__globalBatchRemovePunctuation();
                                    }
                                })();
                            "#;
                            let _ = window.eval(js_code);
                        }
                    }
                    _ => {}
                }
            });

            #[cfg(target_os = "macos")]
            {
                // macOS 特定配置已完成
            }

            Ok(())
        })
        .plugin(tauri_plugin_shell::init())
        .plugin(
            tauri_plugin_prevent_default::Builder::new()
                // 只阻止右键菜单，允许复制、粘贴、剪切等编辑快捷键
                .with_flags(Flags::CONTEXT_MENU)
                .build(),
        )
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![greet, read_srt, write_srt, read_audio_file, generate_audio_waveform, trigger_open_file])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

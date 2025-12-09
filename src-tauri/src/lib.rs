mod srt_parser;
mod waveform_generator;
mod whisper_transcriber;
mod sensevoice_transcriber;
mod firered_corrector;

use srt_parser::{read_srt_file, write_srt_file, SRTFile, SubtitleEntry};
use whisper_transcriber::{
    get_available_models, download_model, delete_model, transcribe_audio, cancel_transcription, WhisperModelInfo,
};
use sensevoice_transcriber::{
    check_sensevoice_env, install_sensevoice_env, transcribe_with_sensevoice, 
    uninstall_sensevoice_env, cancel_sensevoice_transcription, SenseVoiceEnvStatus,
};
use firered_corrector::{
    check_firered_env, install_firered_env, correct_with_firered, correct_single_entry,
    uninstall_firered_env, cancel_firered_correction, preload_firered_service, is_service_running,
    FireRedEnvStatus, CorrectionEntry, SingleCorrectionResult,
};
use waveform_generator::{generate_waveform_with_progress, ProgressCallback};
use std::fs;
use tauri::menu::{MenuBuilder, MenuItem, PredefinedMenuItem, SubmenuBuilder};
use tauri::{Emitter, Manager};
use tauri_plugin_prevent_default::Flags;
use tauri_plugin_log::{Target, TargetKind, TimezoneStrategy, RotationStrategy};
use log::info;

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

/// 获取日志目录路径
#[tauri::command]
fn get_log_path(app_handle: tauri::AppHandle) -> Result<String, String> {
    let log_dir = app_handle.path().app_log_dir().map_err(|e| e.to_string())?;
    Ok(log_dir.to_string_lossy().to_string())
}

/// 在系统文件管理器中打开日志目录
#[tauri::command]
fn show_log_in_folder(app_handle: tauri::AppHandle) -> Result<(), String> {
    let log_dir = app_handle.path().app_log_dir().map_err(|e| e.to_string())?;

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&log_dir)
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(&log_dir)
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(&log_dir)
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

/// 获取可用的 Whisper 模型列表
#[tauri::command]
fn get_whisper_models() -> Result<Vec<WhisperModelInfo>, String> {
    get_available_models()
}

/// 下载 Whisper 模型
#[tauri::command]
async fn download_whisper_model(
    window: tauri::Window,
    model_size: String,
) -> Result<String, String> {
    download_model(&model_size, window).await
}

/// 转录音频文件为字幕
#[tauri::command]
async fn transcribe_audio_to_subtitles(
    window: tauri::Window,
    audio_path: String,
    model_size: String,
    language: String,
) -> Result<Vec<SubtitleEntry>, String> {
    transcribe_audio(audio_path, model_size, language, window).await
}

/// 删除 Whisper 模型
#[tauri::command]
fn delete_whisper_model(model_size: String) -> Result<String, String> {
    delete_model(&model_size)
}

/// 取消转录任务
#[tauri::command]
fn cancel_transcription_task() {
    cancel_transcription();
}

// ============ SenseVoice 相关命令 ============

/// 检查 SenseVoice 环境状态
#[tauri::command]
fn check_sensevoice_env_status() -> SenseVoiceEnvStatus {
    check_sensevoice_env()
}

/// 安装 SenseVoice 环境
#[tauri::command]
async fn install_sensevoice(window: tauri::Window) -> Result<String, String> {
    install_sensevoice_env(window).await
}

/// 使用 SenseVoice 转录音频
#[tauri::command]
async fn transcribe_with_sensevoice_model(
    window: tauri::Window,
    audio_path: String,
    language: String,
) -> Result<Vec<SubtitleEntry>, String> {
    transcribe_with_sensevoice(audio_path, language, window).await
}

/// 卸载 SenseVoice 环境
#[tauri::command]
fn uninstall_sensevoice() -> Result<String, String> {
    uninstall_sensevoice_env()
}

/// 取消 SenseVoice 转录
#[tauri::command]
fn cancel_sensevoice_task() {
    cancel_sensevoice_transcription();
}

// ============ FireRedASR 校正相关命令 ============

/// 检查 FireRedASR 环境状态
#[tauri::command]
fn check_firered_env_status() -> FireRedEnvStatus {
    check_firered_env()
}

/// 安装 FireRedASR 环境
#[tauri::command]
async fn install_firered(window: tauri::Window) -> Result<String, String> {
    install_firered_env(window).await
}

/// 使用 FireRedASR 校正字幕
#[tauri::command]
async fn correct_subtitles_with_firered(
    window: tauri::Window,
    srt_path: String,
    audio_path: String,
    language: String,
) -> Result<Vec<CorrectionEntry>, String> {
    correct_with_firered(srt_path, audio_path, language, window).await
}

/// 卸载 FireRedASR 环境
#[tauri::command]
fn uninstall_firered() -> Result<String, String> {
    uninstall_firered_env()
}

/// 取消 FireRedASR 校正
#[tauri::command]
fn cancel_firered_task() {
    cancel_firered_correction();
}

/// 预加载 FireRedASR 服务（启动服务并加载模型）
#[tauri::command]
async fn preload_firered() -> Result<String, String> {
    preload_firered_service().await
}

/// 检查 FireRedASR 服务是否运行
#[tauri::command]
fn is_firered_service_running() -> bool {
    is_service_running()
}

/// 校正单条字幕
#[tauri::command]
async fn correct_single_subtitle(
    audio_path: String,
    start_ms: u32,
    end_ms: u32,
    original_text: String,
    language: String,
) -> Result<SingleCorrectionResult, String> {
    correct_single_entry(audio_path, start_ms, end_ms, original_text, language).await
}

/// 打开模型目录
#[tauri::command]
fn open_whisper_model_dir() -> Result<(), String> {
    let model_dir = whisper_transcriber::get_model_dir()?;
    
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&model_dir)
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(&model_dir)
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(&model_dir)
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

/// 最近文件信息
#[derive(serde::Deserialize, Clone)]
#[allow(dead_code)]
struct RecentFileInfo {
    path: String,
    name: String,
}

/// 更新最近文件菜单
#[tauri::command]
fn update_recent_files_menu(app_handle: tauri::AppHandle, files: Vec<RecentFileInfo>) -> Result<(), String> {
    use tauri::menu::{MenuBuilder, SubmenuBuilder};
    
    // 获取当前菜单
    if let Some(_window) = app_handle.get_webview_window("main") {
        // 创建新的最近文件子菜单
        let mut recent_menu_builder = SubmenuBuilder::new(&app_handle, "打开最近的文件");
        
        if files.is_empty() {
            recent_menu_builder = recent_menu_builder.text("no-recent", "无最近文件");
        } else {
            for (index, file) in files.iter().enumerate() {
                let menu_id = format!("recent-{}", index);
                recent_menu_builder = recent_menu_builder.text(&menu_id, &file.name);
            }
        }
        
        recent_menu_builder = recent_menu_builder
            .separator()
            .text("clear-recent", "清除最近文件");
        
        let recent_menu = recent_menu_builder.build().map_err(|e| e.to_string())?;
        
        // 重建整个菜单
        #[cfg(target_os = "macos")]
        {
            use tauri::menu::{MenuItem, PredefinedMenuItem};
            
            let app_menu = SubmenuBuilder::new(&app_handle, "SRT Editor")
                .text("about", "关于 SRT Editor")
                .separator()
                .text("quit", "退出 SRT Editor")
                .build()
                .map_err(|e| e.to_string())?;

            let open_item = MenuItem::with_id(&app_handle, "open", "打开", true, Some("CmdOrCtrl+O")).map_err(|e| e.to_string())?;
            let save_item = MenuItem::with_id(&app_handle, "save", "保存", true, Some("CmdOrCtrl+S")).map_err(|e| e.to_string())?;
            let close_tab_item = MenuItem::with_id(&app_handle, "close-tab", "关闭标签页", true, Some("CmdOrCtrl+W")).map_err(|e| e.to_string())?;
            let close_window_item = MenuItem::with_id(&app_handle, "close-window", "关闭窗口", true, Some("Cmd+Q")).map_err(|e| e.to_string())?;
            let file_menu = SubmenuBuilder::new(&app_handle, "文件")
                .item(&open_item)
                .item(&recent_menu)
                .item(&save_item)
                .separator()
                .item(&close_tab_item)
                .item(&close_window_item)
                .build()
                .map_err(|e| e.to_string())?;

            let edit_menu = SubmenuBuilder::new(&app_handle, "编辑")
                .item(&PredefinedMenuItem::undo(&app_handle, Some("撤销")).map_err(|e| e.to_string())?)
                .item(&PredefinedMenuItem::redo(&app_handle, Some("重做")).map_err(|e| e.to_string())?)
                .separator()
                .item(&PredefinedMenuItem::cut(&app_handle, Some("剪切")).map_err(|e| e.to_string())?)
                .item(&PredefinedMenuItem::copy(&app_handle, Some("复制")).map_err(|e| e.to_string())?)
                .item(&PredefinedMenuItem::paste(&app_handle, Some("粘贴")).map_err(|e| e.to_string())?)
                .item(&PredefinedMenuItem::select_all(&app_handle, Some("全选")).map_err(|e| e.to_string())?)
                .separator()
                .text("batch-ai-correction", "批量 AI 字幕校正")
                .separator()
                .text("batch-add-cjk-spaces", "批量添加中英文空格")
                .text("batch-remove-html", "批量移除HTML标签")
                .text("batch-remove-punctuation", "批量删除标点符号")
                .build()
                .map_err(|e| e.to_string())?;

            let menu = MenuBuilder::new(&app_handle)
                .item(&app_menu)
                .item(&file_menu)
                .item(&edit_menu)
                .build()
                .map_err(|e| e.to_string())?;

            app_handle.set_menu(menu).map_err(|e| e.to_string())?;
        }

        #[cfg(target_os = "windows")]
        {
            use tauri::menu::{MenuItem, PredefinedMenuItem};
            
            let open_item = MenuItem::with_id(&app_handle, "open", "打开", true, Some("CmdOrCtrl+O")).map_err(|e| e.to_string())?;
            let save_item = MenuItem::with_id(&app_handle, "save", "保存", true, Some("CmdOrCtrl+S")).map_err(|e| e.to_string())?;
            let close_tab_item = MenuItem::with_id(&app_handle, "close-tab", "关闭标签页", true, Some("CmdOrCtrl+W")).map_err(|e| e.to_string())?;
            let close_window_item = MenuItem::with_id(&app_handle, "close-window", "关闭窗口", true, Some("Alt+F4")).map_err(|e| e.to_string())?;
            let file_menu = SubmenuBuilder::new(&app_handle, "文件")
                .item(&open_item)
                .item(&recent_menu)
                .item(&save_item)
                .separator()
                .item(&close_tab_item)
                .item(&close_window_item)
                .build()
                .map_err(|e| e.to_string())?;

            let edit_menu = SubmenuBuilder::new(&app_handle, "编辑")
                .item(&PredefinedMenuItem::undo(&app_handle, Some("撤销")).map_err(|e| e.to_string())?)
                .item(&PredefinedMenuItem::redo(&app_handle, Some("重做")).map_err(|e| e.to_string())?)
                .separator()
                .item(&PredefinedMenuItem::cut(&app_handle, Some("剪切")).map_err(|e| e.to_string())?)
                .item(&PredefinedMenuItem::copy(&app_handle, Some("复制")).map_err(|e| e.to_string())?)
                .item(&PredefinedMenuItem::paste(&app_handle, Some("粘贴")).map_err(|e| e.to_string())?)
                .item(&PredefinedMenuItem::select_all(&app_handle, Some("全选")).map_err(|e| e.to_string())?)
                .separator()
                .text("batch-ai-correction", "批量 AI 字幕校正")
                .separator()
                .text("batch-add-cjk-spaces", "批量添加中英文空格")
                .text("batch-remove-html", "批量移除HTML标签")
                .text("batch-remove-punctuation", "批量删除标点符号")
                .build()
                .map_err(|e| e.to_string())?;

            let menu = MenuBuilder::new(&app_handle)
                .item(&file_menu)
                .item(&edit_menu)
                .build()
                .map_err(|e| e.to_string())?;

            // 使用 app_handle.set_menu 而不是 window.set_menu
            app_handle.set_menu(menu).map_err(|e| e.to_string())?;
        }
    }
    
    Ok(())
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

                // 创建 打开最近的文件 子菜单
                let recent_menu = SubmenuBuilder::new(app, "打开最近的文件")
                    .text("no-recent", "无最近文件")
                    .separator()
                    .text("clear-recent", "清除最近文件")
                    .build()?;

                // 创建 文件 菜单（macOS 使用 Cmd）
                let open_item = MenuItem::with_id(app, "open", "打开", true, Some("CmdOrCtrl+O"))?;
                let save_item = MenuItem::with_id(app, "save", "保存", true, Some("CmdOrCtrl+S"))?;
                let close_tab_item = MenuItem::with_id(app, "close-tab", "关闭标签页", true, Some("CmdOrCtrl+W"))?;
                let close_window_item = MenuItem::with_id(app, "close-window", "关闭窗口", true, Some("Cmd+Q"))?;
                let file_menu = SubmenuBuilder::new(app, "文件")
                    .item(&open_item)
                    .item(&recent_menu)
                    .item(&save_item)
                    .separator()
                    .item(&close_tab_item)
                    .item(&close_window_item)
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
                    .text("batch-ai-correction", "批量 AI 字幕校正")
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
                // 创建 打开最近的文件 子菜单
                let recent_menu = SubmenuBuilder::new(app, "打开最近的文件")
                    .text("no-recent", "无最近文件")
                    .separator()
                    .text("clear-recent", "清除最近文件")
                    .build()?;

                // 创建 文件 菜单（Windows 使用 Ctrl）
                let open_item = MenuItem::with_id(app, "open", "打开", true, Some("CmdOrCtrl+O"))?;
                let save_item = MenuItem::with_id(app, "save", "保存", true, Some("CmdOrCtrl+S"))?;
                let close_tab_item = MenuItem::with_id(app, "close-tab", "关闭标签页", true, Some("CmdOrCtrl+W"))?;
                let close_window_item = MenuItem::with_id(app, "close-window", "关闭窗口", true, Some("Alt+F4"))?;
                let file_menu = SubmenuBuilder::new(app, "文件")
                    .item(&open_item)
                    .item(&recent_menu)
                    .item(&save_item)
                    .separator()
                    .item(&close_tab_item)
                    .item(&close_window_item)
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
                    .text("batch-ai-correction", "批量 AI 字幕校正")
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

            info!("应用启动完成");

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
                    "close-tab" => {
                        if let Some(window) = app_handle.get_webview_window("main") {
                            let js_code = r#"
                                (async () => {
                                    if (window.__globalCloseCurrentTab && typeof window.__globalCloseCurrentTab === 'function') {
                                        await window.__globalCloseCurrentTab();
                                    }
                                })();
                            "#;
                            let _ = window.eval(js_code);
                        }
                    }
                    "close-window" => {
                        if let Some(window) = app_handle.get_webview_window("main") {
                            let _ = window.close();
                        }
                    }
                    "batch-ai-correction" => {
                        if let Some(window) = app_handle.get_webview_window("main") {
                            let js_code = r#"
                                (async () => {
                                    if (window.__globalBatchAICorrection && typeof window.__globalBatchAICorrection === 'function') {
                                        await window.__globalBatchAICorrection();
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
                    "clear-recent" => {
                        if let Some(window) = app_handle.get_webview_window("main") {
                            let js_code = r#"
                                (async () => {
                                    if (window.__globalClearRecentFiles && typeof window.__globalClearRecentFiles === 'function') {
                                        await window.__globalClearRecentFiles();
                                    }
                                })();
                            "#;
                            let _ = window.eval(js_code);
                        }
                    }
                    id if id.starts_with("recent-") => {
                        // 处理最近文件点击
                        if let Some(window) = app_handle.get_webview_window("main") {
                            let index = id.strip_prefix("recent-").unwrap_or("0");
                            let js_code = format!(r#"
                                (async () => {{
                                    if (window.__globalOpenRecentFile && typeof window.__globalOpenRecentFile === 'function') {{
                                        await window.__globalOpenRecentFile({});
                                    }}
                                }})();
                            "#, index);
                            let _ = window.eval(&js_code);
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
        .plugin(
            tauri_plugin_log::Builder::new()
                .targets([
                    // 开发环境输出到控制台
                    Target::new(TargetKind::Stdout),
                    // 日志文件，自动存储在系统日志目录
                    Target::new(TargetKind::LogDir { file_name: Some("srt-editor".into()) }),
                ])
                .timezone_strategy(TimezoneStrategy::UseLocal)
                // 日志轮转：保留所有日志文件，文件名包含日期（超过 40KB 后轮转）
                .rotation_strategy(RotationStrategy::KeepAll)
                // 默认只记录 INFO 级别
                .level(log::LevelFilter::Info)
                // 我们的应用在开发环境记录 DEBUG
                .level_for("tauri_app_lib", if cfg!(debug_assertions) {
                    log::LevelFilter::Debug
                } else {
                    log::LevelFilter::Info
                })
                // 前端日志在开发环境记录 DEBUG
                .level_for("webview", if cfg!(debug_assertions) {
                    log::LevelFilter::Debug
                } else {
                    log::LevelFilter::Info
                })
                // 过滤掉第三方库的 DEBUG 日志
                .level_for("symphonia_core", log::LevelFilter::Warn)
                .level_for("symphonia_bundle_mp3", log::LevelFilter::Warn)
                .level_for("symphonia_metadata", log::LevelFilter::Warn)
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            greet,
            read_srt,
            write_srt,
            read_audio_file,
            generate_audio_waveform,
            trigger_open_file,
            update_recent_files_menu,
            get_log_path,
            show_log_in_folder,
            get_whisper_models,
            download_whisper_model,
            delete_whisper_model,
            open_whisper_model_dir,
            transcribe_audio_to_subtitles,
            cancel_transcription_task,
            // SenseVoice 相关
            check_sensevoice_env_status,
            install_sensevoice,
            transcribe_with_sensevoice_model,
            uninstall_sensevoice,
            cancel_sensevoice_task,
            // FireRedASR 校正相关
            check_firered_env_status,
            install_firered,
            correct_subtitles_with_firered,
            correct_single_subtitle,
            preload_firered,
            is_firered_service_running,
            uninstall_firered,
            cancel_firered_task
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

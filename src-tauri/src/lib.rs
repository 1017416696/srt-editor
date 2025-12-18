mod srt_parser;
mod waveform_generator;
mod whisper_python_transcriber;
mod sensevoice_transcriber;
mod firered_corrector;

use srt_parser::{
    read_srt_file, write_srt_file, SRTFile, SubtitleEntry,
    export_to_txt, export_to_vtt, export_to_markdown, export_to_fcpxml,
    check_file_permission, unlock_file, FilePermissionCheck,
};
use whisper_python_transcriber::{
    check_whisper_env, install_whisper_env, transcribe_with_whisper,
    uninstall_whisper_env, uninstall_whisper_env_by_type, switch_whisper_env,
    cancel_whisper_transcription, cancel_whisper_model_download,
    get_whisper_models, delete_whisper_model, open_whisper_model_dir,
    download_whisper_model,
    WhisperEnvStatus, WhisperModelInfo,
};
use sensevoice_transcriber::{
    check_sensevoice_env, install_sensevoice_env, transcribe_with_sensevoice, 
    uninstall_sensevoice_env, uninstall_sensevoice_env_by_type, switch_sensevoice_env,
    cancel_sensevoice_transcription, cancel_sensevoice_model_download, SenseVoiceEnvStatus,
    get_sensevoice_models, download_sensevoice_model, delete_sensevoice_model, open_sensevoice_model_dir,
    SenseVoiceModelInfo,
};
use firered_corrector::{
    check_firered_env, install_firered_env, correct_with_firered, correct_single_entry,
    uninstall_firered_env, uninstall_firered_env_by_type, switch_firered_env,
    cancel_firered_correction, cancel_firered_model_download, preload_firered_service, is_service_running,
    preload_audio_for_correction,
    get_firered_models, download_firered_model, delete_firered_model, open_firered_model_dir,
    FireRedEnvStatus, CorrectionEntry, SingleCorrectionResult, FireRedModelInfo,
};
use waveform_generator::{generate_waveform_with_progress, ProgressCallback};
use std::fs;
use std::sync::Mutex;
use tauri::menu::{MenuBuilder, MenuItem, PredefinedMenuItem, SubmenuBuilder};
use tauri::{Emitter, Manager};
use tauri_plugin_prevent_default::Flags;
use tauri_plugin_log::{Target, TargetKind, TimezoneStrategy, RotationStrategy};
use log::info;
use once_cell::sync::Lazy;

// 全局状态：存储通过文件关联打开的待处理文件路径
static PENDING_FILE_OPEN: Lazy<Mutex<Option<String>>> = Lazy::new(|| Mutex::new(None));

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

/// 检查文件写入权限
#[tauri::command]
fn check_file_write_permission(file_path: String) -> FilePermissionCheck {
    check_file_permission(&file_path)
}

/// 解锁文件
#[tauri::command]
fn unlock_file_cmd(file_path: String) -> Result<(), String> {
    unlock_file(&file_path)
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
    app_handle: tauri::AppHandle,
    file_path: String,
    target_samples: Option<usize>,
) -> Result<Vec<f32>, String> {
    let samples = target_samples.unwrap_or(2000);
    
    let (tx, rx) = std::sync::mpsc::channel();
    let app_handle_clone = app_handle.clone();
    let file_path_clone = file_path.clone();
    
    // 在后台线程执行波形生成
    std::thread::spawn(move || {
        let app_for_callback = app_handle_clone.clone();
        let callback: ProgressCallback = Box::new(move |progress| {
            let _ = app_for_callback.emit("waveform-progress", progress);
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

/// 检查文件是否存在
#[tauri::command]
fn check_file_exists(file_path: String) -> bool {
    std::path::Path::new(&file_path).exists()
}

/// 获取并清除待打开的文件路径（用于文件关联打开）
#[tauri::command]
fn get_pending_file_open() -> Option<String> {
    let mut pending = PENDING_FILE_OPEN.lock().unwrap();
    pending.take()
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

// ============ Whisper 相关命令 ============

/// 检查 Whisper 环境状态
#[tauri::command]
fn check_whisper_env_status() -> WhisperEnvStatus {
    check_whisper_env()
}

/// 安装 Whisper 环境
#[tauri::command]
async fn install_whisper(window: tauri::Window, use_gpu: Option<bool>) -> Result<String, String> {
    install_whisper_env(window, use_gpu.unwrap_or(false)).await
}

/// 获取可用的 Whisper 模型列表
#[tauri::command]
fn get_whisper_models_cmd() -> Vec<WhisperModelInfo> {
    get_whisper_models()
}

/// 转录音频文件为字幕
#[tauri::command]
async fn transcribe_audio_to_subtitles(
    window: tauri::Window,
    audio_path: String,
    model_size: String,
    language: String,
) -> Result<Vec<SubtitleEntry>, String> {
    transcribe_with_whisper(audio_path, model_size, language, window).await
}

/// 下载 Whisper 模型
#[tauri::command]
async fn download_whisper_model_cmd(window: tauri::Window, model_name: String) -> Result<String, String> {
    download_whisper_model(&model_name, window).await
}

/// 删除 Whisper 模型
#[tauri::command]
fn delete_whisper_model_cmd(model_size: String) -> Result<String, String> {
    delete_whisper_model(&model_size)
}

/// 取消转录任务
#[tauri::command]
fn cancel_whisper_task() {
    cancel_whisper_transcription();
}

/// 取消 Whisper 模型下载
#[tauri::command]
fn cancel_whisper_model_download_cmd() {
    cancel_whisper_model_download();
}

/// 卸载 Whisper 环境
#[tauri::command]
fn uninstall_whisper() -> Result<String, String> {
    uninstall_whisper_env()
}

/// 卸载指定类型的 Whisper 环境
#[tauri::command]
fn uninstall_whisper_by_type(use_gpu: bool) -> Result<String, String> {
    uninstall_whisper_env_by_type(use_gpu)
}

/// 切换 Whisper 环境
#[tauri::command]
fn switch_whisper(use_gpu: bool) -> Result<String, String> {
    switch_whisper_env(use_gpu)
}

/// 打开 Whisper 模型目录
#[tauri::command]
fn open_whisper_model_dir_cmd() -> Result<(), String> {
    open_whisper_model_dir()
}

// ============ SenseVoice 相关命令 ============

/// 检查 SenseVoice 环境状态
#[tauri::command]
fn check_sensevoice_env_status() -> SenseVoiceEnvStatus {
    check_sensevoice_env()
}

/// 安装 SenseVoice 环境
#[tauri::command]
async fn install_sensevoice(window: tauri::Window, use_gpu: Option<bool>) -> Result<String, String> {
    install_sensevoice_env(window, use_gpu.unwrap_or(false)).await
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

/// 卸载指定类型的 SenseVoice 环境
#[tauri::command]
fn uninstall_sensevoice_by_type(use_gpu: bool) -> Result<String, String> {
    uninstall_sensevoice_env_by_type(use_gpu)
}

/// 切换 SenseVoice 环境
#[tauri::command]
fn switch_sensevoice(use_gpu: bool) -> Result<String, String> {
    switch_sensevoice_env(use_gpu)
}

/// 取消 SenseVoice 转录
#[tauri::command]
fn cancel_sensevoice_task() {
    cancel_sensevoice_transcription();
}

/// 取消 SenseVoice 模型下载
#[tauri::command]
fn cancel_sensevoice_model_download_cmd() {
    cancel_sensevoice_model_download();
}

/// 获取 SenseVoice 模型列表
#[tauri::command]
fn get_sensevoice_model_list() -> Vec<SenseVoiceModelInfo> {
    get_sensevoice_models()
}

/// 下载 SenseVoice 模型
#[tauri::command]
async fn download_sensevoice_model_cmd(window: tauri::Window, model_name: String) -> Result<String, String> {
    download_sensevoice_model(&model_name, window).await
}

/// 删除 SenseVoice 模型
#[tauri::command]
fn delete_sensevoice_model_cmd(model_name: String) -> Result<String, String> {
    delete_sensevoice_model(&model_name)
}

/// 打开 SenseVoice 模型目录
#[tauri::command]
fn open_sensevoice_model_dir_cmd() -> Result<(), String> {
    open_sensevoice_model_dir()
}

// ============ FireRedASR 校正相关命令 ============

/// 检查 FireRedASR 环境状态
#[tauri::command]
fn check_firered_env_status() -> FireRedEnvStatus {
    check_firered_env()
}

/// 安装 FireRedASR 环境
#[tauri::command]
async fn install_firered(window: tauri::Window, use_gpu: Option<bool>) -> Result<String, String> {
    install_firered_env(window, use_gpu.unwrap_or(false)).await
}

/// 使用 FireRedASR 校正字幕
#[tauri::command]
async fn correct_subtitles_with_firered(
    window: tauri::Window,
    srt_path: String,
    audio_path: String,
    language: String,
    preserve_case: Option<bool>,
) -> Result<Vec<CorrectionEntry>, String> {
    correct_with_firered(srt_path, audio_path, language, preserve_case.unwrap_or(true), window).await
}

/// 卸载 FireRedASR 环境
#[tauri::command]
fn uninstall_firered() -> Result<String, String> {
    uninstall_firered_env()
}

/// 卸载指定类型的 FireRedASR 环境
#[tauri::command]
fn uninstall_firered_by_type(use_gpu: bool) -> Result<String, String> {
    uninstall_firered_env_by_type(use_gpu)
}

/// 切换 FireRedASR 环境
#[tauri::command]
fn switch_firered(use_gpu: bool) -> Result<String, String> {
    switch_firered_env(use_gpu)
}

/// 取消 FireRedASR 校正
#[tauri::command]
fn cancel_firered_task() {
    cancel_firered_correction();
}

/// 更新菜单项启用状态
#[tauri::command]
fn update_menu_item_enabled(app_handle: tauri::AppHandle, menu_id: String, enabled: bool) -> Result<(), String> {
    use tauri::menu::MenuItemKind;
    use tauri::Wry;
    
    if let Some(menu) = app_handle.menu() {
        // 遍历菜单项查找目标
        fn find_and_update(items: &[MenuItemKind<Wry>], menu_id: &str, enabled: bool) -> bool {
            for item in items {
                match item {
                    MenuItemKind::MenuItem(menu_item) => {
                        if menu_item.id().0 == menu_id {
                            let _ = menu_item.set_enabled(enabled);
                            return true;
                        }
                    }
                    MenuItemKind::Submenu(submenu) => {
                        if let Ok(sub_items) = submenu.items() {
                            if find_and_update(&sub_items, menu_id, enabled) {
                                return true;
                            }
                        }
                    }
                    MenuItemKind::Check(check_item) => {
                        if check_item.id().0 == menu_id {
                            let _ = check_item.set_enabled(enabled);
                            return true;
                        }
                    }
                    _ => {}
                }
            }
            false
        }
        
        if let Ok(items) = menu.items() {
            find_and_update(&items, &menu_id, enabled);
        }
    }
    Ok(())
}

/// 取消 FireRedASR 模型下载
#[tauri::command]
fn cancel_firered_model_download_cmd() {
    cancel_firered_model_download();
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

/// 预加载音频文件到 FireRedASR 服务缓存
#[tauri::command]
async fn preload_audio_for_firered(audio_path: String) -> Result<String, String> {
    preload_audio_for_correction(audio_path).await
}

/// 获取 FireRedASR 可用模型列表
#[tauri::command]
fn get_firered_models_cmd() -> Vec<FireRedModelInfo> {
    get_firered_models()
}

/// 下载 FireRedASR 模型
#[tauri::command]
async fn download_firered_model_cmd(window: tauri::Window, model_name: String) -> Result<String, String> {
    download_firered_model(&model_name, window).await
}

/// 删除 FireRedASR 模型
#[tauri::command]
fn delete_firered_model_cmd(model_name: String) -> Result<String, String> {
    delete_firered_model(&model_name)
}

/// 打开 FireRedASR 模型目录
#[tauri::command]
fn open_firered_model_dir_cmd() -> Result<(), String> {
    open_firered_model_dir()
}

/// 校正单条字幕
#[tauri::command]
async fn correct_single_subtitle(
    audio_path: String,
    start_ms: u32,
    end_ms: u32,
    original_text: String,
    language: String,
    preserve_case: Option<bool>,
) -> Result<SingleCorrectionResult, String> {
    correct_single_entry(audio_path, start_ms, end_ms, original_text, language, preserve_case.unwrap_or(true)).await
}

// ============ 导出功能 ============

/// 导出为 TXT 格式（纯文本）
#[tauri::command]
fn export_txt(file_path: String, entries: Vec<SubtitleEntry>) -> Result<(), String> {
    export_to_txt(&file_path, &entries)
}

/// 导出为 VTT 格式（WebVTT）
#[tauri::command]
fn export_vtt(file_path: String, entries: Vec<SubtitleEntry>) -> Result<(), String> {
    export_to_vtt(&file_path, &entries)
}

/// 导出为 Markdown 格式
#[tauri::command]
fn export_markdown(file_path: String, entries: Vec<SubtitleEntry>) -> Result<(), String> {
    export_to_markdown(&file_path, &entries)
}

/// 导出为 FCPXML 格式（Final Cut Pro）
#[tauri::command]
fn export_fcpxml(
    file_path: String,
    entries: Vec<SubtitleEntry>,
    fps: f64,
    position_x: Option<i32>,
    position_y: Option<i32>,
) -> Result<(), String> {
    export_to_fcpxml(
        &file_path,
        &entries,
        fps,
        position_x.unwrap_or(0),
        position_y.unwrap_or(-415),
    )
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
            
            let app_menu = SubmenuBuilder::new(&app_handle, "VoSub")
                .text("about", "关于 VoSub")
                .separator()
                .text("quit", "退出 VoSub")
                .build()
                .map_err(|e| e.to_string())?;

            let open_item = MenuItem::with_id(&app_handle, "open", "打开", true, Some("CmdOrCtrl+O")).map_err(|e| e.to_string())?;
            let save_item = MenuItem::with_id(&app_handle, "save", "保存", true, Some("CmdOrCtrl+S")).map_err(|e| e.to_string())?;
            let export_dialog_item = MenuItem::with_id(&app_handle, "export-dialog", "导出", true, Some("Cmd+E")).map_err(|e| e.to_string())?;
            let close_tab_item = MenuItem::with_id(&app_handle, "close-tab", "关闭标签页", true, Some("CmdOrCtrl+W")).map_err(|e| e.to_string())?;
            let close_window_item = MenuItem::with_id(&app_handle, "close-window", "关闭窗口", true, Some("Cmd+Q")).map_err(|e| e.to_string())?;
            let add_dict_item = MenuItem::with_id(&app_handle, "add-to-dictionary", "添加到本地词典", true, Some("Cmd+D")).map_err(|e| e.to_string())?;
            let clear_corrections_item = MenuItem::with_id(&app_handle, "clear-all-corrections", "清除所有校正标记", false, None::<&str>).map_err(|e| e.to_string())?;
            let file_menu = SubmenuBuilder::new(&app_handle, "文件")
                .item(&open_item)
                .item(&recent_menu)
                .item(&save_item)
                .item(&export_dialog_item)
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
                .item(&add_dict_item)
                .separator()
                .text("batch-ai-correction", "批量 AI 字幕校正")
                .item(&clear_corrections_item)
                .separator()
                .text("batch-add-cjk-spaces", "批量添加中英文空格")
                .text("batch-remove-html", "批量移除HTML标签")
                .text("batch-remove-punctuation", "批量删除标点符号")
                .separator()
                .text("batch-to-uppercase", "批量转换为大写")
                .text("batch-to-lowercase", "批量转换为小写")
                .text("batch-to-capitalize", "批量首字母大写")
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
            let export_dialog_item = MenuItem::with_id(&app_handle, "export-dialog", "导出", true, Some("Ctrl+E")).map_err(|e| e.to_string())?;
            let close_tab_item = MenuItem::with_id(&app_handle, "close-tab", "关闭标签页", true, Some("CmdOrCtrl+W")).map_err(|e| e.to_string())?;
            let close_window_item = MenuItem::with_id(&app_handle, "close-window", "关闭窗口", true, Some("Alt+F4")).map_err(|e| e.to_string())?;
            let add_dict_item = MenuItem::with_id(&app_handle, "add-to-dictionary", "添加到本地词典", true, Some("Ctrl+D")).map_err(|e| e.to_string())?;
            let clear_corrections_item = MenuItem::with_id(&app_handle, "clear-all-corrections", "清除所有校正标记", false, None::<&str>).map_err(|e| e.to_string())?;
            let file_menu = SubmenuBuilder::new(&app_handle, "文件")
                .item(&open_item)
                .item(&recent_menu)
                .item(&save_item)
                .item(&export_dialog_item)
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
                .item(&add_dict_item)
                .separator()
                .text("batch-ai-correction", "批量 AI 字幕校正")
                .item(&clear_corrections_item)
                .separator()
                .text("batch-add-cjk-spaces", "批量添加中英文空格")
                .text("batch-remove-html", "批量移除HTML标签")
                .text("batch-remove-punctuation", "批量删除标点符号")
                .separator()
                .text("batch-to-uppercase", "批量转换为大写")
                .text("batch-to-lowercase", "批量转换为小写")
                .text("batch-to-capitalize", "批量首字母大写")
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
            // Windows: 处理命令行参数中的文件路径
            #[cfg(target_os = "windows")]
            {
                let args: Vec<String> = std::env::args().collect();
                // 第一个参数是程序路径，从第二个开始检查
                for arg in args.iter().skip(1) {
                    if arg.to_lowercase().ends_with(".srt") && std::path::Path::new(arg).exists() {
                        info!("Windows: 通过命令行参数打开 SRT 文件: {}", arg);
                        if let Ok(mut pending) = PENDING_FILE_OPEN.lock() {
                            *pending = Some(arg.clone());
                        }
                        break; // 只处理第一个文件
                    }
                }
            }
            
            #[cfg(target_os = "macos")]
            {
                // 在 macOS 上，第一个子菜单会自动成为应用菜单
                let app_menu = SubmenuBuilder::new(app, "VoSub")
                    .text("about", "关于 VoSub")
                    .separator()
                    .text("quit", "退出 VoSub")
                    .build()?;

                // 创建 打开最近的文件 子菜单
                let recent_menu = SubmenuBuilder::new(app, "打开最近的文件")
                    .text("no-recent", "无最近文件")
                    .separator()
                    .text("clear-recent", "清除最近文件")
                    .build()?;

                // 创建 导出 子菜单
                let export_menu = SubmenuBuilder::new(app, "导出")
                    .text("export-txt", "导出为 TXT")
                    .text("export-vtt", "导出为 VTT")
                    .text("export-srt", "导出为 SRT")
                    .text("export-markdown", "导出为 Markdown")
                    .separator()
                    .text("export-fcpxml", "导出为 FCPXML...")
                    .build()?;

                // 创建 文件 菜单（macOS 使用 Cmd）
                let open_item = MenuItem::with_id(app, "open", "打开", true, Some("CmdOrCtrl+O"))?;
                let save_item = MenuItem::with_id(app, "save", "保存", true, Some("CmdOrCtrl+S"))?;
                let export_dialog_item = MenuItem::with_id(app, "export-dialog", "导出...", true, Some("Cmd+E"))?;
                let close_tab_item = MenuItem::with_id(app, "close-tab", "关闭标签页", true, Some("CmdOrCtrl+W"))?;
                let close_window_item = MenuItem::with_id(app, "close-window", "关闭窗口", true, Some("Cmd+Q"))?;
                let add_dict_item = MenuItem::with_id(app, "add-to-dictionary", "添加到本地词典", true, Some("Cmd+D"))?;
                let clear_corrections_item = MenuItem::with_id(app, "clear-all-corrections", "清除所有校正标记", false, None::<&str>)?;
                let file_menu = SubmenuBuilder::new(app, "文件")
                    .item(&open_item)
                    .item(&recent_menu)
                    .item(&save_item)
                    .item(&export_dialog_item)
                    .item(&export_menu)
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
                    .item(&add_dict_item)
                    .separator()
                    .text("batch-ai-correction", "批量 AI 字幕校正")
                    .item(&clear_corrections_item)
                    .separator()
                    .text("batch-add-cjk-spaces", "批量添加中英文空格")
                    .text("batch-remove-html", "批量移除HTML标签")
                    .text("batch-remove-punctuation", "批量删除标点符号")
                    .separator()
                    .text("batch-to-uppercase", "批量转换为大写")
                    .text("batch-to-lowercase", "批量转换为小写")
                    .text("batch-to-capitalize", "批量首字母大写")
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

                // 创建 导出 子菜单
                let export_menu = SubmenuBuilder::new(app, "导出")
                    .text("export-txt", "导出为 TXT")
                    .text("export-vtt", "导出为 VTT")
                    .text("export-srt", "导出为 SRT")
                    .text("export-markdown", "导出为 Markdown")
                    .separator()
                    .text("export-fcpxml", "导出为 FCPXML...")
                    .build()?;

                // 创建 文件 菜单（Windows 使用 Ctrl）
                let open_item = MenuItem::with_id(app, "open", "打开", true, Some("CmdOrCtrl+O"))?;
                let save_item = MenuItem::with_id(app, "save", "保存", true, Some("CmdOrCtrl+S"))?;
                let export_dialog_item = MenuItem::with_id(app, "export-dialog", "导出...", true, Some("Ctrl+E"))?;
                let close_tab_item = MenuItem::with_id(app, "close-tab", "关闭标签页", true, Some("CmdOrCtrl+W"))?;
                let close_window_item = MenuItem::with_id(app, "close-window", "关闭窗口", true, Some("Alt+F4"))?;
                let add_dict_item = MenuItem::with_id(app, "add-to-dictionary", "添加到本地词典", true, Some("Ctrl+D"))?;
                let clear_corrections_item = MenuItem::with_id(app, "clear-all-corrections", "清除所有校正标记", false, None::<&str>)?;
                let file_menu = SubmenuBuilder::new(app, "文件")
                    .item(&open_item)
                    .item(&recent_menu)
                    .item(&save_item)
                    .item(&export_dialog_item)
                    .item(&export_menu)
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
                    .item(&add_dict_item)
                    .separator()
                    .text("batch-ai-correction", "批量 AI 字幕校正")
                    .item(&clear_corrections_item)
                    .separator()
                    .text("batch-add-cjk-spaces", "批量添加中英文空格")
                    .text("batch-remove-html", "批量移除HTML标签")
                    .text("batch-remove-punctuation", "批量删除标点符号")
                    .separator()
                    .text("batch-to-uppercase", "批量转换为大写")
                    .text("batch-to-lowercase", "批量转换为小写")
                    .text("batch-to-capitalize", "批量首字母大写")
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
                    "clear-all-corrections" => {
                        if let Some(window) = app_handle.get_webview_window("main") {
                            let js_code = r#"
                                (async () => {
                                    if (window.__globalClearAllCorrections && typeof window.__globalClearAllCorrections === 'function') {
                                        await window.__globalClearAllCorrections();
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
                    "batch-to-uppercase" => {
                        if let Some(window) = app_handle.get_webview_window("main") {
                            let js_code = r#"
                                (async () => {
                                    if (window.__globalBatchToUpperCase && typeof window.__globalBatchToUpperCase === 'function') {
                                        await window.__globalBatchToUpperCase();
                                    }
                                })();
                            "#;
                            let _ = window.eval(js_code);
                        }
                    }
                    "batch-to-lowercase" => {
                        if let Some(window) = app_handle.get_webview_window("main") {
                            let js_code = r#"
                                (async () => {
                                    if (window.__globalBatchToLowerCase && typeof window.__globalBatchToLowerCase === 'function') {
                                        await window.__globalBatchToLowerCase();
                                    }
                                })();
                            "#;
                            let _ = window.eval(js_code);
                        }
                    }
                    "batch-to-capitalize" => {
                        if let Some(window) = app_handle.get_webview_window("main") {
                            let js_code = r#"
                                (async () => {
                                    if (window.__globalBatchToCapitalize && typeof window.__globalBatchToCapitalize === 'function') {
                                        await window.__globalBatchToCapitalize();
                                    }
                                })();
                            "#;
                            let _ = window.eval(js_code);
                        }
                    }
                    "add-to-dictionary" => {
                        if let Some(window) = app_handle.get_webview_window("main") {
                            let js_code = r#"
                                (async () => {
                                    if (window.__globalQuickAddDictionary && typeof window.__globalQuickAddDictionary === 'function') {
                                        await window.__globalQuickAddDictionary();
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
                    // 导出对话框（Cmd+E）
                    "export-dialog" => {
                        if let Some(window) = app_handle.get_webview_window("main") {
                            let js_code = r#"
                                (async () => {
                                    if (window.__globalShowExportDialog && typeof window.__globalShowExportDialog === 'function') {
                                        await window.__globalShowExportDialog();
                                    }
                                })();
                            "#;
                            let _ = window.eval(js_code);
                        }
                    }
                    // 导出功能
                    id if id.starts_with("export-") => {
                        if let Some(window) = app_handle.get_webview_window("main") {
                            let format = id.strip_prefix("export-").unwrap_or("txt");
                            let js_code = format!(r#"
                                (async () => {{
                                    if (window.__globalExportSubtitles && typeof window.__globalExportSubtitles === 'function') {{
                                        await window.__globalExportSubtitles('{}');
                                    }}
                                }})();
                            "#, format);
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
                    Target::new(TargetKind::LogDir { file_name: Some("vosub".into()) }),
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
            check_file_write_permission,
            unlock_file_cmd,
            read_audio_file,
            generate_audio_waveform,
            trigger_open_file,
            check_file_exists,
            get_pending_file_open,
            update_recent_files_menu,
            get_log_path,
            show_log_in_folder,
            // Whisper 相关
            check_whisper_env_status,
            install_whisper,
            get_whisper_models_cmd,
            download_whisper_model_cmd,
            delete_whisper_model_cmd,
            open_whisper_model_dir_cmd,
            transcribe_audio_to_subtitles,
            cancel_whisper_task,
            cancel_whisper_model_download_cmd,
            uninstall_whisper,
            uninstall_whisper_by_type,
            switch_whisper,
            // SenseVoice 相关
            check_sensevoice_env_status,
            install_sensevoice,
            transcribe_with_sensevoice_model,
            uninstall_sensevoice,
            uninstall_sensevoice_by_type,
            switch_sensevoice,
            cancel_sensevoice_task,
            cancel_sensevoice_model_download_cmd,
            get_sensevoice_model_list,
            download_sensevoice_model_cmd,
            delete_sensevoice_model_cmd,
            open_sensevoice_model_dir_cmd,
            // FireRedASR 校正相关
            check_firered_env_status,
            install_firered,
            correct_subtitles_with_firered,
            correct_single_subtitle,
            preload_firered,
            is_firered_service_running,
            preload_audio_for_firered,
            uninstall_firered,
            uninstall_firered_by_type,
            switch_firered,
            cancel_firered_task,
            cancel_firered_model_download_cmd,
            update_menu_item_enabled,
            get_firered_models_cmd,
            download_firered_model_cmd,
            delete_firered_model_cmd,
            open_firered_model_dir_cmd,
            // 导出功能
            export_txt,
            export_vtt,
            export_markdown,
            export_fcpxml
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app_handle, _event| {
            // macOS: 处理文件关联打开事件（RunEvent::Opened 仅在 macOS 上可用）
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Opened { urls } = _event {
                for url in urls {
                    // 将 file:// URL 转换为路径
                    if let Ok(path) = url.to_file_path() {
                        if let Some(path_str) = path.to_str() {
                            // 检查是否是 .srt 文件
                            if path_str.to_lowercase().ends_with(".srt") {
                                info!("通过文件关联打开 SRT 文件: {}", path_str);
                                let path_string = path_str.to_string();
                                
                                // 存储到全局状态（供前端启动后查询）
                                if let Ok(mut pending) = PENDING_FILE_OPEN.lock() {
                                    *pending = Some(path_string.clone());
                                }
                                
                                // 同时尝试发送事件（如果前端已准备好）
                                if let Some(window) = _app_handle.get_webview_window("main") {
                                    let _ = window.emit("file-association-open", path_string);
                                }
                            }
                        }
                    }
                }
            }
        });
}

use crate::srt_parser::TimeStamp;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use tauri::{Emitter, Window};
use once_cell::sync::Lazy;

// 全局取消标志（校正任务）
static FIRERED_CANCELLED: Lazy<Arc<AtomicBool>> = Lazy::new(|| Arc::new(AtomicBool::new(false)));

// 模型下载任务ID，用于取消旧的下载任务
static FIRERED_MODEL_DOWNLOAD_TASK_ID: Lazy<AtomicU64> = Lazy::new(|| AtomicU64::new(0));

/// 取消当前校正任务
pub fn cancel_firered_correction() {
    FIRERED_CANCELLED.store(true, Ordering::SeqCst);
}

/// 取消当前模型下载任务
pub fn cancel_firered_model_download() {
    // 增加任务ID使当前下载任务失效
    FIRERED_MODEL_DOWNLOAD_TASK_ID.fetch_add(1, Ordering::SeqCst);
    log::info!("FireRedASR model download cancelled by user");
}

/// 生成新的模型下载任务ID
fn new_firered_model_download_task_id() -> u64 {
    FIRERED_MODEL_DOWNLOAD_TASK_ID.fetch_add(1, Ordering::SeqCst) + 1
}

/// 检查模型下载任务是否仍然有效
fn is_firered_model_download_task_valid(task_id: u64) -> bool {
    FIRERED_MODEL_DOWNLOAD_TASK_ID.load(Ordering::SeqCst) == task_id
}

/// 重置取消标志
fn reset_cancellation() {
    FIRERED_CANCELLED.store(false, Ordering::SeqCst);
}

/// 检查是否已取消
fn is_cancelled() -> bool {
    FIRERED_CANCELLED.load(Ordering::SeqCst)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FireRedProgress {
    pub progress: f32,
    pub current_text: String,
    pub status: String,
}

/// 单个环境的状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FireRedEnvInfo {
    pub installed: bool,
    pub ready: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FireRedEnvStatus {
    pub uv_installed: bool,
    pub cpu_env: FireRedEnvInfo,
    pub gpu_env: FireRedEnvInfo,
    pub active_env: String,  // "cpu", "gpu", or "none"
    // 兼容旧字段
    pub env_exists: bool,
    pub ready: bool,
    pub is_gpu: bool,
}

/// FireRedASR 模型信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FireRedModelInfo {
    pub name: String,
    pub size: String,
    pub downloaded: bool,
    pub partial_size: Option<u64>,
}

/// FireRedASR 模型文件信息
struct ModelFileInfo {
    name: &'static str,
    size: u64,
    is_lfs: bool,
}

/// FireRedASR-AED-L 模型需要下载的文件列表
const FIRERED_AED_L_FILES: &[ModelFileInfo] = &[
    ModelFileInfo { name: "model.pth.tar", size: 4678597714, is_lfs: true },
    ModelFileInfo { name: "train_bpe1000.model", size: 251707, is_lfs: true },
    ModelFileInfo { name: "cmvn.ark", size: 1311, is_lfs: true },
    ModelFileInfo { name: "dict.txt", size: 71448, is_lfs: false },
    ModelFileInfo { name: "cmvn.txt", size: 2985, is_lfs: false },
    ModelFileInfo { name: "configuration.json", size: 86, is_lfs: false },
];

/// 校正结果条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrectionEntry {
    pub id: u32,
    pub start_time: TimeStamp,
    pub end_time: TimeStamp,
    pub original: String,
    pub corrected: String,
    pub has_diff: bool,
}

/// Python 脚本输出的校正结果
#[derive(Debug, Deserialize)]
struct CorrectionResult {
    entries: Vec<CorrectionEntryRaw>,
}

#[derive(Debug, Deserialize)]
struct CorrectionEntryRaw {
    id: u32,
    start_ms: u32,
    end_ms: u32,
    original: String,
    corrected: String,
    has_diff: bool,
}

/// 获取 FireRedASR 环境基础目录
fn get_firered_base_dir() -> Result<PathBuf, String> {
    let home_dir = dirs::home_dir()
        .ok_or_else(|| "Failed to get home directory".to_string())?;
    
    Ok(home_dir.join(".config").join("vosub"))
}

/// 获取旧版 FireRedASR 环境目录（用于迁移）
fn get_legacy_firered_env_dir() -> Result<PathBuf, String> {
    let base_dir = get_firered_base_dir()?;
    Ok(base_dir.join("firered-env"))
}

/// 迁移旧版环境到新目录结构
fn migrate_legacy_firered_env() -> Result<bool, String> {
    let legacy_dir = get_legacy_firered_env_dir()?;
    
    if !legacy_dir.exists() {
        return Ok(false); // 没有旧环境，无需迁移
    }
    
    // 检查是否已经迁移过（新目录已存在）
    let cpu_dir = get_firered_cpu_env_dir()?;
    let gpu_dir = get_firered_gpu_env_dir()?;
    
    if cpu_dir.exists() || gpu_dir.exists() {
        // 新目录已存在，删除旧目录
        let _ = std::fs::remove_dir_all(&legacy_dir);
        return Ok(false);
    }
    
    // 旧版本都是 CPU 版本，迁移到 CPU 目录
    std::fs::rename(&legacy_dir, &cpu_dir)
        .map_err(|e| format!("迁移旧环境失败: {}", e))?;
    
    // 设置激活的环境
    set_firered_active_env_type("cpu")?;
    
    Ok(true)
}

/// 获取 FireRedASR CPU 环境目录
pub fn get_firered_cpu_env_dir() -> Result<PathBuf, String> {
    let base_dir = get_firered_base_dir()?;
    Ok(base_dir.join("firered-env-cpu"))
}

/// 获取 FireRedASR GPU 环境目录
pub fn get_firered_gpu_env_dir() -> Result<PathBuf, String> {
    let base_dir = get_firered_base_dir()?;
    Ok(base_dir.join("firered-env-gpu"))
}

/// 获取当前激活的环境配置文件路径
fn get_firered_active_env_config_path() -> Result<PathBuf, String> {
    let base_dir = get_firered_base_dir()?;
    Ok(base_dir.join("firered-active-env"))
}

/// 获取当前激活的环境类型
pub fn get_firered_active_env_type() -> String {
    get_firered_active_env_config_path()
        .ok()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "none".to_string())
}

/// 设置当前激活的环境类型
pub fn set_firered_active_env_type(env_type: &str) -> Result<(), String> {
    let config_path = get_firered_active_env_config_path()?;
    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("创建配置目录失败: {}", e))?;
    }
    std::fs::write(&config_path, env_type)
        .map_err(|e| format!("写入配置失败: {}", e))?;
    Ok(())
}

/// 获取 FireRedASR 环境目录（兼容旧代码，返回当前激活的环境）
pub fn get_firered_env_dir() -> Result<PathBuf, String> {
    let active = get_firered_active_env_type();
    match active.as_str() {
        "gpu" => get_firered_gpu_env_dir(),
        "cpu" => get_firered_cpu_env_dir(),
        _ => {
            // 如果没有激活的环境，检查哪个存在
            let gpu_dir = get_firered_gpu_env_dir()?;
            if gpu_dir.exists() {
                return Ok(gpu_dir);
            }
            let cpu_dir = get_firered_cpu_env_dir()?;
            if cpu_dir.exists() {
                return Ok(cpu_dir);
            }
            // 默认返回 CPU 目录
            Ok(cpu_dir)
        }
    }
}

/// 获取 FireRedASR 模型缓存目录
pub fn get_firered_model_dir() -> Result<PathBuf, String> {
    let home_dir = dirs::home_dir()
        .ok_or_else(|| "Failed to get home directory".to_string())?;
    
    // 使用与 ModelScope 兼容的缓存目录
    let model_dir = home_dir.join(".cache").join("modelscope").join("hub").join("FireRedTeam");
    
    Ok(model_dir)
}

/// 获取模型文件路径（检查多种可能的路径格式）
fn get_firered_model_path(model_name: &str) -> Result<PathBuf, String> {
    let home_dir = dirs::home_dir()
        .ok_or_else(|| "Failed to get home directory".to_string())?;
    
    let hub_dir = home_dir.join(".cache").join("modelscope").join("hub");
    
    // 路径格式1: ~/.cache/modelscope/hub/FireRedTeam/FireRedASR-AED-L
    let path1 = hub_dir.join("FireRedTeam").join(model_name);
    if path1.exists() {
        return Ok(path1);
    }
    
    // 路径格式2: ~/.cache/modelscope/hub/models/FireRedTeam/FireRedASR-AED-L (Mac 上的路径)
    let path2 = hub_dir.join("models").join("FireRedTeam").join(model_name);
    if path2.exists() {
        return Ok(path2);
    }
    
    // 路径格式3: ~/.cache/modelscope/hub/models--FireRedTeam--FireRedASR-AED-L/snapshots/xxx
    let models_dir = hub_dir.join(format!("models--FireRedTeam--{}", model_name));
    if models_dir.exists() {
        let snapshots_dir = models_dir.join("snapshots");
        if snapshots_dir.exists() {
            if let Ok(entries) = std::fs::read_dir(&snapshots_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        return Ok(path);
                    }
                }
            }
        }
        return Ok(models_dir);
    }
    
    // 默认返回路径格式1（用于新下载）
    Ok(path1)
}

/// 检查 FireRedASR 模型是否已下载
pub fn is_firered_model_downloaded(model_name: &str) -> bool {
    let home_dir = match dirs::home_dir() {
        Some(dir) => dir,
        None => return false,
    };
    
    let hub_dir = home_dir.join(".cache").join("modelscope").join("hub");
    
    // 检查路径格式1
    let path1 = hub_dir.join("FireRedTeam").join(model_name);
    if check_firered_model_files_exist(&path1) {
        return true;
    }
    
    // 检查路径格式2
    let path2 = hub_dir.join("models").join("FireRedTeam").join(model_name);
    if check_firered_model_files_exist(&path2) {
        return true;
    }
    
    // 检查路径格式3
    let models_dir = hub_dir.join(format!("models--FireRedTeam--{}", model_name));
    if models_dir.exists() {
        let snapshots_dir = models_dir.join("snapshots");
        if snapshots_dir.exists() {
            if let Ok(entries) = std::fs::read_dir(&snapshots_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() && check_firered_model_files_exist(&path) {
                        return true;
                    }
                }
            }
        }
        if check_firered_model_files_exist(&models_dir) {
            return true;
        }
    }
    
    false
}

/// 检查模型文件是否存在
fn check_firered_model_files_exist(model_path: &std::path::Path) -> bool {
    if !model_path.exists() {
        return false;
    }
    
    // 检查主要模型文件是否存在
    let model_pth = model_path.join("model.pth.tar");
    let config = model_path.join("configuration.json");
    
    model_pth.exists() && config.exists()
}

/// 获取已下载的部分大小
pub fn get_firered_partial_size(model_name: &str) -> u64 {
    let model_path = match get_firered_model_path(model_name) {
        Ok(path) => path,
        Err(_) => return 0,
    };
    
    let mut total_partial = 0u64;
    
    // 检查已下载的文件大小
    for file_info in FIRERED_AED_L_FILES {
        let file_path = model_path.join(file_info.name);
        let part_path = model_path.join(format!("{}.part", file_info.name));
        
        if file_path.exists() {
            if let Ok(meta) = std::fs::metadata(&file_path) {
                total_partial += meta.len();
            }
        } else if part_path.exists() {
            if let Ok(meta) = std::fs::metadata(&part_path) {
                total_partial += meta.len();
            }
        }
    }
    
    total_partial
}

/// 获取 FireRedASR 可用模型列表
pub fn get_firered_models() -> Vec<FireRedModelInfo> {
    let downloaded = is_firered_model_downloaded("FireRedASR-AED-L");
    let partial_size = if !downloaded {
        let size = get_firered_partial_size("FireRedASR-AED-L");
        if size > 0 { Some(size) } else { None }
    } else {
        None
    };
    
    vec![
        FireRedModelInfo {
            name: "FireRedASR-AED-L".to_string(),
            size: "~4.4 GB".to_string(),
            downloaded,
            partial_size,
        },
    ]
}

/// 下载 FireRedASR 模型（支持断点续传）
pub async fn download_firered_model(model_name: &str, window: Window) -> Result<String, String> {
    use std::fs::{self, OpenOptions};
    use std::io::Write;
    
    // 生成新的任务ID，使之前的下载任务失效
    let task_id = new_firered_model_download_task_id();
    
    // 检查环境是否就绪
    let env_status = check_firered_env();
    if !env_status.ready {
        return Err("FireRedASR 环境未安装，请先安装环境".to_string());
    }
    
    // 检查是否已下载
    if is_firered_model_downloaded(model_name) {
        return Ok(format!("{} 模型已下载", model_name));
    }
    
    let model_path = get_firered_model_path(model_name)?;
    
    // 创建模型目录
    if !model_path.exists() {
        fs::create_dir_all(&model_path)
            .map_err(|e| format!("创建模型目录失败: {}", e))?;
    }
    
    // 计算总大小
    let total_size: u64 = FIRERED_AED_L_FILES.iter().map(|f| f.size).sum();
    let mut downloaded_total: u64 = 0;
    
    // 发送初始进度
    let _ = window.emit("firered-model-progress", FireRedProgress {
        progress: 0.0,
        current_text: "0.0%".to_string(),
        status: "downloading".to_string(),
    });
    
    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;
    
    // 下载每个文件
    for file_info in FIRERED_AED_L_FILES.iter() {
        // 检查任务是否仍然有效
        if !is_firered_model_download_task_valid(task_id) {
            return Err("下载已取消".to_string());
        }
        
        let file_path = model_path.join(file_info.name);
        let part_path = model_path.join(format!("{}.part", file_info.name));
        
        // 如果文件已存在且大小正确，跳过
        if file_path.exists() {
            if let Ok(meta) = fs::metadata(&file_path) {
                if meta.len() == file_info.size {
                    downloaded_total += file_info.size;
                    continue;
                }
            }
        }
        
        // 检查部分下载
        let existing_size = if part_path.exists() {
            fs::metadata(&part_path).map(|m| m.len()).unwrap_or(0)
        } else {
            0
        };
        
        // 构建下载 URL
        let download_url = format!(
            "https://modelscope.cn/models/FireRedTeam/{}/resolve/master/{}",
            model_name, file_info.name
        );
        
        // 发送进度
        let progress = (downloaded_total as f32 / total_size as f32) * 100.0;
        let _ = window.emit("firered-model-progress", FireRedProgress {
            progress,
            current_text: format!("{:.1}%", progress),
            status: "downloading".to_string(),
        });
        
        // 构建请求
        let mut request = client.get(&download_url);
        if existing_size > 0 {
            request = request.header("Range", format!("bytes={}-", existing_size));
        }
        
        let response = request.send().await
            .map_err(|e| format!("下载 {} 失败: {}", file_info.name, e))?;
        
        let status = response.status();
        let is_partial = status == reqwest::StatusCode::PARTIAL_CONTENT;
        
        if !status.is_success() && !is_partial {
            return Err(format!("下载 {} 失败: HTTP {}", file_info.name, status));
        }
        
        // 确定实际起始位置
        let actual_start = if is_partial { existing_size } else {
            if existing_size > 0 {
                let _ = fs::remove_file(&part_path);
            }
            0
        };
        
        // 打开文件
        let (mut file, mut file_downloaded) = if actual_start > 0 {
            let file = OpenOptions::new()
                .append(true)
                .open(&part_path)
                .map_err(|e| format!("打开部分文件失败: {}", e))?;
            (file, actual_start)
        } else {
            let file = fs::File::create(&part_path)
                .map_err(|e| format!("创建文件失败: {}", e))?;
            (file, 0u64)
        };
        
        // 流式下载
        let mut response = response;
        while let Some(chunk) = response.chunk().await
            .map_err(|e| format!("读取数据失败: {}", e))? 
        {
            // 检查任务是否仍然有效
            if !is_firered_model_download_task_valid(task_id) {
                return Err("下载已取消".to_string());
            }
            
            file.write_all(&chunk)
                .map_err(|e| format!("写入文件失败: {}", e))?;
            
            file_downloaded += chunk.len() as u64;
            
            // 更新进度
            let current_total = downloaded_total + file_downloaded;
            let progress = (current_total as f32 / total_size as f32) * 100.0;
            let _ = window.emit("firered-model-progress", FireRedProgress {
                progress,
                current_text: format!("{:.1}%", progress),
                status: "downloading".to_string(),
            });
        }
        
        file.flush().map_err(|e| format!("刷新文件失败: {}", e))?;
        drop(file);
        
        // 验证文件大小
        let final_size = fs::metadata(&part_path).map(|m| m.len()).unwrap_or(0);
        if final_size != file_info.size {
            return Err(format!(
                "文件 {} 下载不完整: 期望 {} 字节, 实际 {} 字节",
                file_info.name, file_info.size, final_size
            ));
        }
        
        // 重命名为最终文件
        fs::rename(&part_path, &file_path)
            .map_err(|e| format!("重命名文件失败: {}", e))?;
        
        downloaded_total += file_info.size;
    }
    
    // 发送完成进度
    let _ = window.emit("firered-model-progress", FireRedProgress {
        progress: 100.0,
        current_text: "模型下载完成！".to_string(),
        status: "completed".to_string(),
    });
    
    Ok(format!("{} 模型下载成功", model_name))
}

/// 删除 FireRedASR 模型
pub fn delete_firered_model(model_name: &str) -> Result<String, String> {
    let model_path = get_firered_model_path(model_name)?;
    
    if !model_path.exists() {
        return Err(format!("模型 {} 未下载", model_name));
    }
    
    std::fs::remove_dir_all(&model_path)
        .map_err(|e| format!("删除模型失败: {}", e))?;
    
    Ok(format!("模型 {} 已删除", model_name))
}

/// 打开 FireRedASR 模型目录
pub fn open_firered_model_dir() -> Result<(), String> {
    // 尝试获取实际的模型路径（如果已下载）
    let model_dir = if is_firered_model_downloaded("FireRedASR-AED-L") {
        // 如果模型已下载，直接打开模型目录
        match get_firered_model_path("FireRedASR-AED-L") {
            Ok(path) => path,
            Err(_) => get_firered_model_dir()?,
        }
    } else {
        // 如果模型未下载，打开 modelscope hub 目录
        let home_dir = dirs::home_dir()
            .ok_or_else(|| "Failed to get home directory".to_string())?;
        home_dir.join(".cache").join("modelscope").join("hub")
    };
    
    // 如果目录不存在，创建它
    if !model_dir.exists() {
        std::fs::create_dir_all(&model_dir)
            .map_err(|e| format!("创建目录失败: {}", e))?;
    }
    
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

/// 获取 Python 脚本目录
fn get_scripts_dir() -> Result<PathBuf, String> {
    let home_dir = dirs::home_dir()
        .ok_or_else(|| "Failed to get home directory".to_string())?;
    
    let scripts_dir = home_dir
        .join(".config")
        .join("vosub")
        .join("scripts");
    
    if !scripts_dir.exists() {
        std::fs::create_dir_all(&scripts_dir)
            .map_err(|e| format!("Failed to create scripts directory: {}", e))?;
    }
    
    Ok(scripts_dir)
}

/// 获取指定环境的 Python 可执行文件路径
fn get_python_path_for_env(env_dir: &PathBuf) -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        env_dir.join("Scripts").join("python.exe")
    }
    
    #[cfg(not(target_os = "windows"))]
    {
        env_dir.join("bin").join("python")
    }
}

/// 获取 Python 可执行文件路径（当前激活的环境）
fn get_python_path() -> Result<PathBuf, String> {
    let env_dir = get_firered_env_dir()?;
    Ok(get_python_path_for_env(&env_dir))
}

/// 检查 uv 是否已安装
fn check_uv_installed() -> bool {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        
        // 先尝试 PATH 中的 uv
        let path_check = Command::new("uv")
            .arg("--version")
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        
        if path_check {
            return true;
        }
        
        // 检查默认安装路径 %USERPROFILE%\.local\bin\uv.exe
        if let Some(home) = std::env::var_os("USERPROFILE") {
            let uv_path = std::path::PathBuf::from(home).join(".local").join("bin").join("uv.exe");
            if uv_path.exists() {
                return Command::new(&uv_path)
                    .arg("--version")
                    .creation_flags(CREATE_NO_WINDOW)
                    .output()
                    .map(|o| o.status.success())
                    .unwrap_or(false);
            }
        }
        
        false
    }
    
    #[cfg(not(target_os = "windows"))]
    {
        // 先尝试 PATH 中的 uv
        let path_check = Command::new("uv")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        
        if path_check {
            return true;
        }
        
        // 检查默认安装路径 ~/.local/bin/uv
        if let Some(home) = std::env::var_os("HOME") {
            let uv_path = std::path::PathBuf::from(home).join(".local").join("bin").join("uv");
            if uv_path.exists() {
                return Command::new(&uv_path)
                    .arg("--version")
                    .output()
                    .map(|o| o.status.success())
                    .unwrap_or(false);
            }
        }
        
        false
    }
}

/// 获取 uv 可执行文件路径
fn get_uv_path() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        
        // 先检查 PATH 中的 uv
        if Command::new("uv")
            .arg("--version")
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            return Some(PathBuf::from("uv"));
        }
        // 检查默认安装路径
        if let Some(home) = std::env::var_os("USERPROFILE") {
            let uv_path = PathBuf::from(home).join(".local").join("bin").join("uv.exe");
            if uv_path.exists() {
                return Some(uv_path);
            }
        }
        None
    }
    
    #[cfg(not(target_os = "windows"))]
    {
        // 先检查 PATH 中的 uv
        if Command::new("uv")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            return Some(PathBuf::from("uv"));
        }
        // 检查默认安装路径
        if let Some(home) = std::env::var_os("HOME") {
            let uv_path = PathBuf::from(home).join(".local").join("bin").join("uv");
            if uv_path.exists() {
                return Some(uv_path);
            }
        }
        None
    }
}

/// 检查指定环境是否就绪
fn check_firered_env_ready(env_dir: &PathBuf) -> bool {
    let python_path = get_python_path_for_env(env_dir);
    if !env_dir.exists() || !python_path.exists() {
        return false;
    }
    
    #[cfg(target_os = "windows")]
    {
        let fireredasr_path = env_dir.join("Lib").join("site-packages").join("fireredasr");
        fireredasr_path.exists()
    }
    #[cfg(not(target_os = "windows"))]
    {
        let site_packages = env_dir.join("lib");
        if site_packages.exists() {
            std::fs::read_dir(&site_packages)
                .ok()
                .and_then(|entries| {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_dir() && path.file_name().map(|n| n.to_string_lossy().starts_with("python")).unwrap_or(false) {
                            let sp = path.join("site-packages").join("fireredasr");
                            if sp.exists() {
                                return Some(true);
                            }
                        }
                    }
                    None
                })
                .unwrap_or(false)
        } else {
            false
        }
    }
}

/// 检查 FireRedASR 环境状态（快速检查，不启动 Python）
pub fn check_firered_env() -> FireRedEnvStatus {
    // 先尝试迁移旧版环境
    let _ = migrate_legacy_firered_env();
    
    let uv_installed = check_uv_installed();
    
    // 检查 CPU 环境
    let cpu_dir = get_firered_cpu_env_dir().unwrap_or_default();
    let cpu_installed = cpu_dir.exists();
    let cpu_ready = check_firered_env_ready(&cpu_dir);
    
    // 检查 GPU 环境
    let gpu_dir = get_firered_gpu_env_dir().unwrap_or_default();
    let gpu_installed = gpu_dir.exists();
    let gpu_ready = check_firered_env_ready(&gpu_dir);
    
    // 获取当前激活的环境
    let mut active_env = get_firered_active_env_type();
    
    // 如果激活的环境不存在，自动切换到可用的环境
    if active_env == "gpu" && !gpu_ready {
        if cpu_ready {
            let _ = set_firered_active_env_type("cpu");
            active_env = "cpu".to_string();
        } else {
            let _ = set_firered_active_env_type("none");
            active_env = "none".to_string();
        }
    } else if active_env == "cpu" && !cpu_ready {
        if gpu_ready {
            let _ = set_firered_active_env_type("gpu");
            active_env = "gpu".to_string();
        } else {
            let _ = set_firered_active_env_type("none");
            active_env = "none".to_string();
        }
    } else if active_env == "none" {
        // 自动选择一个可用的环境
        if gpu_ready {
            let _ = set_firered_active_env_type("gpu");
            active_env = "gpu".to_string();
        } else if cpu_ready {
            let _ = set_firered_active_env_type("cpu");
            active_env = "cpu".to_string();
        }
    }
    
    // 兼容旧字段
    let env_exists = cpu_installed || gpu_installed;
    let ready = cpu_ready || gpu_ready;
    let is_gpu = active_env == "gpu";
    
    FireRedEnvStatus {
        uv_installed,
        cpu_env: FireRedEnvInfo {
            installed: cpu_installed,
            ready: cpu_ready,
        },
        gpu_env: FireRedEnvInfo {
            installed: gpu_installed,
            ready: gpu_ready,
        },
        active_env,
        env_exists,
        ready,
        is_gpu,
    }
}


/// 安装 FireRedASR 环境
/// use_gpu: 是否安装 GPU 版本（需要 NVIDIA 显卡和 CUDA）
pub async fn install_firered_env(window: Window, use_gpu: bool) -> Result<String, String> {
    reset_cancellation();
    
    // 获取 uv 路径
    let uv_path = get_uv_path()
        .ok_or("请先安装 uv 包管理器。访问 https://docs.astral.sh/uv/getting-started/installation/ 了解安装方法")?;
    
    // 根据版本选择对应的环境目录
    let env_dir = if use_gpu {
        get_firered_gpu_env_dir()?
    } else {
        get_firered_cpu_env_dir()?
    };
    
    let version_type = if use_gpu { "GPU" } else { "CPU" };
    
    let _ = window.emit("firered-progress", FireRedProgress {
        progress: 10.0,
        current_text: format!("正在创建 Python 虚拟环境（{} 版本）...", version_type),
        status: "installing".to_string(),
    });
    
    if is_cancelled() {
        return Err("安装已取消".to_string());
    }
    
    // 创建虚拟环境
    let output = Command::new(&uv_path)
        .args(["venv", env_dir.to_str().unwrap(), "--python", "3.11"])
        .output()
        .map_err(|e| format!("创建虚拟环境失败: {}", e))?;
    
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("创建虚拟环境失败: {}", stderr));
    }
    
    if is_cancelled() {
        return Err("安装已取消".to_string());
    }
    
    let _ = window.emit("firered-progress", FireRedProgress {
        progress: 30.0,
        current_text: format!("正在安装 PyTorch {} 版本（可能需要几分钟）...", version_type),
        status: "installing".to_string(),
    });
    
    let python_path = get_python_path_for_env(&env_dir);
    
    // 根据是否使用 GPU 选择不同的 PyTorch 安装方式
    let output = if use_gpu {
        // GPU 版本：安装 CUDA 12.4 版本的 PyTorch
        Command::new(&uv_path)
            .args([
                "pip", "install",
                "--python", python_path.to_str().unwrap(),
                "torch", "torchaudio",
                "--index-url", "https://download.pytorch.org/whl/cu124"
            ])
            .output()
            .map_err(|e| format!("安装 PyTorch GPU 版本失败: {}", e))?
    } else {
        // CPU 版本
        Command::new(&uv_path)
            .args([
                "pip", "install",
                "--python", python_path.to_str().unwrap(),
                "torch", "torchaudio",
                "--index-url", "https://download.pytorch.org/whl/cpu"
            ])
            .output()
            .map_err(|e| format!("安装 PyTorch 失败: {}", e))?
    };
    
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("安装 PyTorch 失败: {}", stderr));
    }
    
    if is_cancelled() {
        return Err("安装已取消".to_string());
    }
    
    let _ = window.emit("firered-progress", FireRedProgress {
        progress: 60.0,
        current_text: "正在安装 FireRedASR...".to_string(),
        status: "installing".to_string(),
    });
    
    // 安装 fireredasr 及依赖（包含 modelscope 用于从国内源下载模型）
    let output = Command::new(&uv_path)
        .args([
            "pip", "install",
            "--python", python_path.to_str().unwrap(),
            "fireredasr", "pydub", "transformers", "sentencepiece", "modelscope"
        ])
        .output()
        .map_err(|e| format!("安装 FireRedASR 失败: {}", e))?;
    
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("安装 FireRedASR 失败: {}", stderr));
    }
    
    if is_cancelled() {
        return Err("安装已取消".to_string());
    }
    
    let _ = window.emit("firered-progress", FireRedProgress {
        progress: 85.0,
        current_text: "正在配置校正脚本...".to_string(),
        status: "installing".to_string(),
    });
    
    write_correction_script()?;
    
    // 设置为当前激活的环境
    let env_type = if use_gpu { "gpu" } else { "cpu" };
    set_firered_active_env_type(env_type)?;
    
    let _ = window.emit("firered-progress", FireRedProgress {
        progress: 100.0,
        current_text: format!("FireRedASR {} 版本安装完成！", version_type),
        status: "completed".to_string(),
    });
    
    Ok(format!("FireRedASR {} 版本安装成功", version_type))
}

/// 切换当前使用的 FireRedASR 环境
pub fn switch_firered_env(use_gpu: bool) -> Result<String, String> {
    let env_type = if use_gpu { "gpu" } else { "cpu" };
    let env_dir = if use_gpu {
        get_firered_gpu_env_dir()?
    } else {
        get_firered_cpu_env_dir()?
    };
    
    if !check_firered_env_ready(&env_dir) {
        return Err(format!("{} 环境未安装或不完整", if use_gpu { "GPU" } else { "CPU" }));
    }
    
    // 停止当前服务（如果正在运行），以便使用新环境
    stop_service();
    
    set_firered_active_env_type(env_type)?;
    Ok(format!("已切换到 {} 版本", if use_gpu { "GPU" } else { "CPU" }))
}

/// 在 Rust 端检测 GPU 信息
fn detect_gpu_info(python_path: &std::path::Path) -> Option<String> {
    // 使用 Python 快速检测 GPU
    let detect_script = r#"
import sys
try:
    import torch
    if torch.cuda.is_available():
        name = torch.cuda.get_device_name(0)
        print(f"GPU ({name})")
    else:
        print("CPU")
except:
    print("CPU")
"#;
    
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        
        let output = Command::new(python_path)
            .args(["-c", detect_script])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .ok()?;
        
        if output.status.success() {
            let result = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !result.is_empty() {
                return Some(result);
            }
        }
    }
    
    #[cfg(not(target_os = "windows"))]
    {
        let output = Command::new(python_path)
            .args(["-c", detect_script])
            .output()
            .ok()?;
        
        if output.status.success() {
            let result = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !result.is_empty() {
                return Some(result);
            }
        }
    }
    
    None
}

/// 写入 Python 校正脚本
fn write_correction_script() -> Result<(), String> {
    let scripts_dir = get_scripts_dir()?;
    let script_path = scripts_dir.join("firered_correct.py");
    
    let script_content = r#"#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""FireRedASR 字幕校正脚本 v4 - 使用文件传递进度"""

import sys
import os
import json
import tempfile

# 进度文件路径（通过环境变量传入）
PROGRESS_FILE = os.environ.get('FIRERED_PROGRESS_FILE', '')

def emit_progress(progress, current, total, text, device_info=False):
    """写入进度到文件"""
    if not PROGRESS_FILE:
        return
    try:
        msg = {
            "progress": progress,
            "current": current,
            "total": total,
            "text": text,
            "device_info": device_info
        }
        with open(PROGRESS_FILE, 'w', encoding='utf-8') as f:
            json.dump(msg, f, ensure_ascii=False)
    except:
        pass

import argparse
import re
from pydub import AudioSegment

def get_firered_model_path():
    """获取 FireRedASR 模型路径"""
    home = os.path.expanduser("~")
    hub_dir = os.path.join(home, ".cache", "modelscope", "hub")
    
    # 路径格式1: ~/.cache/modelscope/hub/FireRedTeam/FireRedASR-AED-L
    path1 = os.path.join(hub_dir, "FireRedTeam", "FireRedASR-AED-L")
    if os.path.exists(path1):
        return path1
    
    # 路径格式2: ~/.cache/modelscope/hub/models/FireRedTeam/FireRedASR-AED-L
    path2 = os.path.join(hub_dir, "models", "FireRedTeam", "FireRedASR-AED-L")
    if os.path.exists(path2):
        return path2
    
    # 路径格式3: ~/.cache/modelscope/hub/models--FireRedTeam--FireRedASR-AED-L/snapshots/xxx
    models_dir = os.path.join(hub_dir, "models--FireRedTeam--FireRedASR-AED-L")
    if os.path.exists(models_dir):
        snapshots_dir = os.path.join(models_dir, "snapshots")
        if os.path.exists(snapshots_dir):
            for entry in os.listdir(snapshots_dir):
                entry_path = os.path.join(snapshots_dir, entry)
                if os.path.isdir(entry_path):
                    return entry_path
        return models_dir
    
    # 默认返回路径格式1
    return path1

def parse_srt_time(time_str):
    """解析 SRT 时间格式 00:00:01,000 -> 毫秒"""
    parts = time_str.replace(',', ':').split(':')
    hours, minutes, seconds, ms = int(parts[0]), int(parts[1]), int(parts[2]), int(parts[3])
    return hours * 3600000 + minutes * 60000 + seconds * 1000 + ms

def parse_srt(srt_path):
    """解析 SRT 文件"""
    with open(srt_path, 'r', encoding='utf-8') as f:
        content = f.read()
    
    # 按空行分割
    blocks = re.split(r'\n\s*\n', content.strip())
    entries = []
    
    for block in blocks:
        lines = block.strip().split('\n')
        if len(lines) >= 3:
            try:
                entry_id = int(lines[0])
                time_line = lines[1]
                text = '\n'.join(lines[2:])
                
                # 解析时间
                times = time_line.split(' --> ')
                start_ms = parse_srt_time(times[0].strip())
                end_ms = parse_srt_time(times[1].strip())
                
                entries.append({
                    'id': entry_id,
                    'start_ms': start_ms,
                    'end_ms': end_ms,
                    'text': text
                })
            except (ValueError, IndexError):
                continue
    
    return entries

def preserve_original_case(original, corrected):
    """保留原始文本中英文字母的大小写
    
    FireRedASR 会将英文字母转换为大写，这个函数将校正后的文本中的英文字母
    恢复为原始文本中对应位置的大小写。
    
    算法：
    1. 提取原始文本中的所有英文字母及其大小写
    2. 提取校正文本中的所有英文字母
    3. 如果两者的字母序列（忽略大小写）相同，则按原始大小写恢复
    """
    if not original or not corrected:
        return corrected
    
    # 提取原始文本中的英文字母
    orig_letters = [(i, c) for i, c in enumerate(original) if c.isalpha() and c.isascii()]
    
    # 提取校正文本中的英文字母位置
    corr_letter_positions = [i for i, c in enumerate(corrected) if c.isalpha() and c.isascii()]
    
    # 如果字母数量不同，无法简单映射，返回原始校正结果
    if len(orig_letters) != len(corr_letter_positions):
        return corrected
    
    # 检查字母序列是否相同（忽略大小写）
    orig_seq = ''.join(c.lower() for _, c in orig_letters)
    corr_seq = ''.join(corrected[i].lower() for i in corr_letter_positions)
    
    if orig_seq != corr_seq:
        # 字母序列不同，说明校正改变了内容，保留校正结果
        return corrected
    
    # 恢复原始大小写
    result = list(corrected)
    for (_, orig_char), corr_pos in zip(orig_letters, corr_letter_positions):
        if orig_char.isupper():
            result[corr_pos] = result[corr_pos].upper()
        else:
            result[corr_pos] = result[corr_pos].lower()
    
    return ''.join(result)

def load_firered_model_from_local(model_dir):
    """从本地目录加载 FireRedASR AED 模型"""
    emit_progress(1, 0, 0, "正在加载 PyTorch...", False)
    import torch
    import argparse as argparse_module
    
    emit_progress(1.5, 0, 0, "正在加载 FireRedASR 模块...", False)
    from fireredasr.data.asr_feat import ASRFeatExtractor
    from fireredasr.models.fireredasr_aed import FireRedAsrAed
    from fireredasr.tokenizer.aed_tokenizer import ChineseCharEnglishSpmTokenizer
    
    # 修复 PyTorch 2.6+ 的兼容性问题
    torch.serialization.add_safe_globals([argparse_module.Namespace])
    
    # 检测是否有 GPU 可用
    use_gpu = torch.cuda.is_available()
    device = "cuda" if use_gpu else "cpu"
    device_name = torch.cuda.get_device_name(0) if use_gpu else "CPU"
    
    # 发送设备信息
    emit_progress(2, 0, 0, f"使用设备: {device.upper()} ({device_name})", True)
    
    emit_progress(2.5, 0, 0, "正在加载特征提取器...", False)
    cmvn_path = os.path.join(model_dir, "cmvn.ark")
    feat_extractor = ASRFeatExtractor(cmvn_path)
    
    emit_progress(3, 0, 0, "正在加载模型权重 (约4.4GB)...", False)
    model_path = os.path.join(model_dir, "model.pth.tar")
    package = torch.load(model_path, map_location=lambda storage, loc: storage, weights_only=False)
    model = FireRedAsrAed.from_args(package["args"])
    model.load_state_dict(package["model_state_dict"], strict=True)
    model.eval()
    
    # 如果有 GPU，将模型移到 GPU
    if use_gpu:
        emit_progress(3.5, 0, 0, "正在将模型移至 GPU...", False)
        model = model.cuda()
    
    emit_progress(4, 0, 0, "正在加载分词器...", False)
    dict_path = os.path.join(model_dir, "dict.txt")
    spm_model = os.path.join(model_dir, "train_bpe1000.model")
    tokenizer = ChineseCharEnglishSpmTokenizer(dict_path, spm_model)
    
    return feat_extractor, model, tokenizer, use_gpu

def correct_subtitles(srt_path, audio_path, language="zh", preserve_case=True):
    """使用 FireRedASR 校正字幕"""
    import torch
    
    # 解析 SRT
    emit_progress(0.2, 0, 0, "正在解析字幕文件...", False)
    entries = parse_srt(srt_path)
    if not entries:
        return {"entries": []}
    
    # 加载音频
    emit_progress(0.3, 0, 0, f"正在加载音频文件...", False)
    audio = AudioSegment.from_file(audio_path)
    
    # 加载模型 (使用本地已下载的模型)
    model_dir = get_firered_model_path()
    if not os.path.exists(model_dir):
        raise RuntimeError(f"模型未下载，请先在设置中下载 FireRedASR-AED-L 模型")
    feat_extractor, model, tokenizer, use_gpu = load_firered_model_from_local(model_dir)
    
    # 发送模型加载完成消息
    device_str = f"GPU: {torch.cuda.get_device_name(0)}" if use_gpu else "CPU"
    emit_progress(5, 0, len(entries), f"模型加载完成 ({device_str})，开始校正 {len(entries)} 条字幕...", False)
    
    # 创建临时目录
    tmp_dir = tempfile.mkdtemp()
    
    results = []
    total = len(entries)
    
    try:
        for i, entry in enumerate(entries):
            # 输出进度（JSON 格式，包含当前字幕信息）
            # 进度从 5% 开始（前面 0-5% 是设备检测和模型加载），到 100% 结束
            progress = 5 + (i + 1) / total * 95
            text_preview = entry['text'][:30].replace('\n', ' ')
            emit_progress(progress, i + 1, total, text_preview, False)
            
            start_ms = entry['start_ms']
            end_ms = entry['end_ms']
            original_text = entry['text']
            
            # 切分音频片段，转换为单声道 16kHz（FireRedASR 要求）
            chunk = audio[start_ms:end_ms]
            chunk = chunk.set_channels(1)  # 转为单声道
            chunk = chunk.set_frame_rate(16000)  # 16kHz 采样率
            chunk_file = os.path.join(tmp_dir, f"chunk_{i}.wav")
            chunk.export(chunk_file, format="wav")
            
            # 识别
            try:
                # 提取特征
                feats, lengths, _ = feat_extractor([chunk_file])
                
                # 如果使用 GPU，将数据移到 GPU
                if use_gpu:
                    feats = feats.cuda()
                    lengths = lengths.cuda()
                
                # 使用模型进行识别
                hyps = model.transcribe(
                    feats,
                    lengths,
                    beam_size=1,
                    nbest=1,
                    decode_max_len=0,
                    softmax_smoothing=1.0,
                    length_penalty=0.0,
                    eos_penalty=1.0,
                )
                
                # 解码结果
                if hyps:
                    hyp = hyps[0][0]  # 取第一个结果的 1-best
                    hyp_ids = [int(id) for id in hyp["yseq"].cpu()]
                    corrected_text = tokenizer.detokenize(hyp_ids).strip()
                else:
                    corrected_text = ""
                
                # 如果启用了保留大小写，恢复原始英文大小写
                if preserve_case and corrected_text:
                    corrected_text = preserve_original_case(original_text, corrected_text)
            except Exception as e:
                print(f"识别片段 {i+1} 失败: {e}", file=sys.stderr)
                import traceback
                traceback.print_exc(file=sys.stderr)
                corrected_text = original_text
            
            # 清理临时文件
            os.remove(chunk_file)
            
            # 比较差异 - 如果识别结果为空，使用原文，不算差异
            final_text = corrected_text if corrected_text else original_text
            has_diff = original_text.strip() != final_text.strip()
            
            results.append({
                "id": entry['id'],
                "start_ms": start_ms,
                "end_ms": end_ms,
                "original": original_text,
                "corrected": final_text,
                "has_diff": has_diff
            })
    finally:
        # 清理临时目录
        try:
            os.rmdir(tmp_dir)
        except:
            pass
    
    return {"entries": results}

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("srt_path", help="SRT 字幕文件路径")
    parser.add_argument("audio_path", help="音频文件路径")
    parser.add_argument("--language", default="zh", help="语言代码")
    parser.add_argument("--output", help="输出 JSON 文件路径")
    parser.add_argument("--preserve-case", action="store_true", default=True, help="保留原始英文大小写")
    parser.add_argument("--no-preserve-case", action="store_false", dest="preserve_case", help="不保留原始英文大小写")
    args = parser.parse_args()
    
    try:
        result = correct_subtitles(args.srt_path, args.audio_path, args.language, args.preserve_case)
        if args.output:
            with open(args.output, "w", encoding="utf-8") as f:
                json.dump(result, f, ensure_ascii=False, indent=2)
        else:
            print(json.dumps(result, ensure_ascii=False))
    except Exception as e:
        print(json.dumps({"error": str(e)}), file=sys.stderr)
        sys.exit(1)

if __name__ == "__main__":
    main()
"#;
    
    std::fs::write(&script_path, script_content)
        .map_err(|e| format!("写入脚本失败: {}", e))?;
    
    log::info!("[FireRed] 脚本已写入: {:?}", script_path);
    
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&script_path)
            .map_err(|e| format!("获取文件权限失败: {}", e))?
            .permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&script_path, perms)
            .map_err(|e| format!("设置文件权限失败: {}", e))?;
    }
    
    Ok(())
}


/// 毫秒转 TimeStamp
fn ms_to_timestamp(ms: u32) -> TimeStamp {
    TimeStamp {
        hours: ms / 3600000,
        minutes: (ms % 3600000) / 60000,
        seconds: (ms % 60000) / 1000,
        milliseconds: ms % 1000,
    }
}

/// 使用 FireRedASR 校正字幕
pub async fn correct_with_firered(
    srt_path: String,
    audio_path: String,
    language: String,
    preserve_case: bool,
    window: Window,
) -> Result<Vec<CorrectionEntry>, String> {
    reset_cancellation();
    
    // 检查环境
    let env_status = check_firered_env();
    if !env_status.ready {
        return Err("FireRedASR 环境未安装，请先安装环境".to_string());
    }
    
    let python_path = get_python_path()?;
    let scripts_dir = get_scripts_dir()?;
    let script_path = scripts_dir.join("firered_correct.py");
    
    // 确保脚本是最新的
    write_correction_script()?;
    
    // 记录开始时间
    let start_time = std::time::Instant::now();
    log::info!("[FireRed] ========== AI 校正开始 ==========");
    
    // 在 Rust 端检测 GPU 信息
    let device_info = detect_gpu_info(&python_path);
    let device_text = if let Some(ref info) = device_info {
        format!("使用设备: {}", info)
    } else {
        "使用设备: 检测中...".to_string()
    };
    
    let _ = window.emit("firered-progress", FireRedProgress {
        progress: 1.0,
        current_text: device_text.clone(),
        status: "loading".to_string(),
    });
    log::info!("[FireRed] {}", device_text);
    
    let _ = window.emit("firered-progress", FireRedProgress {
        progress: 2.0,
        current_text: "正在加载 FireRedASR 模型...".to_string(),
        status: "loading".to_string(),
    });
    
    if is_cancelled() {
        return Err("校正已取消".to_string());
    }
    
    // 创建临时输出文件
    let output_path = std::env::temp_dir().join(format!("firered_output_{}.json", std::process::id()));
    
    // 映射语言代码
    let lang_code = match language.as_str() {
        "zh" => "zh",
        "en" => "en",
        "ja" => "ja",
        "ko" => "ko",
        "yue" => "yue",
        _ => "zh",
    };
    
    // 构建命令参数
    let mut args = vec![
        script_path.to_str().unwrap().to_string(),
        srt_path.clone(),
        audio_path.clone(),
        "--language".to_string(), lang_code.to_string(),
        "--output".to_string(), output_path.to_str().unwrap().to_string(),
    ];
    
    // 添加大小写保留参数
    if preserve_case {
        args.push("--preserve-case".to_string());
    } else {
        args.push("--no-preserve-case".to_string());
    }
    
    // 创建进度文件
    let progress_file = std::env::temp_dir().join(format!("firered_progress_{}.json", std::process::id()));
    let _ = std::fs::remove_file(&progress_file); // 确保文件不存在
    
    // 执行 Python 脚本
    let mut child = Command::new(&python_path)
        .args(&args)
        .env("FIRERED_PROGRESS_FILE", progress_file.to_str().unwrap())
        .spawn()
        .map_err(|e| format!("执行校正脚本失败: {}", e))?;
    
    // 轮询进度文件
    let mut last_progress: f32 = 2.0;
    loop {
        // 检查进程是否结束
        match child.try_wait() {
            Ok(Some(status)) => {
                // 进程已结束
                let _ = std::fs::remove_file(&progress_file);
                if !status.success() {
                    let _ = std::fs::remove_file(&output_path);
                    return Err(format!("校正脚本执行失败 (退出码: {:?})", status.code()));
                }
                break;
            }
            Ok(None) => {
                // 进程仍在运行，检查进度文件
            }
            Err(e) => {
                let _ = std::fs::remove_file(&progress_file);
                let _ = std::fs::remove_file(&output_path);
                return Err(format!("检查进程状态失败: {}", e));
            }
        }
        
        // 检查是否取消
        if is_cancelled() {
            let _ = child.kill();
            let _ = std::fs::remove_file(&progress_file);
            let _ = std::fs::remove_file(&output_path);
            return Err("校正已取消".to_string());
        }
        
        // 读取进度文件
        if let Ok(content) = std::fs::read_to_string(&progress_file) {
            if let Ok(progress_info) = serde_json::from_str::<serde_json::Value>(&content) {
                let progress = progress_info["progress"].as_f64().unwrap_or(0.0) as f32;
                let current = progress_info["current"].as_i64().unwrap_or(0);
                let total = progress_info["total"].as_i64().unwrap_or(0);
                let text = progress_info["text"].as_str().unwrap_or("");
                
                // 只有进度变化时才更新
                if progress > last_progress {
                    last_progress = progress;
                    
                    let display_text = if current > 0 {
                        format!("正在进行 AI 校正 ({}/{})", current, total)
                    } else {
                        text.to_string()
                    };
                    
                    // 只记录关键日志（模型加载阶段，进度 < 6%）
                    if progress < 6.0 {
                        log::info!("[FireRed] {}", display_text);
                    }
                    
                    let _ = window.emit("firered-progress", FireRedProgress {
                        progress,
                        current_text: display_text,
                        status: "correcting".to_string(),
                    });
                }
            }
        }
        
        // 短暂休眠，避免 CPU 占用过高
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    
    // 清理进度文件
    let _ = std::fs::remove_file(&progress_file);
    
    // 读取结果
    let result_json = std::fs::read_to_string(&output_path)
        .map_err(|e| format!("读取校正结果失败: {}", e))?;
    
    let _ = std::fs::remove_file(&output_path);
    
    // 解析 JSON
    let result: CorrectionResult = serde_json::from_str(&result_json)
        .map_err(|e| format!("解析校正结果失败: {}", e))?;
    
    // 转换为 CorrectionEntry
    let entries: Vec<CorrectionEntry> = result.entries.into_iter().map(|e| {
        CorrectionEntry {
            id: e.id,
            start_time: ms_to_timestamp(e.start_ms),
            end_time: ms_to_timestamp(e.end_ms),
            original: e.original,
            corrected: e.corrected,
            has_diff: e.has_diff,
        }
    }).collect();
    
    let diff_count = entries.iter().filter(|e| e.has_diff).count();
    let total_count = entries.len();
    
    // 记录结束时间和总耗时
    let elapsed = start_time.elapsed();
    let elapsed_secs = elapsed.as_secs_f64();
    let elapsed_str = if elapsed_secs >= 60.0 {
        format!("{:.0}分{:.1}秒", elapsed_secs / 60.0, elapsed_secs % 60.0)
    } else {
        format!("{:.1}秒", elapsed_secs)
    };
    
    log::info!("[FireRed] 校正完成: 共 {} 条字幕，发现 {} 处差异，耗时 {}", total_count, diff_count, elapsed_str);
    log::info!("[FireRed] ========== AI 校正结束 ==========");
    
    let _ = window.emit("firered-progress", FireRedProgress {
        progress: 100.0,
        current_text: format!("校正完成！共 {} 条，{} 处差异，耗时 {}", total_count, diff_count, elapsed_str),
        status: "completed".to_string(),
    });
    
    Ok(entries)
}

/// 单条字幕校正结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SingleCorrectionResult {
    pub original: String,
    pub corrected: String,
    pub has_diff: bool,
}

/// 写入服务脚本
fn write_service_script() -> Result<PathBuf, String> {
    let scripts_dir = get_scripts_dir()?;
    let script_path = scripts_dir.join("firered_service.py");
    
    let script_content = r#"#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""FireRedASR 持久化服务 - 模型只加载一次，音频缓存优化"""

import sys
import json
import os
import tempfile
import hashlib
from http.server import HTTPServer, BaseHTTPRequestHandler
import urllib.parse

# 全局模型变量
MODEL = None
# 音频缓存：{audio_path: (mtime, AudioSegment)}
AUDIO_CACHE = {}
# 最大缓存数量
MAX_CACHE_SIZE = 3

def get_firered_model_path():
    """获取 FireRedASR 模型路径"""
    home = os.path.expanduser("~")
    hub_dir = os.path.join(home, ".cache", "modelscope", "hub")
    
    # 路径格式1: ~/.cache/modelscope/hub/FireRedTeam/FireRedASR-AED-L
    path1 = os.path.join(hub_dir, "FireRedTeam", "FireRedASR-AED-L")
    if os.path.exists(path1):
        return path1
    
    # 路径格式2: ~/.cache/modelscope/hub/models/FireRedTeam/FireRedASR-AED-L
    path2 = os.path.join(hub_dir, "models", "FireRedTeam", "FireRedASR-AED-L")
    if os.path.exists(path2):
        return path2
    
    # 路径格式3: ~/.cache/modelscope/hub/models--FireRedTeam--FireRedASR-AED-L/snapshots/xxx
    models_dir = os.path.join(hub_dir, "models--FireRedTeam--FireRedASR-AED-L")
    if os.path.exists(models_dir):
        snapshots_dir = os.path.join(models_dir, "snapshots")
        if os.path.exists(snapshots_dir):
            for entry in os.listdir(snapshots_dir):
                entry_path = os.path.join(snapshots_dir, entry)
                if os.path.isdir(entry_path):
                    return entry_path
        return models_dir
    
    # 默认返回路径格式1
    return path1

def load_model():
    global MODEL
    if MODEL is None:
        import torch
        import argparse
        from fireredasr.data.asr_feat import ASRFeatExtractor
        from fireredasr.models.fireredasr_aed import FireRedAsrAed
        from fireredasr.tokenizer.aed_tokenizer import ChineseCharEnglishSpmTokenizer
        
        # 修复 PyTorch 2.6+ 的兼容性问题
        torch.serialization.add_safe_globals([argparse.Namespace])
        
        # 检测是否有 GPU 可用
        use_gpu = torch.cuda.is_available()
        device = "cuda" if use_gpu else "cpu"
        device_name = torch.cuda.get_device_name(0) if use_gpu else "CPU"
        print(f"使用设备: {device.upper()} ({device_name})", file=sys.stderr, flush=True)
        
        print("Loading FireRedASR model...", file=sys.stderr)
        model_dir = get_firered_model_path()
        if not os.path.exists(model_dir):
            raise RuntimeError(f"模型未下载，请先在设置中下载 FireRedASR-AED-L 模型")
        
        cmvn_path = os.path.join(model_dir, "cmvn.ark")
        feat_extractor = ASRFeatExtractor(cmvn_path)
        
        model_path = os.path.join(model_dir, "model.pth.tar")
        package = torch.load(model_path, map_location=lambda storage, loc: storage, weights_only=False)
        model = FireRedAsrAed.from_args(package["args"])
        model.load_state_dict(package["model_state_dict"], strict=True)
        model.eval()
        
        # 如果有 GPU，将模型移到 GPU
        if use_gpu:
            model = model.cuda()
        
        dict_path = os.path.join(model_dir, "dict.txt")
        spm_model = os.path.join(model_dir, "train_bpe1000.model")
        tokenizer = ChineseCharEnglishSpmTokenizer(dict_path, spm_model)
        
        MODEL = (feat_extractor, model, tokenizer, use_gpu)
        print("Model loaded!", file=sys.stderr)
    return MODEL

def get_cached_audio(audio_path):
    """获取缓存的音频，如果文件已修改则重新加载"""
    global AUDIO_CACHE
    from pydub import AudioSegment
    
    try:
        mtime = os.path.getmtime(audio_path)
    except OSError:
        mtime = 0
    
    if audio_path in AUDIO_CACHE:
        cached_mtime, cached_audio = AUDIO_CACHE[audio_path]
        if cached_mtime == mtime:
            return cached_audio
    
    # 加载新音频
    audio = AudioSegment.from_file(audio_path)
    
    # 清理旧缓存
    if len(AUDIO_CACHE) >= MAX_CACHE_SIZE:
        # 删除最早的缓存
        oldest_key = next(iter(AUDIO_CACHE))
        del AUDIO_CACHE[oldest_key]
    
    AUDIO_CACHE[audio_path] = (mtime, audio)
    return audio

def preserve_original_case(original, corrected):
    """保留原始文本中英文字母的大小写"""
    if not original or not corrected:
        return corrected
    
    # 提取原始文本中的英文字母
    orig_letters = [(i, c) for i, c in enumerate(original) if c.isalpha() and c.isascii()]
    
    # 提取校正文本中的英文字母位置
    corr_letter_positions = [i for i, c in enumerate(corrected) if c.isalpha() and c.isascii()]
    
    # 如果字母数量不同，无法简单映射，返回原始校正结果
    if len(orig_letters) != len(corr_letter_positions):
        return corrected
    
    # 检查字母序列是否相同（忽略大小写）
    orig_seq = ''.join(c.lower() for _, c in orig_letters)
    corr_seq = ''.join(corrected[i].lower() for i in corr_letter_positions)
    
    if orig_seq != corr_seq:
        return corrected
    
    # 恢复原始大小写
    result = list(corrected)
    for (_, orig_char), corr_pos in zip(orig_letters, corr_letter_positions):
        if orig_char.isupper():
            result[corr_pos] = result[corr_pos].upper()
        else:
            result[corr_pos] = result[corr_pos].lower()
    
    return ''.join(result)

class Handler(BaseHTTPRequestHandler):
    def log_message(self, format, *args):
        pass  # 禁用日志
    
    def do_POST(self):
        content_length = int(self.headers['Content-Length'])
        post_data = self.rfile.read(content_length)
        params = json.loads(post_data.decode('utf-8'))
        
        try:
            audio_path = params['audio_path']
            start_ms = params['start_ms']
            end_ms = params['end_ms']
            original_text = params['original_text']
            language = params.get('language', 'zh')
            preserve_case = params.get('preserve_case', True)
            
            # 使用缓存的音频
            audio = get_cached_audio(audio_path)
            chunk = audio[start_ms:end_ms]
            chunk = chunk.set_channels(1)
            chunk = chunk.set_frame_rate(16000)
            
            tmp_file = tempfile.NamedTemporaryFile(suffix='.wav', delete=False)
            chunk.export(tmp_file.name, format='wav')
            tmp_file.close()
            
            # 识别
            feat_extractor, model, tokenizer, use_gpu = load_model()
            
            # 提取特征
            feats, lengths, _ = feat_extractor([tmp_file.name])
            
            # 如果使用 GPU，将数据移到 GPU
            if use_gpu:
                feats = feats.cuda()
                lengths = lengths.cuda()
            
            # 使用模型进行识别
            hyps = model.transcribe(
                feats,
                lengths,
                beam_size=1,
                nbest=1,
                decode_max_len=0,
                softmax_smoothing=1.0,
                length_penalty=0.0,
                eos_penalty=1.0,
            )
            
            # 解码结果
            if hyps:
                hyp = hyps[0][0]  # 取第一个结果的 1-best
                hyp_ids = [int(id) for id in hyp["yseq"].cpu()]
                corrected = tokenizer.detokenize(hyp_ids).strip()
            else:
                corrected = ""
            
            # 如果启用了保留大小写，恢复原始英文大小写
            if preserve_case and corrected:
                corrected = preserve_original_case(original_text, corrected)
            
            os.remove(tmp_file.name)
            
            response = {
                "original": original_text,
                "corrected": corrected,
                "has_diff": original_text.strip() != corrected
            }
            
            self.send_response(200)
            self.send_header('Content-Type', 'application/json')
            self.end_headers()
            self.wfile.write(json.dumps(response, ensure_ascii=False).encode('utf-8'))
        except Exception as e:
            self.send_response(500)
            self.send_header('Content-Type', 'application/json')
            self.end_headers()
            self.wfile.write(json.dumps({"error": str(e)}).encode('utf-8'))
    
    def do_GET(self):
        if self.path == '/health':
            self.send_response(200)
            self.send_header('Content-Type', 'text/plain')
            self.end_headers()
            self.wfile.write(b'ok')
        elif self.path == '/preload':
            try:
                load_model()
                self.send_response(200)
                self.send_header('Content-Type', 'text/plain')
                self.end_headers()
                self.wfile.write(b'model loaded')
            except Exception as e:
                self.send_response(500)
                self.end_headers()
                self.wfile.write(str(e).encode())
        elif self.path.startswith('/preload_audio?'):
            # 预加载音频文件
            try:
                query = urllib.parse.urlparse(self.path).query
                params = urllib.parse.parse_qs(query)
                audio_path = params.get('path', [''])[0]
                if audio_path:
                    get_cached_audio(audio_path)
                    self.send_response(200)
                    self.send_header('Content-Type', 'text/plain')
                    self.end_headers()
                    self.wfile.write(b'audio cached')
                else:
                    self.send_response(400)
                    self.end_headers()
                    self.wfile.write(b'missing path')
            except Exception as e:
                self.send_response(500)
                self.end_headers()
                self.wfile.write(str(e).encode())
        else:
            self.send_response(404)
            self.end_headers()

if __name__ == "__main__":
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 18765
    server = HTTPServer(('127.0.0.1', port), Handler)
    print(f"FireRedASR service running on port {port}", file=sys.stderr)
    server.serve_forever()
"#;
    
    std::fs::write(&script_path, script_content)
        .map_err(|e| format!("写入服务脚本失败: {}", e))?;
    
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&script_path)
            .map_err(|e| format!("获取文件权限失败: {}", e))?
            .permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&script_path, perms)
            .map_err(|e| format!("设置文件权限失败: {}", e))?;
    }
    
    Ok(script_path)
}

// 服务状态缓存
static SERVICE_RUNNING: Lazy<Arc<std::sync::atomic::AtomicBool>> = Lazy::new(|| Arc::new(std::sync::atomic::AtomicBool::new(false)));
static LAST_SERVICE_CHECK: Lazy<Arc<std::sync::Mutex<std::time::Instant>>> = Lazy::new(|| Arc::new(std::sync::Mutex::new(std::time::Instant::now())));

/// 检查服务是否运行（带缓存，5秒内不重复检查）
pub fn is_service_running() -> bool {
    // 如果缓存显示服务运行中，且距离上次检查不超过5秒，直接返回
    let cached = SERVICE_RUNNING.load(Ordering::SeqCst);
    if cached {
        if let Ok(last_check) = LAST_SERVICE_CHECK.lock() {
            if last_check.elapsed().as_secs() < 5 {
                return true;
            }
        }
    }
    
    // 实际检查服务状态
    let running = check_service_health();
    SERVICE_RUNNING.store(running, Ordering::SeqCst);
    if let Ok(mut last_check) = LAST_SERVICE_CHECK.lock() {
        *last_check = std::time::Instant::now();
    }
    running
}

/// 实际检查服务健康状态
fn check_service_health() -> bool {
    std::process::Command::new("curl")
        .args(["-s", "-o", "/dev/null", "-w", "%{http_code}", "--connect-timeout", "1", "http://127.0.0.1:18765/health"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "200")
        .unwrap_or(false)
}

/// 停止 FireRedASR 服务
fn stop_service() {
    // 尝试通过 pkill 停止服务
    let _ = std::process::Command::new("pkill")
        .args(["-f", "firered_service.py"])
        .output();
    
    // 等待服务停止
    for _ in 0..10 {
        if !is_service_running() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
}

/// 启动服务（如果已运行则直接返回）
fn start_service() -> Result<(), String> {
    // 快速检查：如果服务已经在运行，直接返回
    if is_service_running() {
        return Ok(());
    }
    
    // 服务未运行，需要启动
    let script_path = write_service_script()?;
    let python_path = get_python_path()?;
    
    // 后台启动服务
    std::process::Command::new(&python_path)
        .arg(&script_path)
        .arg("18765")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("启动服务失败: {}", e))?;
    
    // 等待服务启动（缩短等待时间）
    for _ in 0..20 {
        std::thread::sleep(std::time::Duration::from_millis(300));
        if check_service_health() {
            SERVICE_RUNNING.store(true, Ordering::SeqCst);
            return Ok(());
        }
    }
    
    Err("服务启动超时".to_string())
}

/// 预加载 FireRedASR 服务和模型
/// 这个函数会启动服务并预加载模型，以便后续的单条校正更快
pub async fn preload_firered_service() -> Result<String, String> {
    // 检查环境
    let env_status = check_firered_env();
    if !env_status.ready {
        return Err("FireRedASR 环境未安装".to_string());
    }
    
    // 启动服务
    start_service()?;
    
    // 使用 reqwest 调用 /preload 端点预加载模型
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;
    
    let response = client
        .get("http://127.0.0.1:18765/preload")
        .send()
        .await
        .map_err(|e| format!("预加载请求失败: {}", e))?;
    
    if response.status().is_success() {
        Ok("FireRedASR 服务已启动并预加载模型".to_string())
    } else {
        Err(format!("预加载失败: HTTP {}", response.status()))
    }
}

/// 预加载音频文件到服务缓存
/// 在打开音频文件时调用，可以加速后续的单条校正
pub async fn preload_audio_for_correction(audio_path: String) -> Result<String, String> {
    // 检查环境
    let env_status = check_firered_env();
    if !env_status.ready {
        return Err("FireRedASR 环境未安装".to_string());
    }
    
    // 确保服务运行
    start_service()?;
    
    // 使用 reqwest 调用预加载音频端点
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;
    
    // 简单的 URL 编码
    let encoded_path = audio_path
        .replace('%', "%25")
        .replace(' ', "%20")
        .replace('&', "%26")
        .replace('?', "%3F")
        .replace('#', "%23");
    let url = format!("http://127.0.0.1:18765/preload_audio?path={}", encoded_path);
    
    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("预加载音频请求失败: {}", e))?;
    
    if response.status().is_success() {
        Ok("音频已预加载到缓存".to_string())
    } else {
        Err(format!("预加载音频失败: HTTP {}", response.status()))
    }
}

/// 校正单条字幕（使用持久化服务）
pub async fn correct_single_entry(
    audio_path: String,
    start_ms: u32,
    end_ms: u32,
    original_text: String,
    language: String,
    preserve_case: bool,
) -> Result<SingleCorrectionResult, String> {
    // 检查环境
    let env_status = check_firered_env();
    if !env_status.ready {
        return Err("FireRedASR 环境未安装，请先安装环境".to_string());
    }
    
    // 确保服务运行
    start_service()?;
    
    // 映射语言代码
    let lang_code = match language.as_str() {
        "zh" => "zh",
        "en" => "en",
        "ja" => "ja",
        "ko" => "ko",
        "yue" => "yue",
        _ => "zh",
    };
    
    // 构建请求
    let request_body = serde_json::json!({
        "audio_path": audio_path,
        "start_ms": start_ms,
        "end_ms": end_ms,
        "original_text": original_text,
        "language": lang_code,
        "preserve_case": preserve_case
    });
    
    // 使用 reqwest 发送请求（比 curl 更快，无需启动新进程）
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;
    
    let response = client
        .post("http://127.0.0.1:18765/")
        .header("Content-Type", "application/json")
        .body(request_body.to_string())
        .send()
        .await
        .map_err(|e| format!("请求服务失败: {}", e))?;
    
    if !response.status().is_success() {
        return Err(format!("校正失败: HTTP {}", response.status()));
    }
    
    let result_json = response.text().await
        .map_err(|e| format!("读取响应失败: {}", e))?;
    
    // 检查是否有错误
    if result_json.contains("\"error\"") {
        return Err(format!("校正服务返回错误: {}", result_json));
    }
    
    // 解析 JSON
    let result: SingleCorrectionResult = serde_json::from_str(&result_json)
        .map_err(|e| format!("解析校正结果失败: {} - 原始响应: {}", e, result_json))?;
    
    Ok(result)
}

/// 卸载指定的 FireRedASR 环境
pub fn uninstall_firered_env_by_type(use_gpu: bool) -> Result<String, String> {
    let env_dir = if use_gpu {
        get_firered_gpu_env_dir()?
    } else {
        get_firered_cpu_env_dir()?
    };
    
    let version_type = if use_gpu { "GPU" } else { "CPU" };
    
    if !env_dir.exists() {
        return Err(format!("{} 环境未安装", version_type));
    }
    
    // 如果卸载的是当前激活的环境，需要切换
    let active = get_firered_active_env_type();
    let is_active = (use_gpu && active == "gpu") || (!use_gpu && active == "cpu");
    
    if is_active {
        // 停止服务
        stop_service();
    }
    
    std::fs::remove_dir_all(&env_dir)
        .map_err(|e| format!("删除环境目录失败: {}", e))?;
    
    // 如果卸载的是当前激活的环境，尝试切换到另一个
    if is_active {
        let other_dir = if use_gpu {
            get_firered_cpu_env_dir()?
        } else {
            get_firered_gpu_env_dir()?
        };
        
        if check_firered_env_ready(&other_dir) {
            let other_type = if use_gpu { "cpu" } else { "gpu" };
            let _ = set_firered_active_env_type(other_type);
        } else {
            let _ = set_firered_active_env_type("none");
        }
    }
    
    Ok(format!("FireRedASR {} 版本已卸载", version_type))
}

/// 卸载 FireRedASR 环境（兼容旧接口，卸载当前激活的环境）
pub fn uninstall_firered_env() -> Result<String, String> {
    let active = get_firered_active_env_type();
    match active.as_str() {
        "gpu" => uninstall_firered_env_by_type(true),
        "cpu" => uninstall_firered_env_by_type(false),
        _ => {
            // 尝试卸载两个环境
            let cpu_result = uninstall_firered_env_by_type(false);
            let gpu_result = uninstall_firered_env_by_type(true);
            
            if cpu_result.is_ok() || gpu_result.is_ok() {
                Ok("FireRedASR 环境已卸载".to_string())
            } else {
                Err("没有已安装的 FireRedASR 环境".to_string())
            }
        }
    }
}

use crate::srt_parser::{SubtitleEntry, TimeStamp};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use tauri::{Emitter, Window};
use once_cell::sync::Lazy;

// 全局取消标志（转录任务）
static SENSEVOICE_CANCELLED: Lazy<Arc<AtomicBool>> = Lazy::new(|| Arc::new(AtomicBool::new(false)));

// 模型下载任务ID，用于取消旧的下载任务
static SENSEVOICE_MODEL_DOWNLOAD_TASK_ID: Lazy<AtomicU64> = Lazy::new(|| AtomicU64::new(0));

/// 取消当前转录任务
pub fn cancel_sensevoice_transcription() {
    SENSEVOICE_CANCELLED.store(true, Ordering::SeqCst);
}

/// 取消当前模型下载任务
pub fn cancel_sensevoice_model_download() {
    // 增加任务ID使当前下载任务失效
    SENSEVOICE_MODEL_DOWNLOAD_TASK_ID.fetch_add(1, Ordering::SeqCst);
    log::info!("SenseVoice model download cancelled by user");
}

/// 生成新的模型下载任务ID
fn new_sensevoice_model_download_task_id() -> u64 {
    SENSEVOICE_MODEL_DOWNLOAD_TASK_ID.fetch_add(1, Ordering::SeqCst) + 1
}

/// 检查模型下载任务是否仍然有效
fn is_sensevoice_model_download_task_valid(task_id: u64) -> bool {
    SENSEVOICE_MODEL_DOWNLOAD_TASK_ID.load(Ordering::SeqCst) == task_id
}

/// 重置取消标志
fn reset_cancellation() {
    SENSEVOICE_CANCELLED.store(false, Ordering::SeqCst);
}

/// 检查是否已取消
fn is_cancelled() -> bool {
    SENSEVOICE_CANCELLED.load(Ordering::SeqCst)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SenseVoiceProgress {
    pub progress: f32,
    pub current_text: String,
    pub status: String,
}

/// 单个环境的状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SenseVoiceEnvInfo {
    pub installed: bool,
    pub ready: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SenseVoiceEnvStatus {
    pub uv_installed: bool,
    pub cpu_env: SenseVoiceEnvInfo,
    pub gpu_env: SenseVoiceEnvInfo,
    pub active_env: String,  // "cpu", "gpu", or "none"
    // 兼容旧字段
    pub env_exists: bool,
    pub ready: bool,
    pub is_gpu: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SenseVoiceModelInfo {
    pub name: String,
    pub size: String,
    pub downloaded: bool,
    pub partial_size: Option<u64>,
}

/// SenseVoice 模型文件信息
struct ModelFileInfo {
    name: &'static str,
    size: u64,
    is_lfs: bool,
}

/// SenseVoiceSmall 模型需要下载的文件列表
const SENSEVOICE_SMALL_FILES: &[ModelFileInfo] = &[
    ModelFileInfo { name: "model.pt", size: 936291369, is_lfs: true },
    ModelFileInfo { name: "chn_jpn_yue_eng_ko_spectok.bpe.model", size: 377341, is_lfs: true },
    ModelFileInfo { name: "configuration.json", size: 396, is_lfs: false },
    ModelFileInfo { name: "config.yaml", size: 1855, is_lfs: false },
    ModelFileInfo { name: "am.mvn", size: 11203, is_lfs: false },
    ModelFileInfo { name: "tokens.json", size: 352064, is_lfs: false },
];

/// 获取 SenseVoice 模型缓存目录
pub fn get_sensevoice_model_dir() -> Result<PathBuf, String> {
    let home_dir = dirs::home_dir()
        .ok_or_else(|| "Failed to get home directory".to_string())?;
    
    // 使用与 ModelScope 兼容的缓存目录
    let model_dir = home_dir.join(".cache").join("modelscope").join("hub").join("iic");
    
    Ok(model_dir)
}

/// 获取模型文件路径（检查多种可能的路径格式）
fn get_sensevoice_model_path(model_name: &str) -> Result<PathBuf, String> {
    let home_dir = dirs::home_dir()
        .ok_or_else(|| "Failed to get home directory".to_string())?;
    
    let hub_dir = home_dir.join(".cache").join("modelscope").join("hub");
    
    // 路径格式1: ~/.cache/modelscope/hub/iic/SenseVoiceSmall
    let path1 = hub_dir.join("iic").join(model_name);
    if path1.exists() {
        return Ok(path1);
    }
    
    // 路径格式2: ~/.cache/modelscope/hub/models/iic/SenseVoiceSmall (Mac 上的路径)
    let path2 = hub_dir.join("models").join("iic").join(model_name);
    if path2.exists() {
        return Ok(path2);
    }
    
    // 路径格式3: ~/.cache/modelscope/hub/models--iic--SenseVoiceSmall/snapshots/xxx
    // ModelScope 某些版本可能使用这种格式
    let models_dir = hub_dir.join(format!("models--iic--{}", model_name));
    if models_dir.exists() {
        // 查找 snapshots 目录下的最新版本
        let snapshots_dir = models_dir.join("snapshots");
        if snapshots_dir.exists() {
            if let Ok(entries) = std::fs::read_dir(&snapshots_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        // 返回第一个找到的 snapshot 目录
                        return Ok(path);
                    }
                }
            }
        }
        // 如果没有 snapshots，直接返回 models 目录
        return Ok(models_dir);
    }
    
    // 默认返回路径格式1（用于新下载）
    Ok(path1)
}

/// 获取部分下载文件路径
fn get_sensevoice_part_file_path(model_name: &str, file_name: &str) -> Result<PathBuf, String> {
    let model_path = get_sensevoice_model_path(model_name)?;
    Ok(model_path.join(format!("{}.part", file_name)))
}

/// 检查 SenseVoice 模型是否已下载
pub fn is_sensevoice_model_downloaded(model_name: &str) -> bool {
    let home_dir = match dirs::home_dir() {
        Some(dir) => dir,
        None => return false,
    };
    
    let hub_dir = home_dir.join(".cache").join("modelscope").join("hub");
    
    // 检查路径格式1: ~/.cache/modelscope/hub/iic/SenseVoiceSmall
    let path1 = hub_dir.join("iic").join(model_name);
    if check_model_files_exist(&path1) {
        return true;
    }
    
    // 检查路径格式2: ~/.cache/modelscope/hub/models/iic/SenseVoiceSmall (Mac 上的路径)
    let path2 = hub_dir.join("models").join("iic").join(model_name);
    if check_model_files_exist(&path2) {
        return true;
    }
    
    // 检查路径格式3: ~/.cache/modelscope/hub/models--iic--SenseVoiceSmall/snapshots/xxx
    let models_dir = hub_dir.join(format!("models--iic--{}", model_name));
    if models_dir.exists() {
        let snapshots_dir = models_dir.join("snapshots");
        if snapshots_dir.exists() {
            if let Ok(entries) = std::fs::read_dir(&snapshots_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() && check_model_files_exist(&path) {
                        return true;
                    }
                }
            }
        }
        // 也检查 models 目录本身
        if check_model_files_exist(&models_dir) {
            return true;
        }
    }
    
    false
}

/// 检查模型文件是否存在
fn check_model_files_exist(model_path: &std::path::Path) -> bool {
    if !model_path.exists() {
        return false;
    }
    
    // 检查主要模型文件是否存在
    let model_pt = model_path.join("model.pt");
    let config = model_path.join("configuration.json");
    
    model_pt.exists() && config.exists()
}

/// 获取已下载的部分大小
pub fn get_sensevoice_partial_size(model_name: &str) -> u64 {
    let model_path = match get_sensevoice_model_path(model_name) {
        Ok(path) => path,
        Err(_) => return 0,
    };
    
    let mut total_partial = 0u64;
    
    // 检查已下载的文件大小
    for file_info in SENSEVOICE_SMALL_FILES {
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

/// 获取 SenseVoice 可用模型列表
pub fn get_sensevoice_models() -> Vec<SenseVoiceModelInfo> {
    let downloaded = is_sensevoice_model_downloaded("SenseVoiceSmall");
    let partial_size = if !downloaded {
        let size = get_sensevoice_partial_size("SenseVoiceSmall");
        if size > 0 { Some(size) } else { None }
    } else {
        None
    };
    
    vec![
        SenseVoiceModelInfo {
            name: "SenseVoiceSmall".to_string(),
            size: "~893 MB".to_string(),
            downloaded,
            partial_size,
        },
    ]
}

/// 下载 SenseVoice 模型（支持断点续传）
pub async fn download_sensevoice_model(model_name: &str, window: Window) -> Result<String, String> {
    use std::fs::{self, OpenOptions};
    use std::io::Write;
    
    // 生成新的任务ID，使之前的下载任务失效
    let task_id = new_sensevoice_model_download_task_id();
    
    // 检查环境是否就绪
    let env_status = check_sensevoice_env();
    if !env_status.ready {
        return Err("SenseVoice 环境未安装，请先安装环境".to_string());
    }
    
    // 检查是否已下载
    if is_sensevoice_model_downloaded(model_name) {
        return Ok(format!("{} 模型已下载", model_name));
    }
    
    let model_path = get_sensevoice_model_path(model_name)?;
    
    // 创建模型目录
    if !model_path.exists() {
        fs::create_dir_all(&model_path)
            .map_err(|e| format!("创建模型目录失败: {}", e))?;
    }
    
    // 计算总大小
    let total_size: u64 = SENSEVOICE_SMALL_FILES.iter().map(|f| f.size).sum();
    let mut downloaded_total: u64 = 0;
    
    // 发送初始进度
    let _ = window.emit("sensevoice-model-progress", SenseVoiceProgress {
        progress: 0.0,
        current_text: format!("正在下载 {} 模型...", model_name),
        status: "downloading".to_string(),
    });
    
    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;
    
    // 下载每个文件
    for (file_idx, file_info) in SENSEVOICE_SMALL_FILES.iter().enumerate() {
        // 检查任务是否仍然有效
        if !is_sensevoice_model_download_task_valid(task_id) {
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
            "https://modelscope.cn/models/iic/{}/resolve/master/{}",
            model_name, file_info.name
        );
        
        // 发送进度
        let progress = (downloaded_total as f32 / total_size as f32) * 100.0;
        let _ = window.emit("sensevoice-model-progress", SenseVoiceProgress {
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
            if !is_sensevoice_model_download_task_valid(task_id) {
                return Err("下载已取消".to_string());
            }
            
            file.write_all(&chunk)
                .map_err(|e| format!("写入文件失败: {}", e))?;
            
            file_downloaded += chunk.len() as u64;
            
            // 更新进度
            let current_total = downloaded_total + file_downloaded;
            let progress = (current_total as f32 / total_size as f32) * 100.0;
            let _ = window.emit("sensevoice-model-progress", SenseVoiceProgress {
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
    let _ = window.emit("sensevoice-model-progress", SenseVoiceProgress {
        progress: 100.0,
        current_text: "模型下载完成！".to_string(),
        status: "completed".to_string(),
    });
    
    Ok(format!("{} 模型下载成功", model_name))
}

/// 删除 SenseVoice 模型
pub fn delete_sensevoice_model(model_name: &str) -> Result<String, String> {
    let model_path = get_sensevoice_model_path(model_name)?;
    
    if !model_path.exists() {
        return Err(format!("模型 {} 未下载", model_name));
    }
    
    std::fs::remove_dir_all(&model_path)
        .map_err(|e| format!("删除模型失败: {}", e))?;
    
    Ok(format!("模型 {} 已删除", model_name))
}

/// 打开 SenseVoice 模型目录
pub fn open_sensevoice_model_dir() -> Result<(), String> {
    let model_dir = get_sensevoice_model_dir()?;
    
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

/// Python 脚本输出的转录结果
#[derive(Debug, Deserialize)]
struct TranscriptionResult {
    segments: Vec<TranscriptionSegment>,
}

#[derive(Debug, Deserialize)]
struct TranscriptionSegment {
    start: f64,  // 秒
    end: f64,    // 秒
    text: String,
}

/// 获取 SenseVoice 环境基础目录
fn get_sensevoice_base_dir() -> Result<PathBuf, String> {
    let home_dir = dirs::home_dir()
        .ok_or_else(|| "Failed to get home directory".to_string())?;
    
    Ok(home_dir.join(".config").join("vosub"))
}

/// 获取旧版 SenseVoice 环境目录（用于迁移）
fn get_legacy_sensevoice_env_dir() -> Result<PathBuf, String> {
    let base_dir = get_sensevoice_base_dir()?;
    Ok(base_dir.join("sensevoice-env"))
}

/// 检查旧版环境是否是 GPU 版本
fn is_legacy_env_gpu() -> bool {
    get_legacy_sensevoice_env_dir()
        .ok()
        .map(|p| p.join(".gpu_version").exists())
        .unwrap_or(false)
}

/// 迁移旧版环境到新目录结构
fn migrate_legacy_env() -> Result<bool, String> {
    let legacy_dir = get_legacy_sensevoice_env_dir()?;
    
    if !legacy_dir.exists() {
        return Ok(false); // 没有旧环境，无需迁移
    }
    
    // 检查是否已经迁移过（新目录已存在）
    let cpu_dir = get_sensevoice_cpu_env_dir()?;
    let gpu_dir = get_sensevoice_gpu_env_dir()?;
    
    if cpu_dir.exists() || gpu_dir.exists() {
        // 新目录已存在，删除旧目录
        let _ = std::fs::remove_dir_all(&legacy_dir);
        return Ok(false);
    }
    
    // 判断旧环境是 CPU 还是 GPU 版本
    let is_gpu = is_legacy_env_gpu();
    let target_dir = if is_gpu { &gpu_dir } else { &cpu_dir };
    
    // 重命名目录
    std::fs::rename(&legacy_dir, target_dir)
        .map_err(|e| format!("迁移旧环境失败: {}", e))?;
    
    // 设置激活的环境
    let env_type = if is_gpu { "gpu" } else { "cpu" };
    set_active_env_type(env_type)?;
    
    Ok(true)
}

/// 获取 SenseVoice CPU 环境目录
pub fn get_sensevoice_cpu_env_dir() -> Result<PathBuf, String> {
    let base_dir = get_sensevoice_base_dir()?;
    Ok(base_dir.join("sensevoice-env-cpu"))
}

/// 获取 SenseVoice GPU 环境目录
pub fn get_sensevoice_gpu_env_dir() -> Result<PathBuf, String> {
    let base_dir = get_sensevoice_base_dir()?;
    Ok(base_dir.join("sensevoice-env-gpu"))
}

/// 获取当前激活的环境配置文件路径
fn get_active_env_config_path() -> Result<PathBuf, String> {
    let base_dir = get_sensevoice_base_dir()?;
    Ok(base_dir.join("sensevoice-active-env"))
}

/// 获取当前激活的环境类型
pub fn get_active_env_type() -> String {
    get_active_env_config_path()
        .ok()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "none".to_string())
}

/// 设置当前激活的环境类型
pub fn set_active_env_type(env_type: &str) -> Result<(), String> {
    let config_path = get_active_env_config_path()?;
    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("创建配置目录失败: {}", e))?;
    }
    std::fs::write(&config_path, env_type)
        .map_err(|e| format!("写入配置失败: {}", e))?;
    Ok(())
}

/// 获取 SenseVoice 环境目录（兼容旧代码，返回当前激活的环境）
pub fn get_sensevoice_env_dir() -> Result<PathBuf, String> {
    let active = get_active_env_type();
    match active.as_str() {
        "gpu" => get_sensevoice_gpu_env_dir(),
        "cpu" => get_sensevoice_cpu_env_dir(),
        _ => {
            // 如果没有激活的环境，检查哪个存在
            let gpu_dir = get_sensevoice_gpu_env_dir()?;
            if gpu_dir.exists() {
                return Ok(gpu_dir);
            }
            let cpu_dir = get_sensevoice_cpu_env_dir()?;
            if cpu_dir.exists() {
                return Ok(cpu_dir);
            }
            // 默认返回 CPU 目录
            Ok(cpu_dir)
        }
    }
}

/// 获取 Python 脚本目录
pub fn get_scripts_dir() -> Result<PathBuf, String> {
    let home_dir = dirs::home_dir()
        .ok_or_else(|| "Failed to get home directory".to_string())?;
    
    let scripts_dir = home_dir
        .join(".config")
        .join("vosub")
        .join("scripts");
    
    // 创建目录
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
    let env_dir = get_sensevoice_env_dir()?;
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
fn check_env_ready(env_dir: &PathBuf) -> bool {
    let python_path = get_python_path_for_env(env_dir);
    if !env_dir.exists() || !python_path.exists() {
        return false;
    }
    
    #[cfg(target_os = "windows")]
    {
        let funasr_path = env_dir.join("Lib").join("site-packages").join("funasr");
        funasr_path.exists()
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
                            let sp = path.join("site-packages").join("funasr");
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

/// 检查 SenseVoice 环境状态（快速检查，不启动 Python）
pub fn check_sensevoice_env() -> SenseVoiceEnvStatus {
    // 先尝试迁移旧版环境
    let _ = migrate_legacy_env();
    
    let uv_installed = check_uv_installed();
    
    // 检查 CPU 环境
    let cpu_dir = get_sensevoice_cpu_env_dir().unwrap_or_default();
    let cpu_installed = cpu_dir.exists();
    let cpu_ready = check_env_ready(&cpu_dir);
    
    // 检查 GPU 环境
    let gpu_dir = get_sensevoice_gpu_env_dir().unwrap_or_default();
    let gpu_installed = gpu_dir.exists();
    let gpu_ready = check_env_ready(&gpu_dir);
    
    // 获取当前激活的环境
    let mut active_env = get_active_env_type();
    
    // 如果激活的环境不存在，自动切换到可用的环境
    if active_env == "gpu" && !gpu_ready {
        if cpu_ready {
            let _ = set_active_env_type("cpu");
            active_env = "cpu".to_string();
        } else {
            let _ = set_active_env_type("none");
            active_env = "none".to_string();
        }
    } else if active_env == "cpu" && !cpu_ready {
        if gpu_ready {
            let _ = set_active_env_type("gpu");
            active_env = "gpu".to_string();
        } else {
            let _ = set_active_env_type("none");
            active_env = "none".to_string();
        }
    } else if active_env == "none" {
        // 自动选择一个可用的环境
        if gpu_ready {
            let _ = set_active_env_type("gpu");
            active_env = "gpu".to_string();
        } else if cpu_ready {
            let _ = set_active_env_type("cpu");
            active_env = "cpu".to_string();
        }
    }
    
    // 兼容旧字段
    let env_exists = cpu_installed || gpu_installed;
    let ready = cpu_ready || gpu_ready;
    let is_gpu = active_env == "gpu";
    
    SenseVoiceEnvStatus {
        uv_installed,
        cpu_env: SenseVoiceEnvInfo {
            installed: cpu_installed,
            ready: cpu_ready,
        },
        gpu_env: SenseVoiceEnvInfo {
            installed: gpu_installed,
            ready: gpu_ready,
        },
        active_env,
        env_exists,
        ready,
        is_gpu,
    }
}


/// 安装 SenseVoice 环境
/// use_gpu: 是否安装 GPU 版本（需要 NVIDIA 显卡和 CUDA）
pub async fn install_sensevoice_env(window: Window, use_gpu: bool) -> Result<String, String> {
    reset_cancellation();
    
    // 获取 uv 路径
    let uv_path = get_uv_path()
        .ok_or("请先安装 uv 包管理器。访问 https://docs.astral.sh/uv/getting-started/installation/ 了解安装方法")?;
    
    // 根据版本选择对应的环境目录
    let env_dir = if use_gpu {
        get_sensevoice_gpu_env_dir()?
    } else {
        get_sensevoice_cpu_env_dir()?
    };
    
    let version_type = if use_gpu { "GPU" } else { "CPU" };
    
    // 发送进度
    let _ = window.emit("sensevoice-progress", SenseVoiceProgress {
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
    
    // 发送进度
    let _ = window.emit("sensevoice-progress", SenseVoiceProgress {
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
    
    // 发送进度
    let _ = window.emit("sensevoice-progress", SenseVoiceProgress {
        progress: 60.0,
        current_text: "正在安装 FunASR...".to_string(),
        status: "installing".to_string(),
    });
    
    // 安装 funasr
    let output = Command::new(&uv_path)
        .args([
            "pip", "install",
            "--python", python_path.to_str().unwrap(),
            "funasr", "modelscope", "pydub"
        ])
        .output()
        .map_err(|e| format!("安装 FunASR 失败: {}", e))?;
    
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("安装 FunASR 失败: {}", stderr));
    }
    
    // 写入 Python 转录脚本
    let _ = window.emit("sensevoice-progress", SenseVoiceProgress {
        progress: 90.0,
        current_text: "正在配置转录脚本...".to_string(),
        status: "installing".to_string(),
    });
    
    write_transcription_script()?;
    
    // 设置为当前激活的环境
    let env_type = if use_gpu { "gpu" } else { "cpu" };
    set_active_env_type(env_type)?;
    
    // 完成
    let _ = window.emit("sensevoice-progress", SenseVoiceProgress {
        progress: 100.0,
        current_text: format!("SenseVoice {} 版本安装完成！", version_type),
        status: "completed".to_string(),
    });
    
    Ok(format!("SenseVoice {} 版本安装成功", version_type))
}

/// 切换当前使用的 SenseVoice 环境
pub fn switch_sensevoice_env(use_gpu: bool) -> Result<String, String> {
    let env_type = if use_gpu { "gpu" } else { "cpu" };
    let env_dir = if use_gpu {
        get_sensevoice_gpu_env_dir()?
    } else {
        get_sensevoice_cpu_env_dir()?
    };
    
    if !check_env_ready(&env_dir) {
        return Err(format!("{} 环境未安装或不完整", if use_gpu { "GPU" } else { "CPU" }));
    }
    
    set_active_env_type(env_type)?;
    Ok(format!("已切换到 {} 版本", if use_gpu { "GPU" } else { "CPU" }))
}

/// 卸载指定的 SenseVoice 环境
pub fn uninstall_sensevoice_env_by_type(use_gpu: bool) -> Result<String, String> {
    let env_dir = if use_gpu {
        get_sensevoice_gpu_env_dir()?
    } else {
        get_sensevoice_cpu_env_dir()?
    };
    
    let version_type = if use_gpu { "GPU" } else { "CPU" };
    
    if env_dir.exists() {
        std::fs::remove_dir_all(&env_dir)
            .map_err(|e| format!("删除 {} 环境目录失败: {}", version_type, e))?;
    }
    
    // 如果卸载的是当前激活的环境，切换到另一个可用的环境
    let active = get_active_env_type();
    let uninstalled_type = if use_gpu { "gpu" } else { "cpu" };
    if active == uninstalled_type {
        // 检查另一个环境是否可用
        let other_dir = if use_gpu {
            get_sensevoice_cpu_env_dir()?
        } else {
            get_sensevoice_gpu_env_dir()?
        };
        
        if check_env_ready(&other_dir) {
            let other_type = if use_gpu { "cpu" } else { "gpu" };
            set_active_env_type(other_type)?;
        } else {
            set_active_env_type("none")?;
        }
    }
    
    Ok(format!("SenseVoice {} 环境已卸载", version_type))
}

/// 写入 Python 转录脚本
fn write_transcription_script() -> Result<(), String> {
    let scripts_dir = get_scripts_dir()?;
    let script_path = scripts_dir.join("sensevoice_transcribe.py");
    
    // 简化版脚本 - 直接使用 VAD 分段，不做额外合并
    // 增加进度输出到 stderr
    // 自动检测 CUDA 可用性，有 GPU 就用 GPU，否则用 CPU
    let script_content = r#"#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""SenseVoice 转录脚本 - 参考 jianchang512/sense-api 实现"""

import sys
import io
import json
import re
import argparse
import os
import tempfile
import time

# Windows 上强制使用 UTF-8 编码
if sys.platform == 'win32':
    sys.stderr = io.TextIOWrapper(sys.stderr.buffer, encoding='utf-8', errors='replace')
    sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding='utf-8', errors='replace')

import torch
from pydub import AudioSegment

# 自动检测设备：优先使用 CUDA，否则使用 CPU
def get_device():
    if torch.cuda.is_available():
        return "cuda"
    return "cpu"

def get_device_info():
    """获取设备信息，包含 GPU 型号和显存"""
    if torch.cuda.is_available():
        gpu_name = torch.cuda.get_device_name(0)
        gpu_mem = torch.cuda.get_device_properties(0).total_memory / (1024**3)  # GB
        return f"cuda:{gpu_name}:{gpu_mem:.1f}GB"
    return "cpu::"

DEVICE = get_device()
DEVICE_INFO = get_device_info()

def emit_progress(current, total, status, message=""):
    """输出进度信息到 stderr（JSON 格式）"""
    progress = {
        "type": "progress",
        "current": current,
        "total": total,
        "percent": round(current / total * 100, 1) if total > 0 else 0,
        "status": status,
        "message": message
    }
    sys.stderr.write(json.dumps(progress, ensure_ascii=False) + '\n')
    sys.stderr.flush()

def clean_text(text):
    """清理特殊标签和不需要的字符"""
    # 移除 <|xxx|> 标签
    text = re.sub(r'<\|[^|]+\|>', '', text)
    # 保留中日韩英文、数字和常见标点
    allowed = re.compile(r'[^\u4e00-\u9fff\u3040-\u309f\u30a0-\u30ff\uac00-\ud7af'
                         r'a-zA-Z0-9\s.,!?;:，。！？；：、""''（）\-]+')
    text = re.sub(allowed, '', text).strip()
    # 移除句末的句号（中文和英文）
    text = re.sub(r'[。.]+$', '', text)
    return text

def transcribe(audio_path, language="auto"):
    from funasr import AutoModel
    from funasr.utils.postprocess_utils import rich_transcription_postprocess
    
    # 输出设备信息（包含 GPU 型号）
    print(f"DEVICE_INFO:{DEVICE_INFO}", flush=True)
    
    emit_progress(0, 100, "loading", "正在加载语音模型...")
    
    # 加载 VAD 模型（关键参数：max_end_silence_time=250 让分段更敏感）
    vad_model = AutoModel(
        model="fsmn-vad",
        max_single_segment_time=15000,  # 最长 15 秒
        max_end_silence_time=250,       # 静音 250ms 就分段
        device=DEVICE
    )
    
    emit_progress(5, 100, "loading", "正在加载语音模型...")
    
    # 加载 SenseVoice 模型
    model = AutoModel(
        model="iic/SenseVoiceSmall",
        trust_remote_code=True,
        device=DEVICE
    )
    
    emit_progress(10, 100, "vad", "正在识别语音内容...")
    
    # VAD 分段
    vad_res = vad_model.generate(input=audio_path)
    if not vad_res or not vad_res[0].get("value"):
        return {"segments": []}
    
    segments = vad_res[0]["value"]
    total_segments = len(segments)
    
    # 计算音频总时长用于预估
    audio = AudioSegment.from_file(audio_path)
    audio_duration_sec = len(audio) / 1000.0
    
    emit_progress(15, 100, "transcribing", "正在识别语音内容...")
    
    # 创建临时目录
    tmp_dir = tempfile.mkdtemp()
    
    all_segments = []
    start_time = time.time()
    
    try:
        for idx, seg in enumerate(segments):
            start_ms, end_ms = seg[0], seg[1]
            
            # 计算进度（15% - 95% 用于转录）
            progress = 15 + int((idx / total_segments) * 80)
            
            emit_progress(progress, 100, "transcribing", "正在识别语音内容...")
            
            # 切分音频片段
            chunk = audio[start_ms:end_ms]
            chunk_file = os.path.join(tmp_dir, f"{start_ms}_{end_ms}.wav")
            chunk.export(chunk_file, format="wav")
            
            # 转录
            res = model.generate(input=chunk_file, language=language, use_itn=True)
            if not res:
                os.remove(chunk_file)
                continue
            
            text = res[0].get("text", "")
            if not text:
                os.remove(chunk_file)
                continue
            
            try:
                text = rich_transcription_postprocess(text)
            except:
                text = clean_text(text)
            
            text = clean_text(text)
            if text:
                all_segments.append({
                    "start": round(start_ms / 1000.0, 3),
                    "end": round(end_ms / 1000.0, 3),
                    "text": text
                })
            
            # 删除临时文件
            os.remove(chunk_file)
    finally:
        # 清理临时目录
        try:
            os.rmdir(tmp_dir)
        except:
            pass
    
    emit_progress(100, 100, "completed", "转录完成")
    return {"segments": all_segments}

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("audio_path")
    parser.add_argument("--language", default="auto")
    parser.add_argument("--output")
    args = parser.parse_args()
    
    try:
        result = transcribe(args.audio_path, args.language)
        if args.output:
            with open(args.output, "w", encoding="utf-8") as f:
                json.dump(result, f, ensure_ascii=False)
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
    
    // 设置可执行权限 (Unix)
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


/// Python 脚本输出的进度信息
#[derive(Debug, Deserialize)]
struct PythonProgress {
    #[serde(rename = "type")]
    msg_type: String,
    percent: f32,
    status: String,
    message: String,
}

/// 使用 SenseVoice 转录音频
pub async fn transcribe_with_sensevoice(
    audio_path: String,
    language: String,
    window: Window,
) -> Result<Vec<SubtitleEntry>, String> {
    reset_cancellation();
    
    // 记录开始时间
    let start_time = std::time::Instant::now();
    
    // 检查环境
    let env_status = check_sensevoice_env();
    if !env_status.ready {
        return Err("SenseVoice 环境未安装，请先安装环境".to_string());
    }
    
    let python_path = get_python_path()?;
    let scripts_dir = get_scripts_dir()?;
    let script_path = scripts_dir.join("sensevoice_transcribe.py");
    
    // 每次都更新脚本，确保使用最新版本
    write_transcription_script()?;
    
    // 发送初始进度
    let _ = window.emit("transcription-progress", SenseVoiceProgress {
        progress: 0.0,
        current_text: "正在启动转录...".to_string(),
        status: "loading".to_string(),
    });
    
    if is_cancelled() {
        return Err("转录已取消".to_string());
    }
    
    // 创建临时输出文件
    let output_path = std::env::temp_dir().join(format!("sensevoice_output_{}.json", std::process::id()));
    
    // 映射语言代码
    let lang_code = match language.as_str() {
        "zh" => "zh",
        "en" => "en",
        "ja" => "ja",
        "ko" => "ko",
        "yue" => "yue",  // 粤语
        _ => "auto",
    };
    
    // 确定设备（用于传递给 Python 脚本，实际设备信息由 Python 返回）
    let _device = if env_status.is_gpu { "cuda" } else { "cpu" };
    
    // 使用 spawn 启动进程，以便异步读取 stderr
    use std::process::Stdio;
    use std::io::{BufRead, BufReader};
    
    #[cfg(target_os = "windows")]
    let mut child = {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        Command::new(&python_path)
            .args([
                "-u",  // 强制无缓冲模式
                script_path.to_str().unwrap(),
                &audio_path,
                "--language", lang_code,
                "--output", output_path.to_str().unwrap(),
            ])
            .env("PYTHONUNBUFFERED", "1")
            .stderr(Stdio::piped())
            .stdout(Stdio::piped())
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map_err(|e| format!("执行转录脚本失败: {}", e))?
    };
    
    #[cfg(not(target_os = "windows"))]
    let mut child = Command::new(&python_path)
        .args([
            "-u",  // 强制无缓冲模式
            script_path.to_str().unwrap(),
            &audio_path,
            "--language", lang_code,
            "--output", output_path.to_str().unwrap(),
        ])
        .env("PYTHONUNBUFFERED", "1")
        .stderr(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|e| format!("执行转录脚本失败: {}", e))?;
    
    // 获取 stderr 用于读取进度
    let stderr = child.stderr.take()
        .ok_or_else(|| "无法获取 stderr".to_string())?;
    
    // 获取 stdout 用于读取设备信息
    let stdout = child.stdout.take();
    
    // 用于日志的参数
    let audio_path_for_log = audio_path.clone();
    let lang_code_for_log = lang_code.to_string();
    
    // 在后台线程读取 stdout，解析设备信息
    let stdout_handle = std::thread::spawn(move || {
        if let Some(stdout) = stdout {
            let reader = BufReader::new(stdout);
            for line in reader.lines().flatten() {
                // 解析 DEVICE_INFO:设备类型:GPU型号:显存 格式
                if line.starts_with("DEVICE_INFO:") {
                    let content = line.trim_start_matches("DEVICE_INFO:");
                    let parts: Vec<&str> = content.splitn(3, ':').collect();
                    let device_str = if parts.len() >= 2 && parts[0] == "cuda" && !parts[1].is_empty() {
                        if parts.len() >= 3 && !parts[2].is_empty() {
                            format!("CUDA ({}, {})", parts[1], parts[2])
                        } else {
                            format!("CUDA ({})", parts[1])
                        }
                    } else {
                        "CPU".to_string()
                    };
                    log::info!(
                        "开始语音转录: 音频文件={}, 模型=SenseVoiceSmall, 语言={}, 设备={}",
                        audio_path_for_log, lang_code_for_log, device_str
                    );
                }
            }
        }
    });
    
    // 在后台线程读取 stderr 并发送进度
    let window_clone = window.clone();
    let stderr_handle = std::thread::spawn(move || {
        let reader = BufReader::new(stderr);
        let mut last_error = String::new();
        
        for line in reader.lines() {
            if let Ok(line) = line {
                // 尝试解析为进度 JSON
                if let Ok(progress) = serde_json::from_str::<PythonProgress>(&line) {
                    if progress.msg_type == "progress" {
                        let _ = window_clone.emit("transcription-progress", SenseVoiceProgress {
                            progress: progress.percent,
                            current_text: progress.message,
                            status: progress.status,
                        });
                    }
                } else {
                    // 非进度信息，可能是错误
                    last_error = line;
                }
            }
        }
        last_error
    });
    
    // 等待进程完成
    let status = child.wait()
        .map_err(|e| format!("等待进程失败: {}", e))?;
    
    // 等待 stdout 线程完成
    let _ = stdout_handle.join();
    
    // 获取 stderr 线程的结果
    let last_error = stderr_handle.join()
        .map_err(|_| "读取 stderr 线程失败".to_string())?;
    
    if is_cancelled() {
        // 清理临时文件
        let _ = std::fs::remove_file(&output_path);
        return Err("转录已取消".to_string());
    }
    
    if !status.success() {
        let _ = std::fs::remove_file(&output_path);
        if !last_error.is_empty() {
            // 尝试解析错误 JSON
            if let Ok(err_json) = serde_json::from_str::<serde_json::Value>(&last_error) {
                if let Some(err_msg) = err_json.get("error").and_then(|v| v.as_str()) {
                    return Err(format!("转录失败: {}", err_msg));
                }
            }
            return Err(format!("转录失败: {}", last_error));
        }
        return Err("转录失败: 未知错误".to_string());
    }
    
    // 读取结果
    let result_json = std::fs::read_to_string(&output_path)
        .map_err(|e| format!("读取转录结果失败: {}", e))?;
    
    // 清理临时文件
    let _ = std::fs::remove_file(&output_path);
    
    // 解析 JSON
    let result: TranscriptionResult = serde_json::from_str(&result_json)
        .map_err(|e| format!("解析转录结果失败: {}", e))?;
    
    // 转换为字幕条目
    let mut entries = Vec::new();
    for (i, segment) in result.segments.iter().enumerate() {
        let start_ms = (segment.start * 1000.0) as u32;
        let end_ms = (segment.end * 1000.0) as u32;
        
        let start_time = TimeStamp {
            hours: start_ms / 3600000,
            minutes: (start_ms % 3600000) / 60000,
            seconds: (start_ms % 60000) / 1000,
            milliseconds: start_ms % 1000,
        };
        
        let end_time = TimeStamp {
            hours: end_ms / 3600000,
            minutes: (end_ms % 3600000) / 60000,
            seconds: (end_ms % 60000) / 1000,
            milliseconds: end_ms % 1000,
        };
        
        entries.push(SubtitleEntry {
            id: (i + 1) as u32,
            start_time,
            end_time,
            text: segment.text.trim().to_string(),
        });
    }
    
    // 计算耗时
    let elapsed = start_time.elapsed();
    let elapsed_secs = elapsed.as_secs_f64();
    
    log::info!(
        "语音转录完成: 音频文件={}, 模型=SenseVoiceSmall, 耗时={:.2}秒, 生成{}条字幕",
        audio_path, elapsed_secs, entries.len()
    );
    
    // 发送完成
    let _ = window.emit("transcription-progress", SenseVoiceProgress {
        progress: 100.0,
        current_text: "转录完成".to_string(),
        status: "completed".to_string(),
    });
    
    Ok(entries)
}

/// 卸载 SenseVoice 环境（兼容旧接口，卸载当前激活的环境）
pub fn uninstall_sensevoice_env() -> Result<String, String> {
    let active = get_active_env_type();
    match active.as_str() {
        "gpu" => uninstall_sensevoice_env_by_type(true),
        "cpu" => uninstall_sensevoice_env_by_type(false),
        _ => {
            // 如果没有激活的环境，尝试卸载所有存在的环境
            let cpu_dir = get_sensevoice_cpu_env_dir()?;
            let gpu_dir = get_sensevoice_gpu_env_dir()?;
            
            if cpu_dir.exists() {
                std::fs::remove_dir_all(&cpu_dir)
                    .map_err(|e| format!("删除 CPU 环境目录失败: {}", e))?;
            }
            if gpu_dir.exists() {
                std::fs::remove_dir_all(&gpu_dir)
                    .map_err(|e| format!("删除 GPU 环境目录失败: {}", e))?;
            }
            
            set_active_env_type("none")?;
            Ok("SenseVoice 环境已卸载".to_string())
        }
    }
}

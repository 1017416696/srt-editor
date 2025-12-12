use crate::srt_parser::{SubtitleEntry, TimeStamp};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{Emitter, Window};
use once_cell::sync::Lazy;

// 全局取消标志
static SENSEVOICE_CANCELLED: Lazy<Arc<AtomicBool>> = Lazy::new(|| Arc::new(AtomicBool::new(false)));

/// 取消当前转录任务
pub fn cancel_sensevoice_transcription() {
    SENSEVOICE_CANCELLED.store(true, Ordering::SeqCst);
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SenseVoiceEnvStatus {
    pub uv_installed: bool,
    pub env_exists: bool,
    pub ready: bool,
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

/// 获取模型文件路径
fn get_sensevoice_model_path(model_name: &str) -> Result<PathBuf, String> {
    let model_dir = get_sensevoice_model_dir()?;
    Ok(model_dir.join(model_name))
}

/// 获取部分下载文件路径
fn get_sensevoice_part_file_path(model_name: &str, file_name: &str) -> Result<PathBuf, String> {
    let model_path = get_sensevoice_model_path(model_name)?;
    Ok(model_path.join(format!("{}.part", file_name)))
}

/// 检查 SenseVoice 模型是否已下载
pub fn is_sensevoice_model_downloaded(model_name: &str) -> bool {
    let model_path = match get_sensevoice_model_path(model_name) {
        Ok(path) => path,
        Err(_) => return false,
    };
    
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
    
    reset_cancellation();
    
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
            // 检查是否取消
            if is_cancelled() {
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

/// 获取 SenseVoice 环境目录
pub fn get_sensevoice_env_dir() -> Result<PathBuf, String> {
    let home_dir = dirs::home_dir()
        .ok_or_else(|| "Failed to get home directory".to_string())?;
    
    let env_dir = home_dir
        .join(".config")
        .join("srt-editor")
        .join("sensevoice-env");
    
    Ok(env_dir)
}

/// 获取 Python 脚本目录
pub fn get_scripts_dir() -> Result<PathBuf, String> {
    let home_dir = dirs::home_dir()
        .ok_or_else(|| "Failed to get home directory".to_string())?;
    
    let scripts_dir = home_dir
        .join(".config")
        .join("srt-editor")
        .join("scripts");
    
    // 创建目录
    if !scripts_dir.exists() {
        std::fs::create_dir_all(&scripts_dir)
            .map_err(|e| format!("Failed to create scripts directory: {}", e))?;
    }
    
    Ok(scripts_dir)
}

/// 获取 Python 可执行文件路径
fn get_python_path() -> Result<PathBuf, String> {
    let env_dir = get_sensevoice_env_dir()?;
    
    #[cfg(target_os = "windows")]
    let python_path = env_dir.join("Scripts").join("python.exe");
    
    #[cfg(not(target_os = "windows"))]
    let python_path = env_dir.join("bin").join("python");
    
    Ok(python_path)
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

/// 检查 SenseVoice 环境状态（快速检查，不启动 Python）
pub fn check_sensevoice_env() -> SenseVoiceEnvStatus {
    let uv_installed = check_uv_installed();
    let env_dir = get_sensevoice_env_dir().unwrap_or_default();
    let python_path = get_python_path().unwrap_or_default();
    
    let env_exists = env_dir.exists() && python_path.exists();
    
    // 快速检查：只检查 site-packages 中是否存在 funasr 目录
    // 这比启动 Python 导入模块快得多
    let ready = if env_exists {
        #[cfg(target_os = "windows")]
        {
            // Windows: Lib/site-packages/funasr
            let funasr_path = env_dir.join("Lib").join("site-packages").join("funasr");
            funasr_path.exists()
        }
        #[cfg(not(target_os = "windows"))]
        {
            // Linux/macOS: lib/pythonX.X/site-packages/funasr
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
    } else {
        false
    };
    
    SenseVoiceEnvStatus {
        uv_installed,
        env_exists,
        ready,
    }
}


/// 安装 SenseVoice 环境
pub async fn install_sensevoice_env(window: Window) -> Result<String, String> {
    reset_cancellation();
    
    // 获取 uv 路径
    let uv_path = get_uv_path()
        .ok_or("请先安装 uv 包管理器。访问 https://docs.astral.sh/uv/getting-started/installation/ 了解安装方法")?;
    
    let env_dir = get_sensevoice_env_dir()?;
    
    // 发送进度
    let _ = window.emit("sensevoice-progress", SenseVoiceProgress {
        progress: 10.0,
        current_text: "正在创建 Python 虚拟环境...".to_string(),
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
        current_text: "正在安装 PyTorch（可能需要几分钟）...".to_string(),
        status: "installing".to_string(),
    });
    
    // 安装 PyTorch (CPU 版本，体积较小)
    let python_path = get_python_path()?;
    let output = Command::new(&uv_path)
        .args([
            "pip", "install",
            "--python", python_path.to_str().unwrap(),
            "torch", "torchaudio",
            "--index-url", "https://download.pytorch.org/whl/cpu"
        ])
        .output()
        .map_err(|e| format!("安装 PyTorch 失败: {}", e))?;
    
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
    
    // 完成
    let _ = window.emit("sensevoice-progress", SenseVoiceProgress {
        progress: 100.0,
        current_text: "SenseVoice 环境安装完成！".to_string(),
        status: "completed".to_string(),
    });
    
    Ok("SenseVoice 环境安装成功".to_string())
}

/// 写入 Python 转录脚本
fn write_transcription_script() -> Result<(), String> {
    let scripts_dir = get_scripts_dir()?;
    let script_path = scripts_dir.join("sensevoice_transcribe.py");
    
    // 简化版脚本 - 直接使用 VAD 分段，不做额外合并
    // 增加进度输出到 stderr
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

from pydub import AudioSegment

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
    
    emit_progress(0, 100, "loading", "正在加载语音模型...")
    
    # 加载 VAD 模型（关键参数：max_end_silence_time=250 让分段更敏感）
    vad_model = AutoModel(
        model="fsmn-vad",
        max_single_segment_time=15000,  # 最长 15 秒
        max_end_silence_time=250,       # 静音 250ms 就分段
        device="cpu"
    )
    
    emit_progress(5, 100, "loading", "正在加载 SenseVoice 模型...")
    
    # 加载 SenseVoice 模型
    model = AutoModel(
        model="iic/SenseVoiceSmall",
        trust_remote_code=True,
        device="cpu"
    )
    
    emit_progress(10, 100, "vad", "正在分析语音片段...")
    
    # VAD 分段
    vad_res = vad_model.generate(input=audio_path)
    if not vad_res or not vad_res[0].get("value"):
        return {"segments": []}
    
    segments = vad_res[0]["value"]
    total_segments = len(segments)
    
    # 计算音频总时长用于预估
    audio = AudioSegment.from_file(audio_path)
    audio_duration_sec = len(audio) / 1000.0
    
    emit_progress(15, 100, "transcribing", f"共 {total_segments} 个片段，音频时长 {audio_duration_sec:.1f} 秒")
    
    # 创建临时目录
    tmp_dir = tempfile.mkdtemp()
    
    all_segments = []
    start_time = time.time()
    
    try:
        for idx, seg in enumerate(segments):
            start_ms, end_ms = seg[0], seg[1]
            
            # 计算进度（15% - 95% 用于转录）
            progress = 15 + int((idx / total_segments) * 80)
            
            # 计算预估剩余时间
            elapsed = time.time() - start_time
            if idx > 0:
                avg_time_per_seg = elapsed / idx
                remaining_segs = total_segments - idx
                eta_sec = avg_time_per_seg * remaining_segs
                if eta_sec < 60:
                    eta_str = f"预计剩余 {int(eta_sec)} 秒"
                else:
                    eta_str = f"预计剩余 {int(eta_sec / 60)} 分 {int(eta_sec % 60)} 秒"
            else:
                eta_str = "正在估算..."
            
            emit_progress(progress, 100, "transcribing", f"正在转录 {idx + 1}/{total_segments}，{eta_str}")
            
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
    
    emit_progress(100, 100, "completed", f"转录完成，共 {len(all_segments)} 条字幕")
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
        current_text: "正在启动 SenseVoice...".to_string(),
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
    
    // 发送完成
    let _ = window.emit("transcription-progress", SenseVoiceProgress {
        progress: 100.0,
        current_text: format!("转录完成！生成了 {} 条字幕", entries.len()),
        status: "completed".to_string(),
    });
    
    Ok(entries)
}

/// 卸载 SenseVoice 环境
pub fn uninstall_sensevoice_env() -> Result<String, String> {
    let env_dir = get_sensevoice_env_dir()?;
    
    if env_dir.exists() {
        std::fs::remove_dir_all(&env_dir)
            .map_err(|e| format!("删除环境目录失败: {}", e))?;
    }
    
    Ok("SenseVoice 环境已卸载".to_string())
}

use crate::srt_parser::{SubtitleEntry, TimeStamp};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use tauri::{Emitter, Window};
use once_cell::sync::Lazy;

// 全局取消标志（转录任务）
static WHISPER_CANCELLED: Lazy<Arc<AtomicBool>> = Lazy::new(|| Arc::new(AtomicBool::new(false)));

// 模型下载任务ID，用于取消旧的下载任务
static WHISPER_MODEL_DOWNLOAD_TASK_ID: Lazy<AtomicU64> = Lazy::new(|| AtomicU64::new(0));

/// 取消当前转录任务
pub fn cancel_whisper_transcription() {
    WHISPER_CANCELLED.store(true, Ordering::SeqCst);
}

/// 取消当前模型下载任务
pub fn cancel_whisper_model_download() {
    WHISPER_MODEL_DOWNLOAD_TASK_ID.fetch_add(1, Ordering::SeqCst);
    log::info!("Whisper model download cancelled by user");
}

/// 生成新的模型下载任务ID
fn new_whisper_model_download_task_id() -> u64 {
    WHISPER_MODEL_DOWNLOAD_TASK_ID.fetch_add(1, Ordering::SeqCst) + 1
}

/// 检查模型下载任务是否仍然有效
fn is_whisper_model_download_task_valid(task_id: u64) -> bool {
    WHISPER_MODEL_DOWNLOAD_TASK_ID.load(Ordering::SeqCst) == task_id
}

/// 重置取消标志
fn reset_cancellation() {
    WHISPER_CANCELLED.store(false, Ordering::SeqCst);
}

/// 检查是否已取消
fn is_cancelled() -> bool {
    WHISPER_CANCELLED.load(Ordering::SeqCst)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhisperProgress {
    pub progress: f32,
    pub current_text: String,
    pub status: String,
}

/// 单个环境的状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhisperEnvInfo {
    pub installed: bool,
    pub ready: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhisperEnvStatus {
    pub uv_installed: bool,
    pub cpu_env: WhisperEnvInfo,
    pub gpu_env: WhisperEnvInfo,
    pub active_env: String,  // "cpu", "gpu", or "none"
    // 兼容旧字段
    pub env_exists: bool,
    pub ready: bool,
    pub is_gpu: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhisperModelInfo {
    pub name: String,
    pub size: String,
    pub downloaded: bool,
    pub partial_size: Option<u64>,
}

/// 获取 Whisper 环境基础目录
fn get_whisper_base_dir() -> Result<PathBuf, String> {
    let home_dir = dirs::home_dir()
        .ok_or_else(|| "Failed to get home directory".to_string())?;
    
    Ok(home_dir.join(".config").join("srt-editor"))
}

/// 获取 Whisper CPU 环境目录
pub fn get_whisper_cpu_env_dir() -> Result<PathBuf, String> {
    let base_dir = get_whisper_base_dir()?;
    Ok(base_dir.join("whisper-env-cpu"))
}

/// 获取 Whisper GPU 环境目录
pub fn get_whisper_gpu_env_dir() -> Result<PathBuf, String> {
    let base_dir = get_whisper_base_dir()?;
    Ok(base_dir.join("whisper-env-gpu"))
}

/// 获取当前激活的环境配置文件路径
fn get_whisper_active_env_config_path() -> Result<PathBuf, String> {
    let base_dir = get_whisper_base_dir()?;
    Ok(base_dir.join("whisper-active-env"))
}

/// 获取当前激活的环境类型
pub fn get_whisper_active_env_type() -> String {
    get_whisper_active_env_config_path()
        .ok()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "none".to_string())
}

/// 设置当前激活的环境类型
pub fn set_whisper_active_env_type(env_type: &str) -> Result<(), String> {
    let config_path = get_whisper_active_env_config_path()?;
    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("创建配置目录失败: {}", e))?;
    }
    std::fs::write(&config_path, env_type)
        .map_err(|e| format!("写入配置失败: {}", e))?;
    Ok(())
}

/// 获取 Whisper 环境目录（兼容旧代码，返回当前激活的环境）
pub fn get_whisper_env_dir() -> Result<PathBuf, String> {
    let active = get_whisper_active_env_type();
    match active.as_str() {
        "gpu" => get_whisper_gpu_env_dir(),
        "cpu" => get_whisper_cpu_env_dir(),
        _ => {
            // 如果没有激活的环境，检查哪个存在
            let gpu_dir = get_whisper_gpu_env_dir()?;
            if gpu_dir.exists() {
                return Ok(gpu_dir);
            }
            let cpu_dir = get_whisper_cpu_env_dir()?;
            if cpu_dir.exists() {
                return Ok(cpu_dir);
            }
            // 默认返回 CPU 目录
            Ok(cpu_dir)
        }
    }
}

/// 获取模型缓存目录
pub fn get_whisper_model_dir() -> Result<PathBuf, String> {
    let home_dir = dirs::home_dir()
        .ok_or_else(|| "Failed to get home directory".to_string())?;
    
    // faster-whisper 使用 huggingface 缓存目录
    let model_dir = home_dir.join(".cache").join("huggingface").join("hub");
    
    Ok(model_dir)
}

/// 获取 Python 脚本目录
pub fn get_scripts_dir() -> Result<PathBuf, String> {
    let home_dir = dirs::home_dir()
        .ok_or_else(|| "Failed to get home directory".to_string())?;
    
    let scripts_dir = home_dir
        .join(".config")
        .join("srt-editor")
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
    let env_dir = get_whisper_env_dir()?;
    Ok(get_python_path_for_env(&env_dir))
}

/// 检查 uv 是否已安装
fn check_uv_installed() -> bool {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        
        let path_check = Command::new("uv")
            .arg("--version")
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        
        if path_check {
            return true;
        }
        
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
        let path_check = Command::new("uv")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        
        if path_check {
            return true;
        }
        
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
        
        if Command::new("uv")
            .arg("--version")
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            return Some(PathBuf::from("uv"));
        }
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
        if Command::new("uv")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            return Some(PathBuf::from("uv"));
        }
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
fn check_whisper_env_ready(env_dir: &PathBuf) -> bool {
    let python_path = get_python_path_for_env(env_dir);
    if !env_dir.exists() || !python_path.exists() {
        return false;
    }
    
    #[cfg(target_os = "windows")]
    {
        let faster_whisper_path = env_dir.join("Lib").join("site-packages").join("faster_whisper");
        faster_whisper_path.exists()
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
                            let sp = path.join("site-packages").join("faster_whisper");
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

/// 检查 Whisper 环境状态
pub fn check_whisper_env() -> WhisperEnvStatus {
    let uv_installed = check_uv_installed();
    
    // 检查 CPU 环境
    let cpu_dir = get_whisper_cpu_env_dir().unwrap_or_default();
    let cpu_installed = cpu_dir.exists();
    let cpu_ready = check_whisper_env_ready(&cpu_dir);
    
    // 检查 GPU 环境
    let gpu_dir = get_whisper_gpu_env_dir().unwrap_or_default();
    let gpu_installed = gpu_dir.exists();
    let gpu_ready = check_whisper_env_ready(&gpu_dir);
    
    // 获取当前激活的环境
    let mut active_env = get_whisper_active_env_type();
    
    // 如果激活的环境不存在，自动切换到可用的环境
    if active_env == "gpu" && !gpu_ready {
        if cpu_ready {
            let _ = set_whisper_active_env_type("cpu");
            active_env = "cpu".to_string();
        } else {
            let _ = set_whisper_active_env_type("none");
            active_env = "none".to_string();
        }
    } else if active_env == "cpu" && !cpu_ready {
        if gpu_ready {
            let _ = set_whisper_active_env_type("gpu");
            active_env = "gpu".to_string();
        } else {
            let _ = set_whisper_active_env_type("none");
            active_env = "none".to_string();
        }
    } else if active_env == "none" {
        // 自动选择一个可用的环境
        if gpu_ready {
            let _ = set_whisper_active_env_type("gpu");
            active_env = "gpu".to_string();
        } else if cpu_ready {
            let _ = set_whisper_active_env_type("cpu");
            active_env = "cpu".to_string();
        }
    }
    
    // 兼容旧字段
    let env_exists = cpu_installed || gpu_installed;
    let ready = cpu_ready || gpu_ready;
    let is_gpu = active_env == "gpu";
    
    WhisperEnvStatus {
        uv_installed,
        cpu_env: WhisperEnvInfo {
            installed: cpu_installed,
            ready: cpu_ready,
        },
        gpu_env: WhisperEnvInfo {
            installed: gpu_installed,
            ready: gpu_ready,
        },
        active_env,
        env_exists,
        ready,
        is_gpu,
    }
}


/// 安装 Whisper 环境
/// use_gpu: 是否安装 GPU 版本（需要 NVIDIA 显卡和 CUDA）
pub async fn install_whisper_env(window: Window, use_gpu: bool) -> Result<String, String> {
    reset_cancellation();
    
    // 获取 uv 路径
    let uv_path = get_uv_path()
        .ok_or("请先安装 uv 包管理器。访问 https://docs.astral.sh/uv/getting-started/installation/ 了解安装方法")?;
    
    // 根据版本选择对应的环境目录
    let env_dir = if use_gpu {
        get_whisper_gpu_env_dir()?
    } else {
        get_whisper_cpu_env_dir()?
    };
    
    let version_type = if use_gpu { "GPU" } else { "CPU" };
    
    // 发送进度
    let _ = window.emit("whisper-progress", WhisperProgress {
        progress: 10.0,
        current_text: format!("正在创建 Python 虚拟环境（{} 版本）...", version_type),
        status: "installing".to_string(),
    });
    
    if is_cancelled() {
        return Err("安装已取消".to_string());
    }
    
    // 创建虚拟环境
    #[cfg(target_os = "windows")]
    let output = {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        Command::new(&uv_path)
            .args(["venv", env_dir.to_str().unwrap(), "--python", "3.11"])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map_err(|e| format!("创建虚拟环境失败: {}", e))?
    };
    
    #[cfg(not(target_os = "windows"))]
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
    let _ = window.emit("whisper-progress", WhisperProgress {
        progress: 30.0,
        current_text: format!("正在安装 PyTorch {} 版本（可能需要几分钟）...", version_type),
        status: "installing".to_string(),
    });
    
    let python_path = get_python_path_for_env(&env_dir);
    
    // 根据是否使用 GPU 选择不同的 PyTorch 安装方式
    #[cfg(target_os = "windows")]
    let output = {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        if use_gpu {
            Command::new(&uv_path)
                .args([
                    "pip", "install",
                    "--python", python_path.to_str().unwrap(),
                    "torch", "torchaudio",
                    "--index-url", "https://download.pytorch.org/whl/cu124"
                ])
                .creation_flags(CREATE_NO_WINDOW)
                .output()
                .map_err(|e| format!("安装 PyTorch GPU 版本失败: {}", e))?
        } else {
            Command::new(&uv_path)
                .args([
                    "pip", "install",
                    "--python", python_path.to_str().unwrap(),
                    "torch", "torchaudio",
                    "--index-url", "https://download.pytorch.org/whl/cpu"
                ])
                .creation_flags(CREATE_NO_WINDOW)
                .output()
                .map_err(|e| format!("安装 PyTorch 失败: {}", e))?
        }
    };
    
    #[cfg(not(target_os = "windows"))]
    let output = if use_gpu {
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
    let _ = window.emit("whisper-progress", WhisperProgress {
        progress: 60.0,
        current_text: "正在安装 faster-whisper...".to_string(),
        status: "installing".to_string(),
    });
    
    // 安装 faster-whisper
    // 安装 faster-whisper 和 huggingface_hub（用于模型下载）
    #[cfg(target_os = "windows")]
    let output = {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        Command::new(&uv_path)
            .args([
                "pip", "install",
                "--python", python_path.to_str().unwrap(),
                "faster-whisper", "pydub", "huggingface_hub"
            ])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map_err(|e| format!("安装 faster-whisper 失败: {}", e))?
    };
    
    #[cfg(not(target_os = "windows"))]
    let output = Command::new(&uv_path)
        .args([
            "pip", "install",
            "--python", python_path.to_str().unwrap(),
            "faster-whisper", "pydub", "huggingface_hub"
        ])
        .output()
        .map_err(|e| format!("安装 faster-whisper 失败: {}", e))?;
    
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("安装 faster-whisper 失败: {}", stderr));
    }
    
    // 写入 Python 转录脚本
    let _ = window.emit("whisper-progress", WhisperProgress {
        progress: 90.0,
        current_text: "正在配置转录脚本...".to_string(),
        status: "installing".to_string(),
    });
    
    write_transcription_script()?;
    
    // 设置为当前激活的环境
    let env_type = if use_gpu { "gpu" } else { "cpu" };
    set_whisper_active_env_type(env_type)?;
    
    // 完成
    let _ = window.emit("whisper-progress", WhisperProgress {
        progress: 100.0,
        current_text: format!("Whisper {} 版本安装完成！", version_type),
        status: "completed".to_string(),
    });
    
    Ok(format!("Whisper {} 版本安装成功", version_type))
}

/// 切换当前使用的 Whisper 环境
pub fn switch_whisper_env(use_gpu: bool) -> Result<String, String> {
    let env_type = if use_gpu { "gpu" } else { "cpu" };
    let env_dir = if use_gpu {
        get_whisper_gpu_env_dir()?
    } else {
        get_whisper_cpu_env_dir()?
    };
    
    if !check_whisper_env_ready(&env_dir) {
        return Err(format!("{} 环境未安装或不完整", if use_gpu { "GPU" } else { "CPU" }));
    }
    
    set_whisper_active_env_type(env_type)?;
    Ok(format!("已切换到 {} 版本", if use_gpu { "GPU" } else { "CPU" }))
}

/// 卸载 Whisper 环境
pub fn uninstall_whisper_env() -> Result<String, String> {
    let env_dir = get_whisper_env_dir()?;
    
    if !env_dir.exists() {
        return Err("Whisper 环境未安装".to_string());
    }
    
    std::fs::remove_dir_all(&env_dir)
        .map_err(|e| format!("卸载失败: {}", e))?;
    
    // 重置激活状态
    set_whisper_active_env_type("none")?;
    
    Ok("Whisper 环境已卸载".to_string())
}

/// 卸载指定类型的 Whisper 环境
pub fn uninstall_whisper_env_by_type(use_gpu: bool) -> Result<String, String> {
    let env_dir = if use_gpu {
        get_whisper_gpu_env_dir()?
    } else {
        get_whisper_cpu_env_dir()?
    };
    
    let version_type = if use_gpu { "GPU" } else { "CPU" };
    
    if !env_dir.exists() {
        return Err(format!("{} 环境未安装", version_type));
    }
    
    std::fs::remove_dir_all(&env_dir)
        .map_err(|e| format!("卸载失败: {}", e))?;
    
    // 如果卸载的是当前激活的环境，重置激活状态
    let active = get_whisper_active_env_type();
    if (use_gpu && active == "gpu") || (!use_gpu && active == "cpu") {
        // 检查另一个环境是否可用
        let other_dir = if use_gpu {
            get_whisper_cpu_env_dir()?
        } else {
            get_whisper_gpu_env_dir()?
        };
        
        if check_whisper_env_ready(&other_dir) {
            set_whisper_active_env_type(if use_gpu { "cpu" } else { "gpu" })?;
        } else {
            set_whisper_active_env_type("none")?;
        }
    }
    
    Ok(format!("{} 环境已卸载", version_type))
}

/// 写入转录脚本
fn write_transcription_script() -> Result<(), String> {
    let scripts_dir = get_scripts_dir()?;
    let script_path = scripts_dir.join("whisper_transcribe.py");
    
    let script_content = r#"#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Whisper 转录脚本 - 使用 faster-whisper（带实时进度）
"""

import sys
import os
import json
import argparse

# 强制禁用输出缓冲
os.environ["PYTHONUNBUFFERED"] = "1"

from faster_whisper import WhisperModel

def log(msg):
    """输出日志并立即刷新"""
    print(msg, flush=True)
    sys.stdout.flush()

def get_audio_duration(audio_path: str) -> float:
    """获取音频时长（秒）"""
    try:
        from pydub import AudioSegment
        audio = AudioSegment.from_file(audio_path)
        return len(audio) / 1000.0
    except:
        return 0.0

def transcribe(audio_path: str, model_size: str, language: str, device: str = "auto", output_path: str = None):
    """转录音频文件，实时输出进度"""
    
    import torch
    
    # 确定设备
    if device == "auto":
        device = "cuda" if torch.cuda.is_available() else "cpu"
        compute_type = "float16" if device == "cuda" else "int8"
    elif device == "cuda":
        compute_type = "float16"
    else:
        compute_type = "int8"
    
    # 输出设备信息（包含 GPU 型号和显存）
    if device == "cuda" and torch.cuda.is_available():
        gpu_name = torch.cuda.get_device_name(0)
        gpu_mem = torch.cuda.get_device_properties(0).total_memory / (1024**3)  # GB
        log(f"DEVICE_INFO:cuda:{gpu_name}:{gpu_mem:.1f}GB")
    else:
        log("DEVICE_INFO:cpu::")
    
    # 输出加载状态
    log("STATUS:loading")
    
    # 预先获取音频时长用于进度估算
    audio_duration = get_audio_duration(audio_path)
    log(f"DURATION:{audio_duration:.1f}")
    
    # 加载模型
    model = WhisperModel(model_size, device=device, compute_type=compute_type)
    
    # 输出转录状态，同时传递估算信息
    log("STATUS:transcribing")
    
    # 转录 - segments 是生成器
    segments, info = model.transcribe(
        audio_path,
        language=language if language != "auto" else None,
        beam_size=5,
        vad_filter=True,
        vad_parameters=dict(min_silence_duration_ms=500),
    )
    
    total_duration = info.duration if info.duration and info.duration > 0 else audio_duration or 1.0
    
    # 收集结果，输出每个 segment 的进度
    results = []
    for segment in segments:
        results.append({
            "start": segment.start,
            "end": segment.end,
            "text": segment.text.strip()
        })
        
        # 基于实际 segment 更新进度
        progress = min((segment.end / total_duration) * 100, 95.0)
        text_preview = segment.text.strip()[:30]
        log(f"PROGRESS:{progress:.1f}:{text_preview}")
    
    # 写入结果文件
    result = {
        "segments": results,
        "language": info.language,
        "duration": info.duration
    }
    
    if output_path:
        with open(output_path, "w", encoding="utf-8") as f:
            json.dump(result, f, ensure_ascii=False, indent=2)
    
    log("STATUS:completed")
    return result

def main():
    parser = argparse.ArgumentParser(description="Whisper 转录")
    parser.add_argument("--audio", required=True, help="音频文件路径")
    parser.add_argument("--model", default="base", help="模型大小")
    parser.add_argument("--language", default="auto", help="语言代码")
    parser.add_argument("--device", default="auto", help="设备: auto, cpu, cuda")
    parser.add_argument("--output", required=True, help="输出 JSON 文件路径")
    
    args = parser.parse_args()
    
    try:
        result = transcribe(args.audio, args.model, args.language, args.device, args.output)
        log(json.dumps({"status": "success", "segments": len(result["segments"])}))
    except Exception as e:
        print(f"ERROR:{str(e)}", file=sys.stderr, flush=True)
        sys.exit(1)

if __name__ == "__main__":
    main()
"#;
    
    std::fs::write(&script_path, script_content)
        .map_err(|e| format!("写入脚本失败: {}", e))?;
    
    Ok(())
}


/// Python 脚本输出的转录结果
#[derive(Debug, Deserialize)]
struct TranscriptionResult {
    segments: Vec<TranscriptionSegment>,
    #[allow(dead_code)]
    language: Option<String>,
    #[allow(dead_code)]
    duration: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct TranscriptionSegment {
    start: f64,
    end: f64,
    text: String,
}

/// 使用 Whisper 转录音频
pub async fn transcribe_with_whisper(
    audio_path: String,
    model_size: String,
    language: String,
    window: Window,
) -> Result<Vec<SubtitleEntry>, String> {
    use std::io::{BufRead, BufReader};
    use std::process::Stdio;
    
    reset_cancellation();
    
    // 记录开始时间
    let start_time = std::time::Instant::now();
    
    // 检查环境
    let env_status = check_whisper_env();
    if !env_status.ready {
        return Err("Whisper 环境未安装，请先安装环境".to_string());
    }
    
    let python_path = get_python_path()?;
    let scripts_dir = get_scripts_dir()?;
    let script_path = scripts_dir.join("whisper_transcribe.py");
    
    // 总是更新脚本以确保使用最新版本
    write_transcription_script()?;
    
    // 创建临时输出文件
    let temp_dir = std::env::temp_dir();
    let output_path = temp_dir.join(format!("whisper_result_{}.json", std::process::id()));
    
    // 发送初始进度
    let _ = window.emit("transcription-progress", WhisperProgress {
        progress: 0.0,
        current_text: "正在启动转录...".to_string(),
        status: "starting".to_string(),
    });
    
    if is_cancelled() {
        return Err("转录已取消".to_string());
    }
    
    // 确定设备
    let device = if env_status.is_gpu { "cuda" } else { "cpu" };
    
    // 运行 Python 脚本，使用 Stdio::piped() 实时读取输出
    #[cfg(target_os = "windows")]
    let mut child = {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        Command::new(&python_path)
            .args([
                "-u",  // unbuffered output
                script_path.to_str().unwrap(),
                "--audio", &audio_path,
                "--model", &model_size,
                "--language", &language,
                "--device", device,
                "--output", output_path.to_str().unwrap(),
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map_err(|e| format!("运行转录脚本失败: {}", e))?
    };
    
    #[cfg(not(target_os = "windows"))]
    let mut child = Command::new(&python_path)
        .args([
            "-u",  // unbuffered output
            script_path.to_str().unwrap(),
            "--audio", &audio_path,
            "--model", &model_size,
            "--language", &language,
            "--device", device,
            "--output", output_path.to_str().unwrap(),
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("运行转录脚本失败: {}", e))?;
    
    // 获取 stdout 和 stderr
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    
    // 用于进度模拟的共享状态
    let audio_duration = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let is_transcribing = Arc::new(AtomicBool::new(false));
    let transcribe_done = Arc::new(AtomicBool::new(false));
    let current_progress = Arc::new(std::sync::atomic::AtomicU64::new(0));
    
    let window_clone = window.clone();
    let audio_duration_clone = audio_duration.clone();
    let is_transcribing_clone = is_transcribing.clone();
    let transcribe_done_clone = transcribe_done.clone();
    let current_progress_clone = current_progress.clone();
    
    // 用于日志的参数
    let audio_path_for_log = audio_path.clone();
    let model_size_for_log = model_size.clone();
    let language_for_log = language.clone();
    
    // 在后台线程读取 stdout，解析进度
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
                        "开始语音转录: 音频文件={}, 模型=faster-whisper-{}, 语言={}, 设备={}",
                        audio_path_for_log, model_size_for_log, language_for_log, device_str
                    );
                    continue;
                }
                
                // 解析 DURATION:xxx 格式
                if line.starts_with("DURATION:") {
                    if let Ok(duration) = line.trim_start_matches("DURATION:").parse::<f64>() {
                        audio_duration_clone.store((duration * 1000.0) as u64, Ordering::SeqCst);
                    }
                    continue;
                }
                
                // 解析 STATUS:xxx 格式
                if line.starts_with("STATUS:") {
                    let status = line.trim_start_matches("STATUS:");
                    let (progress, text, status_str) = match status {
                        "loading" => (5.0, "正在加载 Whisper 模型...".to_string(), "loading"),
                        "transcribing" => {
                            is_transcribing_clone.store(true, Ordering::SeqCst);
                            (10.0, "模型加载完成，开始转录...".to_string(), "transcribing")
                        },
                        "completed" => {
                            transcribe_done_clone.store(true, Ordering::SeqCst);
                            (99.0, "转录完成，正在处理结果...".to_string(), "processing")
                        },
                        _ => continue,
                    };
                    current_progress_clone.store((progress * 100.0) as u64, Ordering::SeqCst);
                    let _ = window_clone.emit("transcription-progress", WhisperProgress {
                        progress,
                        current_text: text,
                        status: status_str.to_string(),
                    });
                }
                // 解析 PROGRESS:百分比:文本 格式
                else if line.starts_with("PROGRESS:") {
                    let content = line.trim_start_matches("PROGRESS:");
                    if let Some((pct_str, text)) = content.split_once(':') {
                        if let Ok(pct) = pct_str.parse::<f32>() {
                            // 将进度映射到 10-95 范围
                            let mapped_progress = 10.0 + (pct * 0.85);
                            current_progress_clone.store((mapped_progress * 100.0) as u64, Ordering::SeqCst);
                            let _ = window_clone.emit("transcription-progress", WhisperProgress {
                                progress: mapped_progress,
                                current_text: format!("正在转录: {}", text),
                                status: "transcribing".to_string(),
                            });
                        }
                    }
                }
            }
        }
    });
    
    // stderr 读取线程
    let stderr_handle = std::thread::spawn(move || {
        let mut stderr_output = String::new();
        if let Some(stderr) = stderr {
            let reader = BufReader::new(stderr);
            for line in reader.lines().flatten() {
                if line.contains("ERROR") || line.contains("error") || line.contains("Error") {
                    log::error!("Whisper transcribe error: {}", line);
                    stderr_output.push_str(&line);
                    stderr_output.push('\n');
                } else {
                    log::debug!("Whisper transcribe stderr: {}", line);
                }
            }
        }
        stderr_output
    });
    
    // 进度模拟线程 - 在 Rust 端模拟进度
    let window_for_progress = window.clone();
    let model_size_clone = model_size.clone();
    let is_gpu = env_status.is_gpu;
    let progress_handle = std::thread::spawn(move || {
        // 等待转录开始
        while !is_transcribing.load(Ordering::SeqCst) && !transcribe_done.load(Ordering::SeqCst) {
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        
        if transcribe_done.load(Ordering::SeqCst) {
            return;
        }
        
        let duration_ms = audio_duration.load(Ordering::SeqCst);
        if duration_ms == 0 {
            return;
        }
        
        let duration_secs = duration_ms as f64 / 1000.0;
        
        // 根据模型大小和设备估算处理速度（相对于实时播放速度的倍数）
        // GPU 通常比实时快很多，根据你的实际测试调整
        let speed_factor = if is_gpu { 12.0 } else { 2.0 };
        let model_factor = match model_size_clone.as_str() {
            "tiny" => 3.0,
            "base" => 2.0,
            "small" => 1.2,
            "medium" => 0.8,
            "large-v2" | "large-v3" => 0.55,
            _ => 1.0,
        };
        let estimated_time = duration_secs / (speed_factor * model_factor);
        
        let start_time = std::time::Instant::now();
        
        while !transcribe_done.load(Ordering::SeqCst) {
            let elapsed = start_time.elapsed().as_secs_f64();
            // 使用非线性进度曲线
            let simulated_progress = ((elapsed / estimated_time).powf(0.7) * 85.0).min(85.0);
            let mapped_progress = 10.0 + simulated_progress;
            
            // 只有当模拟进度大于当前进度时才更新
            let current = current_progress.load(Ordering::SeqCst) as f64 / 100.0;
            if mapped_progress > current {
                current_progress.store((mapped_progress * 100.0) as u64, Ordering::SeqCst);
                let _ = window_for_progress.emit("transcription-progress", WhisperProgress {
                    progress: mapped_progress as f32,
                    current_text: "正在识别语音内容...".to_string(),
                    status: "transcribing".to_string(),
                });
            }
            
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
    });
    
    // 等待所有线程完成
    let _ = stdout_handle.join();
    let stderr_output = stderr_handle.join().unwrap_or_default();
    let _ = progress_handle.join();
    
    // 等待进程结束
    let status = child.wait().map_err(|e| format!("等待转录完成失败: {}", e))?;
    
    if is_cancelled() {
        let _ = std::fs::remove_file(&output_path);
        return Err("转录已取消".to_string());
    }
    
    if !status.success() {
        let _ = std::fs::remove_file(&output_path);
        return Err(format!("转录失败: {}", stderr_output));
    }
    
    // 读取结果
    let result_json = std::fs::read_to_string(&output_path)
        .map_err(|e| format!("读取转录结果失败: {}", e))?;
    
    // 删除临时文件
    let _ = std::fs::remove_file(&output_path);
    
    let result: TranscriptionResult = serde_json::from_str(&result_json)
        .map_err(|e| format!("解析转录结果失败: {}", e))?;
    
    // 转换为字幕条目
    let entries: Vec<SubtitleEntry> = result.segments
        .iter()
        .enumerate()
        .map(|(i, seg)| {
            let start_ms = (seg.start * 1000.0) as u32;
            let end_ms = (seg.end * 1000.0) as u32;
            
            SubtitleEntry {
                id: (i + 1) as u32,
                start_time: TimeStamp {
                    hours: start_ms / 3600000,
                    minutes: (start_ms % 3600000) / 60000,
                    seconds: (start_ms % 60000) / 1000,
                    milliseconds: start_ms % 1000,
                },
                end_time: TimeStamp {
                    hours: end_ms / 3600000,
                    minutes: (end_ms % 3600000) / 60000,
                    seconds: (end_ms % 60000) / 1000,
                    milliseconds: end_ms % 1000,
                },
                text: seg.text.clone(),
            }
        })
        .collect();
    
    // 计算耗时
    let elapsed = start_time.elapsed();
    let elapsed_secs = elapsed.as_secs_f64();
    
    log::info!(
        "语音转录完成: 音频文件={}, 模型=faster-whisper-{}, 耗时={:.2}秒, 生成{}条字幕",
        audio_path, model_size, elapsed_secs, entries.len()
    );
    
    // 发送完成进度
    let _ = window.emit("transcription-progress", WhisperProgress {
        progress: 100.0,
        current_text: format!("转录完成！共生成 {} 条字幕，耗时 {:.1} 秒", entries.len(), elapsed_secs),
        status: "completed".to_string(),
    });
    
    // 短暂延迟让前端有时间显示 100% 进度
    std::thread::sleep(std::time::Duration::from_millis(500));
    
    Ok(entries)
}

/// 获取已下载的部分大小（用于断点续传显示）
pub fn get_whisper_partial_size(model_name: &str) -> u64 {
    let home_dir = match dirs::home_dir() {
        Some(dir) => dir,
        None => return 0,
    };
    
    let hub_dir = home_dir.join(".cache").join("huggingface").join("hub");
    let model_dir_name = format!("models--Systran--faster-whisper-{}", model_name);
    let model_path = hub_dir.join(&model_dir_name);
    
    if !model_path.exists() {
        return 0;
    }
    
    let mut total_size = 0u64;
    
    // 检查 blobs 目录中已下载的文件大小
    let blobs_dir = model_path.join("blobs");
    if blobs_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&blobs_dir) {
            for entry in entries.flatten() {
                if let Ok(meta) = entry.metadata() {
                    if meta.is_file() {
                        total_size += meta.len();
                    }
                }
            }
        }
    }
    
    // 也检查 snapshots 目录中的文件
    let snapshots_dir = model_path.join("snapshots");
    if snapshots_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&snapshots_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    if let Ok(files) = std::fs::read_dir(&path) {
                        for file in files.flatten() {
                            if let Ok(meta) = file.metadata() {
                                if meta.is_file() {
                                    total_size += meta.len();
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    
    total_size
}

/// 获取可用的 Whisper 模型列表
pub fn get_whisper_models() -> Vec<WhisperModelInfo> {
    // faster-whisper 支持的模型
    let models = vec![
        ("tiny", "~75 MB"),
        ("base", "~145 MB"),
        ("small", "~488 MB"),
        ("medium", "~1.5 GB"),
        ("large-v2", "~3.1 GB"),
        ("large-v3", "~3.1 GB"),
    ];
    
    models.iter().map(|(name, size)| {
        // 检查模型是否已下载（faster-whisper 会自动下载到 huggingface 缓存）
        let downloaded = check_model_downloaded(name);
        
        // 如果未完全下载，检查部分下载大小
        let partial_size = if !downloaded {
            let size = get_whisper_partial_size(name);
            if size > 0 { Some(size) } else { None }
        } else {
            None
        };
        
        WhisperModelInfo {
            name: name.to_string(),
            size: size.to_string(),
            downloaded,
            partial_size,
        }
    }).collect()
}

/// 检查模型是否已下载
fn check_model_downloaded(model_name: &str) -> bool {
    let home_dir = match dirs::home_dir() {
        Some(dir) => dir,
        None => return false,
    };
    
    let hub_dir = home_dir.join(".cache").join("huggingface").join("hub");
    
    // 检查方式1: HuggingFace Hub 缓存格式
    // 格式: ~/.cache/huggingface/hub/models--Systran--faster-whisper-{model_name}
    let model_dir_name = format!("models--Systran--faster-whisper-{}", model_name);
    let model_path = hub_dir.join(&model_dir_name);
    
    if model_path.exists() {
        let snapshots_dir = model_path.join("snapshots");
        if snapshots_dir.exists() {
            if let Ok(entries) = std::fs::read_dir(&snapshots_dir) {
                // 检查是否有任何 snapshot 目录，且目录中有 model.bin 文件
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        // 检查是否有模型文件（必须同时存在 model.bin 和 config.json）
                        let model_bin = path.join("model.bin");
                        let config_json = path.join("config.json");
                        if model_bin.exists() && config_json.exists() {
                            return true;
                        }
                    }
                }
            }
        }
    }
    
    // 检查方式2: snapshot_download 直接下载的格式
    // 可能在 ~/.cache/huggingface/hub/Systran--faster-whisper-{model_name}
    let alt_model_dir = hub_dir.join(format!("Systran--faster-whisper-{}", model_name));
    if alt_model_dir.exists() {
        let model_bin = alt_model_dir.join("model.bin");
        let config_json = alt_model_dir.join("config.json");
        if model_bin.exists() && config_json.exists() {
            return true;
        }
    }
    
    false
}

/// 打开模型目录
pub fn open_whisper_model_dir() -> Result<(), String> {
    let home_dir = dirs::home_dir()
        .ok_or_else(|| "无法获取用户目录".to_string())?;
    
    let model_dir = home_dir
        .join(".cache")
        .join("huggingface")
        .join("hub");
    
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

/// 删除 Whisper 模型
pub fn delete_whisper_model(model_name: &str) -> Result<String, String> {
    let home_dir = dirs::home_dir()
        .ok_or_else(|| "无法获取用户目录".to_string())?;
    
    let model_dir_name = format!("models--Systran--faster-whisper-{}", model_name);
    let model_path = home_dir
        .join(".cache")
        .join("huggingface")
        .join("hub")
        .join(&model_dir_name);
    
    if !model_path.exists() {
        return Err(format!("模型 {} 未下载", model_name));
    }
    
    std::fs::remove_dir_all(&model_path)
        .map_err(|e| format!("删除模型失败: {}", e))?;
    
    Ok(format!("模型 {} 已删除", model_name))
}


/// 下载 Whisper 模型（使用 Python 脚本预下载）
pub async fn download_whisper_model(model_name: &str, window: Window) -> Result<String, String> {
    use std::io::{BufRead, BufReader};
    use std::process::Stdio;
    
    // 检查环境是否就绪
    let env_status = check_whisper_env();
    if !env_status.ready {
        return Err("Whisper 环境未安装，请先安装环境".to_string());
    }
    
    let python_path = get_python_path()?;
    let scripts_dir = get_scripts_dir()?;
    let download_script_path = scripts_dir.join("whisper_download_model.py");
    
    // 确保下载脚本存在
    write_download_script()?;
    
    // 发送初始进度
    let _ = window.emit("whisper-model-progress", WhisperProgress {
        progress: 0.0,
        current_text: format!("正在下载 {} 模型...", model_name),
        status: "downloading".to_string(),
    });
    
    // 运行 Python 脚本下载模型，实时读取输出
    #[cfg(target_os = "windows")]
    let mut child = {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        Command::new(&python_path)
            .args([
                "-u",  // unbuffered output
                download_script_path.to_str().unwrap(),
                "--model", model_name,
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map_err(|e| format!("运行下载脚本失败: {}", e))?
    };
    
    #[cfg(not(target_os = "windows"))]
    let mut child = Command::new(&python_path)
        .args([
            "-u",  // unbuffered output
            download_script_path.to_str().unwrap(),
            "--model", model_name,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("运行下载脚本失败: {}", e))?;
    
    // 读取 stdout
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    
    let window_clone = window.clone();
    let model_name_clone = model_name.to_string();
    
    // 在后台线程读取 stdout
    let stdout_handle = std::thread::spawn(move || {
        if let Some(stdout) = stdout {
            let reader = BufReader::new(stdout);
            for line in reader.lines().flatten() {
                log::info!("Whisper download: {}", line);
                
                // 解析 PROGRESS:xx 格式
                if line.starts_with("PROGRESS:") {
                    if let Ok(pct) = line.trim_start_matches("PROGRESS:").parse::<f32>() {
                        let _ = window_clone.emit("whisper-model-progress", WhisperProgress {
                            progress: pct,
                            current_text: format!("正在下载 {} 模型... {:.0}%", model_name_clone, pct),
                            status: "downloading".to_string(),
                        });
                    }
                }
                
                if line.contains("SUCCESS") {
                    let _ = window_clone.emit("whisper-model-progress", WhisperProgress {
                        progress: 100.0,
                        current_text: format!("模型 {} 下载完成！", model_name_clone),
                        status: "completed".to_string(),
                    });
                }
            }
        }
    });
    
    let mut stderr_output = String::new();
    if let Some(stderr) = stderr {
        let reader = BufReader::new(stderr);
        for line in reader.lines().flatten() {
            // 只记录错误信息，忽略警告
            if line.contains("ERROR") || line.contains("error") {
                log::error!("Whisper download error: {}", line);
                stderr_output.push_str(&line);
                stderr_output.push('\n');
            } else {
                log::debug!("Whisper download stderr: {}", line);
            }
        }
    }
    
    let _ = stdout_handle.join();
    
    let status = child.wait().map_err(|e| format!("等待下载完成失败: {}", e))?;
    
    if !status.success() {
        return Err(format!("下载模型失败: {}", stderr_output));
    }
    
    // 发送完成进度
    let _ = window.emit("whisper-model-progress", WhisperProgress {
        progress: 100.0,
        current_text: format!("模型 {} 下载完成！", model_name),
        status: "completed".to_string(),
    });
    
    Ok(format!("模型 {} 下载成功", model_name))
}

/// 写入模型下载脚本
fn write_download_script() -> Result<(), String> {
    let scripts_dir = get_scripts_dir()?;
    let script_path = scripts_dir.join("whisper_download_model.py");
    
    let script_content = r#"#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Whisper 模型下载脚本 - 预下载 faster-whisper 模型（带进度显示）
"""

import sys
import os
import argparse

# 禁用 symlinks 警告
os.environ["HF_HUB_DISABLE_SYMLINKS_WARNING"] = "1"

from huggingface_hub import hf_hub_download, list_repo_files

# 模型仓库映射
MODEL_REPOS = {
    "tiny": "Systran/faster-whisper-tiny",
    "base": "Systran/faster-whisper-base",
    "small": "Systran/faster-whisper-small",
    "medium": "Systran/faster-whisper-medium",
    "large-v2": "Systran/faster-whisper-large-v2",
    "large-v3": "Systran/faster-whisper-large-v3",
}

def download_model(model_size: str):
    """下载指定大小的模型"""
    if model_size not in MODEL_REPOS:
        raise ValueError(f"未知的模型大小: {model_size}，可选: {list(MODEL_REPOS.keys())}")
    
    repo_id = MODEL_REPOS[model_size]
    
    print(f"正在下载 {model_size} 模型 ({repo_id})...", flush=True)
    print("PROGRESS:0", flush=True)
    
    try:
        # 获取仓库中的所有文件
        files = list_repo_files(repo_id)
        total_files = len(files)
        
        print(f"共 {total_files} 个文件需要下载", flush=True)
        
        # 逐个下载文件
        for i, filename in enumerate(files):
            progress = (i / total_files) * 100
            print(f"PROGRESS:{progress:.1f}", flush=True)
            print(f"正在下载: {filename}", flush=True)
            
            hf_hub_download(
                repo_id=repo_id,
                filename=filename,
            )
        
        print("PROGRESS:100", flush=True)
        print(f"模型 {model_size} 下载完成！", flush=True)
        return True
        
    except Exception as e:
        raise Exception(f"下载失败: {e}")

def main():
    parser = argparse.ArgumentParser(description="下载 Whisper 模型")
    parser.add_argument("--model", required=True, help="模型大小: tiny, base, small, medium, large-v2, large-v3")
    
    args = parser.parse_args()
    
    try:
        download_model(args.model)
        print("SUCCESS", flush=True)
    except Exception as e:
        print(f"ERROR: {e}", file=sys.stderr, flush=True)
        sys.exit(1)

if __name__ == "__main__":
    main()
"#;
    
    std::fs::write(&script_path, script_content)
        .map_err(|e| format!("写入下载脚本失败: {}", e))?;
    
    Ok(())
}

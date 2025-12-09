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
    Command::new("uv")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// 检查 SenseVoice 环境状态
pub fn check_sensevoice_env() -> SenseVoiceEnvStatus {
    let uv_installed = check_uv_installed();
    let env_dir = get_sensevoice_env_dir().unwrap_or_default();
    let python_path = get_python_path().unwrap_or_default();
    
    let env_exists = env_dir.exists() && python_path.exists();
    
    // 检查是否安装了 funasr
    let ready = if env_exists {
        Command::new(&python_path)
            .args(["-c", "import funasr; print('ok')"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
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
    
    // 检查 uv 是否安装
    if !check_uv_installed() {
        return Err("请先安装 uv 包管理器。访问 https://docs.astral.sh/uv/getting-started/installation/ 了解安装方法".to_string());
    }
    
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
    let output = Command::new("uv")
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
    let output = Command::new("uv")
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
    let output = Command::new("uv")
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
    let script_content = r#"#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""SenseVoice 转录脚本 - 参考 jianchang512/sense-api 实现"""

import sys
import json
import re
import argparse
import os
import tempfile
from pydub import AudioSegment

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
    
    # 加载 VAD 模型（关键参数：max_end_silence_time=250 让分段更敏感）
    vad_model = AutoModel(
        model="fsmn-vad",
        max_single_segment_time=15000,  # 最长 15 秒
        max_end_silence_time=250,       # 静音 250ms 就分段
        device="cpu"
    )
    
    # 加载 SenseVoice 模型
    model = AutoModel(
        model="iic/SenseVoiceSmall",
        trust_remote_code=True,
        device="cpu"
    )
    
    # VAD 分段
    vad_res = vad_model.generate(input=audio_path)
    if not vad_res or not vad_res[0].get("value"):
        return {"segments": []}
    
    # 加载音频
    audio = AudioSegment.from_file(audio_path)
    
    # 创建临时目录
    tmp_dir = tempfile.mkdtemp()
    
    all_segments = []
    try:
        for seg in vad_res[0]["value"]:
            start_ms, end_ms = seg[0], seg[1]
            
            # 切分音频片段
            chunk = audio[start_ms:end_ms]
            chunk_file = os.path.join(tmp_dir, f"{start_ms}_{end_ms}.wav")
            chunk.export(chunk_file, format="wav")
            
            # 转录
            res = model.generate(input=chunk_file, language=language, use_itn=True)
            if not res:
                continue
            
            text = res[0].get("text", "")
            if not text:
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
    
    // 发送进度
    let _ = window.emit("transcription-progress", SenseVoiceProgress {
        progress: 10.0,
        current_text: "正在加载 SenseVoice 模型...".to_string(),
        status: "loading".to_string(),
    });
    
    if is_cancelled() {
        return Err("转录已取消".to_string());
    }
    
    // 创建临时输出文件
    let output_path = std::env::temp_dir().join(format!("sensevoice_output_{}.json", std::process::id()));
    
    // 发送进度
    let _ = window.emit("transcription-progress", SenseVoiceProgress {
        progress: 0.0,  // SenseVoice 没有真实进度，设为 0
        current_text: "正在使用 SenseVoice 转录...".to_string(),
        status: "transcribing".to_string(),
    });
    
    // 映射语言代码
    let lang_code = match language.as_str() {
        "zh" => "zh",
        "en" => "en",
        "ja" => "ja",
        "ko" => "ko",
        "yue" => "yue",  // 粤语
        _ => "auto",
    };
    
    // 执行 Python 脚本
    let output = Command::new(&python_path)
        .args([
            script_path.to_str().unwrap(),
            &audio_path,
            "--language", lang_code,
            "--output", output_path.to_str().unwrap(),
        ])
        .output()
        .map_err(|e| format!("执行转录脚本失败: {}", e))?;
    
    if is_cancelled() {
        // 清理临时文件
        let _ = std::fs::remove_file(&output_path);
        return Err("转录已取消".to_string());
    }
    
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("转录失败: {}", stderr));
    }
    
    // 发送进度
    let _ = window.emit("transcription-progress", SenseVoiceProgress {
        progress: 80.0,
        current_text: "正在生成字幕...".to_string(),
        status: "converting".to_string(),
    });
    
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

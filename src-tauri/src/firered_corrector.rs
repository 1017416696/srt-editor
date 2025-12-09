use crate::srt_parser::TimeStamp;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{Emitter, Window};
use once_cell::sync::Lazy;

// 全局取消标志
static FIRERED_CANCELLED: Lazy<Arc<AtomicBool>> = Lazy::new(|| Arc::new(AtomicBool::new(false)));

/// 取消当前校正任务
pub fn cancel_firered_correction() {
    FIRERED_CANCELLED.store(true, Ordering::SeqCst);
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FireRedEnvStatus {
    pub uv_installed: bool,
    pub env_exists: bool,
    pub ready: bool,
}

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

/// 获取 FireRedASR 环境目录
pub fn get_firered_env_dir() -> Result<PathBuf, String> {
    let home_dir = dirs::home_dir()
        .ok_or_else(|| "Failed to get home directory".to_string())?;
    
    let env_dir = home_dir
        .join(".config")
        .join("srt-editor")
        .join("firered-env");
    
    Ok(env_dir)
}

/// 获取 Python 脚本目录
fn get_scripts_dir() -> Result<PathBuf, String> {
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

/// 获取 Python 可执行文件路径
fn get_python_path() -> Result<PathBuf, String> {
    let env_dir = get_firered_env_dir()?;
    
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

/// 检查 FireRedASR 环境状态（快速检查，不启动 Python）
pub fn check_firered_env() -> FireRedEnvStatus {
    let uv_installed = check_uv_installed();
    let env_dir = get_firered_env_dir().unwrap_or_default();
    let python_path = get_python_path().unwrap_or_default();
    
    let env_exists = env_dir.exists() && python_path.exists();
    
    // 快速检查：只检查 site-packages 中是否存在 fireredasr 目录
    // 这比启动 Python 导入模块快得多
    let ready = if env_exists {
        let site_packages = env_dir.join("lib");
        // 在 macOS/Linux 上，site-packages 在 lib/pythonX.Y/site-packages
        // 我们检查是否存在 fireredasr 相关文件
        if site_packages.exists() {
            // 遍历查找 site-packages 目录
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
    } else {
        false
    };
    
    FireRedEnvStatus {
        uv_installed,
        env_exists,
        ready,
    }
}


/// 安装 FireRedASR 环境
pub async fn install_firered_env(window: Window) -> Result<String, String> {
    reset_cancellation();
    
    if !check_uv_installed() {
        return Err("请先安装 uv 包管理器。访问 https://docs.astral.sh/uv/getting-started/installation/ 了解安装方法".to_string());
    }
    
    let env_dir = get_firered_env_dir()?;
    
    let _ = window.emit("firered-progress", FireRedProgress {
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
    
    let _ = window.emit("firered-progress", FireRedProgress {
        progress: 30.0,
        current_text: "正在安装 PyTorch（可能需要几分钟）...".to_string(),
        status: "installing".to_string(),
    });
    
    // 安装 PyTorch (CPU 版本)
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
    
    let _ = window.emit("firered-progress", FireRedProgress {
        progress: 60.0,
        current_text: "正在安装 FireRedASR...".to_string(),
        status: "installing".to_string(),
    });
    
    // 安装 fireredasr 及依赖
    let output = Command::new("uv")
        .args([
            "pip", "install",
            "--python", python_path.to_str().unwrap(),
            "fireredasr", "pydub", "transformers", "sentencepiece"
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
        progress: 75.0,
        current_text: "正在下载 FireRedASR 模型（约 600MB，请耐心等待）...".to_string(),
        status: "installing".to_string(),
    });
    
    // 预下载模型
    let output = Command::new(&python_path)
        .args(["-c", r#"
from fireredasr.models.fireredasr import FireRedAsr
print('Downloading model...')
model = FireRedAsr.from_pretrained('aed')
print('Model downloaded successfully')
"#])
        .output()
        .map_err(|e| format!("下载模型失败: {}", e))?;
    
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // 模型下载失败不阻止安装完成，只是警告
        eprintln!("模型预下载失败（首次使用时会自动下载）: {}", stderr);
    }
    
    let _ = window.emit("firered-progress", FireRedProgress {
        progress: 90.0,
        current_text: "正在配置校正脚本...".to_string(),
        status: "installing".to_string(),
    });
    
    write_correction_script()?;
    
    let _ = window.emit("firered-progress", FireRedProgress {
        progress: 100.0,
        current_text: "FireRedASR 环境安装完成！".to_string(),
        status: "completed".to_string(),
    });
    
    Ok("FireRedASR 环境安装成功".to_string())
}

/// 写入 Python 校正脚本
fn write_correction_script() -> Result<(), String> {
    let scripts_dir = get_scripts_dir()?;
    let script_path = scripts_dir.join("firered_correct.py");
    
    let script_content = r#"#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""FireRedASR 字幕校正脚本"""

import sys
import json
import argparse
import os
import re
import tempfile
from pydub import AudioSegment

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

def correct_subtitles(srt_path, audio_path, language="zh", preserve_case=True):
    """使用 FireRedASR 校正字幕"""
    import torch
    import argparse
    
    # 修复 PyTorch 2.6+ 的兼容性问题
    # 添加 argparse.Namespace 到安全全局变量列表
    torch.serialization.add_safe_globals([argparse.Namespace])
    
    from fireredasr.models.fireredasr import FireRedAsr
    
    # 解析 SRT
    entries = parse_srt(srt_path)
    if not entries:
        return {"entries": []}
    
    # 加载音频
    audio = AudioSegment.from_file(audio_path)
    
    # 加载模型 (aed = Attention Encoder Decoder, 速度快精度高)
    print("正在加载 FireRedASR 模型...", file=sys.stderr)
    model = FireRedAsr.from_pretrained("aed")
    
    # 创建临时目录
    tmp_dir = tempfile.mkdtemp()
    
    results = []
    total = len(entries)
    
    try:
        for i, entry in enumerate(entries):
            # 输出进度（JSON 格式，包含当前字幕信息）
            progress = (i + 1) / total * 100
            progress_info = json.dumps({
                "progress": progress,
                "current": i + 1,
                "total": total,
                "text": entry['text'][:50]  # 截取前50个字符
            }, ensure_ascii=False)
            print(f"PROGRESS:{progress_info}", file=sys.stderr, flush=True)
            
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
                # FireRedAsr.transcribe(batch_uttid, batch_wav_path, args)
                res = model.transcribe([f"utt_{i}"], [chunk_file], {"language": language})
                corrected_text = res[0].get("text", "").strip() if res else ""
                
                # 如果启用了保留大小写，恢复原始英文大小写
                if preserve_case and corrected_text:
                    corrected_text = preserve_original_case(original_text, corrected_text)
            except Exception as e:
                print(f"识别片段 {i+1} 失败: {e}", file=sys.stderr)
                corrected_text = original_text
            
            # 清理临时文件
            os.remove(chunk_file)
            
            # 比较差异
            has_diff = original_text.strip() != corrected_text
            
            results.append({
                "id": entry['id'],
                "start_ms": start_ms,
                "end_ms": end_ms,
                "original": original_text,
                "corrected": corrected_text if corrected_text else original_text,
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
    use std::process::Stdio;
    use std::io::{BufRead, BufReader};
    
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
    
    let _ = window.emit("firered-progress", FireRedProgress {
        progress: 0.0,
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
    
    // 执行 Python 脚本（使用 piped stderr 来读取进度）
    let mut child = Command::new(&python_path)
        .args(&args)
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("执行校正脚本失败: {}", e))?;
    
    // 读取 stderr 中的进度信息
    if let Some(stderr) = child.stderr.take() {
        let reader = BufReader::new(stderr);
        for line in reader.lines() {
            if is_cancelled() {
                let _ = child.kill();
                let _ = std::fs::remove_file(&output_path);
                return Err("校正已取消".to_string());
            }
            
            if let Ok(line) = line {
                // 解析进度信息
                if line.starts_with("PROGRESS:") {
                    let json_str = &line[9..];
                    if let Ok(progress_info) = serde_json::from_str::<serde_json::Value>(json_str) {
                        let progress = progress_info["progress"].as_f64().unwrap_or(0.0) as f32;
                        let current = progress_info["current"].as_i64().unwrap_or(0);
                        let total = progress_info["total"].as_i64().unwrap_or(0);
                        let text = progress_info["text"].as_str().unwrap_or("");
                        
                        let _ = window.emit("firered-progress", FireRedProgress {
                            progress,
                            current_text: format!("正在校正 {}/{}: {}", current, total, text),
                            status: "correcting".to_string(),
                        });
                    }
                }
            }
        }
    }
    
    // 等待进程完成
    let status = child.wait().map_err(|e| format!("等待进程失败: {}", e))?;
    
    if is_cancelled() {
        let _ = std::fs::remove_file(&output_path);
        return Err("校正已取消".to_string());
    }
    
    if !status.success() {
        let _ = std::fs::remove_file(&output_path);
        return Err("校正脚本执行失败".to_string());
    }
    
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
    
    let _ = window.emit("firered-progress", FireRedProgress {
        progress: 100.0,
        current_text: format!("校正完成！发现 {} 处差异", diff_count),
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
"""FireRedASR 持久化服务 - 模型只加载一次"""

import sys
import json
import os
import tempfile
from http.server import HTTPServer, BaseHTTPRequestHandler
import urllib.parse

# 全局模型变量
MODEL = None

def load_model():
    global MODEL
    if MODEL is None:
        import torch
        import argparse
        torch.serialization.add_safe_globals([argparse.Namespace])
        from fireredasr.models.fireredasr import FireRedAsr
        print("Loading FireRedASR model...", file=sys.stderr)
        MODEL = FireRedAsr.from_pretrained("aed")
        print("Model loaded!", file=sys.stderr)
    return MODEL

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
            from pydub import AudioSegment
            
            audio_path = params['audio_path']
            start_ms = params['start_ms']
            end_ms = params['end_ms']
            original_text = params['original_text']
            language = params.get('language', 'zh')
            preserve_case = params.get('preserve_case', True)
            
            # 切分音频
            audio = AudioSegment.from_file(audio_path)
            chunk = audio[start_ms:end_ms]
            chunk = chunk.set_channels(1)
            chunk = chunk.set_frame_rate(16000)
            
            tmp_file = tempfile.NamedTemporaryFile(suffix='.wav', delete=False)
            chunk.export(tmp_file.name, format='wav')
            tmp_file.close()
            
            # 识别
            model = load_model()
            result = model.transcribe(["utt"], [tmp_file.name], {"language": language})
            corrected = result[0].get("text", "").strip() if result else ""
            
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

/// 检查服务是否运行
pub fn is_service_running() -> bool {
    std::process::Command::new("curl")
        .args(["-s", "-o", "/dev/null", "-w", "%{http_code}", "http://127.0.0.1:18765/health"])
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

/// 启动服务
fn start_service() -> Result<(), String> {
    // 总是先写入最新的脚本
    let script_path = write_service_script()?;
    
    // 如果服务已经在运行，先停止它以使用新脚本
    if is_service_running() {
        stop_service();
    }
    
    let python_path = get_python_path()?;
    
    // 后台启动服务
    std::process::Command::new(&python_path)
        .arg(&script_path)
        .arg("18765")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("启动服务失败: {}", e))?;
    
    // 等待服务启动
    for _ in 0..30 {
        std::thread::sleep(std::time::Duration::from_millis(500));
        if is_service_running() {
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
    
    // 调用 /preload 端点预加载模型
    let output = Command::new("curl")
        .args(["-s", "http://127.0.0.1:18765/preload"])
        .output()
        .map_err(|e| format!("预加载请求失败: {}", e))?;
    
    if output.status.success() {
        Ok("FireRedASR 服务已启动并预加载模型".to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("预加载失败: {}", stderr))
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
    
    // 发送请求到服务
    let output = Command::new("curl")
        .args([
            "-s",
            "-X", "POST",
            "-H", "Content-Type: application/json",
            "-d", &request_body.to_string(),
            "http://127.0.0.1:18765/"
        ])
        .output()
        .map_err(|e| format!("请求服务失败: {}", e))?;
    
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("校正失败: {}", stderr));
    }
    
    // 从 stdout 读取结果
    let result_json = String::from_utf8_lossy(&output.stdout);
    
    // 检查是否有错误
    if result_json.contains("\"error\"") {
        return Err(format!("校正服务返回错误: {}", result_json));
    }
    
    // 解析 JSON
    let result: SingleCorrectionResult = serde_json::from_str(&result_json)
        .map_err(|e| format!("解析校正结果失败: {} - 原始响应: {}", e, result_json))?;
    
    Ok(result)
}

/// 卸载 FireRedASR 环境
pub fn uninstall_firered_env() -> Result<String, String> {
    let env_dir = get_firered_env_dir()?;
    
    if env_dir.exists() {
        std::fs::remove_dir_all(&env_dir)
            .map_err(|e| format!("删除环境目录失败: {}", e))?;
    }
    
    Ok("FireRedASR 环境已卸载".to_string())
}

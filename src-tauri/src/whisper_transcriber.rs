use crate::srt_parser::{SubtitleEntry, TimeStamp};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::fs;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use tauri::{Window, Emitter};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};
use symphonia::core::audio::{AudioBufferRef, Signal};
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use once_cell::sync::Lazy;

// 全局取消标志（转录）
static TRANSCRIPTION_CANCELLED: Lazy<Arc<AtomicBool>> = Lazy::new(|| Arc::new(AtomicBool::new(false)));

// 下载任务ID，用于取消旧的下载任务
static DOWNLOAD_TASK_ID: Lazy<AtomicU64> = Lazy::new(|| AtomicU64::new(0));

/// 取消当前转录任务
pub fn cancel_transcription() {
    TRANSCRIPTION_CANCELLED.store(true, Ordering::SeqCst);
}

/// 重置取消标志
fn reset_cancellation() {
    TRANSCRIPTION_CANCELLED.store(false, Ordering::SeqCst);
}

/// 检查是否已取消
fn is_cancelled() -> bool {
    TRANSCRIPTION_CANCELLED.load(Ordering::SeqCst)
}

/// 生成新的下载任务ID，同时使旧任务失效
fn new_download_task_id() -> u64 {
    DOWNLOAD_TASK_ID.fetch_add(1, Ordering::SeqCst) + 1
}

/// 检查下载任务是否仍然有效
fn is_download_task_valid(task_id: u64) -> bool {
    DOWNLOAD_TASK_ID.load(Ordering::SeqCst) == task_id
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionProgress {
    pub progress: f32,
    pub current_text: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhisperModelInfo {
    pub name: String,
    pub size: String,
    pub downloaded: bool,
    pub path: Option<String>,
}

/// Get the model directory path
pub fn get_model_dir() -> Result<PathBuf, String> {
    let home_dir = dirs::home_dir()
        .ok_or_else(|| "Failed to get home directory".to_string())?;

    let model_dir = home_dir.join(".config").join("srt-editor").join("models");

    // Create directory if it doesn't exist
    if !model_dir.exists() {
        fs::create_dir_all(&model_dir)
            .map_err(|e| format!("Failed to create model directory: {}", e))?;
    }

    Ok(model_dir)
}

/// Get model file name for a specific model size
fn get_model_filename(model_size: &str) -> String {
    match model_size {
        "large" => "ggml-large-v3.bin".to_string(),
        "turbo" => "ggml-large-v3-turbo.bin".to_string(),
        _ => format!("ggml-{}.bin", model_size),
    }
}

/// Get model file path for a specific model size
pub fn get_model_path(model_size: &str) -> Result<PathBuf, String> {
    let model_dir = get_model_dir()?;
    let model_file = model_dir.join(get_model_filename(model_size));
    Ok(model_file)
}

/// Check if a model is downloaded
pub fn is_model_downloaded(model_size: &str) -> Result<bool, String> {
    let model_path = get_model_path(model_size)?;
    Ok(model_path.exists())
}

/// Get list of available Whisper models
pub fn get_available_models() -> Result<Vec<WhisperModelInfo>, String> {
    let models = vec![
        ("tiny", "75 MB"),
        ("base", "142 MB"),
        ("small", "466 MB"),
        ("medium", "1.5 GB"),
        ("large", "2.9 GB"),
        ("turbo", "1.5 GB"),
    ];

    let mut model_list = Vec::new();

    for (name, size) in models {
        let downloaded = is_model_downloaded(name).unwrap_or(false);
        let path = if downloaded {
            get_model_path(name).ok().map(|p| p.to_string_lossy().to_string())
        } else {
            None
        };

        model_list.push(WhisperModelInfo {
            name: name.to_string(),
            size: size.to_string(),
            downloaded,
            path,
        });
    }

    Ok(model_list)
}

/// Delete a downloaded Whisper model
pub fn delete_model(model_size: &str) -> Result<String, String> {
    let model_path = get_model_path(model_size)?;
    
    if !model_path.exists() {
        return Err(format!("Model {} is not downloaded", model_size));
    }
    
    fs::remove_file(&model_path)
        .map_err(|e| format!("Failed to delete model: {}", e))?;
    
    Ok(format!("Successfully deleted {} model", model_size))
}

/// Get partial download file path
fn get_part_file_path(model_size: &str) -> Result<PathBuf, String> {
    let model_path = get_model_path(model_size)?;
    Ok(model_path.with_extension("bin.part"))
}

/// Check if there's a partial download for a model
pub fn get_partial_download_size(model_size: &str) -> Result<u64, String> {
    let part_path = get_part_file_path(model_size)?;
    if part_path.exists() {
        fs::metadata(&part_path)
            .map(|m| m.len())
            .map_err(|e| format!("Failed to get partial file size: {}", e))
    } else {
        Ok(0)
    }
}

/// Download Whisper model from Hugging Face with resume support
pub async fn download_model(model_size: &str, window: Window) -> Result<String, String> {
    use std::fs::OpenOptions;
    use std::io::Write;

    // 生成新的任务ID，使之前的下载任务失效
    let task_id = new_download_task_id();

    let model_path = get_model_path(model_size)?;
    let part_path = get_part_file_path(model_size)?;

    log::info!(
        "Starting download for Whisper {} model, task_id: {}",
        model_size,
        task_id
    );

    // Check if already downloaded
    if model_path.exists() {
        log::info!("Whisper {} model already exists, skipping download", model_size);
        return Ok(format!("Model {} already downloaded", model_size));
    }

    // Check for existing partial download
    let existing_size = if part_path.exists() {
        let size = fs::metadata(&part_path).map(|m| m.len()).unwrap_or(0);
        log::info!(
            "Found partial download for Whisper {}: {:.1} MB",
            model_size,
            size as f64 / 1024.0 / 1024.0
        );
        size
    } else {
        0
    };

    // Download URL from ggerganov/whisper.cpp models
    let filename = get_model_filename(model_size);
    let download_url = format!(
        "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/{}",
        filename
    );

    let client = reqwest::Client::new();

    // Emit initial progress (use model-download-progress event to avoid conflict with transcription)
    let _ = window.emit(
        "model-download-progress",
        TranscriptionProgress {
            progress: 0.0,
            current_text: if existing_size > 0 {
                format!("Resuming {} model download...", model_size)
            } else {
                format!("Starting {} model download...", model_size)
            },
            status: "downloading".to_string(),
        },
    );

    // Build request with Range header if we have partial download
    let mut request = client.get(&download_url);
    if existing_size > 0 {
        request = request.header("Range", format!("bytes={}-", existing_size));
    }

    let response = request
        .send()
        .await
        .map_err(|e| format!("Failed to download model: {}", e))?;

    // Check response status
    let status = response.status();
    let is_partial = status == reqwest::StatusCode::PARTIAL_CONTENT;

    if !status.is_success() && !is_partial {
        log::error!("Download failed: HTTP {}", status);
        return Err(format!("Failed to download model: HTTP {}", status));
    }

    // Determine total size and start position from response headers
    let (total_size, actual_start) = if is_partial {
        // Parse Content-Range header: "bytes 12345-67890/123456"
        let content_range = response
            .headers()
            .get("content-range")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");

        let total = content_range
            .split('/')
            .last()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);

        log::info!(
            "Resuming Whisper {} download: {:.1} MB / {:.1} MB ({:.1}%)",
            model_size,
            existing_size as f64 / 1024.0 / 1024.0,
            total as f64 / 1024.0 / 1024.0,
            (existing_size as f64 / total as f64) * 100.0
        );

        (total, existing_size)
    } else {
        // Full download - get size from Content-Length
        let total = response.content_length().unwrap_or(0);
        log::info!(
            "Starting fresh Whisper {} download: {:.1} MB",
            model_size,
            total as f64 / 1024.0 / 1024.0
        );
        // Server doesn't support range, remove partial file
        if existing_size > 0 {
            log::warn!("Server doesn't support resume, restarting download");
            let _ = fs::remove_file(&part_path);
        }
        (total, 0u64)
    };

    // Open file for writing
    let (mut file, mut downloaded) = if actual_start > 0 {
        let file = OpenOptions::new()
            .append(true)
            .open(&part_path)
            .map_err(|e| format!("Failed to open partial file: {}", e))?;
        (file, actual_start)
    } else {
        let file =
            fs::File::create(&part_path).map_err(|e| format!("Failed to create file: {}", e))?;
        (file, 0u64)
    };

    // Emit progress with correct initial state
    let initial_progress = if total_size > 0 {
        (downloaded as f32 / total_size as f32) * 100.0
    } else {
        0.0
    };

    let _ = window.emit(
        "model-download-progress",
        TranscriptionProgress {
            progress: initial_progress,
            current_text: if actual_start > 0 {
                format!(
                    "Resuming {} model download... {:.1}%",
                    model_size, initial_progress
                )
            } else {
                format!("Downloading {} model...", model_size)
            },
            status: "downloading".to_string(),
        },
    );

    // Stream download directly to file
    let mut response = response;
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|e| format!("Failed to read chunk: {}", e))?
    {
        // 检查任务是否仍然有效（是否有新的下载任务启动）
        if !is_download_task_valid(task_id) {
            log::info!(
                "Download task {} cancelled (new task started), downloaded {} bytes",
                task_id,
                downloaded
            );
            return Err("Download cancelled: new download started".to_string());
        }

        file.write_all(&chunk)
            .map_err(|e| format!("Failed to write chunk: {}", e))?;

        downloaded += chunk.len() as u64;

        if total_size > 0 {
            let progress = (downloaded as f32 / total_size as f32) * 100.0;
            let _ = window.emit(
                "model-download-progress",
                TranscriptionProgress {
                    progress,
                    current_text: format!("Downloading {} model... {:.1}%", model_size, progress),
                    status: "downloading".to_string(),
                },
            );
        }
    }

    // Ensure all data is written
    file.flush().map_err(|e| format!("Failed to flush file: {}", e))?;
    drop(file);

    // Verify download completed
    let final_size = fs::metadata(&part_path).map(|m| m.len()).unwrap_or(0);

    if total_size > 0 && final_size != total_size {
        log::error!(
            "Download incomplete: expected {} bytes, got {} bytes",
            total_size,
            final_size
        );
        return Err(format!(
            "Download incomplete: expected {} bytes, got {} bytes. Please retry to resume.",
            total_size, final_size
        ));
    }

    // Rename partial file to final file
    fs::rename(&part_path, &model_path)
        .map_err(|e| format!("Failed to finalize model file: {}", e))?;

    log::info!(
        "Whisper {} model downloaded successfully ({:.1} MB)",
        model_size,
        final_size as f64 / 1024.0 / 1024.0
    );

    // Emit completion
    let _ = window.emit(
        "model-download-progress",
        TranscriptionProgress {
            progress: 100.0,
            current_text: format!("Model {} downloaded successfully", model_size),
            status: "completed".to_string(),
        },
    );

    Ok(format!("Successfully downloaded {} model", model_size))
}

/// Read audio file and convert to 16kHz mono using Symphonia
fn read_audio_file_symphonia(file_path: &str) -> Result<Vec<f32>, String> {
    let file = std::fs::File::open(file_path)
        .map_err(|e| format!("Failed to open audio file: {}", e))?;

    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(extension) = std::path::Path::new(file_path).extension() {
        hint.with_extension(&extension.to_string_lossy());
    }

    let format_opts = FormatOptions::default();
    let metadata_opts = MetadataOptions::default();
    let decoder_opts = DecoderOptions::default();

    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &format_opts, &metadata_opts)
        .map_err(|e| format!("Failed to probe audio file: {}", e))?;

    let mut format = probed.format;
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != symphonia::core::codecs::CODEC_TYPE_NULL)
        .ok_or_else(|| "No audio track found".to_string())?;

    let track_id = track.id;
    let sample_rate = track.codec_params.sample_rate.ok_or("No sample rate found")?;

    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &decoder_opts)
        .map_err(|e| format!("Failed to create decoder: {}", e))?;

    let mut samples = Vec::new();

    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(symphonia::core::errors::Error::IoError(e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(e) => return Err(format!("Failed to read packet: {}", e)),
        };

        if packet.track_id() != track_id {
            continue;
        }

        let decoded = decoder
            .decode(&packet)
            .map_err(|e| format!("Failed to decode packet: {}", e))?;

        // Convert to f32 samples and mono
        match decoded {
            AudioBufferRef::F32(buf) => {
                if buf.spec().channels.count() == 1 {
                    samples.extend_from_slice(buf.chan(0));
                } else {
                    let num_channels = buf.spec().channels.count();
                    let frame_count = buf.frames();
                    for frame_idx in 0..frame_count {
                        let mut sum = 0.0;
                        for ch in 0..num_channels {
                            sum += buf.chan(ch)[frame_idx];
                        }
                        samples.push(sum / num_channels as f32);
                    }
                }
            }
            AudioBufferRef::S16(buf) => {
                if buf.spec().channels.count() == 1 {
                    for &sample in buf.chan(0) {
                        samples.push(sample as f32 / 32768.0);
                    }
                } else {
                    let num_channels = buf.spec().channels.count();
                    let frame_count = buf.frames();
                    for frame_idx in 0..frame_count {
                        let mut sum = 0.0;
                        for ch in 0..num_channels {
                            sum += buf.chan(ch)[frame_idx] as f32 / 32768.0;
                        }
                        samples.push(sum / num_channels as f32);
                    }
                }
            }
            AudioBufferRef::S32(buf) => {
                if buf.spec().channels.count() == 1 {
                    for &sample in buf.chan(0) {
                        samples.push(sample as f32 / 2147483648.0);
                    }
                } else {
                    let num_channels = buf.spec().channels.count();
                    let frame_count = buf.frames();
                    for frame_idx in 0..frame_count {
                        let mut sum = 0.0;
                        for ch in 0..num_channels {
                            sum += buf.chan(ch)[frame_idx] as f32 / 2147483648.0;
                        }
                        samples.push(sum / num_channels as f32);
                    }
                }
            }
            _ => return Err("Unsupported audio format".to_string()),
        }
    }

    // Resample to 16kHz if needed
    if sample_rate != 16000 {
        samples = resample_audio(&samples, sample_rate, 16000);
    }

    Ok(samples)
}

/// Simple linear resampling
fn resample_audio(samples: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    if from_rate == to_rate {
        return samples.to_vec();
    }

    let ratio = from_rate as f64 / to_rate as f64;
    let output_len = (samples.len() as f64 / ratio) as usize;
    let mut output = Vec::with_capacity(output_len);

    for i in 0..output_len {
        let src_idx = (i as f64 * ratio) as usize;
        if src_idx < samples.len() {
            output.push(samples[src_idx]);
        }
    }

    output
}

/// Transcribe audio file to subtitle entries using Whisper with timestamps
pub async fn transcribe_audio(
    audio_path: String,
    model_size: String,
    language: String,
    window: Window,
) -> Result<Vec<SubtitleEntry>, String> {
    // 重置取消标志
    reset_cancellation();
    
    // Check if model is downloaded
    if !is_model_downloaded(&model_size)? {
        return Err(format!(
            "Model {} is not downloaded. Please download it first.",
            model_size
        ));
    }

    let model_path = get_model_path(&model_size)?;

    // Emit progress
    let _ = window.emit("transcription-progress", TranscriptionProgress {
        progress: 0.0,
        current_text: "Loading audio file...".to_string(),
        status: "loading".to_string(),
    });

    // 检查是否取消
    if is_cancelled() {
        return Err("转录已取消".to_string());
    }

    // Load audio file
    let samples = read_audio_file_symphonia(&audio_path)?;

    // 检查是否取消
    if is_cancelled() {
        return Err("转录已取消".to_string());
    }

    // Emit progress
    let _ = window.emit("transcription-progress", TranscriptionProgress {
        progress: 10.0,
        current_text: "Loading Whisper model...".to_string(),
        status: "loading".to_string(),
    });

    // Load Whisper model
    let ctx = WhisperContext::new_with_params(
        model_path.to_str().unwrap(),
        WhisperContextParameters::default(),
    )
    .map_err(|e| format!("Failed to load Whisper model: {}", e))?;

    // 检查是否取消
    if is_cancelled() {
        return Err("转录已取消".to_string());
    }

    // Create transcription parameters
    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);
    params.set_translate(false);
    params.set_language(Some(&language));
    params.set_n_threads(4);
    params.set_single_segment(false);

    // Emit progress
    let _ = window.emit("transcription-progress", TranscriptionProgress {
        progress: 20.0,
        current_text: "Transcribing audio...".to_string(),
        status: "transcribing".to_string(),
    });

    // Create state and transcribe
    let mut state = ctx.create_state().map_err(|e| format!("Failed to create state: {}", e))?;
    state
        .full(params, &samples)
        .map_err(|e| format!("Failed to transcribe: {}", e))?;

    // 检查是否取消（转录完成后）
    if is_cancelled() {
        return Err("转录已取消".to_string());
    }

    // Emit progress
    let _ = window.emit("transcription-progress", TranscriptionProgress {
        progress: 80.0,
        current_text: "Converting to subtitles...".to_string(),
        status: "converting".to_string(),
    });

    // Extract segments with timestamps
    let num_segments = state.full_n_segments();

    let mut entries = Vec::new();

    for i in 0..num_segments {
        // 检查是否取消
        if is_cancelled() {
            return Err("转录已取消".to_string());
        }
        
        let segment = state
            .get_segment(i)
            .ok_or_else(|| format!("Failed to get segment {}", i))?;

        let segment_text = segment
            .to_str()
            .map_err(|e| format!("Failed to get segment text: {}", e))?;

        let start_timestamp = segment.start_timestamp();
        let end_timestamp = segment.end_timestamp();

        // Convert from centiseconds (10ms) to milliseconds
        let start_ms = (start_timestamp * 10) as u32;
        let end_ms = (end_timestamp * 10) as u32;

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
            text: segment_text.trim().to_string(),
        });
    }

    // 最后检查是否取消
    if is_cancelled() {
        return Err("转录已取消".to_string());
    }

    // Emit completion
    let _ = window.emit("transcription-progress", TranscriptionProgress {
        progress: 100.0,
        current_text: format!("Transcription completed! Generated {} subtitles", entries.len()),
        status: "completed".to_string(),
    });

    Ok(entries)
}

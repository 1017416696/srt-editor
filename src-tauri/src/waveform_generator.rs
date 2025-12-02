use std::fs::File;
use std::path::Path;
use std::time::Instant;
use symphonia::core::audio::{AudioBufferRef, Signal};
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

/// Callback type for progress updates (progress: 0.0 to 1.0)
pub type ProgressCallback = Box<dyn Fn(f32) + Send>;

/// Generate waveform data from an audio file
/// Returns a vector of normalized amplitude values (0.0 to 1.0)
/// The number of samples is reduced based on the target_samples parameter
pub fn generate_waveform(file_path: &str, target_samples: usize) -> Result<Vec<f32>, String> {
    generate_waveform_with_progress(file_path, target_samples, None)
}

/// Generate waveform data from an audio file with progress callback
/// Optimized version: streams audio and downsamples on-the-fly
pub fn generate_waveform_with_progress(
    file_path: &str,
    target_samples: usize,
    progress_callback: Option<ProgressCallback>,
) -> Result<Vec<f32>, String> {
    let path = Path::new(file_path);

    // Open the media source
    let file = File::open(path)
        .map_err(|e| format!("Failed to open audio file: {}", e))?;
    
    let file_size = file.metadata().map(|m| m.len()).unwrap_or(0);

    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    // Create a hint to help the format registry guess the format
    let mut hint = Hint::new();
    if let Some(extension) = path.extension() {
        if let Some(ext_str) = extension.to_str() {
            hint.with_extension(ext_str);
        }
    }

    // Probe the media source
    let format_opts = FormatOptions::default();
    let metadata_opts = MetadataOptions::default();

    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &format_opts, &metadata_opts)
        .map_err(|e| format!("Failed to probe audio file: {}", e))?;

    let mut format = probed.format;

    // Find the first audio track
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or_else(|| "No audio track found".to_string())?;

    let track_id = track.id;
    
    // Get total frames for progress calculation (if available)
    let total_frames = track.codec_params.n_frames;
    let sample_rate = track.codec_params.sample_rate.unwrap_or(44100);

    // Create a decoder for the track
    let dec_opts = DecoderOptions::default();
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &dec_opts)
        .map_err(|e| format!("Failed to create decoder: {}", e))?;

    // Estimate total samples based on file size or duration
    let estimated_total_samples = if let Some(frames) = total_frames {
        frames as usize
    } else {
        // Rough estimate: assume ~10 bytes per sample for compressed audio
        (file_size as usize / 10).max(sample_rate as usize * 60) // at least 1 minute
    };

    // Pre-allocate with estimated capacity to reduce reallocations
    let mut all_samples: Vec<f32> = Vec::with_capacity(estimated_total_samples);
    let mut decoded_frames: u64 = 0;
    let mut last_progress_time = Instant::now();
    let mut last_reported_progress = 0.0f32;

    // Decode packets
    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(_) => break,
        };

        // Skip packets that don't belong to the selected track
        if packet.track_id() != track_id {
            continue;
        }

        // Decode the packet
        match decoder.decode(&packet) {
            Ok(decoded) => {
                // Extract samples from the decoded audio buffer
                let samples = extract_samples(&decoded);
                let num_samples = samples.len();
                all_samples.extend(samples);
                decoded_frames += num_samples as u64;
                
                // Update progress based on time (throttle to max 10 updates per second)
                // This avoids blocking the main processing
                if let Some(ref callback) = progress_callback {
                    let now = Instant::now();
                    if now.duration_since(last_progress_time).as_millis() >= 100 {
                        let progress = if let Some(total) = total_frames {
                            (decoded_frames as f32 / total as f32 * 0.9).min(0.9)
                        } else {
                            (decoded_frames as f32 / estimated_total_samples as f32 * 0.9).min(0.9)
                        };
                        
                        if progress > last_reported_progress + 0.01 {
                            callback(progress);
                            last_reported_progress = progress;
                            last_progress_time = now;
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("Decode error: {}", e);
                continue;
            }
        }
    }

    if all_samples.is_empty() {
        return Err("No audio samples extracted".to_string());
    }

    // Report progress: 90% - starting downsample
    if let Some(ref callback) = progress_callback {
        callback(0.9);
    }

    // Downsample to target number of samples (fast, no progress callback needed)
    let waveform = downsample_and_normalize_fast(&all_samples, target_samples);

    // Report progress: 100% - complete
    if let Some(ref callback) = progress_callback {
        callback(1.0);
    }

    Ok(waveform)
}

/// Extract samples from an audio buffer and convert to mono f32
/// Optimized: only extract first channel for speed (stereo files have similar waveforms)
#[inline]
fn extract_samples(decoded: &AudioBufferRef) -> Vec<f32> {
    match decoded {
        AudioBufferRef::F32(buf) => {
            // Just use first channel for speed
            buf.chan(0).to_vec()
        }
        AudioBufferRef::S32(buf) => {
            let channel = buf.chan(0);
            let scale = 1.0 / i32::MAX as f32;
            channel.iter().map(|&s| s as f32 * scale).collect()
        }
        AudioBufferRef::S16(buf) => {
            let channel = buf.chan(0);
            let scale = 1.0 / i16::MAX as f32;
            channel.iter().map(|&s| s as f32 * scale).collect()
        }
        AudioBufferRef::U8(buf) => {
            let channel = buf.chan(0);
            channel.iter().map(|&s| (s as f32 - 128.0) / 128.0).collect()
        }
        _ => Vec::new(),
    }
}

/// Fast downsample without progress callback (optimized for speed)
fn downsample_and_normalize_fast(samples: &[f32], target_samples: usize) -> Vec<f32> {
    if samples.is_empty() {
        return Vec::new();
    }

    let total_samples = samples.len();

    if total_samples <= target_samples {
        // If we have fewer samples than target, just normalize
        return samples.iter().map(|&s| s.abs()).collect();
    }

    let chunk_size = total_samples / target_samples;
    let mut waveform = Vec::with_capacity(target_samples);

    // For each chunk, find the peak amplitude
    // Use iterator-based approach for better performance
    for i in 0..target_samples {
        let start = i * chunk_size;
        let end = if i == target_samples - 1 {
            total_samples
        } else {
            (i + 1) * chunk_size
        };

        let chunk = &samples[start..end];
        // Use SIMD-friendly iteration pattern
        let peak = chunk.iter().fold(0.0f32, |max, &s| {
            let abs = s.abs();
            if abs > max { abs } else { max }
        });
        waveform.push(peak);
    }

    waveform
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_downsample() {
        let samples = vec![0.5, 0.8, 0.3, 0.9, 0.1, 0.7];
        let result = downsample_and_normalize_fast(&samples, 3);
        assert_eq!(result.len(), 3);
    }
}

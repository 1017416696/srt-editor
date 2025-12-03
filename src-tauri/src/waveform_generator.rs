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

/// Waveform data with min/max pairs for professional-grade visualization
/// Each point contains [min, max] representing the amplitude range in that time slice
#[derive(Debug, Clone, serde::Serialize)]
pub struct WaveformData {
    /// Min/Max pairs: [min0, max0, min1, max1, ...] - interleaved for efficient transfer
    pub peaks: Vec<f32>,
    /// Number of points (peaks.len() / 2)
    pub length: usize,
    /// Sample rate of the original audio
    pub sample_rate: u32,
    /// Duration in seconds
    pub duration: f64,
}

/// Generate waveform data from an audio file (legacy API for compatibility)
/// Returns a vector of normalized amplitude values (0.0 to 1.0)
#[allow(dead_code)]
pub fn generate_waveform(file_path: &str, target_samples: usize) -> Result<Vec<f32>, String> {
    let data = generate_waveform_minmax_with_progress(file_path, target_samples, None)?;
    // Convert min/max to single values (use max for compatibility)
    Ok(data.peaks.chunks(2).map(|pair| pair[1]).collect())
}

/// Generate professional min/max waveform data with progress callback
pub fn generate_waveform_with_progress(
    file_path: &str,
    target_samples: usize,
    progress_callback: Option<ProgressCallback>,
) -> Result<Vec<f32>, String> {
    let data = generate_waveform_minmax_with_progress(file_path, target_samples, progress_callback)?;
    // Return interleaved min/max data
    Ok(data.peaks)
}

/// Generate min/max waveform data - the core implementation
pub fn generate_waveform_minmax_with_progress(
    file_path: &str,
    target_samples: usize,
    progress_callback: Option<ProgressCallback>,
) -> Result<WaveformData, String> {
    let path = Path::new(file_path);

    // Open the media source
    let file =
        File::open(path).map_err(|e| format!("Failed to open audio file: {}", e))?;

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
        (file_size as usize / 10).max(sample_rate as usize * 60)
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
                let samples = extract_samples(&decoded);
                let num_samples = samples.len();
                all_samples.extend(samples);
                decoded_frames += num_samples as u64;

                // Update progress (throttle to max 10 updates per second)
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

    // Calculate duration
    let duration = all_samples.len() as f64 / sample_rate as f64;

    // Generate min/max peaks
    let peaks = generate_minmax_peaks(&all_samples, target_samples);

    // Report progress: 100% - complete
    if let Some(ref callback) = progress_callback {
        callback(1.0);
    }

    Ok(WaveformData {
        peaks,
        length: target_samples,
        sample_rate,
        duration,
    })
}

/// Extract samples from an audio buffer and convert to mono f32
/// For stereo, mix both channels for more accurate representation
#[inline]
fn extract_samples(decoded: &AudioBufferRef) -> Vec<f32> {
    match decoded {
        AudioBufferRef::F32(buf) => {
            let channels = buf.spec().channels.count();
            if channels >= 2 {
                // Mix stereo to mono
                let left = buf.chan(0);
                let right = buf.chan(1);
                left.iter()
                    .zip(right.iter())
                    .map(|(&l, &r)| (l + r) * 0.5)
                    .collect()
            } else {
                buf.chan(0).to_vec()
            }
        }
        AudioBufferRef::S32(buf) => {
            let scale = 1.0 / i32::MAX as f32;
            let channels = buf.spec().channels.count();
            if channels >= 2 {
                let left = buf.chan(0);
                let right = buf.chan(1);
                left.iter()
                    .zip(right.iter())
                    .map(|(&l, &r)| ((l as f32 + r as f32) * 0.5) * scale)
                    .collect()
            } else {
                buf.chan(0).iter().map(|&s| s as f32 * scale).collect()
            }
        }
        AudioBufferRef::S16(buf) => {
            let scale = 1.0 / i16::MAX as f32;
            let channels = buf.spec().channels.count();
            if channels >= 2 {
                let left = buf.chan(0);
                let right = buf.chan(1);
                left.iter()
                    .zip(right.iter())
                    .map(|(&l, &r)| ((l as f32 + r as f32) * 0.5) * scale)
                    .collect()
            } else {
                buf.chan(0).iter().map(|&s| s as f32 * scale).collect()
            }
        }
        AudioBufferRef::U8(buf) => {
            let channels = buf.spec().channels.count();
            if channels >= 2 {
                let left = buf.chan(0);
                let right = buf.chan(1);
                left.iter()
                    .zip(right.iter())
                    .map(|(&l, &r)| ((l as f32 - 128.0) + (r as f32 - 128.0)) / 256.0)
                    .collect()
            } else {
                buf.chan(0)
                    .iter()
                    .map(|&s| (s as f32 - 128.0) / 128.0)
                    .collect()
            }
        }
        _ => Vec::new(),
    }
}

/// Generate min/max peaks for professional waveform display
/// Returns interleaved [min0, max0, min1, max1, ...] array
/// The output is normalized to use the full [-1, 1] range for better visualization
fn generate_minmax_peaks(samples: &[f32], target_samples: usize) -> Vec<f32> {
    if samples.is_empty() {
        return Vec::new();
    }

    let total_samples = samples.len();

    // If we have fewer samples than target, each sample becomes its own min/max
    if total_samples <= target_samples {
        let peaks: Vec<f32> = samples.iter().flat_map(|&s| [s, s]).collect();
        return normalize_peaks(peaks);
    }

    let chunk_size = total_samples as f64 / target_samples as f64;
    let mut peaks = Vec::with_capacity(target_samples * 2);

    for i in 0..target_samples {
        let start = (i as f64 * chunk_size) as usize;
        let end = ((i + 1) as f64 * chunk_size) as usize;
        let end = end.min(total_samples);

        if start >= end {
            peaks.push(0.0);
            peaks.push(0.0);
            continue;
        }

        let chunk = &samples[start..end];

        // Find true min and max (preserving sign for proper waveform shape)
        let mut min_val = f32::MAX;
        let mut max_val = f32::MIN;

        for &sample in chunk {
            if sample < min_val {
                min_val = sample;
            }
            if sample > max_val {
                max_val = sample;
            }
        }

        // Clamp to valid range
        min_val = min_val.clamp(-1.0, 1.0);
        max_val = max_val.clamp(-1.0, 1.0);

        peaks.push(min_val);
        peaks.push(max_val);
    }

    // Normalize to use full range for better visualization
    normalize_peaks(peaks)
}

/// Normalize peaks to use the full [-1, 1] range
fn normalize_peaks(mut peaks: Vec<f32>) -> Vec<f32> {
    if peaks.is_empty() {
        return peaks;
    }

    // Find the maximum absolute value
    let mut max_abs: f32 = 0.0;
    for &val in &peaks {
        let abs = val.abs();
        if abs > max_abs {
            max_abs = abs;
        }
    }

    // If max is too small, don't normalize (avoid division by near-zero)
    if max_abs < 0.001 {
        return peaks;
    }

    // Normalize all values
    let scale = 1.0 / max_abs;
    for val in &mut peaks {
        *val *= scale;
    }

    peaks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_minmax_peaks() {
        let samples = vec![0.5, -0.3, 0.8, -0.9, 0.1, -0.7];
        let result = generate_minmax_peaks(&samples, 3);
        // Should have 6 values (3 min/max pairs)
        assert_eq!(result.len(), 6);
        // First pair from [0.5, -0.3]: min=-0.3, max=0.5
        assert_eq!(result[0], -0.3);
        assert_eq!(result[1], 0.5);
    }

    #[test]
    fn test_minmax_peaks_single_chunk() {
        let samples = vec![0.2, -0.5, 0.8, -0.1];
        let result = generate_minmax_peaks(&samples, 1);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], -0.5); // min
        assert_eq!(result[1], 0.8); // max
    }
}

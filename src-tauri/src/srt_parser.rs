use serde::{Deserialize, Serialize};
use std::fs;
use std::fs::OpenOptions;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeStamp {
    pub hours: u32,
    pub minutes: u32,
    pub seconds: u32,
    pub milliseconds: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubtitleEntry {
    pub id: u32,
    #[serde(rename = "startTime")]
    pub start_time: TimeStamp,
    #[serde(rename = "endTime")]
    pub end_time: TimeStamp,
    pub text: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SRTFile {
    pub name: String,
    pub path: String,
    pub entries: Vec<SubtitleEntry>,
    pub encoding: Option<String>,
}

impl TimeStamp {
    /// Parse timestamp from SRT format: HH:MM:SS,mmm
    pub fn parse(s: &str) -> Result<Self, String> {
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() != 3 {
            return Err(format!("Invalid timestamp format: {}", s));
        }

        let hours = parts[0].parse::<u32>()
            .map_err(|e| format!("Invalid hours: {}", e))?;

        let minutes = parts[1].parse::<u32>()
            .map_err(|e| format!("Invalid minutes: {}", e))?;

        let sec_parts: Vec<&str> = parts[2].split(',').collect();
        if sec_parts.len() != 2 {
            return Err(format!("Invalid seconds format: {}", parts[2]));
        }

        let seconds = sec_parts[0].parse::<u32>()
            .map_err(|e| format!("Invalid seconds: {}", e))?;

        let milliseconds = sec_parts[1].parse::<u32>()
            .map_err(|e| format!("Invalid milliseconds: {}", e))?;

        Ok(TimeStamp {
            hours,
            minutes,
            seconds,
            milliseconds,
        })
    }

    /// Convert timestamp to string in SRT format
    pub fn to_string(&self) -> String {
        format!(
            "{:02}:{:02}:{:02},{:03}",
            self.hours, self.minutes, self.seconds, self.milliseconds
        )
    }
}

/// Parse SRT file content
pub fn parse_srt(content: &str) -> Result<Vec<SubtitleEntry>, String> {
    let mut entries = Vec::new();
    let blocks: Vec<&str> = content.split("\n\n").collect();

    for block in blocks {
        let block = block.trim();
        if block.is_empty() {
            continue;
        }

        let lines: Vec<&str> = block.lines().collect();
        if lines.len() < 3 {
            continue; // Skip invalid blocks
        }

        // Parse ID
        let id = lines[0].trim().parse::<u32>()
            .map_err(|e| format!("Invalid subtitle ID: {}", e))?;

        // Parse timestamps
        let timestamp_line = lines[1].trim();
        let times: Vec<&str> = timestamp_line.split(" --> ").collect();
        if times.len() != 2 {
            return Err(format!("Invalid timestamp line: {}", timestamp_line));
        }

        let start_time = TimeStamp::parse(times[0].trim())?;
        let end_time = TimeStamp::parse(times[1].trim())?;

        // Parse text (all remaining lines)
        let text = lines[2..].join("\n");

        entries.push(SubtitleEntry {
            id,
            start_time,
            end_time,
            text,
        });
    }

    Ok(entries)
}

/// Read and parse SRT file
pub fn read_srt_file(file_path: &str) -> Result<SRTFile, String> {
    let path = Path::new(file_path);

    if !path.exists() {
        return Err(format!("File not found: {}", file_path));
    }

    let content = fs::read_to_string(file_path)
        .map_err(|e| format!("Failed to read file: {}", e))?;

    let entries = parse_srt(&content)?;

    let name = path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    Ok(SRTFile {
        name,
        path: file_path.to_string(),
        entries,
        encoding: Some("UTF-8".to_string()),
    })
}

/// 文件权限检查结果
#[derive(Debug, Serialize, Deserialize)]
pub struct FilePermissionCheck {
    pub readable: bool,
    pub writable: bool,
    pub error_message: Option<String>,
    pub is_locked: bool,
}

/// 解锁文件 (移除 macOS 的锁定标志，并修复权限)
#[cfg(target_os = "macos")]
pub fn unlock_file(file_path: &str) -> Result<(), String> {
    use std::process::Command;

    // 1. 尝试移除 uchg 标志
    let _ = Command::new("chflags")
        .args(["nouchg", file_path])
        .output();

    // 2. 尝试添加写入权限
    let output = Command::new("chmod")
        .args(["+w", file_path])
        .output()
        .map_err(|e| format!("执行权限修改命令失败: {}", e))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("修改文件权限失败: {}", stderr))
    }
}

#[cfg(not(target_os = "macos"))]
pub fn unlock_file(_file_path: &str) -> Result<(), String> {
    Ok(())
}

/// 检查文件是否可写入
/// 这个函数会尝试以追加模式打开文件来检测写入权限
/// 不会修改文件内容
pub fn check_file_permission(file_path: &str) -> FilePermissionCheck {
    let path = Path::new(file_path);

    // 检查文件是否存在
    if !path.exists() {
        return FilePermissionCheck {
            readable: false,
            writable: false,
            error_message: Some("文件不存在".to_string()),
            is_locked: false,
        };
    }

    // 检查读取权限
    let readable = fs::read_to_string(file_path).is_ok();

    // 检查写入权限 - 尝试以追加模式打开文件
    let write_result = OpenOptions::new().write(true).append(true).open(file_path);

    let (writable, error_message, is_locked) = match write_result {
        Ok(_) => (true, None, false),
        Err(e) => {
            let is_permission_denied = e.kind() == std::io::ErrorKind::PermissionDenied;

            // 在 macOS 上，Permission denied 通常可以通过解锁来解决
            // 包括：文件被锁定、只读权限、从外部来源获取的文件等
            let is_locked = is_permission_denied && cfg!(target_os = "macos");

            let message = if is_locked {
                "文件已被锁定或没有写入权限。\n\n点击「解锁」按钮可以尝试解除限制并继续编辑。"
                    .to_string()
            } else {
                format!("文件无法写入: {}", e)
            };

            (false, Some(message), is_locked)
        }
    };
    
    FilePermissionCheck {
        readable,
        writable,
        error_message,
        is_locked,
    }
}

/// Write SRT file
pub fn write_srt_file(file_path: &str, entries: &[SubtitleEntry]) -> Result<(), String> {
    let mut content = String::new();

    for (index, entry) in entries.iter().enumerate() {
        // Add subtitle ID (sequence number starting from 1)
        // Always use sequential numbering regardless of original id
        content.push_str(&format!("{}\n", index + 1));

        // Add timestamp line
        content.push_str(&format!(
            "{} --> {}\n",
            entry.start_time.to_string(),
            entry.end_time.to_string()
        ));

        // Add subtitle text
        content.push_str(&format!("{}", entry.text));

        // Add blank line between entries (except for the last one)
        if index < entries.len() - 1 {
            content.push_str("\n\n");
        }
    }

    fs::write(file_path, content)
        .map_err(|e| format!("Failed to write file: {}", e))?;

    println!("Successfully wrote {} subtitles to {}", entries.len(), file_path);
    Ok(())
}

// ============ 导出功能 ============

impl TimeStamp {
    /// Convert to VTT format: HH:MM:SS.mmm
    pub fn to_vtt_string(&self) -> String {
        format!(
            "{:02}:{:02}:{:02}.{:03}",
            self.hours, self.minutes, self.seconds, self.milliseconds
        )
    }

    /// Convert to simple format for Markdown: HH:MM:SS
    pub fn to_simple_string(&self) -> String {
        format!(
            "{:02}:{:02}:{:02}",
            self.hours, self.minutes, self.seconds
        )
    }

    /// Convert to total milliseconds
    pub fn to_ms(&self) -> u64 {
        (self.hours as u64 * 3600 + self.minutes as u64 * 60 + self.seconds as u64) * 1000
            + self.milliseconds as u64
    }

    /// Convert to frames at given frame rate
    pub fn to_frames(&self, fps: f64) -> u64 {
        let total_seconds = self.hours as f64 * 3600.0
            + self.minutes as f64 * 60.0
            + self.seconds as f64
            + self.milliseconds as f64 / 1000.0;
        (total_seconds * fps).round() as u64
    }
}

/// Export to TXT (plain text, subtitles only)
pub fn export_to_txt(file_path: &str, entries: &[SubtitleEntry]) -> Result<(), String> {
    let content: String = entries
        .iter()
        .map(|e| e.text.clone())
        .collect::<Vec<_>>()
        .join("\n");

    fs::write(file_path, content)
        .map_err(|e| format!("Failed to write TXT file: {}", e))?;

    println!("Successfully exported {} subtitles to TXT: {}", entries.len(), file_path);
    Ok(())
}

/// Export to VTT (WebVTT format)
pub fn export_to_vtt(file_path: &str, entries: &[SubtitleEntry]) -> Result<(), String> {
    let mut content = String::from("WEBVTT\n\n");

    for (index, entry) in entries.iter().enumerate() {
        // Cue identifier (optional but useful)
        content.push_str(&format!("{}\n", index + 1));

        // Timestamp line (VTT uses . instead of ,)
        content.push_str(&format!(
            "{} --> {}\n",
            entry.start_time.to_vtt_string(),
            entry.end_time.to_vtt_string()
        ));

        // Subtitle text
        content.push_str(&entry.text);

        // Blank line between cues
        if index < entries.len() - 1 {
            content.push_str("\n\n");
        }
    }

    fs::write(file_path, content)
        .map_err(|e| format!("Failed to write VTT file: {}", e))?;

    println!("Successfully exported {} subtitles to VTT: {}", entries.len(), file_path);
    Ok(())
}

/// Export to Markdown
pub fn export_to_markdown(file_path: &str, entries: &[SubtitleEntry]) -> Result<(), String> {
    let mut content = String::from("# 视频脚本\n\n");

    for entry in entries {
        content.push_str(&format!(
            "**[{} - {}]** {}\n\n",
            entry.start_time.to_simple_string(),
            entry.end_time.to_simple_string(),
            entry.text.replace('\n', " ")
        ));
    }

    fs::write(file_path, content.trim_end())
        .map_err(|e| format!("Failed to write Markdown file: {}", e))?;

    println!("Successfully exported {} subtitles to Markdown: {}", entries.len(), file_path);
    Ok(())
}

/// Export to FCPXML (Final Cut Pro XML)
/// fps: frame rate (e.g., 24.0, 25.0, 29.97, 30.0, 60.0)
/// position_x: subtitle X position (default: 0)
/// position_y: subtitle Y position (default: -415)
pub fn export_to_fcpxml(
    file_path: &str,
    entries: &[SubtitleEntry],
    fps: f64,
    position_x: i32,
    position_y: i32,
) -> Result<(), String> {
    // Calculate frame duration and format name based on fps
    let (frame_duration, format_name) = match fps as u32 {
        24 => ("100/2400s", "FFVideoFormat1080p24"),
        25 => ("100/2500s", "FFVideoFormat1080p25"),
        30 => ("100/3000s", "FFVideoFormat1080p30"),
        60 => ("100/6000s", "FFVideoFormat1080p60"),
        _ => ("100/2500s", "FFVideoFormat1080p25"), // Default to 25fps
    };

    // Find total duration from last subtitle (in the same time base)
    let total_duration = entries
        .last()
        .map(|e| e.end_time.to_frames(fps) * 100)
        .unwrap_or(0);
    
    let time_base = (fps * 100.0) as u64;
    let gap_duration = total_duration + 10000;

    let mut content = format!(r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE fcpxml>

<fcpxml version="1.8">
  <resources>
    <format id="r1" name="{}" frameDuration="{}" width="1920" height="1080" colorSpace="1-1-1 (Rec. 709)"/>
    <effect id="r2" name="自定" uid=".../Titles.localized/Build In:Out.localized/Custom.localized/Custom.moti"/>
  </resources>
  <library>
    <event name="Subtitles">
      <project name="Subtitles">
        <sequence duration="{}/{}s" format="r1" tcStart="0s" tcFormat="NDF" audioLayout="stereo" audioRate="48k">
          <spine>
            <gap name="空隙" offset="0s" duration="{}/{}s">
"#, format_name, frame_duration, gap_duration, time_base, gap_duration, time_base);

    for (index, entry) in entries.iter().enumerate() {
        // Convert time to the same time base (fps * 100)
        let start_units = entry.start_time.to_frames(fps) * 100;
        let end_units = entry.end_time.to_frames(fps) * 100;
        let duration_units = end_units - start_units;

        // Escape XML special characters
        let escaped_text = entry.text
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&apos;")
            .replace('\n', " ");

        content.push_str(&format!(
            r#"<title name="{} - 自定" lane="1" offset="{}/{}s" ref="r2" duration="{}/{}s">
<param name="位置" key="9999/10199/10201/1/100/101" value="{} {}"/>
<param name="对齐" key="9999/10199/10201/2/354/1002961760/401" value="1 (居中)"/>
<param name="Out Sequencing" key="9999/10199/10201/4/10233/201/202" value="0 (到)"/>

<text>
  <text-style ref="ts{}">{}</text-style>
</text>
<text-style-def id="ts{}">
  <text-style font="PingFang SC" fontSize="62" fontFace="Semibold" fontColor="1 1 1 1" bold="1" strokeColor="0.329705 0.329721 0.329713 1" strokeWidth="-1" shadowColor="0 0 0 0.75" shadowOffset="3 315" kerning="1.24" alignment="center"/>
</text-style-def>
</title>
"#,
            escaped_text.chars().take(20).collect::<String>(),
            start_units, time_base,
            duration_units, time_base,
            position_x, position_y,
            index + 1,
            escaped_text,
            index + 1
        ));
    }

    content.push_str(r#"            </gap>
          </spine>
        </sequence>
      </project>
    </event>
  </library>
</fcpxml>"#);

    fs::write(file_path, content)
        .map_err(|e| format!("Failed to write FCPXML file: {}", e))?;

    println!("Successfully exported {} subtitles to FCPXML ({}fps): {}", entries.len(), fps, file_path);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timestamp_parse() {
        let ts = TimeStamp::parse("00:01:23,456").unwrap();
        assert_eq!(ts.hours, 0);
        assert_eq!(ts.minutes, 1);
        assert_eq!(ts.seconds, 23);
        assert_eq!(ts.milliseconds, 456);
    }

    #[test]
    fn test_timestamp_to_string() {
        let ts = TimeStamp {
            hours: 0,
            minutes: 1,
            seconds: 23,
            milliseconds: 456,
        };
        assert_eq!(ts.to_string(), "00:01:23,456");
    }

    #[test]
    fn test_parse_srt() {
        let content = r#"1
00:00:01,000 --> 00:00:04,000
This is the first subtitle

2
00:00:05,000 --> 00:00:08,000
This is the second subtitle"#;

        let entries = parse_srt(content).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].id, 1);
        assert_eq!(entries[0].text, "This is the first subtitle");
    }
}

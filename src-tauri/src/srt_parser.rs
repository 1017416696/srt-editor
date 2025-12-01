use serde::{Deserialize, Serialize};
use std::fs;
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

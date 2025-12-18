# VoSub - Product Overview

VoSub is a professional desktop subtitle editor for SRT (SubRip) files. Built with Tauri 2, Vue 3, and Rust.

## Core Features

- Real-time subtitle editing with undo/redo (100 action history)
- Audio waveform visualization with WaveSurfer.js
- Multi-format audio support (MP3, WAV, AAC, FLAC, OGG)
- Time conflict detection (overlapping subtitles)
- Multi-track subtitle display
- Batch text processing (HTML removal, punctuation, case conversion, CJK spacing)
- Export to TXT, VTT, Markdown, FCPXML (Final Cut Pro)
- AI-powered transcription (Whisper, SenseVoice) and correction (FireRedASR)
- Tab-based multi-file editing
- File association for .srt files

## Target Platforms

- macOS (Safari 16+)
- Windows (Chrome 107+)

## UI Language

The application UI is in Chinese (简体中文). Menu items, dialogs, and user-facing text should be in Chinese.

## Current Version

2.9.0 (Cargo.toml) | 2.6.0 (package.json)

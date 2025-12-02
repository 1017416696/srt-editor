import type { SubtitleEntry, HistoryAction } from './subtitle'

// 单个标签页的字幕状态
export interface TabSubtitleState {
  entries: SubtitleEntry[]
  currentEntryId: number | null
  editingEntryId: number | null
  history: HistoryAction[]
  historyIndex: number
  savedHistoryIndex: number
  searchQuery: string
  searchResults: number[]
  currentSearchIndex: number
  filePath: string | null
}

// 单个标签页的音频状态
export interface TabAudioState {
  filePath: string | null
  fileName: string | null
  format: string | null
  waveform: number[] | null
  duration: number
  currentTime: number
  isPlaying: boolean
  volume: number
  playbackRate: number
}

// 编辑器标签页
export interface EditorTab {
  id: string
  fileName: string // 显示名称（从字幕文件路径提取）
  subtitle: TabSubtitleState
  audio: TabAudioState
}

// 创建默认的字幕状态
export function createDefaultSubtitleState(): TabSubtitleState {
  return {
    entries: [],
    currentEntryId: null,
    editingEntryId: null,
    history: [],
    historyIndex: -1,
    savedHistoryIndex: -1,
    searchQuery: '',
    searchResults: [],
    currentSearchIndex: 0,
    filePath: null,
  }
}

// 创建默认的音频状态
export function createDefaultAudioState(): TabAudioState {
  return {
    filePath: null,
    fileName: null,
    format: null,
    waveform: null,
    duration: 0,
    currentTime: 0,
    isPlaying: false,
    volume: 1,
    playbackRate: 1,
  }
}

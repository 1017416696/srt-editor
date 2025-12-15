/**
 * SRT 字幕编辑器核心类型定义
 */

/**
 * 时间戳格式 (SRT 标准: HH:MM:SS,mmm)
 */
export interface TimeStamp {
  hours: number
  minutes: number
  seconds: number
  milliseconds: number
}

/**
 * 单条字幕条目
 */
export interface SubtitleEntry {
  id: number // 序号
  startTime: TimeStamp // 开始时间
  endTime: TimeStamp // 结束时间
  text: string // 字幕文本
  isEditing?: boolean // 是否正在编辑
  hasConflict?: boolean // 是否存在时间冲突
  trackNumber?: number // 轨道号 (0=默认轨道, 1=冲突轨道)
  needsCorrection?: boolean // 是否需要二次校正（有 AI 校正建议待确认）
  correctionSuggestion?: string // AI 校正建议文本
}

/**
 * 音频文件信息
 */
export interface AudioFile {
  name: string // 文件名
  path: string // 文件路径
  duration: number // 时长(秒)
  format: string // 格式 (mp3/wav/m4a)
  sampleRate?: number // 采样率
  waveform?: number[] // 波形数据
}

/**
 * SRT 文件信息
 */
export interface SRTFile {
  name: string // 文件名
  path: string // 文件路径
  entries: SubtitleEntry[] // 字幕条目列表
  encoding?: string // 文件编码
}

/**
 * 播放器状态
 */
export interface PlayerState {
  isPlaying: boolean // 是否正在播放
  currentTime: number // 当前播放时间(秒)
  duration: number // 总时长(秒)
  volume: number // 音量 (0-1)
  playbackRate: number // 播放速率 (0.5-2.0)
}

/**
 * 编辑历史操作类型
 */
export enum HistoryActionType {
  TEXT_EDIT = 'TEXT_EDIT', // 文本编辑
  TIME_EDIT = 'TIME_EDIT', // 时间编辑
  DELETE = 'DELETE', // 删除字幕
  ADD = 'ADD', // 新增字幕
  SPLIT = 'SPLIT', // 分割字幕
  SPLIT_MULTIPLE = 'SPLIT_MULTIPLE', // 多段分割字幕
  MERGE = 'MERGE', // 合并字幕
  BATCH = 'BATCH', // 批量操作
}

/**
 * 编辑历史记录
 */
export interface HistoryAction {
  type: HistoryActionType
  timestamp: number // 操作时间戳
  entryId: number // 相关字幕 ID
  before: Partial<SubtitleEntry> // 操作前状态
  after: Partial<SubtitleEntry> // 操作后状态
  description?: string // 操作描述
  // 分割操作专用字段
  newEntryId?: number // 分割产生的新字幕 ID
  newEntry?: Partial<SubtitleEntry> // 分割产生的新字幕数据
  // 多段分割操作专用字段
  splitSegments?: Partial<SubtitleEntry>[] // 分割产生的所有字幕数据（用于撤销）
  // 合并操作专用字段
  mergedEntries?: Partial<SubtitleEntry>[] // 被合并的所有字幕数据（用于撤销）
}

/**
 * 编辑器配置
 */
export interface EditorConfig {
  autoSave: boolean // 自动保存
  autoscroll: boolean // 自动滚动
  showWaveform: boolean // 显示波形
  showKeyboardHints: boolean // 显示快捷键提示
  theme: 'light' | 'dark' // 主题
  language: 'zh-CN' | 'en-US' // 界面语言
  newSubtitleDuration: number // 新增字幕默认时长(秒)
}

/**
 * 快捷键配置
 */
export interface KeyBinding {
  key: string // 键名
  description: string // 功能描述
  action: string // 动作名称
}

/**
 * 批量操作类型
 */
export enum BatchOperationType {
  REMOVE_HTML_TAGS = 'REMOVE_HTML_TAGS', // 移除 HTML 标签
  REMOVE_PUNCTUATION = 'REMOVE_PUNCTUATION', // 移除标点符号
  TIME_OFFSET = 'TIME_OFFSET', // 时间偏移
}

/**
 * 时间冲突检测结果
 */
export interface TimeConflict {
  entryId: number
  conflictWithId: number
  overlapDuration: number // 重叠时长(毫秒)
}

/**
 * 查找结果
 */
export interface SearchResult {
  entryId: number
  matchIndex: number // 匹配位置
  matchLength: number // 匹配长度
}

/**
 * 文件权限检查结果
 */
export interface FilePermissionCheck {
  readable: boolean // 是否可读
  writable: boolean // 是否可写
  error_message: string | null // 错误信息
  is_locked: boolean // 是否被 macOS 锁定
}

/**
 * FireRedASR 字幕校正相关类型定义
 */

import type { TimeStamp } from './subtitle'

/**
 * FireRedASR 环境状态
 */
export interface FireRedEnvStatus {
  uv_installed: boolean
  env_exists: boolean
  ready: boolean
}

/**
 * 校正进度
 */
export interface FireRedProgress {
  progress: number
  current_text: string
  status: 'installing' | 'loading' | 'correcting' | 'processing' | 'completed' | 'error'
}

/**
 * 校正结果条目
 */
export interface CorrectionEntry {
  id: number
  start_time: TimeStamp
  end_time: TimeStamp
  original: string
  corrected: string
  has_diff: boolean
}

/**
 * 用户选择状态
 */
export type CorrectionChoice = 'original' | 'corrected'

/**
 * 带选择状态的校正条目
 */
export interface CorrectionEntryWithChoice extends CorrectionEntry {
  choice: CorrectionChoice
}

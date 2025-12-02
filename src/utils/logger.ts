/**
 * 日志模块 - 基于 tauri-plugin-log
 *
 * 日志级别：
 * - DEBUG: 开发环境详细信息（生产环境不记录）
 * - INFO: 关键操作记录
 * - WARN: 警告信息
 * - ERROR: 错误信息
 *
 * 日志文件位置：
 * - macOS: ~/Library/Logs/com.srt-editor/
 * - Windows: %APPDATA%/com.srt-editor/logs/
 * - Linux: ~/.config/com.srt-editor/logs/
 *
 * 日志文件命名：srt-editor.log（超过 40KB 后会创建带日期的新文件）
 */

import {
  debug as tauriDebug,
  info as tauriInfo,
  warn as tauriWarn,
  error as tauriError,
  attachConsole,
} from '@tauri-apps/plugin-log'

// 初始化日志（连接控制台输出）
let initialized = false
export const initLogger = async () => {
  if (initialized) return
  try {
    await attachConsole()
    initialized = true
  } catch (e) {
    console.error('Failed to attach console for logging:', e)
  }
}

// 格式化日志消息
const formatMessage = (message: string, context?: Record<string, unknown>): string => {
  if (!context || Object.keys(context).length === 0) {
    return message
  }
  const contextStr = Object.entries(context)
    .map(([key, value]) => `${key}=${JSON.stringify(value)}`)
    .join(' ')
  return `${message} ${contextStr}`
}

// 日志接口
export const logger = {
  /**
   * 调试日志 - 仅开发环境记录
   */
  debug: (message: string, context?: Record<string, unknown>) => {
    tauriDebug(formatMessage(message, context))
  },

  /**
   * 信息日志 - 关键操作
   */
  info: (message: string, context?: Record<string, unknown>) => {
    tauriInfo(formatMessage(message, context))
  },

  /**
   * 警告日志
   */
  warn: (message: string, context?: Record<string, unknown>) => {
    tauriWarn(formatMessage(message, context))
  },

  /**
   * 错误日志
   */
  error: (message: string, context?: Record<string, unknown>) => {
    tauriError(formatMessage(message, context))
  },
}

export default logger

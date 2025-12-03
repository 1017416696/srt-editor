/**
 * 波形对齐工具函数
 * 用于将字幕时间对齐到音频波形中的语音区域
 */

import type { TimeStamp } from '@/types/subtitle'

/**
 * 将时间戳转换为毫秒
 */
export function timestampToMs(ts: TimeStamp): number {
  return ts.hours * 3600000 + ts.minutes * 60000 + ts.seconds * 1000 + ts.milliseconds
}

/**
 * 将毫秒转换为时间戳
 */
export function msToTimestamp(ms: number): TimeStamp {
  const hours = Math.floor(ms / 3600000)
  const minutes = Math.floor((ms % 3600000) / 60000)
  const seconds = Math.floor((ms % 60000) / 1000)
  const milliseconds = Math.floor(ms % 1000)
  return { hours, minutes, seconds, milliseconds }
}

/**
 * 查找语音区域的起始和结束点
 * 
 * @param waveformData 波形数据（min/max 交替存储）
 * @param duration 音频总时长（秒）
 * @param currentStartMs 当前字幕开始时间（毫秒）
 * @param currentEndMs 当前字幕结束时间（毫秒）
 * @param searchWindowMs 搜索窗口大小（毫秒），默认 2000ms
 * @returns 语音区域的起始和结束时间（毫秒），如果未找到返回 null
 */
export function findVoiceRegion(
  waveformData: number[],
  duration: number,
  currentStartMs: number,
  currentEndMs: number,
  searchWindowMs: number = 2000
): { startMs: number; endMs: number } | null {
  if (!waveformData || waveformData.length === 0 || duration <= 0) {
    return null
  }

  const durationMs = duration * 1000
  const numPoints = waveformData.length / 2
  const msPerPoint = durationMs / numPoints

  // 搜索范围：字幕区域前后扩展
  const searchStartMs = Math.max(0, currentStartMs - searchWindowMs)
  const searchEndMs = Math.min(durationMs, currentEndMs + searchWindowMs)
  const searchStartIdx = Math.floor(searchStartMs / msPerPoint)
  const searchEndIdx = Math.min(Math.ceil(searchEndMs / msPerPoint), numPoints)

  // 提取振幅数据
  const amplitudes: number[] = []
  for (let i = searchStartIdx; i < searchEndIdx; i++) {
    const maxVal = Math.abs(waveformData[i * 2 + 1] || 0)
    amplitudes.push(maxVal)
  }

  if (amplitudes.length < 10) {
    return null
  }

  // 计算动态阈值
  const sortedAmps = [...amplitudes].sort((a, b) => a - b)
  const lowPercentile = sortedAmps[Math.floor(sortedAmps.length * 0.2)] ?? 0
  const highPercentile = sortedAmps[Math.floor(sortedAmps.length * 0.8)] ?? 0
  
  if (highPercentile - lowPercentile < 0.03) {
    return null
  }

  const threshold = lowPercentile + (highPercentile - lowPercentile) * 0.35

  // 平滑振幅
  const windowSize = 2
  const smoothedAmps: number[] = []
  for (let i = 0; i < amplitudes.length; i++) {
    let sum = 0
    let count = 0
    for (let j = Math.max(0, i - windowSize); j <= Math.min(amplitudes.length - 1, i + windowSize); j++) {
      sum += amplitudes[j] ?? 0
      count++
    }
    smoothedAmps.push(sum / count)
  }

  // 找到所有语音区域（连续高于阈值的段落）
  const regions: Array<{ start: number; end: number }> = []
  let regionStart: number | null = null

  for (let i = 0; i < smoothedAmps.length; i++) {
    const isVoice = (smoothedAmps[i] ?? 0) >= threshold

    if (isVoice && regionStart === null) {
      regionStart = i
    } else if (!isVoice && regionStart !== null) {
      regions.push({ start: regionStart, end: i - 1 })
      regionStart = null
    }
  }

  if (regionStart !== null) {
    regions.push({ start: regionStart, end: smoothedAmps.length - 1 })
  }

  if (regions.length === 0) {
    return null
  }

  // 合并相邻区域（间隔小于 150ms）
  const mergeGapPoints = Math.ceil(150 / msPerPoint)
  const mergedRegions: typeof regions = []

  for (const region of regions) {
    const last = mergedRegions[mergedRegions.length - 1]
    if (last && region.start - last.end <= mergeGapPoints) {
      last.end = region.end
    } else {
      mergedRegions.push({ ...region })
    }
  }

  // 找到与当前字幕重叠最多或最近的区域
  const currentStartIdx = Math.floor((currentStartMs - searchStartMs) / msPerPoint)
  const currentEndIdx = Math.floor((currentEndMs - searchStartMs) / msPerPoint)

  let bestRegion = mergedRegions[0]!
  let bestScore = -Infinity

  for (const region of mergedRegions) {
    // 计算重叠量
    const overlapStart = Math.max(region.start, currentStartIdx)
    const overlapEnd = Math.min(region.end, currentEndIdx)
    const overlap = Math.max(0, overlapEnd - overlapStart)

    // 计算距离（如果没有重叠）
    const distance = overlap > 0 ? 0 : Math.min(
      Math.abs(region.start - currentEndIdx),
      Math.abs(region.end - currentStartIdx)
    )

    // 区域长度（更长的区域更可能是完整语音）
    const length = region.end - region.start

    // 评分：重叠越多越好，距离越近越好，长度适中更好
    const score = overlap * 10 - distance + Math.min(length, 100)

    if (score > bestScore) {
      bestScore = score
      bestRegion = region
    }
  }

  // 转换回毫秒
  // 开始时间提前 150ms，让字幕先于语音出现
  // 结束时间延后 50ms，确保语音完整显示
  const startPaddingMs = 150
  const endPaddingMs = 50
  const resultStartMs = Math.max(0, searchStartMs + bestRegion.start * msPerPoint - startPaddingMs)
  const resultEndMs = Math.min(durationMs, searchStartMs + (bestRegion.end + 1) * msPerPoint + endPaddingMs)

  return {
    startMs: Math.round(resultStartMs),
    endMs: Math.round(resultEndMs)
  }
}

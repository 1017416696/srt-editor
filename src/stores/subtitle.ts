import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import type {
  SubtitleEntry,
  SRTFile,
  TimeConflict,
  HistoryAction,
  TimeStamp,
} from '@/types/subtitle'
import { HistoryActionType } from '@/types/subtitle'
import { timeStampToMs, getDuration } from '@/utils/time'

export const useSubtitleStore = defineStore('subtitle', () => {
  // 状态
  const srtFile = ref<SRTFile | null>(null)
  const entries = ref<SubtitleEntry[]>([])
  const currentEntryId = ref<number | null>(null)
  const editingEntryId = ref<number | null>(null)

  // 编辑历史
  const history = ref<HistoryAction[]>([])
  const historyIndex = ref(-1)

  // 查找状态
  const searchQuery = ref('')
  const searchResults = ref<number[]>([])
  const currentSearchIndex = ref(0)

  // 计算属性
  const currentEntry = computed(() => {
    if (currentEntryId.value === null) return null
    return entries.value.find((e) => e.id === currentEntryId.value) || null
  })

  const editingEntry = computed(() => {
    if (editingEntryId.value === null) return null
    return entries.value.find((e) => e.id === editingEntryId.value) || null
  })

  const hasUnsavedChanges = computed(() => {
    return historyIndex.value >= 0
  })

  const canUndo = computed(() => historyIndex.value >= 0)
  const canRedo = computed(() => historyIndex.value < history.value.length - 1)

  // 检测时间冲突
  const detectTimeConflicts = (): TimeConflict[] => {
    const conflicts: TimeConflict[] = []
    const sortedEntries = [...entries.value].sort((a, b) => {
      const aStart = timeStampToMs(a.startTime)
      const bStart = timeStampToMs(b.startTime)
      return aStart - bStart
    })

    for (let i = 0; i < sortedEntries.length - 1; i++) {
      const current = sortedEntries[i]
      const next = sortedEntries[i + 1]

      if (!current || !next) continue

      const currentEnd = timeStampToMs(current.endTime)
      const nextStart = timeStampToMs(next.startTime)

      if (currentEnd > nextStart) {
        const overlapDuration = currentEnd - nextStart
        conflicts.push({
          entryId: current.id,
          conflictWithId: next.id,
          overlapDuration,
        })
      }
    }

    // 标记有冲突的条目
    entries.value.forEach((entry) => {
      entry.hasConflict = conflicts.some(
        (c) => c.entryId === entry.id || c.conflictWithId === entry.id,
      )
    })

    return conflicts
  }

  // 分配字幕到轨道 (支持最多 2 个轨道)
  const assignSubtitleToTracks = () => {
    // 重置所有字幕的轨道号
    entries.value.forEach((entry) => {
      entry.trackNumber = 0
    })

    // 按开始时间排序
    const sortedEntries = [...entries.value].sort((a, b) => {
      return timeStampToMs(a.startTime) - timeStampToMs(b.startTime)
    })

    // 记录每条轨道的占用区间: Map<trackNumber, Array<{startMs, endMs}>>
    const trackOccupancy: Map<number, Array<{ startMs: number; endMs: number }>> =
      new Map()

    // 为每个字幕分配轨道
    sortedEntries.forEach((entry) => {
      const startMs = timeStampToMs(entry.startTime)
      const endMs = timeStampToMs(entry.endTime)

      // 找第一个不冲突的轨道
      let assignedTrack = 0

      // 只支持最多 2 个轨道 (0 和 1)
      for (let track = 0; track <= 1; track++) {
        if (!trackOccupancy.has(track)) {
          trackOccupancy.set(track, [])
        }

        const occupied = trackOccupancy.get(track)!
        // 检查是否与当前轨道上的任何区间冲突
        const hasConflict = occupied.some((seg) => {
          // endMs > startMs 表示有冲突（根据冲突定义）
          return endMs > seg.startMs && startMs < seg.endMs
        })

        if (!hasConflict) {
          assignedTrack = track
          break
        }
      }

      // 将字幕分配到找到的轨道
      const actualEntry = entries.value.find((e) => e.id === entry.id)
      if (actualEntry) {
        actualEntry.trackNumber = assignedTrack
      }

      // 记录轨道占用
      const track = trackOccupancy.get(assignedTrack)!
      track.push({ startMs, endMs })
    })
  }

  // 加载 SRT 文件
  const loadSRTFile = (file: SRTFile) => {
    srtFile.value = file
    entries.value = file.entries
    currentEntryId.value = entries.value.length > 0 ? (entries.value[0]?.id ?? null) : null
    history.value = []
    historyIndex.value = -1
    detectTimeConflicts()
    assignSubtitleToTracks()
  }

  // 根据播放时间获取当前字幕
  const getCurrentEntryByTime = (currentTime: number) => {
    const currentMs = currentTime * 1000

    const entry = entries.value.find((e) => {
      const startMs = timeStampToMs(e.startTime)
      const endMs = timeStampToMs(e.endTime)
      return currentMs >= startMs && currentMs <= endMs
    })

    if (entry && entry.id !== currentEntryId.value) {
      currentEntryId.value = entry.id
    }

    return entry || null
  }

  // 编辑字幕文本
  const updateEntryText = (entryId: number, newText: string) => {
    const entry = entries.value.find((e) => e.id === entryId)
    if (!entry) return

    const oldText = entry.text
    if (oldText === newText) return

    // 记录历史
    addHistory({
      type: HistoryActionType.TEXT_EDIT,
      timestamp: Date.now(),
      entryId,
      before: { text: oldText },
      after: { text: newText },
    })

    entry.text = newText
  }

  // 编辑时间
  const updateEntryTime = (
    entryId: number,
    startTime?: any,
    endTime?: any,
  ) => {
    const entry = entries.value.find((e) => e.id === entryId)
    if (!entry) return

    const before: any = {}
    const after: any = {}

    if (startTime) {
      before.startTime = entry.startTime
      after.startTime = startTime
      entry.startTime = startTime
    }

    if (endTime) {
      before.endTime = entry.endTime
      after.endTime = endTime
      entry.endTime = endTime
    }

    addHistory({
      type: HistoryActionType.TIME_EDIT,
      timestamp: Date.now(),
      entryId,
      before,
      after,
    })

    detectTimeConflicts()
    assignSubtitleToTracks()
  }

  // 删除字幕
  const deleteEntry = (entryId: number) => {
    const index = entries.value.findIndex((e) => e.id === entryId)
    if (index === -1) return

    const entry = entries.value[index]

    addHistory({
      type: HistoryActionType.DELETE,
      timestamp: Date.now(),
      entryId,
      before: { ...entry },
      after: {},
    })

    // 记录当前选中的字幕在数组中的位置（用于重新编号后恢复选中）
    const currentSelectedIndex = entries.value.findIndex((e) => e.id === currentEntryId.value)

    entries.value.splice(index, 1)

    // 重新编号：从 1 开始连续编号
    entries.value.forEach((e, i) => {
      e.id = i + 1
    })

    // 调整当前选中
    if (currentEntryId.value === entryId) {
      // 删除的是当前选中的，选中下一个或上一个
      const newEntry = entries.value[index] || entries.value[index - 1]
      currentEntryId.value = newEntry?.id || null
    } else if (currentSelectedIndex !== -1) {
      // 删除的不是当前选中的，根据原位置更新 id
      const newIndex = currentSelectedIndex > index ? currentSelectedIndex - 1 : currentSelectedIndex
      currentEntryId.value = entries.value[newIndex]?.id || null
    }

    detectTimeConflicts()
    assignSubtitleToTracks()
  }

  // 分割字幕
  const splitEntry = (entryId: number, splitTimeMs: number) => {
    const entry = entries.value.find((e) => e.id === entryId)
    if (!entry) return null

    const startMs = timeStampToMs(entry.startTime)
    const endMs = timeStampToMs(entry.endTime)

    // 确保分割点在字幕时间范围内
    if (splitTimeMs <= startMs || splitTimeMs >= endMs) {
      return null
    }

    // 辅助函数：毫秒转时间戳
    const msToTimeStamp = (ms: number): TimeStamp => {
      const totalSeconds = Math.floor(ms / 1000)
      return {
        hours: Math.floor(totalSeconds / 3600),
        minutes: Math.floor((totalSeconds % 3600) / 60),
        seconds: totalSeconds % 60,
        milliseconds: ms % 1000,
      }
    }

    // 保存原始数据用于历史记录
    const originalEntry = { ...entry }

    // 修改原字幕的结束时间为分割点
    entry.endTime = msToTimeStamp(splitTimeMs)

    // 创建新字幕，从分割点开始到原结束时间
    const index = entries.value.findIndex((e) => e.id === entryId)
    const newEntry: SubtitleEntry = {
      id: entryId + 1, // 临时 id，后面会重新编号
      startTime: msToTimeStamp(splitTimeMs),
      endTime: msToTimeStamp(endMs),
      text: entry.text, // 复制原文本
    }

    // 在原字幕后面插入新字幕
    entries.value.splice(index + 1, 0, newEntry)

    // 重新编号：从 1 开始连续编号
    entries.value.forEach((e, i) => {
      e.id = i + 1
    })

    // 获取新字幕的实际 id
    const newId = index + 2

    addHistory({
      type: HistoryActionType.BATCH,
      timestamp: Date.now(),
      entryId: entryId,
      before: { ...originalEntry },
      after: {},
      description: `分割字幕 #${entryId}`,
    })

    detectTimeConflicts()
    assignSubtitleToTracks()

    return newId
  }

  // 新增字幕
  const addEntry = (afterId?: number) => {
    // 默认时长 3 秒
    const DEFAULT_DURATION_MS = 3000

    // 辅助函数：毫秒转时间戳
    const msToTimeStamp = (ms: number): TimeStamp => {
      const totalSeconds = Math.floor(ms / 1000)
      return {
        hours: Math.floor(totalSeconds / 3600),
        minutes: Math.floor((totalSeconds % 3600) / 60),
        seconds: totalSeconds % 60,
        milliseconds: ms % 1000,
      }
    }

    // 计算新增字幕的时间
    let startTime: TimeStamp
    let endTime: TimeStamp

    if (afterId !== undefined) {
      // 在指定字幕后面插入
      const afterEntry = entries.value.find((e) => e.id === afterId)

      if (afterEntry) {
        // 新字幕的开始时间 = 前一个字幕的结束时间
        const newStartMs = timeStampToMs(afterEntry.endTime)
        const newEndMs = newStartMs + DEFAULT_DURATION_MS

        startTime = msToTimeStamp(newStartMs)
        endTime = msToTimeStamp(newEndMs)
      } else {
        // 找不到指定字幕，使用默认时间
        startTime = { hours: 0, minutes: 0, seconds: 0, milliseconds: 0 }
        endTime = { hours: 0, minutes: 0, seconds: 3, milliseconds: 0 }
      }
    } else if (entries.value.length > 0) {
      // 在末尾添加，时间接在最后一个字幕之后
      const lastEntry = entries.value[entries.value.length - 1]
      const newStartMs = timeStampToMs(lastEntry.endTime)
      const newEndMs = newStartMs + DEFAULT_DURATION_MS

      startTime = msToTimeStamp(newStartMs)
      endTime = msToTimeStamp(newEndMs)
    } else {
      // 空列表，使用默认时间
      startTime = { hours: 0, minutes: 0, seconds: 0, milliseconds: 0 }
      endTime = { hours: 0, minutes: 0, seconds: 3, milliseconds: 0 }
    }

    // 临时 id，插入后会重新编号
    const tempId = entries.value.length > 0
      ? Math.max(...entries.value.map((e) => e.id)) + 1
      : 1

    const newEntry: SubtitleEntry = {
      id: tempId,
      startTime,
      endTime,
      text: '',
    }

    // 插入字幕
    if (afterId !== undefined) {
      const index = entries.value.findIndex((e) => e.id === afterId)
      entries.value.splice(index + 1, 0, newEntry)
    } else {
      entries.value.push(newEntry)
    }

    // 重新编号：从 1 开始连续编号
    entries.value.forEach((e, i) => {
      e.id = i + 1
    })

    // 找到新插入字幕的实际 id
    const insertedIndex = afterId !== undefined
      ? entries.value.findIndex((e) => e.id === afterId) + 1
      : entries.value.length
    const newId = insertedIndex

    addHistory({
      type: HistoryActionType.ADD,
      timestamp: Date.now(),
      entryId: newId,
      before: {},
      after: { ...entries.value[insertedIndex - 1] },
    })

    currentEntryId.value = newId
    editingEntryId.value = newId

    assignSubtitleToTracks()

    return newId
  }

  // 移除标点符号的辅助函数
  const removePunctuationFromText = (text: string): string => {
    // 移除中文标点和英文标点
    return text.replace(/[，。！？、；：""''（）《》【】…—,.!?;:'"()\[\]{}]/g, '')
  }

  // 为单条字幕移除标点符号
  const removePunctuationForEntry = (entryId: number) => {
    const entry = entries.value.find((e) => e.id === entryId)
    if (!entry) return

    const newText = removePunctuationFromText(entry.text)
    if (newText !== entry.text) {
      updateEntryText(entryId, newText)
    }
  }

  // 批量去除标点符号
  const removePunctuation = () => {
    entries.value.forEach((entry) => {
      entry.text = removePunctuationFromText(entry.text)
    })

    addHistory({
      type: HistoryActionType.BATCH,
      timestamp: Date.now(),
      entryId: -1,
      before: {},
      after: {},
      description: '批量移除标点符号',
    })
  }

  // 批量去除 HTML 标签
  const removeHTMLTags = () => {
    entries.value.forEach((entry) => {
      entry.text = entry.text.replace(/<[^>]*>/g, '')
    })

    addHistory({
      type: HistoryActionType.BATCH,
      timestamp: Date.now(),
      entryId: -1,
      before: {},
      after: {},
      description: '批量移除 HTML 标签',
    })
  }

  // 为文本添加中英文空格的辅助函数
  const addCJKSpacesToText = (text: string): string => {
    let result = text
    // 在中文和英文/数字之间添加空格（中文在前）
    result = result.replace(/([\u4e00-\u9fa5])([A-Za-z0-9])/g, '$1 $2')
    // 在英文/数字和中文之间添加空格（英文/数字在前）
    result = result.replace(/([A-Za-z0-9])([\u4e00-\u9fa5])/g, '$1 $2')
    // 避免重复空格
    result = result.replace(/\s+/g, ' ')
    return result
  }

  // 为单条字幕添加中英文空格
  const addSpacesForEntry = (entryId: number) => {
    const entry = entries.value.find((e) => e.id === entryId)
    if (!entry) return

    const newText = addCJKSpacesToText(entry.text)
    if (newText !== entry.text) {
      updateEntryText(entryId, newText)
    }
  }

  // 批量在中英文/中文数字之间添加空格
  const addSpacesBetweenCJKAndAlphanumeric = () => {
    entries.value.forEach((entry) => {
      entry.text = addCJKSpacesToText(entry.text)
    })

    addHistory({
      type: HistoryActionType.BATCH,
      timestamp: Date.now(),
      entryId: -1,
      before: {},
      after: {},
      description: '批量添加中英文空格',
    })
  }

  // 添加历史记录
  const addHistory = (action: HistoryAction) => {
    // 清除当前位置之后的历史
    history.value = history.value.slice(0, historyIndex.value + 1)
    history.value.push(action)
    historyIndex.value++

    // 限制历史记录数量
    if (history.value.length > 100) {
      history.value.shift()
      historyIndex.value--
    }
  }

  // 撤销
  const undo = () => {
    if (!canUndo.value) return

    const action = history.value[historyIndex.value]
    // TODO: 实现撤销逻辑
    historyIndex.value--
  }

  // 重做
  const redo = () => {
    if (!canRedo.value) return

    historyIndex.value++
    const action = history.value[historyIndex.value]
    // TODO: 实现重做逻辑
  }

  // 查找
  const search = (query: string) => {
    searchQuery.value = query
    searchResults.value = []
    currentSearchIndex.value = 0

    if (!query) return []

    entries.value.forEach((entry) => {
      if (entry.text.toLowerCase().includes(query.toLowerCase())) {
        searchResults.value.push(entry.id)
      }
    })

    return searchResults.value
  }

  // 格式化时间戳为字符串
  const formatTimeStamp = (time: TimeStamp): string => {
    const pad = (num: number, size: number) => num.toString().padStart(size, '0')
    return `${pad(time.hours, 2)}:${pad(time.minutes, 2)}:${pad(time.seconds, 2)},${pad(time.milliseconds, 3)}`
  }

  // 保存到文件
  const saveToFile = async () => {
    if (!srtFile.value) {
      throw new Error('No file loaded')
    }

    // 调用 Tauri 命令保存文件
    const { invoke } = await import('@tauri-apps/api/core')

    // 将当前的字幕条目写回 SRT 文件
    const updatedFile: SRTFile = {
      ...srtFile.value,
      entries: entries.value,
    }

    try {
      await invoke('write_srt', {
        filePath: srtFile.value.path,
        entries: entries.value,
      })

      // 保存成功后，重置历史索引（标记为已保存）
      historyIndex.value = -1
    } catch (error) {
      console.error('Failed to save SRT file:', error)
      throw error
    }
  }

  // 获取当前文件路径
  const currentFilePath = computed(() => srtFile.value?.path || null)

  // 跳转到下一个搜索结果
  const nextSearchResult = () => {
    if (searchResults.value.length === 0) return

    currentSearchIndex.value =
      (currentSearchIndex.value + 1) % searchResults.value.length
    currentEntryId.value = searchResults.value[currentSearchIndex.value] ?? null
  }

  // 跳转到上一个搜索结果
  const prevSearchResult = () => {
    if (searchResults.value.length === 0) return

    currentSearchIndex.value =
      (currentSearchIndex.value - 1 + searchResults.value.length) %
      searchResults.value.length
    currentEntryId.value = searchResults.value[currentSearchIndex.value] ?? null
  }

  return {
    // 状态
    srtFile,
    entries,
    currentEntryId,
    editingEntryId,
    history,
    historyIndex,
    searchQuery,
    searchResults,
    currentSearchIndex,

    // 计算属性
    currentEntry,
    editingEntry,
    hasUnsavedChanges,
    canUndo,
    canRedo,
    currentFilePath,

    // 方法
    loadSRTFile,
    getCurrentEntryByTime,
    updateEntryText,
    updateEntryTime,
    deleteEntry,
    splitEntry,
    addEntry,
    removePunctuation,
    removePunctuationForEntry,
    removeHTMLTags,
    addSpacesForEntry,
    addSpacesBetweenCJKAndAlphanumeric,
    detectTimeConflicts,
    assignSubtitleToTracks,
    undo,
    redo,
    search,
    nextSearchResult,
    prevSearchResult,
    formatTimeStamp,
    saveToFile,
  }
})

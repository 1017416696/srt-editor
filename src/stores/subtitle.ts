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

  // 加载 SRT 文件
  const loadSRTFile = (file: SRTFile) => {
    srtFile.value = file
    entries.value = file.entries
    currentEntryId.value = entries.value.length > 0 ? (entries.value[0]?.id ?? null) : null
    history.value = []
    historyIndex.value = -1
    detectTimeConflicts()
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

    entries.value.splice(index, 1)

    // 调整当前选中
    if (currentEntryId.value === entryId) {
      currentEntryId.value =
        entries.value[index]?.id || entries.value[index - 1]?.id || null
    }

    detectTimeConflicts()
  }

  // 新增字幕
  const addEntry = (afterId?: number) => {
    const newId =
      entries.value.length > 0
        ? Math.max(...entries.value.map((e) => e.id)) + 1
        : 1

    const newEntry: SubtitleEntry = {
      id: newId,
      startTime: { hours: 0, minutes: 0, seconds: 0, milliseconds: 0 },
      endTime: { hours: 0, minutes: 0, seconds: 1, milliseconds: 0 },
      text: '',
    }

    if (afterId !== undefined) {
      const index = entries.value.findIndex((e) => e.id === afterId)
      entries.value.splice(index + 1, 0, newEntry)
    } else {
      entries.value.push(newEntry)
    }

    addHistory({
      type: HistoryActionType.ADD,
      timestamp: Date.now(),
      entryId: newId,
      before: {},
      after: { ...newEntry },
    })

    currentEntryId.value = newId
    editingEntryId.value = newId
  }

  // 批量去除标点符号
  const removePunctuation = () => {
    entries.value.forEach((entry) => {
      entry.text = entry.text.replace(/[，。！？、；：""''（）《》【】…—]/g, '')
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
    addEntry,
    removePunctuation,
    removeHTMLTags,
    detectTimeConflicts,
    undo,
    redo,
    search,
    nextSearchResult,
    prevSearchResult,
    formatTimeStamp,
    saveToFile,
  }
})

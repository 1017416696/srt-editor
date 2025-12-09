import { defineStore } from 'pinia'
import { computed } from 'vue'
import type {
  SubtitleEntry,
  SRTFile,
  TimeConflict,
  HistoryAction,
  TimeStamp,
} from '@/types/subtitle'
import { HistoryActionType } from '@/types/subtitle'
import { timeStampToMs } from '@/utils/time'
import { useConfigStore } from '@/stores/config'
import { useTabManagerStore } from '@/stores/tabManager'
import logger from '@/utils/logger'

export const useSubtitleStore = defineStore('subtitle', () => {
  const tabManager = useTabManagerStore()

  // 从当前激活的 tab 获取状态
  const entries = computed(() => tabManager.activeTab?.subtitle.entries ?? [])
  const currentEntryId = computed({
    get: () => tabManager.activeTab?.subtitle.currentEntryId ?? null,
    set: (val) => {
      if (tabManager.activeTab) {
        tabManager.activeTab.subtitle.currentEntryId = val
      }
    }
  })
  const editingEntryId = computed({
    get: () => tabManager.activeTab?.subtitle.editingEntryId ?? null,
    set: (val) => {
      if (tabManager.activeTab) {
        tabManager.activeTab.subtitle.editingEntryId = val
      }
    }
  })
  const history = computed(() => tabManager.activeTab?.subtitle.history ?? [])
  const historyIndex = computed({
    get: () => tabManager.activeTab?.subtitle.historyIndex ?? -1,
    set: (val) => {
      if (tabManager.activeTab) {
        tabManager.activeTab.subtitle.historyIndex = val
      }
    }
  })
  const savedHistoryIndex = computed({
    get: () => tabManager.activeTab?.subtitle.savedHistoryIndex ?? -1,
    set: (val) => {
      if (tabManager.activeTab) {
        tabManager.activeTab.subtitle.savedHistoryIndex = val
      }
    }
  })
  const searchQuery = computed({
    get: () => tabManager.activeTab?.subtitle.searchQuery ?? '',
    set: (val) => {
      if (tabManager.activeTab) {
        tabManager.activeTab.subtitle.searchQuery = val
      }
    }
  })
  const searchResults = computed(() => tabManager.activeTab?.subtitle.searchResults ?? [])
  const currentSearchIndex = computed({
    get: () => tabManager.activeTab?.subtitle.currentSearchIndex ?? 0,
    set: (val) => {
      if (tabManager.activeTab) {
        tabManager.activeTab.subtitle.currentSearchIndex = val
      }
    }
  })

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
    return historyIndex.value !== savedHistoryIndex.value
  })

  const canUndo = computed(() => historyIndex.value >= 0)
  const canRedo = computed(() => historyIndex.value < history.value.length - 1)

  const currentFilePath = computed(() => tabManager.activeTab?.subtitle.filePath || null)

  // 检测时间冲突（可传入 entries 数组以避免重复获取 computed）
  const detectTimeConflicts = (targetEntries?: SubtitleEntry[]): TimeConflict[] => {
    const currentEntries = targetEntries ?? entries.value
    if (!currentEntries.length) return []

    const conflicts: TimeConflict[] = []
    
    // 预计算所有时间戳，避免重复调用 timeStampToMs
    const entriesWithTime = currentEntries.map(entry => ({
      entry,
      startMs: timeStampToMs(entry.startTime),
      endMs: timeStampToMs(entry.endTime),
    }))
    
    // 按开始时间排序
    entriesWithTime.sort((a, b) => a.startMs - b.startMs)

    // 收集有冲突的 entry id
    const conflictIds = new Set<number>()

    for (let i = 0; i < entriesWithTime.length - 1; i++) {
      const current = entriesWithTime[i]
      const next = entriesWithTime[i + 1]

      if (!current || !next) continue

      if (current.endMs > next.startMs) {
        const overlapDuration = current.endMs - next.startMs
        conflicts.push({
          entryId: current.entry.id,
          conflictWithId: next.entry.id,
          overlapDuration,
        })
        conflictIds.add(current.entry.id)
        conflictIds.add(next.entry.id)
      }
    }

    // 批量标记有冲突的条目
    currentEntries.forEach((entry) => {
      entry.hasConflict = conflictIds.has(entry.id)
    })

    return conflicts
  }

  // 分配字幕到轨道 (支持最多 2 个轨道)（可传入 entries 数组以避免重复获取 computed）
  const assignSubtitleToTracks = (targetEntries?: SubtitleEntry[]) => {
    const currentEntries = targetEntries ?? entries.value
    if (!currentEntries.length) return

    // 预计算所有时间戳，避免重复调用 timeStampToMs
    const entriesWithTime = currentEntries.map(entry => ({
      entry,
      startMs: timeStampToMs(entry.startTime),
      endMs: timeStampToMs(entry.endTime),
    }))
    
    // 按开始时间排序
    entriesWithTime.sort((a, b) => a.startMs - b.startMs)

    // 记录每条轨道的占用区间（使用数组而非 Map，性能更好）
    const trackOccupancy: Array<Array<{ startMs: number; endMs: number }>> = [[], []]

    // 为每个字幕分配轨道
    entriesWithTime.forEach(({ entry, startMs, endMs }) => {
      let assignedTrack = 0

      for (let track = 0; track <= 1; track++) {
        const occupied = trackOccupancy[track]!
        // 只检查可能重叠的区间（已排序，可以优化）
        const hasConflict = occupied.some((seg) => {
          return endMs > seg.startMs && startMs < seg.endMs
        })

        if (!hasConflict) {
          assignedTrack = track
          break
        }
      }

      entry.trackNumber = assignedTrack
      trackOccupancy[assignedTrack]!.push({ startMs, endMs })
    })
  }


  // 加载 SRT 文件（现在由 tabManager 处理创建 tab）
  const loadSRTFile = (file: SRTFile) => {
    // 检查文件是否已经打开
    const existingTab = tabManager.findTabByFilePath(file.path)
    if (existingTab) {
      // 已经打开，切换到该 tab
      tabManager.setActiveTab(existingTab.id)
      return
    }

    // 创建新 tab
    const newTab = tabManager.createTab(file.path, file.entries)
    
    // 直接使用新 tab 的 entries 进行检测和分配，避免通过 computed 属性访问
    // 这样可以避免响应式追踪的开销
    const tabEntries = newTab.subtitle.entries
    detectTimeConflicts(tabEntries)
    assignSubtitleToTracks(tabEntries)
    
    logger.info('SRT 文件加载完成', { path: file.path, entries: file.entries.length })
  }

  // 根据播放时间获取当前字幕
  const getCurrentEntryByTime = (currentTime: number) => {
    const currentMs = currentTime * 1000
    const currentEntries = entries.value

    const entry = currentEntries.find((e) => {
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

    addHistory({
      type: HistoryActionType.TEXT_EDIT,
      timestamp: Date.now(),
      entryId,
      before: { text: oldText },
      after: { text: newText },
    })

    entry.text = newText
  }

  // 拖动开始时的原始时间（用于记录历史）
  let dragOriginalTimes = new Map<number, { startTime: TimeStamp; endTime: TimeStamp }>()

  // 开始拖动时调用，记录原始时间
  const startDragging = (entryIds: number[]) => {
    dragOriginalTimes.clear()
    entryIds.forEach((id) => {
      const entry = entries.value.find((e) => e.id === id)
      if (entry) {
        dragOriginalTimes.set(id, {
          startTime: { ...entry.startTime },
          endTime: { ...entry.endTime },
        })
      }
    })
  }

  // 结束拖动时调用，记录历史
  const endDragging = () => {
    const changedEntries: Array<{
      entryId: number
      before: Partial<SubtitleEntry>
      after: Partial<SubtitleEntry>
    }> = []

    dragOriginalTimes.forEach((original, entryId) => {
      const entry = entries.value.find((e) => e.id === entryId)
      if (!entry) return

      const before: Partial<SubtitleEntry> = {}
      const after: Partial<SubtitleEntry> = {}
      let hasChanges = false

      const oldStartMs = timeStampToMs(original.startTime)
      const newStartMs = timeStampToMs(entry.startTime)
      if (oldStartMs !== newStartMs) {
        before.startTime = { ...original.startTime }
        after.startTime = { ...entry.startTime }
        hasChanges = true
      }

      const oldEndMs = timeStampToMs(original.endTime)
      const newEndMs = timeStampToMs(entry.endTime)
      if (oldEndMs !== newEndMs) {
        before.endTime = { ...original.endTime }
        after.endTime = { ...entry.endTime }
        hasChanges = true
      }

      if (hasChanges) {
        changedEntries.push({ entryId, before, after })
      }
    })

    if (changedEntries.length === 1) {
      const changed = changedEntries[0]!
      addHistory({
        type: HistoryActionType.TIME_EDIT,
        timestamp: Date.now(),
        entryId: changed.entryId,
        before: changed.before,
        after: changed.after,
      })
    } else if (changedEntries.length > 1) {
      changedEntries.forEach(({ entryId, before, after }) => {
        addHistory({
          type: HistoryActionType.TIME_EDIT,
          timestamp: Date.now(),
          entryId,
          before,
          after,
        })
      })
    }

    dragOriginalTimes.clear()
  }

  // 编辑时间
  const updateEntryTime = (
    entryId: number,
    startTime?: TimeStamp,
    endTime?: TimeStamp,
    recordHistory: boolean = false,
  ) => {
    const entry = entries.value.find((e) => e.id === entryId)
    if (!entry) return

    const before: Partial<SubtitleEntry> = {}
    const after: Partial<SubtitleEntry> = {}
    let hasChanges = false

    if (startTime) {
      const oldStartMs = timeStampToMs(entry.startTime)
      const newStartMs = timeStampToMs(startTime)
      if (oldStartMs !== newStartMs) {
        before.startTime = { ...entry.startTime }
        after.startTime = { ...startTime }
        entry.startTime = { ...startTime }
        hasChanges = true
      }
    }

    if (endTime) {
      const oldEndMs = timeStampToMs(entry.endTime)
      const newEndMs = timeStampToMs(endTime)
      if (oldEndMs !== newEndMs) {
        before.endTime = { ...entry.endTime }
        after.endTime = { ...endTime }
        entry.endTime = { ...endTime }
        hasChanges = true
      }
    }

    if (recordHistory && hasChanges) {
      addHistory({
        type: HistoryActionType.TIME_EDIT,
        timestamp: Date.now(),
        entryId,
        before,
        after,
      })
    }

    detectTimeConflicts()
    assignSubtitleToTracks()
  }

  // 删除字幕
  const deleteEntry = (entryId: number) => {
    const currentEntries = entries.value
    const index = currentEntries.findIndex((e) => e.id === entryId)
    if (index === -1) return

    const entry = currentEntries[index]

    logger.info('删除字幕', { entryId, text: entry?.text?.substring(0, 30) })

    addHistory({
      type: HistoryActionType.DELETE,
      timestamp: Date.now(),
      entryId,
      before: { ...entry },
      after: {},
    })

    const currentSelectedIndex = currentEntries.findIndex((e) => e.id === currentEntryId.value)

    currentEntries.splice(index, 1)

    // 重新编号
    currentEntries.forEach((e, i) => {
      e.id = i + 1
    })

    if (currentEntryId.value === entryId) {
      const newEntry = currentEntries[index] || currentEntries[index - 1]
      currentEntryId.value = newEntry?.id || null
    } else if (currentSelectedIndex !== -1) {
      const newIndex = currentSelectedIndex > index ? currentSelectedIndex - 1 : currentSelectedIndex
      currentEntryId.value = currentEntries[newIndex]?.id || null
    }

    detectTimeConflicts()
    assignSubtitleToTracks()
  }

  // 批量删除字幕（不记录历史，不支持撤销）
  const deleteEntries = (entryIds: number[]) => {
    if (entryIds.length === 0) return

    const currentEntries = entries.value
    
    // 按 ID 降序排序，从后往前删除避免索引问题
    const sortedIds = [...entryIds].sort((a, b) => b - a)
    
    // 记录删除前的选中状态
    const currentSelectedIndex = currentEntries.findIndex((e) => e.id === currentEntryId.value)
    const wasSelectedDeleted = entryIds.includes(currentEntryId.value ?? -1)
    
    // 找到最小的被删除索引，用于后续选择
    const minDeletedIndex = Math.min(
      ...entryIds.map(id => currentEntries.findIndex(e => e.id === id)).filter(i => i !== -1)
    )

    logger.info('批量删除字幕', { count: entryIds.length, ids: entryIds })

    // 删除字幕
    sortedIds.forEach(entryId => {
      const index = currentEntries.findIndex((e) => e.id === entryId)
      if (index !== -1) {
        currentEntries.splice(index, 1)
      }
    })

    // 重新编号
    currentEntries.forEach((e, i) => {
      e.id = i + 1
    })

    // 更新选中状态
    if (wasSelectedDeleted) {
      // 选择被删除位置附近的字幕
      const newEntry = currentEntries[minDeletedIndex] || currentEntries[minDeletedIndex - 1] || currentEntries[0]
      currentEntryId.value = newEntry?.id || null
    } else if (currentSelectedIndex !== -1) {
      // 重新计算当前选中字幕的新索引
      const deletedBeforeCurrent = entryIds.filter(id => {
        const idx = entries.value.findIndex(e => e.id === id)
        return idx !== -1 && idx < currentSelectedIndex
      }).length
      const newIndex = currentSelectedIndex - deletedBeforeCurrent
      currentEntryId.value = currentEntries[newIndex]?.id || null
    }

    detectTimeConflicts()
    assignSubtitleToTracks()
  }


  // 分割字幕
  const splitEntry = (entryId: number, splitTimeMs: number) => {
    const currentEntries = entries.value
    const entry = currentEntries.find((e) => e.id === entryId)
    if (!entry) return null

    const startMs = timeStampToMs(entry.startTime)
    const endMs = timeStampToMs(entry.endTime)

    if (splitTimeMs <= startMs || splitTimeMs >= endMs) {
      return null
    }

    const msToTimeStamp = (ms: number): TimeStamp => {
      const totalSeconds = Math.floor(ms / 1000)
      return {
        hours: Math.floor(totalSeconds / 3600),
        minutes: Math.floor((totalSeconds % 3600) / 60),
        seconds: totalSeconds % 60,
        milliseconds: ms % 1000,
      }
    }

    const originalEntry = { ...entry }
    entry.endTime = msToTimeStamp(splitTimeMs)

    const index = currentEntries.findIndex((e) => e.id === entryId)
    const newEntry: SubtitleEntry = {
      id: entryId + 1,
      startTime: msToTimeStamp(splitTimeMs),
      endTime: msToTimeStamp(endMs),
      text: entry.text,
    }

    currentEntries.splice(index + 1, 0, newEntry)

    currentEntries.forEach((e, i) => {
      e.id = i + 1
    })

    const newId = index + 2
    const newEntryData = currentEntries.find((e) => e.id === newId)

    addHistory({
      type: HistoryActionType.SPLIT,
      timestamp: Date.now(),
      entryId: index + 1,
      before: {
        ...originalEntry,
        startTime: { ...originalEntry.startTime },
        endTime: { ...originalEntry.endTime },
      },
      after: {
        ...entry,
        startTime: { ...entry.startTime },
        endTime: { ...entry.endTime },
      },
      newEntryId: newId,
      newEntry: newEntryData
        ? {
            ...newEntryData,
            startTime: { ...newEntryData.startTime },
            endTime: { ...newEntryData.endTime },
          }
        : undefined,
      description: `分割字幕 #${index + 1}`,
    })

    detectTimeConflicts()
    assignSubtitleToTracks()

    logger.info('分割字幕', { entryId: index + 1, newEntryId: newId })

    return newId
  }

  // 合并字幕
  const mergeEntries = (entryIds: number[]) => {
    if (entryIds.length < 2) return null

    const currentEntries = entries.value
    
    // 按 ID 排序，确保顺序正确
    const sortedIds = [...entryIds].sort((a, b) => a - b)
    
    // 获取要合并的字幕
    const entriesToMerge = sortedIds
      .map(id => currentEntries.find(e => e.id === id))
      .filter((e): e is SubtitleEntry => e !== undefined)
    
    if (entriesToMerge.length < 2) return null

    // 检查是否连续（ID 必须相邻）
    for (let i = 1; i < entriesToMerge.length; i++) {
      const prevIndex = currentEntries.findIndex(e => e.id === entriesToMerge[i - 1]!.id)
      const currIndex = currentEntries.findIndex(e => e.id === entriesToMerge[i]!.id)
      if (currIndex !== prevIndex + 1) {
        logger.warn('合并字幕失败：字幕不连续')
        return null
      }
    }

    // 保存原始数据用于撤销
    const mergedEntriesData = entriesToMerge.map(e => ({
      ...e,
      startTime: { ...e.startTime },
      endTime: { ...e.endTime },
    }))

    // 第一条字幕作为合并目标
    const firstEntry = entriesToMerge[0]!
    const lastEntry = entriesToMerge[entriesToMerge.length - 1]!
    const firstIndex = currentEntries.findIndex(e => e.id === firstEntry.id)

    // 合并后的数据
    const mergedText = entriesToMerge.map(e => e.text).join(' ')
    const mergedStartTime = { ...firstEntry.startTime }
    const mergedEndTime = { ...lastEntry.endTime }

    // 更新第一条字幕
    firstEntry.text = mergedText
    firstEntry.endTime = mergedEndTime

    // 删除其余字幕（从后往前删，避免索引问题）
    for (let i = entriesToMerge.length - 1; i > 0; i--) {
      const entryToRemove = entriesToMerge[i]!
      const removeIndex = currentEntries.findIndex(e => e.id === entryToRemove.id)
      if (removeIndex !== -1) {
        currentEntries.splice(removeIndex, 1)
      }
    }

    // 重新编号
    currentEntries.forEach((e, i) => {
      e.id = i + 1
    })

    const newId = firstIndex + 1

    // 记录历史
    addHistory({
      type: HistoryActionType.MERGE,
      timestamp: Date.now(),
      entryId: newId,
      before: mergedEntriesData[0]!,
      after: {
        id: newId,
        startTime: mergedStartTime,
        endTime: mergedEndTime,
        text: mergedText,
      },
      mergedEntries: mergedEntriesData,
      description: `合并 ${mergedEntriesData.length} 条字幕`,
    })

    detectTimeConflicts()
    assignSubtitleToTracks()

    logger.info('合并字幕', { entryIds: sortedIds, newEntryId: newId })

    return newId
  }

  // 新增字幕
  const addEntry = (afterId?: number) => {
    const configStore = useConfigStore()
    const DEFAULT_DURATION_MS = configStore.config.newSubtitleDuration * 1000
    const currentEntries = entries.value

    const msToTimeStamp = (ms: number): TimeStamp => {
      const totalSeconds = Math.floor(ms / 1000)
      return {
        hours: Math.floor(totalSeconds / 3600),
        minutes: Math.floor((totalSeconds % 3600) / 60),
        seconds: totalSeconds % 60,
        milliseconds: ms % 1000,
      }
    }

    let startTime: TimeStamp
    let endTime: TimeStamp

    if (afterId !== undefined) {
      const afterEntry = currentEntries.find((e) => e.id === afterId)

      if (afterEntry) {
        const newStartMs = timeStampToMs(afterEntry.endTime)
        const newEndMs = newStartMs + DEFAULT_DURATION_MS

        startTime = msToTimeStamp(newStartMs)
        endTime = msToTimeStamp(newEndMs)
      } else {
        startTime = { hours: 0, minutes: 0, seconds: 0, milliseconds: 0 }
        endTime = { hours: 0, minutes: 0, seconds: 3, milliseconds: 0 }
      }
    } else if (currentEntries.length > 0) {
      const lastEntry = currentEntries[currentEntries.length - 1]!
      const newStartMs = timeStampToMs(lastEntry.endTime)
      const newEndMs = newStartMs + DEFAULT_DURATION_MS

      startTime = msToTimeStamp(newStartMs)
      endTime = msToTimeStamp(newEndMs)
    } else {
      startTime = { hours: 0, minutes: 0, seconds: 0, milliseconds: 0 }
      endTime = { hours: 0, minutes: 0, seconds: 3, milliseconds: 0 }
    }

    const tempId = currentEntries.length > 0
      ? Math.max(...currentEntries.map((e) => e.id)) + 1
      : 1

    const newEntry: SubtitleEntry = {
      id: tempId,
      startTime,
      endTime,
      text: '',
    }

    let insertedIndex: number
    if (afterId !== undefined) {
      const index = currentEntries.findIndex((e) => e.id === afterId)
      currentEntries.splice(index + 1, 0, newEntry)
      insertedIndex = index + 1
    } else {
      currentEntries.push(newEntry)
      insertedIndex = currentEntries.length - 1
    }

    // 重新编号
    currentEntries.forEach((e, i) => {
      e.id = i + 1
    })

    // 新字幕的 ID 是插入位置 + 1（因为 ID 从 1 开始）
    const newId = insertedIndex + 1

    addHistory({
      type: HistoryActionType.ADD,
      timestamp: Date.now(),
      entryId: newId,
      before: {},
      after: { ...currentEntries[insertedIndex] },
    })

    currentEntryId.value = newId
    editingEntryId.value = newId

    assignSubtitleToTracks()

    return newId
  }

  // 移除标点符号
  const removePunctuationFromText = (text: string): string => {
    const configStore = useConfigStore()
    const punctuation = configStore.punctuationToRemove || ''
    logger.info('删除标点 - 配置', { punctuation, punctuationLength: punctuation.length })
    if (!punctuation) {
      logger.warn('删除标点 - 标点列表为空')
      return text
    }
    // 逐字符删除，避免正则转义问题
    let result = text
    for (const char of punctuation) {
      result = result.split(char).join('')
    }
    logger.info('删除标点 - 结果', { original: text, result })
    return result
  }

  const removePunctuationForEntry = (entryId: number) => {
    const entry = entries.value.find((e) => e.id === entryId)
    if (!entry) return

    const newText = removePunctuationFromText(entry.text)
    if (newText !== entry.text) {
      updateEntryText(entryId, newText)
    }
  }

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

  const addCJKSpacesToText = (text: string): string => {
    let result = text
    result = result.replace(/([\u4e00-\u9fa5])([A-Za-z0-9])/g, '$1 $2')
    result = result.replace(/([A-Za-z0-9])([\u4e00-\u9fa5])/g, '$1 $2')
    result = result.replace(/\s+/g, ' ')
    return result
  }

  const addSpacesForEntry = (entryId: number) => {
    const entry = entries.value.find((e) => e.id === entryId)
    if (!entry) return

    const newText = addCJKSpacesToText(entry.text)
    if (newText !== entry.text) {
      updateEntryText(entryId, newText)
    }
  }

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
    if (!tabManager.activeTab) return
    
    const tab = tabManager.activeTab
    // 清除当前位置之后的历史
    tab.subtitle.history = tab.subtitle.history.slice(0, tab.subtitle.historyIndex + 1)
    tab.subtitle.history.push(action)
    tab.subtitle.historyIndex++

    // 限制历史记录数量
    if (tab.subtitle.history.length > 100) {
      tab.subtitle.history.shift()
      tab.subtitle.historyIndex--
    }
  }

  // 撤销
  const undo = () => {
    if (!canUndo.value || !tabManager.activeTab) return

    const tab = tabManager.activeTab
    const action = tab.subtitle.history[tab.subtitle.historyIndex]
    if (!action) return

    const currentEntries = entries.value

    switch (action.type) {
      case HistoryActionType.TEXT_EDIT: {
        const entry = currentEntries.find((e) => e.id === action.entryId)
        if (entry && action.before.text !== undefined) {
          entry.text = action.before.text
        }
        break
      }

      case HistoryActionType.TIME_EDIT: {
        const entry = currentEntries.find((e) => e.id === action.entryId)
        if (entry) {
          if (action.before.startTime) {
            entry.startTime = { ...action.before.startTime }
          }
          if (action.before.endTime) {
            entry.endTime = { ...action.before.endTime }
          }
          detectTimeConflicts()
          assignSubtitleToTracks()
        }
        break
      }

      case HistoryActionType.SPLIT: {
        const entry = currentEntries.find((e) => e.id === action.entryId)
        if (entry && action.before.endTime) {
          entry.endTime = { ...action.before.endTime }
        }

        if (action.newEntryId !== undefined) {
          const newEntryIndex = currentEntries.findIndex((e) => e.id === action.newEntryId)
          if (newEntryIndex !== -1) {
            currentEntries.splice(newEntryIndex, 1)
            currentEntries.forEach((e, i) => {
              e.id = i + 1
            })
          }
        }

        detectTimeConflicts()
        assignSubtitleToTracks()
        break
      }

      case HistoryActionType.MERGE: {
        // 撤销合并：删除合并后的字幕，恢复所有原始字幕
        if (action.mergedEntries && action.mergedEntries.length > 0) {
          const mergedEntryIndex = currentEntries.findIndex((e) => e.id === action.entryId)
          if (mergedEntryIndex !== -1) {
            // 删除合并后的字幕
            currentEntries.splice(mergedEntryIndex, 1)
            
            // 恢复所有原始字幕
            const restoredEntries = action.mergedEntries.map(e => ({
              id: e.id!,
              startTime: { ...e.startTime! },
              endTime: { ...e.endTime! },
              text: e.text || '',
            }))
            
            currentEntries.splice(mergedEntryIndex, 0, ...restoredEntries)
            
            // 重新编号
            currentEntries.forEach((e, i) => {
              e.id = i + 1
            })
          }
        }

        detectTimeConflicts()
        assignSubtitleToTracks()
        break
      }
    }

    tab.subtitle.historyIndex--
  }

  // 重做
  const redo = () => {
    if (!canRedo.value || !tabManager.activeTab) return

    const tab = tabManager.activeTab
    tab.subtitle.historyIndex++
    const action = tab.subtitle.history[tab.subtitle.historyIndex]
    if (!action) return

    const currentEntries = entries.value

    switch (action.type) {
      case HistoryActionType.TEXT_EDIT: {
        const entry = currentEntries.find((e) => e.id === action.entryId)
        if (entry && action.after.text !== undefined) {
          entry.text = action.after.text
        }
        break
      }

      case HistoryActionType.TIME_EDIT: {
        const entry = currentEntries.find((e) => e.id === action.entryId)
        if (entry) {
          if (action.after.startTime) {
            entry.startTime = { ...action.after.startTime }
          }
          if (action.after.endTime) {
            entry.endTime = { ...action.after.endTime }
          }
          detectTimeConflicts()
          assignSubtitleToTracks()
        }
        break
      }

      case HistoryActionType.SPLIT: {
        const entry = currentEntries.find((e) => e.id === action.entryId)
        if (entry && action.after.endTime) {
          entry.endTime = { ...action.after.endTime }
        }

        if (action.newEntry && action.newEntryId !== undefined) {
          const insertIndex = currentEntries.findIndex((e) => e.id === action.entryId)
          if (insertIndex !== -1) {
            const newEntry: SubtitleEntry = {
              id: action.newEntryId,
              startTime: action.newEntry.startTime!,
              endTime: action.newEntry.endTime!,
              text: action.newEntry.text || '',
            }
            currentEntries.splice(insertIndex + 1, 0, newEntry)
            currentEntries.forEach((e, i) => {
              e.id = i + 1
            })
          }
        }

        detectTimeConflicts()
        assignSubtitleToTracks()
        break
      }

      case HistoryActionType.MERGE: {
        // 重做合并：删除原始字幕，创建合并后的字幕
        if (action.mergedEntries && action.mergedEntries.length > 0 && action.after) {
          const firstEntryIndex = currentEntries.findIndex((e) => e.id === action.entryId)
          if (firstEntryIndex !== -1) {
            // 删除所有原始字幕
            currentEntries.splice(firstEntryIndex, action.mergedEntries.length)
            
            // 插入合并后的字幕
            const mergedEntry: SubtitleEntry = {
              id: action.entryId,
              startTime: { ...action.after.startTime! },
              endTime: { ...action.after.endTime! },
              text: action.after.text || '',
            }
            currentEntries.splice(firstEntryIndex, 0, mergedEntry)
            
            // 重新编号
            currentEntries.forEach((e, i) => {
              e.id = i + 1
            })
          }
        }

        detectTimeConflicts()
        assignSubtitleToTracks()
        break
      }
    }
  }

  // 查找
  const search = (query: string) => {
    if (!tabManager.activeTab) return []
    
    const tab = tabManager.activeTab
    tab.subtitle.searchQuery = query
    tab.subtitle.searchResults = []
    tab.subtitle.currentSearchIndex = 0

    if (!query) return []

    entries.value.forEach((entry) => {
      if (entry.text.toLowerCase().includes(query.toLowerCase())) {
        tab.subtitle.searchResults.push(entry.id)
      }
    })

    return tab.subtitle.searchResults
  }

  // 格式化时间戳
  const formatTimeStamp = (time: TimeStamp): string => {
    const pad = (num: number, size: number) => num.toString().padStart(size, '0')
    return `${pad(time.hours, 2)}:${pad(time.minutes, 2)}:${pad(time.seconds, 2)},${pad(time.milliseconds, 3)}`
  }

  // 保存到文件
  const saveToFile = async () => {
    const filePath = currentFilePath.value
    if (!filePath) {
      throw new Error('No file loaded')
    }

    const { invoke } = await import('@tauri-apps/api/core')

    try {
      await invoke('write_srt', {
        filePath,
        entries: entries.value,
      })

      if (tabManager.activeTab) {
        tabManager.activeTab.subtitle.savedHistoryIndex = tabManager.activeTab.subtitle.historyIndex
      }
      logger.info('文件保存成功', { path: filePath, entries: entries.value.length })
    } catch (error) {
      logger.error('文件保存失败', { path: filePath, error: String(error) })
      throw error
    }
  }

  // 另存为文件
  const saveAsFile = async (newFilePath: string) => {
    if (!tabManager.activeTab) {
      throw new Error('No active tab')
    }

    const { invoke } = await import('@tauri-apps/api/core')

    try {
      await invoke('write_srt', {
        filePath: newFilePath,
        entries: entries.value,
      })

      // 更新当前 tab 的文件路径
      tabManager.activeTab.subtitle.filePath = newFilePath
      tabManager.activeTab.subtitle.savedHistoryIndex = tabManager.activeTab.subtitle.historyIndex
      
      // 更新 tab 标题
      const fileName = newFilePath.split('/').pop() || newFilePath.split('\\').pop() || 'Untitled'
      tabManager.activeTab.title = fileName

      logger.info('文件另存为成功', { path: newFilePath, entries: entries.value.length })
    } catch (error) {
      logger.error('文件另存为失败', { path: newFilePath, error: String(error) })
      throw error
    }
  }

  // 跳转搜索结果
  const nextSearchResult = () => {
    if (!tabManager.activeTab) return
    const tab = tabManager.activeTab
    if (tab.subtitle.searchResults.length === 0) return

    tab.subtitle.currentSearchIndex =
      (tab.subtitle.currentSearchIndex + 1) % tab.subtitle.searchResults.length
    currentEntryId.value = tab.subtitle.searchResults[tab.subtitle.currentSearchIndex] ?? null
  }

  const prevSearchResult = () => {
    if (!tabManager.activeTab) return
    const tab = tabManager.activeTab
    if (tab.subtitle.searchResults.length === 0) return

    tab.subtitle.currentSearchIndex =
      (tab.subtitle.currentSearchIndex - 1 + tab.subtitle.searchResults.length) %
      tab.subtitle.searchResults.length
    currentEntryId.value = tab.subtitle.searchResults[tab.subtitle.currentSearchIndex] ?? null
  }

  // ============ 二次校正标记相关 ============
  
  // 切换单条字幕的校正标记
  const toggleCorrectionMark = (entryId: number) => {
    const entry = entries.value.find((e) => e.id === entryId)
    if (entry) {
      entry.needsCorrection = !entry.needsCorrection
      logger.info('切换校正标记', { entryId, needsCorrection: entry.needsCorrection })
    }
  }

  // 设置单条字幕的校正标记
  const setCorrectionMark = (entryId: number, needsCorrection: boolean) => {
    const entry = entries.value.find((e) => e.id === entryId)
    if (entry) {
      entry.needsCorrection = needsCorrection
    }
  }

  // 批量设置校正标记
  const setCorrectionMarks = (entryIds: number[], needsCorrection: boolean) => {
    entryIds.forEach(id => {
      const entry = entries.value.find((e) => e.id === id)
      if (entry) {
        entry.needsCorrection = needsCorrection
      }
    })
    logger.info('批量设置校正标记', { count: entryIds.length, needsCorrection })
  }

  // 清除所有校正标记
  const clearAllCorrectionMarks = () => {
    entries.value.forEach(entry => {
      entry.needsCorrection = false
      entry.correctionSuggestion = undefined
    })
    logger.info('清除所有校正标记')
  }

  // 获取需要校正的字幕列表
  const getEntriesNeedingCorrection = () => {
    return entries.value.filter(e => e.needsCorrection)
  }

  // 设置校正建议（同时标记为需要校正）
  const setCorrectionSuggestion = (entryId: number, suggestion: string) => {
    const entry = entries.value.find((e) => e.id === entryId)
    if (entry) {
      entry.correctionSuggestion = suggestion
      entry.needsCorrection = true
      logger.info('设置校正建议', { entryId, suggestion: suggestion.substring(0, 30) })
    }
  }

  // 应用校正建议
  const applyCorrectionSuggestion = (entryId: number) => {
    const entry = entries.value.find((e) => e.id === entryId)
    if (entry && entry.correctionSuggestion) {
      const oldText = entry.text
      entry.text = entry.correctionSuggestion
      entry.correctionSuggestion = undefined
      entry.needsCorrection = false
      
      // 记录历史
      addHistory({
        type: HistoryActionType.TEXT_EDIT,
        timestamp: Date.now(),
        entryId,
        before: { text: oldText },
        after: { text: entry.text },
      })
      
      logger.info('应用校正建议', { entryId })
    }
  }

  // 忽略校正建议
  const dismissCorrectionSuggestion = (entryId: number) => {
    const entry = entries.value.find((e) => e.id === entryId)
    if (entry) {
      entry.correctionSuggestion = undefined
      entry.needsCorrection = false
      logger.info('忽略校正建议', { entryId })
    }
  }

  // 需要校正的字幕数量
  const needsCorrectionCount = computed(() => {
    return entries.value.filter(e => e.needsCorrection).length
  })

  return {
    // 状态
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
    needsCorrectionCount,

    // 方法
    loadSRTFile,
    getCurrentEntryByTime,
    updateEntryText,
    updateEntryTime,
    startDragging,
    endDragging,
    deleteEntry,
    deleteEntries,
    splitEntry,
    mergeEntries,
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
    saveAsFile,
    // 校正标记相关
    toggleCorrectionMark,
    setCorrectionMark,
    setCorrectionMarks,
    clearAllCorrectionMarks,
    getEntriesNeedingCorrection,
    setCorrectionSuggestion,
    applyCorrectionSuggestion,
    dismissCorrectionSuggestion,
  }
})

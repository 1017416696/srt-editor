<script setup lang="ts">
import { ref, computed, onMounted, onBeforeUnmount, watch, nextTick } from 'vue'
import { useRouter } from 'vue-router'
import { open } from '@tauri-apps/plugin-dialog'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { useSubtitleStore } from '@/stores/subtitle'
import { useAudioStore } from '@/stores/audio'
import { useConfigStore } from '@/stores/config'
import { useTabManagerStore } from '@/stores/tabManager'
import { timeStampToMs } from '@/utils/time'
import { findVoiceRegion, timestampToMs, msToTimestamp } from '@/utils/waveformAlign'
import type { SRTFile, AudioFile, TimeStamp } from '@/types/subtitle'
import WaveformViewer from '@/components/WaveformViewer.vue'
import SettingsDialog from '@/components/SettingsDialog.vue'
import TitleBar from '@/components/TitleBar.vue'
import { EditorSidebar, AudioEmptyState, TimelineControls, SubtitleListPanel, SubtitleEditPanel } from '@/components/editor'
import { ElMessage, ElMessageBox } from 'element-plus'

const router = useRouter()
const subtitleStore = useSubtitleStore()
const audioStore = useAudioStore()
const configStore = useConfigStore()
const tabManager = useTabManagerStore()

// UI 状态
const searchText = ref('')
const replaceText = ref('')
const showReplace = ref(false)
const selectedEntryId = ref<number | null>(null)
const isUserEditing = ref(false)
const isUserSelectingEntry = ref(false)
const isScissorMode = ref(false)
const isSnapEnabled = ref(false)
const isAltPressed = ref(false)
const selectedSubtitleIds = ref<number[]>([])
const showSearchPanel = ref(false)
const showSettingsDialog = ref(false)

let autoSaveTimer: ReturnType<typeof setTimeout> | null = null
let userSelectionTimer: ReturnType<typeof setTimeout> | null = null
let isSaving = false
let unlistenOpenFile: (() => void) | null = null

// 组件 refs
const waveformViewerRef = ref<InstanceType<typeof WaveformViewer> | null>(null)
const subtitleListPanelRef = ref<InstanceType<typeof SubtitleListPanel> | null>(null)
const subtitleEditPanelRef = ref<InstanceType<typeof SubtitleEditPanel> | null>(null)

// 计算属性
const hasContent = computed(() => subtitleStore.entries.length > 0)
const hasAudio = computed(() => audioStore.currentAudio !== null)

// 当前选中的字幕
const currentEntry = computed(() => {
  if (!selectedEntryId.value) return null
  return subtitleStore.entries.find((e) => e.id === selectedEntryId.value) || null
})

// 计算当前缩放百分比
const waveformZoomLevel = computed(() => {
  return waveformViewerRef.value ? Math.round(waveformViewerRef.value.zoomLevel * 100) : 100
})

// 监听 tab 切换
watch(() => tabManager.activeTabId, () => {
  if (subtitleStore.entries.length > 0) {
    selectedEntryId.value = subtitleStore.entries[0]?.id ?? null
  } else {
    selectedEntryId.value = null
  }
  searchText.value = ''
  showSearchPanel.value = false
})

// 监听搜索文本变化
watch(searchText, (query) => {
  subtitleStore.search(query)
  if (subtitleStore.searchResults.length > 0) {
    selectedEntryId.value = subtitleStore.searchResults[0] ?? null
  }
})

// 监听音频播放进度
watch(() => audioStore.playerState.currentTime, (currentTime) => {
  if (hasAudio.value && !isUserSelectingEntry.value) {
    const entry = subtitleStore.getCurrentEntryByTime(currentTime)
    if (entry && selectedEntryId.value !== entry.id) {
      selectedEntryId.value = entry.id
      nextTick(() => {
        subtitleListPanelRef.value?.scrollToEntry(entry.id)
      })
    }
  }
})

// 切换搜索面板
const toggleSearchPanel = () => {
  showSearchPanel.value = !showSearchPanel.value
  if (showSearchPanel.value) {
    nextTick(() => {
      subtitleListPanelRef.value?.focusSearch()
    })
  } else {
    searchText.value = ''
    replaceText.value = ''
    showReplace.value = false
  }
}

// 打开 SRT 文件
const handleOpenFile = async () => {
  try {
    const selected = await open({
      multiple: false,
      filters: [{ name: 'SRT 字幕文件', extensions: ['srt'] }],
    })
    if (selected) {
      const srtFile = await invoke<SRTFile>('read_srt', { filePath: selected })
      await subtitleStore.loadSRTFile(srtFile)
      configStore.addRecentFile(selected as string)
      if ((window as any).__updateRecentFilesMenu) {
        await (window as any).__updateRecentFilesMenu()
      }
      if (subtitleStore.entries.length > 0) {
        selectedEntryId.value = subtitleStore.entries[0]?.id ?? null
      }
    }
  } catch (error) {
    // 静默处理
  }
}

// 打开音频文件
const handleOpenAudio = async () => {
  try {
    const selected = await open({
      multiple: false,
      filters: [{ name: '音频文件', extensions: ['mp3', 'wav', 'ogg', 'flac', 'm4a', 'aac'] }],
    })
    if (selected && typeof selected === 'string') {
      const fileName = selected.split('/').pop() || 'audio'
      const fileExtension = selected.split('.').pop()?.toLowerCase() || 'mp3'
      const audioFile: AudioFile = { name: fileName, path: selected, duration: 0, format: fileExtension }
      await audioStore.loadAudio(audioFile)
    }
  } catch (error) {
    // 静默处理
  }
}

// 删除音频文件
const handleRemoveAudio = () => {
  if (hasAudio.value) audioStore.unloadAudio()
}

// 保存文件
const handleSave = async () => {
  if (isSaving || !subtitleStore.currentFilePath) return
  isSaving = true
  try {
    await subtitleStore.saveToFile()
  } finally {
    setTimeout(() => { isSaving = false }, 100)
  }
}

// 选择字幕
const selectEntry = (id: number) => {
  selectedEntryId.value = id
  isUserSelectingEntry.value = true
  if (userSelectionTimer) clearTimeout(userSelectionTimer)
  userSelectionTimer = setTimeout(() => {
    isUserSelectingEntry.value = false
    userSelectionTimer = null
  }, 300)

  if (hasAudio.value) {
    const entry = subtitleStore.entries.find((e) => e.id === id)
    if (entry) {
      const timeMs = timeStampToMs(entry.startTime)
      audioStore.seek(timeMs / 1000)
    }
  }
}

// 添加字幕
const openSubtitle = async () => {
  if (audioStore.playerState.isPlaying) audioStore.pause()
  const afterId = selectedEntryId.value ?? undefined
  const newId = subtitleStore.addEntry(afterId)
  selectedEntryId.value = newId
  if (subtitleStore.currentFilePath) {
    try { await subtitleStore.saveToFile() } catch {}
  }
}

// 删除字幕
const handleDeleteEntry = async () => {
  if (!currentEntry.value) return
  const currentId = currentEntry.value.id
  try {
    await ElMessageBox.confirm(`删除后无法恢复，确定删除字幕 #${currentId} 吗？`, '删除确认', {
      confirmButtonText: '删除', cancelButtonText: '取消', type: 'warning',
    })
    if (audioStore.playerState.isPlaying) audioStore.pause()
    const currentIndex = subtitleStore.entries.findIndex((e) => e.id === currentId)
    subtitleStore.deleteEntry(currentId)
    if (subtitleStore.currentFilePath) await subtitleStore.saveToFile()
    if (subtitleStore.entries.length > 0) {
      const nextEntry = subtitleStore.entries[currentIndex] || subtitleStore.entries[currentIndex - 1]
      if (nextEntry) selectedEntryId.value = nextEntry.id
    } else {
      selectedEntryId.value = null
    }
    ElMessage.success({ message: '已删除', duration: 1500 })
  } catch {}
}

// 复制字幕文本
let lastCopyTime = 0
const copySubtitleText = async (id: number) => {
  const now = Date.now()
  if (now - lastCopyTime < 300) return
  lastCopyTime = now
  const entry = subtitleStore.entries.find((e) => e.id === id)
  if (!entry) return
  try {
    await navigator.clipboard.writeText(entry.text)
    ElMessage.success({ message: '已复制', duration: 1500 })
  } catch {
    ElMessage.error({ message: '复制失败', duration: 1500 })
  }
}

// 播放字幕音频
const playSubtitleAudio = (id: number) => {
  if (!hasAudio.value) return
  const entry = subtitleStore.entries.find((e) => e.id === id)
  if (!entry) return
  const timeMs = timeStampToMs(entry.startTime)
  audioStore.seek(timeMs / 1000)
  audioStore.play()
}

// 删除字幕项目
const deleteSubtitleItem = async (id: number) => {
  const entry = subtitleStore.entries.find((e) => e.id === id)
  if (!entry) return
  try {
    await ElMessageBox.confirm(`删除后无法恢复，确定删除字幕 #${id} 吗？`, '删除确认', {
      confirmButtonText: '删除', cancelButtonText: '取消', type: 'warning',
    })
    if (audioStore.playerState.isPlaying) audioStore.pause()
    const currentIndex = subtitleStore.entries.findIndex((e) => e.id === id)
    subtitleStore.deleteEntry(id)
    if (subtitleStore.currentFilePath) await subtitleStore.saveToFile()
    if (selectedEntryId.value === id) {
      if (subtitleStore.entries.length > 0) {
        const nextEntry = subtitleStore.entries[currentIndex] || subtitleStore.entries[currentIndex - 1]
        if (nextEntry) selectedEntryId.value = nextEntry.id
      } else {
        selectedEntryId.value = null
      }
    }
    ElMessage.success({ message: '已删除', duration: 1500 })
  } catch (error) {
    if (error instanceof Error && error.message !== 'cancel') {
      ElMessage.error({ message: '删除失败', duration: 1500 })
    }
  }
}

// 替换功能
const replaceAll = async () => {
  if (!searchText.value) return
  try {
    if (audioStore.playerState.isPlaying) audioStore.pause()
    let modifiedCount = 0
    subtitleStore.entries.forEach((entry) => {
      const newText = entry.text.replaceAll(searchText.value, replaceText.value)
      if (newText !== entry.text) {
        subtitleStore.updateEntryText(entry.id, newText)
        modifiedCount++
      }
    })
    if (modifiedCount > 0) await subtitleStore.saveToFile()
  } catch {}
}

const replaceOne = async () => {
  if (!currentEntry.value || !subtitleStore.searchResults.includes(currentEntry.value.id)) return
  const entry = currentEntry.value
  try {
    if (audioStore.playerState.isPlaying) audioStore.pause()
    const newText = entry.text.replaceAll(searchText.value, replaceText.value)
    if (newText !== entry.text) {
      subtitleStore.updateEntryText(entry.id, newText)
      await subtitleStore.saveToFile()
      const currentIndex = subtitleStore.searchResults.indexOf(entry.id)
      if (currentIndex !== -1 && currentIndex < subtitleStore.searchResults.length - 1) {
        const nextId = subtitleStore.searchResults[currentIndex + 1]
        selectedEntryId.value = nextId ?? null
      }
    }
  } catch {}
}

// 文本编辑相关
const autoSaveCurrentEntry = async () => {
  if (!currentEntry.value) return
  const editPanel = subtitleEditPanelRef.value
  if (!editPanel) return
  // 从编辑面板获取当前编辑的文本（通过 emit 传递）
}

const handleTextUpdate = (text: string) => {
  if (!currentEntry.value) return
  if (audioStore.playerState.isPlaying) audioStore.pause()
  subtitleStore.updateEntryText(currentEntry.value.id, text)
  // 设置防抖保存
  if (autoSaveTimer) clearTimeout(autoSaveTimer)
  autoSaveTimer = setTimeout(async () => {
    if (subtitleStore.currentFilePath) {
      try { await subtitleStore.saveToFile() } catch {}
    }
    autoSaveTimer = null
  }, 1500)
}

const handleTextBlur = async () => {
  isUserEditing.value = false
  if (autoSaveTimer) {
    clearTimeout(autoSaveTimer)
    autoSaveTimer = null
  }
  if (subtitleStore.currentFilePath) {
    try { await subtitleStore.saveToFile() } catch {}
  }
}

const handleTextFocus = () => {
  isUserEditing.value = true
}

const handleTextInput = () => {
  if (audioStore.playerState.isPlaying) audioStore.pause()
}

// 时间编辑相关
const handleTimeUpdate = async (type: 'start' | 'end', value: string) => {
  if (!currentEntry.value) return
  const timeRegex = /^(\d{2}):(\d{2}):(\d{2}),(\d{3})$/
  if (!timeRegex.test(value)) {
    ElMessage.warning({ message: '时间格式不正确，应为 HH:MM:SS,mmm', duration: 2000 })
    return
  }
  const match = value.match(timeRegex)!
  const newTime: TimeStamp = {
    hours: parseInt(match[1] || '0'),
    minutes: parseInt(match[2] || '0'),
    seconds: parseInt(match[3] || '0'),
    milliseconds: parseInt(match[4] || '0')
  }
  if (audioStore.playerState.isPlaying) audioStore.pause()
  if (type === 'start') {
    subtitleStore.updateEntryTime(currentEntry.value.id, newTime, undefined, true)
  } else {
    subtitleStore.updateEntryTime(currentEntry.value.id, undefined, newTime, true)
  }
  if (subtitleStore.currentFilePath) await subtitleStore.saveToFile()
}

const handleAdjustTime = async (type: 'start' | 'end', deltaMs: number) => {
  if (!currentEntry.value) return
  if (audioStore.playerState.isPlaying) audioStore.pause()
  const currentTime = type === 'start' ? currentEntry.value.startTime : currentEntry.value.endTime
  let totalMs = timeStampToMs(currentTime) + deltaMs
  if (totalMs < 0) totalMs = 0
  const newTime: TimeStamp = {
    hours: Math.floor(totalMs / 3600000),
    minutes: Math.floor((totalMs % 3600000) / 60000),
    seconds: Math.floor((totalMs % 60000) / 1000),
    milliseconds: totalMs % 1000
  }
  if (type === 'start') {
    subtitleStore.updateEntryTime(currentEntry.value.id, newTime, undefined, true)
  } else {
    subtitleStore.updateEntryTime(currentEntry.value.id, undefined, newTime, true)
  }
  if (subtitleStore.currentFilePath) await subtitleStore.saveToFile()
}

// 移动字幕位置
const moveSubtitlePosition = async (deltaMs: number) => {
  if (!currentEntry.value) return
  if (audioStore.playerState.isPlaying) audioStore.pause()
  let startMs = timeStampToMs(currentEntry.value.startTime) + deltaMs
  let endMs = timeStampToMs(currentEntry.value.endTime) + deltaMs
  if (startMs < 0) {
    const offset = -startMs
    startMs = 0
    endMs += offset
  }
  const msToTimeStamp = (ms: number): TimeStamp => ({
    hours: Math.floor(ms / 3600000),
    minutes: Math.floor((ms % 3600000) / 60000),
    seconds: Math.floor((ms % 60000) / 1000),
    milliseconds: ms % 1000
  })
  subtitleStore.updateEntryTime(currentEntry.value.id, msToTimeStamp(startMs), msToTimeStamp(endMs), true)
  if (subtitleStore.currentFilePath) await subtitleStore.saveToFile()
}

// 快捷操作
const handleRemoveHTML = () => {
  if (audioStore.playerState.isPlaying) audioStore.pause()
  subtitleStore.removeHTMLTags()
}

const handleAddCJKSpaces = () => {
  if (!currentEntry.value) return
  if (audioStore.playerState.isPlaying) audioStore.pause()
  subtitleStore.addSpacesForEntry(currentEntry.value.id)
}

const handleRemovePunctuation = () => {
  if (!currentEntry.value) return
  if (audioStore.playerState.isPlaying) audioStore.pause()
  subtitleStore.removePunctuationForEntry(currentEntry.value.id)
}

// 波形相关
const handleWaveformSeek = (time: number) => {
  audioStore.seek(time)
}

let saveDebounceTimer: ReturnType<typeof setTimeout> | null = null
const debouncedSave = () => {
  if (saveDebounceTimer) clearTimeout(saveDebounceTimer)
  saveDebounceTimer = setTimeout(() => {
    if (subtitleStore.currentFilePath) {
      subtitleStore.saveToFile().catch(() => {})
    }
  }, 500)
}

const handleSubtitleUpdate = (id: number, startTime: TimeStamp, endTime: TimeStamp) => {
  const entry = subtitleStore.entries.find((e) => e.id === id)
  if (!entry) return
  if (audioStore.playerState.isPlaying) audioStore.pause()
  subtitleStore.updateEntryTime(id, startTime, endTime)
  debouncedSave()
}

const handleSubtitlesUpdate = (updates: Array<{ id: number; startTime: TimeStamp; endTime: TimeStamp }>) => {
  if (audioStore.playerState.isPlaying) audioStore.pause()
  updates.forEach(({ id, startTime, endTime }) => {
    subtitleStore.updateEntryTime(id, startTime, endTime)
  })
  debouncedSave()
}

const handleSubtitlesSelect = (ids: number[]) => {
  selectedSubtitleIds.value = ids
  if (ids.length === 1) selectedEntryId.value = ids[0] ?? null
}

const handleDragStart = (ids: number[]) => {
  subtitleStore.startDragging(ids)
}

const handleDragEnd = () => {
  subtitleStore.endDragging()
}

const handleWaveformDoubleClick = async (id: number) => {
  selectEntry(id)
  await subtitleEditPanelRef.value?.focusTextarea()
}

const handleSubtitleDoubleClick = async (id: number) => {
  await subtitleEditPanelRef.value?.focusTextarea()
}

const handleSplitSubtitle = async (id: number, splitTimeMs: number) => {
  if (audioStore.playerState.isPlaying) audioStore.pause()
  const newId = subtitleStore.splitEntry(id, splitTimeMs)
  if (newId) {
    selectedEntryId.value = newId
    if (subtitleStore.currentFilePath) {
      try { await subtitleStore.saveToFile() } catch {}
    }
  }
  isScissorMode.value = false
}

// 批量删除选中的字幕（时间轴多选删除）
const handleDeleteSelectedSubtitles = async (ids: number[]) => {
  if (ids.length === 0) return
  
  const confirmMessage = ids.length === 1
    ? `删除后无法恢复，确定删除字幕 #${ids[0]} 吗？`
    : `删除后无法恢复，确定删除选中的 ${ids.length} 条字幕吗？`
  
  try {
    await ElMessageBox.confirm(confirmMessage, '删除确认', {
      confirmButtonText: '删除',
      cancelButtonText: '取消',
      type: 'warning',
    })
    
    if (audioStore.playerState.isPlaying) audioStore.pause()
    
    // 使用批量删除方法
    subtitleStore.deleteEntries(ids)
    
    // 清除时间轴选择
    selectedSubtitleIds.value = []
    waveformViewerRef.value?.clearSelection()
    
    // 保存文件
    if (subtitleStore.currentFilePath) {
      try { await subtitleStore.saveToFile() } catch {}
    }
    
    ElMessage.success({ message: `已删除 ${ids.length} 条字幕`, duration: 1500 })
  } catch (error) {
    // 用户取消或其他错误，静默处理
  }
}

// 侧边栏操作
const handleScissor = () => {
  isScissorMode.value = !isScissorMode.value
}

const handleMergeSubtitles = async () => {
  if (selectedSubtitleIds.value.length < 2) return
  if (audioStore.playerState.isPlaying) audioStore.pause()
  if (autoSaveTimer) {
    clearTimeout(autoSaveTimer)
    autoSaveTimer = null
  }
  const newId = subtitleStore.mergeEntries(selectedSubtitleIds.value)
  if (newId) {
    selectedEntryId.value = newId
    selectedSubtitleIds.value = [newId]
    if (subtitleStore.currentFilePath) await subtitleStore.saveToFile()
  }
}

const handleAlignToWaveform = async () => {
  if (!hasAudio.value) {
    ElMessage.warning('请先加载音频文件')
    return
  }
  if (!currentEntry.value) {
    ElMessage.warning('请先选择一条字幕')
    return
  }
  const waveformData = audioStore.audioFile?.waveform
  if (!waveformData || waveformData.length === 0) {
    ElMessage.warning('波形数据未加载')
    return
  }
  if (audioStore.playerState.isPlaying) audioStore.pause()
  const entry = currentEntry.value
  const currentStartMs = timestampToMs(entry.startTime)
  const currentEndMs = timestampToMs(entry.endTime)
  const duration = audioStore.playerState.duration
  const voiceRegion = findVoiceRegion(waveformData, duration, currentStartMs, currentEndMs, 2000)
  if (!voiceRegion) {
    ElMessage.warning('未找到附近的语音区域')
    return
  }
  const newStartTime = msToTimestamp(voiceRegion.startMs)
  const newEndTime = msToTimestamp(voiceRegion.endMs)
  subtitleStore.updateEntryTime(entry.id, newStartTime, newEndTime, true)
  if (subtitleStore.currentFilePath) await subtitleStore.saveToFile()
}

// 缩放控制
const handleZoomChange = (value: number) => {
  waveformViewerRef.value?.setZoom(value / 100)
}

const handleZoomReset = () => {
  waveformViewerRef.value?.fitToWidth()
}

// 返回欢迎页
const goBack = async () => {
  if (audioStore.currentAudio) audioStore.unloadAudio()
  subtitleStore.$reset()
  searchText.value = ''
  replaceText.value = ''
  showReplace.value = false
  selectedEntryId.value = null
  router.push('/')
}

// 关闭搜索
const closeSearch = () => {
  searchText.value = ''
  replaceText.value = ''
  showReplace.value = false
  showSearchPanel.value = false
}

// 键盘快捷键相关
const getKeyModifier = (e: KeyboardEvent): string => {
  const isMac = /Mac|iPhone|iPad|iPod/.test(navigator.platform)
  if (isMac && e.metaKey) return 'Cmd+'
  if (e.ctrlKey) return 'Ctrl+'
  return ''
}

const normalizeKeyName = (key: string): string => {
  const keyMap: Record<string, string> = {
    o: 'o', O: 'o', s: 's', S: 's', z: 'z', Z: 'z', f: 'f', F: 'f', n: 'n', N: 'n',
    '=': '=', '+': '=', '-': '-', _: '-', '0': '0',
  }
  return keyMap[key] || key.toLowerCase()
}

const buildKeyString = (e: KeyboardEvent): string => {
  const modifier = getKeyModifier(e)
  let baseKey = normalizeKeyName(e.key)
  if (e.shiftKey && modifier) baseKey = `Shift+${baseKey}`
  return `${modifier}${baseKey}`
}

const handleAltKeyDown = (e: KeyboardEvent) => {
  if (e.key === 'Alt') isAltPressed.value = true
}

const handleAltKeyUp = (e: KeyboardEvent) => {
  if (e.key === 'Alt') isAltPressed.value = false
}

const handleKeydown = (e: KeyboardEvent) => {
  if (e.key === 'Escape' && isScissorMode.value) {
    e.preventDefault()
    isScissorMode.value = false
    return
  }

  const target = e.target as HTMLElement
  const isInTextInput = target instanceof HTMLTextAreaElement || target instanceof HTMLInputElement

  // Delete/Backspace 键删除时间轴选中的字幕
  if ((e.key === 'Delete' || e.key === 'Backspace') && !isInTextInput) {
    e.preventDefault()
    // 如果时间轴有多选字幕，删除选中的字幕
    if (selectedSubtitleIds.value.length > 0) {
      handleDeleteSelectedSubtitles(selectedSubtitleIds.value)
    } else if (currentEntry.value) {
      // 否则删除当前选中的单个字幕
      handleDeleteEntry()
    }
    return
  }

  const searchInput = subtitleListPanelRef.value?.getSearchInput()
  const replaceInput = subtitleListPanelRef.value?.getReplaceInput()
  const isInSearchInput = target === searchInput
  const isInReplaceInput = target === replaceInput

  const shortcuts = configStore.keyboardShortcuts
  const pressedKey = buildKeyString(e)

  if (isInTextInput) {
    if (shortcuts.save === pressedKey) { e.preventDefault(); handleSave() }
    else if (shortcuts.open === pressedKey) { e.preventDefault(); handleOpenFile() }
    else if (shortcuts.find === pressedKey) {
      e.preventDefault()
      showSearchPanel.value = true
      nextTick(() => subtitleListPanelRef.value?.focusSearch())
    }
    else if (e.key === 'Escape' && (isInSearchInput || isInReplaceInput)) {
      e.preventDefault()
      closeSearch()
      subtitleListPanelRef.value?.blurSearch()
    }
    else if ((e.key === 'ArrowDown' || e.key === 'ArrowUp') && isInSearchInput) {
      e.preventDefault()
      subtitleListPanelRef.value?.blurSearch()
      subtitleListPanelRef.value?.navigateSubtitleList(e.key === 'ArrowDown' ? 'down' : 'up')
    }
    return
  }

  // 全局快捷键
  if (shortcuts.save === pressedKey) { e.preventDefault(); handleSave() }
  else if (shortcuts.open === pressedKey) { e.preventDefault(); handleOpenFile() }
  else if (shortcuts.find === pressedKey) {
    e.preventDefault()
    showSearchPanel.value = true
    nextTick(() => subtitleListPanelRef.value?.focusSearch())
  }
  else if (pressedKey === 'Cmd+r' || pressedKey === 'Ctrl+r') {
    e.preventDefault()
    showSearchPanel.value = true
    showReplace.value = true
    nextTick(() => subtitleListPanelRef.value?.focusSearch())
  }
  else if (shortcuts.copy === pressedKey && currentEntry.value) {
    e.preventDefault()
    copySubtitleText(currentEntry.value.id)
  }
  else if (shortcuts.playPause === pressedKey.toLowerCase()) { e.preventDefault(); audioStore.togglePlay() }
  else if (shortcuts.addEntry === pressedKey) { e.preventDefault(); openSubtitle() }
  else if (shortcuts.deleteEntry === pressedKey) { e.preventDefault(); handleDeleteEntry() }
  else if (e.key === 'ArrowDown') { e.preventDefault(); subtitleListPanelRef.value?.navigateSubtitleList('down') }
  else if (e.key === 'ArrowUp') { e.preventDefault(); subtitleListPanelRef.value?.navigateSubtitleList('up') }
  else if (e.key === 'ArrowLeft' && currentEntry.value) { e.preventDefault(); moveSubtitlePosition(-100) }
  else if (e.key === 'ArrowRight' && currentEntry.value) { e.preventDefault(); moveSubtitlePosition(100) }
  else if ((e.key === 'x' || e.key === 'X') && hasAudio.value) { e.preventDefault(); handleScissor() }
  else if (e.key === 'm' || e.key === 'M') { e.preventDefault(); handleMergeSubtitles() }
  else if ((e.key === 's' || e.key === 'S') && hasAudio.value) { e.preventDefault(); isSnapEnabled.value = !isSnapEnabled.value }
  else if ((e.key === 'a' || e.key === 'A') && hasAudio.value) { e.preventDefault(); handleAlignToWaveform() }
  else if (pressedKey === 'Cmd+,' || pressedKey === 'Ctrl+,') { e.preventDefault(); showSettingsDialog.value = true }
  else if (shortcuts.undo === pressedKey) { e.preventDefault(); subtitleStore.undo() }
  else if (shortcuts.redo === pressedKey) { e.preventDefault(); subtitleStore.redo() }
  else if (shortcuts.zoomIn === pressedKey && hasAudio.value) { e.preventDefault(); waveformViewerRef.value?.zoomIn() }
  else if (shortcuts.zoomOut === pressedKey && hasAudio.value) { e.preventDefault(); waveformViewerRef.value?.zoomOut() }
  else if (shortcuts.zoomReset === pressedKey && hasAudio.value) { e.preventDefault(); handleZoomReset() }
}

// 生命周期
onMounted(async () => {
  if (!tabManager.hasTabs) {
    router.push('/')
    return
  }
  if (subtitleStore.entries.length > 0) {
    selectedEntryId.value = subtitleStore.entries[0]?.id ?? null
  }

  try {
    ;(window as any).__handleMenuOpenFile = async () => await handleOpenFile()
    ;(window as any).__handleMenuSave = async () => await handleSave()
    unlistenOpenFile = await listen<void>('menu:open-file', async () => await handleOpenFile())
    document.removeEventListener('keydown', handleKeydown, true)
    document.addEventListener('keydown', handleKeydown, true)
    document.addEventListener('keydown', handleAltKeyDown)
    document.addEventListener('keyup', handleAltKeyUp)
  } catch (error) {
    console.error('Error setting up menu handlers:', error)
  }
})

onBeforeUnmount(() => {
  if (unlistenOpenFile) { unlistenOpenFile(); unlistenOpenFile = null }
  ;(window as any).__handleMenuOpenFile = null
  ;(window as any).__handleMenuSave = null
  document.removeEventListener('keydown', handleKeydown, true)
  document.removeEventListener('keydown', handleAltKeyDown)
  document.removeEventListener('keyup', handleAltKeyUp)
})
</script>

<template>
  <div class="editor-page">
    <!-- 标题栏区域（含标签页） -->
    <TitleBar />

    <!-- 时间轴区域：顶部全宽 -->
    <div v-if="hasAudio || audioStore.isGeneratingWaveform" class="timeline-section">
      <TimelineControls
        :zoom-level="waveformZoomLevel"
        @zoom-change="handleZoomChange"
        @zoom-reset="handleZoomReset"
        @remove-audio="handleRemoveAudio"
      />

      <!-- 波形和字幕轨道 -->
      <WaveformViewer
        ref="waveformViewerRef"
        :waveform-data="audioStore.audioFile?.waveform"
        :current-time="audioStore.playerState.currentTime"
        :duration="audioStore.playerState.duration"
        :subtitles="subtitleStore.entries"
        :current-subtitle-id="selectedEntryId"
        :is-generating-waveform="audioStore.isGeneratingWaveform"
        :waveform-progress="audioStore.waveformProgress"
        :scissor-mode="isScissorMode"
        :snap-enabled="isSnapEnabled"
        @seek="handleWaveformSeek"
        @update-subtitle="handleSubtitleUpdate"
        @update-subtitles="handleSubtitlesUpdate"
        @select-subtitles="handleSubtitlesSelect"
        @double-click-subtitle="handleWaveformDoubleClick"
        @split-subtitle="handleSplitSubtitle"
        @drag-start="handleDragStart"
        @drag-end="handleDragEnd"
        @delete-selected-subtitles="handleDeleteSelectedSubtitles"
      />
    </div>

    <!-- 音频加载占位符 -->
    <AudioEmptyState v-else @open-audio="handleOpenAudio" />

    <!-- 主内容区：左右分栏 -->
    <div class="content-area">
      <!-- 左侧侧边栏 -->
      <EditorSidebar
        :has-audio="hasAudio"
        :has-current-entry="!!currentEntry"
        :is-scissor-mode="isScissorMode"
        :is-snap-enabled="isSnapEnabled"
        :is-alt-pressed="isAltPressed"
        :show-search-panel="showSearchPanel"
        :can-merge="selectedSubtitleIds.length >= 2"
        @add-subtitle="openSubtitle"
        @toggle-search="toggleSearchPanel"
        @toggle-scissor="handleScissor"
        @merge-subtitles="handleMergeSubtitles"
        @align-to-waveform="handleAlignToWaveform"
        @toggle-snap="isSnapEnabled = !isSnapEnabled"
        @open-settings="showSettingsDialog = true"
      />

      <!-- 左侧字幕列表 -->
      <SubtitleListPanel
        ref="subtitleListPanelRef"
        :entries="subtitleStore.entries"
        :selected-entry-id="selectedEntryId"
        :search-text="searchText"
        :replace-text="replaceText"
        :show-search-panel="showSearchPanel"
        :show-replace="showReplace"
        :search-results="subtitleStore.searchResults"
        :has-audio="hasAudio"
        :current-file-path="subtitleStore.currentFilePath"
        :format-time-stamp="subtitleStore.formatTimeStamp"
        @update:search-text="searchText = $event"
        @update:replace-text="replaceText = $event"
        @update:show-replace="showReplace = $event"
        @select-entry="selectEntry"
        @double-click-entry="handleSubtitleDoubleClick"
        @copy-text="copySubtitleText"
        @play-audio="playSubtitleAudio"
        @delete-entry="deleteSubtitleItem"
        @replace-one="replaceOne"
        @replace-all="replaceAll"
        @close-search="closeSearch"
        @go-back="goBack"
      />

      <!-- 右侧字幕编辑区 -->
      <div class="subtitle-edit-panel">
        <SubtitleEditPanel
          ref="subtitleEditPanelRef"
          :entry="currentEntry"
          :format-time-stamp="subtitleStore.formatTimeStamp"
          @update-text="handleTextUpdate"
          @update-time="handleTimeUpdate"
          @adjust-time="handleAdjustTime"
          @delete-entry="handleDeleteEntry"
          @remove-html="handleRemoveHTML"
          @add-cjk-spaces="handleAddCJKSpaces"
          @remove-punctuation="handleRemovePunctuation"
          @text-focus="handleTextFocus"
          @text-blur="handleTextBlur"
          @text-input="handleTextInput"
        />
      </div>
    </div>

    <!-- 设置弹窗 -->
    <SettingsDialog v-model:visible="showSettingsDialog" />
  </div>
</template>

<style scoped>
.editor-page {
  width: 100%;
  height: 100vh;
  display: flex;
  flex-direction: column;
  background-color: #f5f5f5;
}

/* 时间轴区域 */
.timeline-section {
  width: 100%;
  background: white;
  border-bottom: 1px solid #e5e7eb;
  padding: 0;
  display: flex;
  flex-direction: column;
}

/* 主内容区 */
.content-area {
  flex: 1;
  display: flex;
  overflow: hidden;
  min-height: 0;
}

/* 右侧字幕编辑区 */
.subtitle-edit-panel {
  flex: 1;
  background: linear-gradient(180deg, #f8fafc 0%, #f1f5f9 100%);
  overflow-y: auto;
  display: flex;
  flex-direction: column;
}
</style>

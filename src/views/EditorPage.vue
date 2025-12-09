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
import type { CorrectionEntry, CorrectionEntryWithChoice, FireRedEnvStatus } from '@/types/correction'
import WaveformViewer from '@/components/WaveformViewer.vue'
import SettingsDialog from '@/components/SettingsDialog.vue'
import CorrectionCompareDialog from '@/components/CorrectionCompareDialog.vue'
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
const showCorrectionDialog = ref(false)
const correctionEntries = ref<CorrectionEntry[]>([])
const fireredStatus = ref<FireRedEnvStatus>({ uv_installed: false, env_exists: false, ready: false })
const isCorrecting = ref(false) // 单条校正中
const isBatchCorrecting = ref(false) // 批量校正中
const singleCorrectionResult = ref<{ original: string; corrected: string; has_diff: boolean } | null>(null)
const showOnlyNeedsCorrection = ref(false) // 只显示需要校正的字幕
const correctionProgress = ref({ progress: 0, currentText: '', status: '' }) // 批量校正进度

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
  if (isSaving) return
  
  // 如果没有文件路径，弹出另存为对话框
  if (!subtitleStore.currentFilePath) {
    await handleSaveAs()
    return
  }
  
  isSaving = true
  try {
    await subtitleStore.saveToFile()
  } finally {
    setTimeout(() => { isSaving = false }, 100)
  }
}

// 另存为文件
const handleSaveAs = async () => {
  if (isSaving || subtitleStore.entries.length === 0) return
  
  try {
    const { save } = await import('@tauri-apps/plugin-dialog')
    const filePath = await save({
      filters: [{ name: 'SRT 字幕文件', extensions: ['srt'] }],
      defaultPath: 'untitled.srt'
    })
    
    if (filePath) {
      isSaving = true
      try {
        await subtitleStore.saveAsFile(filePath)
        ElMessage.success('文件保存成功')
      } finally {
        setTimeout(() => { isSaving = false }, 100)
      }
    }
  } catch (error) {
    ElMessage.error(`保存失败：${error instanceof Error ? error.message : String(error)}`)
  }
}

// 选择字幕
const selectEntry = (id: number) => {
  selectedEntryId.value = id
  isUserSelectingEntry.value = true
  // 切换字幕时清除校正结果
  singleCorrectionResult.value = null
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

// FireRedASR 校正相关
const fetchFireredStatus = async () => {
  try {
    fireredStatus.value = await invoke<FireRedEnvStatus>('check_firered_env_status')
    console.log('FireRedASR status:', fireredStatus.value)
    
    // 如果环境已就绪，后台预加载服务（不阻塞 UI）
    if (fireredStatus.value.ready) {
      preloadFireredService()
    }
  } catch (e) {
    console.error('Failed to fetch firered status:', e)
  }
}

// 预加载 FireRedASR 服务（后台执行，不阻塞）
const preloadFireredService = async () => {
  try {
    // 先检查服务是否已经在运行
    const isRunning = await invoke<boolean>('is_firered_service_running')
    if (isRunning) {
      console.log('FireRedASR service already running')
      return
    }
    
    console.log('Preloading FireRedASR service...')
    const result = await invoke<string>('preload_firered')
    console.log('FireRedASR preload result:', result)
  } catch (e) {
    // 预加载失败不影响用户体验，只记录日志
    console.warn('Failed to preload FireRedASR service:', e)
  }
}

const startCorrection = async () => {
  console.log('startCorrection called', {
    hasAudio: hasAudio.value,
    currentFilePath: subtitleStore.currentFilePath,
    fireredReady: fireredStatus.value.ready,
    audioPath: audioStore.audioFile?.path
  })
  
  if (!hasAudio.value) {
    ElMessage.warning('请先加载音频文件')
    return
  }
  
  if (subtitleStore.entries.length === 0) {
    ElMessage.warning('没有字幕内容可校正')
    return
  }
  
  // 如果字幕文件未保存，提示用户先保存
  if (!subtitleStore.currentFilePath) {
    ElMessage.warning('请先保存字幕文件（Cmd+S）')
    return
  }
  
  if (!fireredStatus.value.ready) {
    ElMessage.warning('请先在设置中安装 FireRedASR 环境')
    showSettingsDialog.value = true
    return
  }
  
  if (audioStore.playerState.isPlaying) audioStore.pause()
  
  isBatchCorrecting.value = true
  correctionProgress.value = { progress: 0, currentText: '正在加载模型...', status: 'loading' }
  
  // 监听进度事件（确保进度只会前进不会后退）
  const unlistenProgress = await listen<{ progress: number; current_text: string; status: string }>('firered-progress', (event) => {
    // 只有当新进度大于当前进度时才更新（防止进度回退）
    if (event.payload.progress >= correctionProgress.value.progress) {
      correctionProgress.value = {
        progress: event.payload.progress,
        currentText: event.payload.current_text,
        status: event.payload.status
      }
    }
  })
  
  try {
    console.log('Calling correct_subtitles_with_firered...', {
      srtPath: subtitleStore.currentFilePath,
      audioPath: audioStore.audioFile?.path
    })
    
    const result = await invoke<CorrectionEntry[]>('correct_subtitles_with_firered', {
      srtPath: subtitleStore.currentFilePath,
      audioPath: audioStore.audioFile?.path,
      language: 'zh',
      preserveCase: configStore.fireredPreserveCase
    })
    
    console.log('Correction result:', result)
    
    if (result && result.length > 0) {
      // 将校正结果应用到字幕条目中，标记有差异的字幕
      let diffCount = 0
      for (const entry of result) {
        if (entry.has_diff) {
          subtitleStore.setCorrectionSuggestion(entry.id, entry.corrected)
          diffCount++
        }
      }
      
      if (diffCount > 0) {
        // 自动开启筛选模式，显示需要确认的字幕
        showOnlyNeedsCorrection.value = true
        // 选中第一条需要校正的字幕
        const firstNeedsCorrection = subtitleStore.entries.find(e => e.needsCorrection)
        if (firstNeedsCorrection) {
          selectedEntryId.value = firstNeedsCorrection.id
        }
        ElMessage.success(`校正完成，发现 ${diffCount} 处差异，请逐条确认`)
      } else {
        ElMessage.success('校正完成，未发现差异')
      }
    } else {
      ElMessage.warning('校正完成，但没有返回结果')
    }
  } catch (error) {
    console.error('Correction error:', error)
    ElMessage.error(`校正失败：${error instanceof Error ? error.message : String(error)}`)
  } finally {
    unlistenProgress()
    isBatchCorrecting.value = false
    correctionProgress.value = { progress: 0, currentText: '', status: '' }
  }
}

const handleCorrectionConfirm = async (entries: CorrectionEntryWithChoice[]) => {
  // 应用用户选择的校正结果
  let updatedCount = 0
  
  for (const entry of entries) {
    if (entry.has_diff && entry.choice === 'corrected') {
      const subtitle = subtitleStore.entries.find(e => e.id === entry.id)
      if (subtitle && subtitle.text !== entry.corrected) {
        subtitleStore.updateEntryText(entry.id, entry.corrected)
        updatedCount++
      }
    }
  }
  
  if (updatedCount > 0) {
    if (subtitleStore.currentFilePath) {
      await subtitleStore.saveToFile()
    }
    ElMessage.success(`已应用 ${updatedCount} 处校正`)
  } else {
    ElMessage.info('没有应用任何校正')
  }
  
  showCorrectionDialog.value = false
  correctionEntries.value = []
}

const handleCorrectionCancel = () => {
  showCorrectionDialog.value = false
  correctionEntries.value = []
}

// 单条字幕校正
const handleCorrectSingleEntry = async () => {
  if (!currentEntry.value) {
    ElMessage.warning('请先选择一条字幕')
    return
  }
  
  if (!hasAudio.value) {
    ElMessage.warning('请先加载音频文件')
    return
  }
  
  if (!fireredStatus.value.ready) {
    ElMessage.warning('请先在设置中安装 FireRedASR 环境')
    showSettingsDialog.value = true
    return
  }
  
  if (audioStore.playerState.isPlaying) audioStore.pause()
  
  // 清除之前的校正结果
  singleCorrectionResult.value = null
  isCorrecting.value = true
  
  try {
    const entry = currentEntry.value
    const startMs = timeStampToMs(entry.startTime)
    const endMs = timeStampToMs(entry.endTime)
    
    console.log('Correcting single entry:', { id: entry.id, startMs, endMs, text: entry.text })
    
    const result = await invoke<{ original: string; corrected: string; has_diff: boolean }>('correct_single_subtitle', {
      audioPath: audioStore.audioFile?.path,
      startMs,
      endMs,
      originalText: entry.text,
      language: 'zh',
      preserveCase: configStore.fireredPreserveCase
    })
    
    console.log('Single correction result:', result)
    
    // 显示校正结果（内联）
    singleCorrectionResult.value = result
  } catch (error) {
    console.error('Single correction error:', error)
    ElMessage.error(`校正失败：${error instanceof Error ? error.message : String(error)}`)
  } finally {
    isCorrecting.value = false
  }
}

// 应用校正结果
const handleApplyCorrection = async () => {
  if (!currentEntry.value || !singleCorrectionResult.value) return
  
  subtitleStore.updateEntryText(currentEntry.value.id, singleCorrectionResult.value.corrected)
  if (subtitleStore.currentFilePath) {
    await subtitleStore.saveToFile()
  }
  singleCorrectionResult.value = null
  ElMessage.success('已应用校正')
}

// 忽略校正结果
const handleDismissCorrection = () => {
  singleCorrectionResult.value = null
}

// 切换当前字幕的校正标记
const handleToggleCorrectionMark = () => {
  if (currentEntry.value) {
    const wasMarked = currentEntry.value.needsCorrection
    subtitleStore.toggleCorrectionMark(currentEntry.value.id)
    
    // 如果取消了标记，且处于筛选模式，检查是否还有需要校正的字幕
    if (wasMarked && showOnlyNeedsCorrection.value) {
      const remainingCount = subtitleStore.needsCorrectionCount
      if (remainingCount === 0) {
        // 没有更多需要校正的了，关闭筛选模式
        showOnlyNeedsCorrection.value = false
        ElMessage.success('所有标记已处理完成')
      } else {
        // 自动跳转到下一条需要校正的字幕
        const nextEntry = subtitleStore.entries.find(e => e.needsCorrection)
        if (nextEntry) {
          selectedEntryId.value = nextEntry.id
        }
      }
    }
  }
}

// 应用批量校正建议
const handleApplySuggestion = async () => {
  if (!currentEntry.value) return
  
  subtitleStore.applyCorrectionSuggestion(currentEntry.value.id)
  
  // 保存文件
  if (subtitleStore.currentFilePath) {
    await subtitleStore.saveToFile()
  }
  
  // 自动跳转到下一条需要校正的字幕
  const nextEntry = subtitleStore.entries.find(e => e.needsCorrection && e.id !== currentEntry.value?.id)
  if (nextEntry) {
    selectedEntryId.value = nextEntry.id
    ElMessage.success(`已采用，还有 ${subtitleStore.needsCorrectionCount} 条待确认`)
  } else {
    // 没有更多需要校正的了，关闭筛选
    showOnlyNeedsCorrection.value = false
    ElMessage.success('所有校正已处理完成')
  }
}

// 忽略批量校正建议
const handleDismissSuggestion = () => {
  if (!currentEntry.value) return
  
  subtitleStore.dismissCorrectionSuggestion(currentEntry.value.id)
  
  // 自动跳转到下一条需要校正的字幕
  const nextEntry = subtitleStore.entries.find(e => e.needsCorrection && e.id !== currentEntry.value?.id)
  if (nextEntry) {
    selectedEntryId.value = nextEntry.id
    ElMessage.info(`已忽略，还有 ${subtitleStore.needsCorrectionCount} 条待确认`)
  } else {
    // 没有更多需要校正的了，关闭筛选
    showOnlyNeedsCorrection.value = false
    ElMessage.success('所有校正已处理完成')
  }
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
  // 倍速播放快捷键 (仿 Final Cut Pro)
  else if ((e.key === 'l' || e.key === 'L') && hasAudio.value) {
    e.preventDefault()
    const currentRate = audioStore.playerState.playbackRate
    // 1x → 1.5x → 2x → 循环回 1x
    const nextRate = currentRate >= 2 ? 1 : currentRate >= 1.5 ? 2 : currentRate >= 1 ? 1.5 : 1
    audioStore.setPlaybackRate(nextRate)
  }
  else if ((e.key === 'k' || e.key === 'K') && hasAudio.value) {
    e.preventDefault()
    audioStore.setPlaybackRate(1)
  }
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

  // 后台获取 FireRedASR 状态（不阻塞页面加载）
  fetchFireredStatus()

  try {
    ;(window as any).__handleMenuOpenFile = async () => await handleOpenFile()
    ;(window as any).__handleMenuSave = async () => await handleSave()
    ;(window as any).__globalBatchAICorrection = async () => await startCorrection()
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
  ;(window as any).__globalBatchAICorrection = null
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
        @merge-subtitles="handleMergeSubtitles"
        @align-to-waveform="handleAlignToWaveform"
        @toggle-snap="isSnapEnabled = !isSnapEnabled"
        @enter-scissor-mode="isScissorMode = true"
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
        :has-subtitles="hasContent"
        :firered-ready="fireredStatus.ready"
        :show-only-needs-correction="showOnlyNeedsCorrection"
        :needs-correction-count="subtitleStore.needsCorrectionCount"
        @add-subtitle="openSubtitle"
        @toggle-search="toggleSearchPanel"
        @toggle-scissor="handleScissor"
        @merge-subtitles="handleMergeSubtitles"
        @align-to-waveform="handleAlignToWaveform"
        @toggle-snap="isSnapEnabled = !isSnapEnabled"
        @open-settings="showSettingsDialog = true"
        @start-correction="startCorrection"
        @toggle-correction-filter="showOnlyNeedsCorrection = !showOnlyNeedsCorrection"
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
        :show-only-needs-correction="showOnlyNeedsCorrection"
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
        @toggle-correction-mark="subtitleStore.toggleCorrectionMark"
      />

      <!-- 右侧字幕编辑区 -->
      <div class="subtitle-edit-panel">
        <SubtitleEditPanel
          ref="subtitleEditPanelRef"
          :entry="currentEntry"
          :format-time-stamp="subtitleStore.formatTimeStamp"
          :firered-ready="fireredStatus.ready"
          :is-correcting="isCorrecting"
          :correction-result="singleCorrectionResult"
          :needs-correction-count="subtitleStore.needsCorrectionCount"
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
          @correct-entry="handleCorrectSingleEntry"
          @apply-correction="handleApplyCorrection"
          @dismiss-correction="handleDismissCorrection"
          @toggle-correction-mark="handleToggleCorrectionMark"
          @apply-suggestion="handleApplySuggestion"
          @dismiss-suggestion="handleDismissSuggestion"
        />
      </div>
    </div>

    <!-- 设置弹窗 -->
    <SettingsDialog v-model:visible="showSettingsDialog" />
    
    <!-- 校正对比弹窗 -->
    <CorrectionCompareDialog
      v-model:visible="showCorrectionDialog"
      :entries="correctionEntries"
      :audio-path="audioStore.audioFile?.path"
      @confirm="handleCorrectionConfirm"
      @cancel="handleCorrectionCancel"
    />

    <!-- 批量校正进度弹窗 -->
    <div v-if="isBatchCorrecting" class="correction-progress-overlay">
      <div class="correction-progress-dialog">
        <div class="progress-header">
          <svg class="progress-icon" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"/>
            <path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"/>
          </svg>
          <span>批量 AI 字幕校正</span>
        </div>
        <div class="progress-content">
          <div class="progress-bar-container">
            <div class="progress-bar" :style="{ width: correctionProgress.progress + '%' }"></div>
          </div>
          <div class="progress-text">{{ correctionProgress.currentText }}</div>
          <div class="progress-percent">{{ Math.round(correctionProgress.progress) }}%</div>
        </div>
      </div>
    </div>
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

/* 批量校正进度弹窗 */
.correction-progress-overlay {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: rgba(0, 0, 0, 0.5);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 2000;
}

.correction-progress-dialog {
  background: #fff;
  border-radius: 16px;
  padding: 24px 32px;
  min-width: 400px;
  max-width: 500px;
  box-shadow: 0 20px 60px rgba(0, 0, 0, 0.3);
}

.progress-header {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 20px;
  font-size: 16px;
  font-weight: 600;
  color: #1e293b;
}

.progress-icon {
  color: #3b82f6;
  animation: pulse 2s ease-in-out infinite;
}

@keyframes pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.5; }
}

.progress-content {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.progress-bar-container {
  height: 8px;
  background: #e2e8f0;
  border-radius: 4px;
  overflow: hidden;
}

.progress-bar {
  height: 100%;
  background: linear-gradient(90deg, #3b82f6 0%, #2563eb 100%);
  border-radius: 4px;
  transition: width 0.3s ease;
}

.progress-text {
  font-size: 13px;
  color: #64748b;
  line-height: 1.5;
  min-height: 40px;
  word-break: break-all;
}

.progress-percent {
  font-size: 24px;
  font-weight: 700;
  color: #3b82f6;
  text-align: center;
}
</style>

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
import { useSmartDictionaryStore } from '@/stores/smartDictionary'
import { timeStampToMs } from '@/utils/time'
import { findVoiceRegion, timestampToMs, msToTimestamp } from '@/utils/waveformAlign'
import type { SRTFile, AudioFile, TimeStamp } from '@/types/subtitle'
import type { CorrectionEntry, CorrectionEntryWithChoice, FireRedEnvStatus } from '@/types/correction'
import WaveformViewer from '@/components/WaveformViewer.vue'
import SettingsDialog from '@/components/SettingsDialog.vue'
import CorrectionCompareDialog from '@/components/CorrectionCompareDialog.vue'
import DictionaryPreviewDialog from '@/components/DictionaryPreviewDialog.vue'
import QuickAddDictionaryDialog from '@/components/QuickAddDictionaryDialog.vue'
import SplitSubtitleDialog from '@/components/SplitSubtitleDialog.vue'
import TitleBar from '@/components/TitleBar.vue'
import { EditorSidebar, AudioEmptyState, TimelineControls, SubtitleListPanel, SubtitleEditPanel } from '@/components/editor'
import { ElMessage, ElMessageBox, ElLoading } from 'element-plus'

const router = useRouter()
const subtitleStore = useSubtitleStore()
const audioStore = useAudioStore()
const configStore = useConfigStore()
const tabManager = useTabManagerStore()
const smartDictionary = useSmartDictionaryStore()

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
const fireredStatus = ref<FireRedEnvStatus>({ 
  uv_installed: false, 
  cpu_env: { installed: false, ready: false },
  gpu_env: { installed: false, ready: false },
  active_env: 'none',
  env_exists: false, 
  ready: false,
  is_gpu: false
})
const isCorrecting = ref(false) // 单条校正中
const isBatchCorrecting = ref(false) // 批量校正中
const isCorrectionCancelling = ref(false) // 正在取消校正
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

// 当前正在循环播放的字幕 ID
const loopingEntryId = computed(() => {
  if (!audioStore.segmentLoop.enabled) return null
  // 根据循环的时间范围找到对应的字幕
  const startMs = audioStore.segmentLoop.startTime * 1000
  const endMs = audioStore.segmentLoop.endTime * 1000
  const entry = subtitleStore.entries.find(e => {
    const entryStartMs = timeStampToMs(e.startTime)
    const entryEndMs = timeStampToMs(e.endTime)
    return Math.abs(entryStartMs - startMs) < 10 && Math.abs(entryEndMs - endMs) < 10
  })
  return entry?.id ?? null
})

// 总字数统计
const totalCharCount = computed(() => {
  return subtitleStore.entries.reduce((sum, entry) => sum + entry.text.length, 0)
})

// 当前选中字幕的序号
const currentSubtitleIndex = computed(() => {
  if (!selectedEntryId.value) return 0
  const index = subtitleStore.entries.findIndex(e => e.id === selectedEntryId.value)
  return index !== -1 ? index + 1 : 0
})

// 格式化保存时间
const formattedSaveTime = computed(() => {
  if (!subtitleStore.lastSavedAt) return null
  const date = new Date(subtitleStore.lastSavedAt)
  const hours = date.getHours().toString().padStart(2, '0')
  const minutes = date.getMinutes().toString().padStart(2, '0')
  const seconds = date.getSeconds().toString().padStart(2, '0')
  return `${hours}:${minutes}:${seconds}`
})

// 监听 tab 切换
watch(() => tabManager.activeTabId, async () => {
  // 如果正在校正，取消校正任务
  if (isCorrecting.value) {
    try {
      await invoke('cancel_firered_task')
    } catch (e) {
      console.warn('取消校正任务失败:', e)
    }
  }
  
  // 重置校正状态
  isCorrecting.value = false
  singleCorrectionResult.value = null
  
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

// 监听校正标记数量变化，更新菜单项启用状态
watch(() => subtitleStore.needsCorrectionCount, (count) => {
  invoke('update_menu_item_enabled', {
    menuId: 'clear-all-corrections',
    enabled: count > 0
  }).catch(err => console.error('Failed to update menu item:', err))
}, { immediate: true })

// 监听音频播放进度
watch(() => audioStore.playerState.currentTime, (currentTime) => {
  // 片段循环模式下不自动滚动
  if (hasAudio.value && !isUserSelectingEntry.value && !audioStore.segmentLoop.enabled) {
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
      // 检查文件写入权限
      const permissionCheck = await invoke<{ readable: boolean; writable: boolean; error_message: string | null; is_locked: boolean }>('check_file_write_permission', { filePath: selected })
      
      if (!permissionCheck.writable) {
        if (permissionCheck.is_locked) {
          // 文件被锁定，提供解锁选项
          try {
            await ElMessageBox.confirm(
              '文件已被锁定，无法写入。\n\n点击「解锁」按钮可以解除锁定并继续编辑。',
              '文件已锁定',
              { confirmButtonText: '解锁', cancelButtonText: '取消', type: 'warning' }
            )
            // 用户点击解锁
            await invoke('unlock_file_cmd', { filePath: selected })
          } catch {
            // 用户点击取消
            return
          }
        } else {
          // 其他权限问题
          const warningMessage = permissionCheck.error_message || '文件无法写入。'
          await ElMessageBox.alert(warningMessage, '无法打开文件', { confirmButtonText: '我知道了', type: 'warning', dangerouslyUseHTMLString: true })
          return
        }
      }
      
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
    const errorStr = String(error)
    // 检查是否是权限问题
    if (errorStr.includes('Permission denied') || errorStr.includes('os error 13')) {
      await ElMessageBox.alert(
        `<p><strong>文件保存失败：权限被拒绝</strong></p>
        <p style="margin-top: 10px;">这可能是因为文件从网络下载或从其他人处接收，macOS 为其添加了安全隔离属性。</p>
        <p style="margin-top: 10px;"><strong>解决方法：</strong></p>
        <p>1. 在终端运行命令移除隔离属性</p>
        <p>2. 或使用「另存为」功能将文件保存到其他位置</p>`,
        '保存失败',
        { confirmButtonText: '我知道了', type: 'error', dangerouslyUseHTMLString: true }
      )
    } else {
      ElMessage.error(`保存失败：${error instanceof Error ? error.message : errorStr}`)
    }
  }
}

// 选择字幕
const selectEntry = async (id: number) => {
  // 如果正在校正，取消校正任务
  if (isCorrecting.value) {
    try {
      await invoke('cancel_firered_task')
    } catch (e) {
      console.warn('取消校正任务失败:', e)
    }
    isCorrecting.value = false
  }
  
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

// 播放字幕音频（片段循环模式）
const playSubtitleAudio = (id: number) => {
  if (!hasAudio.value) return
  const entry = subtitleStore.entries.find((e) => e.id === id)
  if (!entry) return
  const startTimeMs = timeStampToMs(entry.startTime)
  const endTimeMs = timeStampToMs(entry.endTime)
  // 使用片段循环播放，不会触发自动滚动
  audioStore.playSegmentLoop(startTimeMs / 1000, endTimeMs / 1000)
}

// 停止字幕音频循环播放
const stopSubtitleAudio = () => {
  audioStore.stopSegmentLoop()
  audioStore.pause()
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

// 🔥 批量应用智能词典（预览模式）
interface DictionaryReplacement {
  id: number
  text: string
  newText: string
  replacements: Array<{ from: string; to: string }>
}

const showDictionaryDialog = ref(false)
const dictionaryItems = ref<DictionaryReplacement[]>([])
const showQuickAddDialog = ref(false)
const quickAddInitialVariant = ref('')

// 分割对话框状态
const showSplitDialog = ref(false)
const splitDialogData = ref<{
  subtitleId: number
  originalText: string
  startTimeMs: number
  endTimeMs: number
  initialSplitTimeMs?: number
}>({
  subtitleId: 0,
  originalText: '',
  startTimeMs: 0,
  endTimeMs: 0
})

const handleApplyDictionary = async () => {
  if (subtitleStore.entries.length === 0) {
    ElMessage.warning('没有字幕内容')
    return
  }
  
  if (smartDictionary.totalCount === 0) {
    ElMessage.warning('词典为空，请先添加词条')
    return
  }
  
  if (audioStore.playerState.isPlaying) audioStore.pause()
  
  // 预览词典替换结果（只收集有替换的）
  const previewItems: DictionaryReplacement[] = []
  for (const entry of subtitleStore.entries) {
    const { result, replacements } = smartDictionary.applyDictionary(entry.text)
    if (replacements.length > 0) {
      previewItems.push({
        id: entry.id,
        text: entry.text,
        newText: result,
        replacements
      })
    }
  }
  
  if (previewItems.length === 0) {
    ElMessage.info('没有找到可替换的内容')
    return
  }
  
  // 显示预览对话框
  dictionaryItems.value = previewItems
  showDictionaryDialog.value = true
}

// 替换单条
const handleDictionaryReplace = async (id: number, newText: string) => {
  subtitleStore.updateEntryText(id, newText)
  if (subtitleStore.currentFilePath) {
    await subtitleStore.saveToFile()
  }
}

// 全部替换
const handleDictionaryReplaceAll = async (items: DictionaryReplacement[]) => {
  for (const item of items) {
    subtitleStore.updateEntryText(item.id, item.newText)
  }
  if (subtitleStore.currentFilePath) {
    await subtitleStore.saveToFile()
  }
  ElMessage.success(`已替换 ${items.length} 条字幕`)
}

const handleDictionaryCancel = () => {
  showDictionaryDialog.value = false
  dictionaryItems.value = []
}

// 快速添加词典 - 从选中文本
const handleQuickAddFromSelection = (selectedText?: string) => {
  // 如果没有传入选中文本，尝试从当前活动的输入框获取
  if (!selectedText) {
    const activeEl = document.activeElement as HTMLInputElement | HTMLTextAreaElement | null
    if (activeEl && (activeEl.tagName === 'INPUT' || activeEl.tagName === 'TEXTAREA')) {
      if (activeEl.selectionStart !== activeEl.selectionEnd) {
        selectedText = activeEl.value.substring(activeEl.selectionStart ?? 0, activeEl.selectionEnd ?? 0)
      }
    }
  }
  quickAddInitialVariant.value = selectedText?.trim() || ''
  showQuickAddDialog.value = true
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

const handleToUpperCase = () => {
  if (!currentEntry.value) return
  if (audioStore.playerState.isPlaying) audioStore.pause()
  subtitleStore.convertEntryToUpperCase(currentEntry.value.id)
}

const handleToLowerCase = () => {
  if (!currentEntry.value) return
  if (audioStore.playerState.isPlaying) audioStore.pause()
  subtitleStore.convertEntryToLowerCase(currentEntry.value.id)
}

const handleToCapitalize = () => {
  if (!currentEntry.value) return
  if (audioStore.playerState.isPlaying) audioStore.pause()
  subtitleStore.convertEntryToCapitalize(currentEntry.value.id)
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
  
  // 获取要分割的字幕信息
  const entry = subtitleStore.entries.find(e => e.id === id)
  if (!entry) return
  
  // 显示分割对话框
  splitDialogData.value = {
    subtitleId: id,
    originalText: entry.text,
    startTimeMs: timeStampToMs(entry.startTime),
    endTimeMs: timeStampToMs(entry.endTime),
    initialSplitTimeMs: splitTimeMs
  }
  showSplitDialog.value = true
  isScissorMode.value = false
}

// 处理分割对话框确认
const handleSplitConfirm = async (segments: Array<{ id: number; startTimeMs: number; endTimeMs: number; text: string }>) => {
  const newId = subtitleStore.splitEntryMultiple(splitDialogData.value.subtitleId, segments)
  if (newId) {
    selectedEntryId.value = newId
    if (subtitleStore.currentFilePath) {
      try { await subtitleStore.saveToFile() } catch {}
    }
  }
  showSplitDialog.value = false
}

const handleSplitCancel = () => {
  showSplitDialog.value = false
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
  
  // 监听进度事件
  const unlistenProgress = await listen<{ progress: number; current_text: string; status: string }>('firered-progress', (event) => {
    const newProgress = event.payload.progress
    const currentProgress = correctionProgress.value.progress
    const newText = event.payload.current_text
    
    // 调试：打印所有进度事件
    console.log('[FireRed Progress Event]', {
      newProgress,
      currentProgress,
      newText,
      status: event.payload.status
    })
    
    // 进度阶段说明：
    // 0-1%: 设备检测（显示 GPU/CPU 信息）
    // 1-2%: 模型加载
    // 5-100%: 实际校正进度
    
    // 始终更新文本内容（让用户看到设备信息等重要消息）
    // 进度条只前进不后退
    correctionProgress.value = {
      progress: Math.max(newProgress, currentProgress),
      currentText: newText,
      status: event.payload.status
    }
    
    // 打印设备信息到控制台
    if (newText.includes('使用设备')) {
      console.log('🔥 FireRedASR', newText)
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
      // 🔥 应用智能词典进行二次纠错
      let dictionaryReplacements = 0
      for (const entry of result) {
        const { result: correctedText, replacements } = smartDictionary.applyDictionary(entry.corrected)
        if (replacements.length > 0) {
          entry.corrected = correctedText
          entry.has_diff = entry.original !== correctedText
          dictionaryReplacements += replacements.length
          console.log('词典替换:', { id: entry.id, replacements })
        }
      }
      if (dictionaryReplacements > 0) {
        console.log(`智能词典共替换 ${dictionaryReplacements} 处`)
      }

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
    // 如果是用户取消，不显示错误
    if (!isCorrectionCancelling.value) {
      ElMessage.error(`校正失败：${error instanceof Error ? error.message : String(error)}`)
    }
  } finally {
    unlistenProgress()
    isBatchCorrecting.value = false
    isCorrectionCancelling.value = false
    correctionProgress.value = { progress: 0, currentText: '', status: '' }
  }
}

// 取消批量校正
const cancelBatchCorrection = async () => {
  if (!isBatchCorrecting.value || isCorrectionCancelling.value) return
  
  isCorrectionCancelling.value = true
  correctionProgress.value.currentText = '正在取消...'
  
  try {
    await invoke('cancel_firered_task')
    ElMessage.info('已取消批量校正')
  } catch (error) {
    console.error('Cancel error:', error)
  }
}

const handleCorrectionConfirm = async (entries: CorrectionEntryWithChoice[]) => {
  // 应用用户选择的校正结果
  let updatedCount = 0
  
  for (const entry of entries) {
    if (!entry.has_diff) continue
    
    // 确定最终要使用的文本
    let newText: string | null = null
    
    if (entry.finalText !== undefined) {
      // 用户进行了细粒度编辑
      newText = entry.finalText
    } else if (entry.choice === 'corrected') {
      // 用户选择采用校正
      newText = entry.corrected
    }
    
    if (newText !== null) {
      const subtitle = subtitleStore.entries.find(e => e.id === entry.id)
      if (subtitle && subtitle.text !== newText) {
        subtitleStore.updateEntryText(entry.id, newText)
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

// 应用校正结果（支持细粒度编辑传入自定义文本）
const handleApplyCorrection = async (customText?: string) => {
  if (!currentEntry.value || !singleCorrectionResult.value) return
  
  const textToApply = customText ?? singleCorrectionResult.value.corrected
  subtitleStore.updateEntryText(currentEntry.value.id, textToApply)
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

// 应用批量校正建议（支持细粒度编辑传入自定义文本）
const handleApplySuggestion = async (customText?: string) => {
  if (!currentEntry.value) return
  
  if (customText !== undefined) {
    // 使用自定义文本
    subtitleStore.updateEntryText(currentEntry.value.id, customText)
    // 清除建议
    subtitleStore.dismissCorrectionSuggestion(currentEntry.value.id)
  } else {
    // 使用原始建议
    subtitleStore.applyCorrectionSuggestion(currentEntry.value.id)
  }
  
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

// 清除所有校正标记
const handleClearAllCorrections = async () => {
  const count = subtitleStore.needsCorrectionCount
  if (count === 0) {
    ElMessage.info('没有需要清除的校正标记')
    return
  }

  try {
    await ElMessageBox.confirm(
      `确定要清除全部 ${count} 条校正标记吗？`,
      '清除校正标记',
      {
        confirmButtonText: '清除',
        cancelButtonText: '取消',
        type: 'warning',
        beforeClose: (action, instance, done) => {
          if (action === 'confirm') {
            instance.confirmButtonLoading = true
            instance.confirmButtonText = '清除中...'

            // 用 setTimeout 让 loading 状态先渲染，再执行清除
            setTimeout(() => {
              subtitleStore.clearAllCorrectionMarks()
              showOnlyNeedsCorrection.value = false
              done()
              ElMessage.success(`已清除 ${count} 条校正标记`)
            }, 50)
          } else {
            done()
          }
        },
      }
    )
  } catch {
    // 用户取消
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
  // 快速添加词典 (Cmd/Ctrl+D)
  else if (pressedKey === 'Cmd+d' || pressedKey === 'Ctrl+d') { e.preventDefault(); handleQuickAddFromSelection() }
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
    ;(window as any).__globalClearAllCorrections = () => handleClearAllCorrections()
    ;(window as any).__globalApplyDictionary = async () => await handleApplyDictionary()
    ;(window as any).__globalQuickAddDictionary = async () => handleQuickAddFromSelection()
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
  ;(window as any).__globalClearAllCorrections = null
  ;(window as any).__globalApplyDictionary = null
  ;(window as any).__globalQuickAddDictionary = null
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
        :show-only-needs-correction="showOnlyNeedsCorrection"
        :needs-correction-count="subtitleStore.needsCorrectionCount"
        
        @add-subtitle="openSubtitle"
        @toggle-search="toggleSearchPanel"
        @toggle-scissor="handleScissor"
        @merge-subtitles="handleMergeSubtitles"
        @align-to-waveform="handleAlignToWaveform"
        @toggle-snap="isSnapEnabled = !isSnapEnabled"
        @open-settings="showSettingsDialog = true"
        @toggle-correction-filter="showOnlyNeedsCorrection = !showOnlyNeedsCorrection"
        @apply-dictionary="handleApplyDictionary"
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
        :looping-entry-id="loopingEntryId"
        @update:search-text="searchText = $event"
        @update:replace-text="replaceText = $event"
        @update:show-replace="showReplace = $event"
        @select-entry="selectEntry"
        @double-click-entry="handleSubtitleDoubleClick"
        @copy-text="copySubtitleText"
        @play-audio="playSubtitleAudio"
        @stop-audio="stopSubtitleAudio"
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
          @to-uppercase="handleToUpperCase"
          @to-lowercase="handleToLowerCase"
          @to-capitalize="handleToCapitalize"
          @text-focus="handleTextFocus"
          @text-blur="handleTextBlur"
          @text-input="handleTextInput"
          @correct-entry="handleCorrectSingleEntry"
          @apply-correction="handleApplyCorrection"
          @dismiss-correction="handleDismissCorrection"
          @toggle-correction-mark="handleToggleCorrectionMark"
          @apply-suggestion="handleApplySuggestion"
          @dismiss-suggestion="handleDismissSuggestion"
          @quick-add-dictionary="handleQuickAddFromSelection"
        />
      </div>
    </div>

    <!-- 全局底部状态栏 -->
    <div class="global-status-bar">
      <!-- 侧边栏+字幕列表区域 -->
      <div class="status-left-section">
        <!-- 文件名（靠左） -->
        <span class="file-name">
          {{ subtitleStore.currentFilePath ? subtitleStore.currentFilePath.split('/').pop() : '未保存.srt' }}
        </span>
        <!-- 字数（居中） -->
        <span class="status-item status-center">
          <span class="status-label">字数</span>
          <span class="status-value">{{ totalCharCount.toLocaleString() }}</span>
        </span>
        <!-- 字幕统计（靠右） -->
        <span class="status-item">
          <span class="status-label">字幕</span>
          <span class="status-value">{{ currentSubtitleIndex }}/{{ subtitleStore.entries.length }}</span>
        </span>
      </div>
      
      <!-- 右侧区域：保存时间 -->
      <div class="status-right">
        <span v-if="formattedSaveTime" class="save-time" title="上次保存时间">
          <svg class="save-icon" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M19 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11l5 5v11a2 2 0 0 1-2 2z"/>
            <polyline points="17 21 17 13 7 13 7 21"/>
            <polyline points="7 3 7 8 15 8"/>
          </svg>
          {{ formattedSaveTime }}
        </span>
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

    <!-- 词典替换预览弹窗 -->
    <DictionaryPreviewDialog
      v-model:visible="showDictionaryDialog"
      :items="dictionaryItems"
      @replace="handleDictionaryReplace"
      @replace-all="handleDictionaryReplaceAll"
      @cancel="handleDictionaryCancel"
    />

    <!-- 快速添加词典弹窗 -->
    <QuickAddDictionaryDialog
      v-model:visible="showQuickAddDialog"
      :initial-variant="quickAddInitialVariant"
    />

    <!-- 分割字幕对话框 -->
    <SplitSubtitleDialog
      v-model:visible="showSplitDialog"
      :subtitle-id="splitDialogData.subtitleId"
      :original-text="splitDialogData.originalText"
      :start-time-ms="splitDialogData.startTimeMs"
      :end-time-ms="splitDialogData.endTimeMs"
      :initial-split-time-ms="splitDialogData.initialSplitTimeMs"
      :waveform-data="audioStore.audioFile?.waveform"
      :audio-duration="audioStore.playerState.duration"
      @confirm="handleSplitConfirm"
      @cancel="handleSplitCancel"
    />

    <!-- 批量校正进度浮窗（右下角非阻塞式） -->
    <Transition name="slide-up">
      <div v-if="isBatchCorrecting" class="correction-progress-float">
        <div class="float-header">
          <div class="float-icon-wrapper">
            <svg class="float-icon" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"/>
              <path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"/>
            </svg>
          </div>
          <div class="float-title-section">
            <span class="float-title">批量 AI 字幕校正</span>
            <span class="float-percent">{{ Math.round(correctionProgress.progress) }}%</span>
          </div>
          <button class="float-cancel-btn" @click="cancelBatchCorrection" title="取消校正">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <line x1="18" y1="6" x2="6" y2="18"/>
              <line x1="6" y1="6" x2="18" y2="18"/>
            </svg>
          </button>
        </div>
        <div class="float-progress-bar">
          <div class="float-progress-fill" :style="{ width: correctionProgress.progress + '%' }"></div>
        </div>
        <div class="float-status">{{ correctionProgress.currentText }}</div>
      </div>
    </Transition>
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

/* 批量校正进度浮窗（右下角非阻塞式） */
.correction-progress-float {
  position: fixed;
  bottom: 48px;
  right: 20px;
  width: 320px;
  background: #fff;
  border-radius: 12px;
  padding: 14px 16px;
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.12), 0 2px 8px rgba(59, 130, 246, 0.08);
  z-index: 1000;
  border: 1px solid #e2e8f0;
}

.float-header {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-bottom: 12px;
}

.float-icon-wrapper {
  width: 32px;
  height: 32px;
  background: linear-gradient(135deg, #eff6ff 0%, #dbeafe 100%);
  border-radius: 8px;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.float-icon {
  color: #3b82f6;
  animation: floatIconPulse 2s ease-in-out infinite;
}

@keyframes floatIconPulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.6; }
}

.float-title-section {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: space-between;
  min-width: 0;
}

.float-title {
  font-size: 13px;
  font-weight: 600;
  color: #1e293b;
}

.float-percent {
  font-size: 13px;
  font-weight: 600;
  color: #3b82f6;
}

.float-cancel-btn {
  width: 28px;
  height: 28px;
  border: none;
  background: #f1f5f9;
  border-radius: 6px;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  color: #64748b;
  transition: all 0.2s;
  flex-shrink: 0;
}

.float-cancel-btn:hover {
  background: #fee2e2;
  color: #ef4444;
}

.float-progress-bar {
  height: 6px;
  background: #f1f5f9;
  border-radius: 3px;
  overflow: hidden;
  margin-bottom: 8px;
}

.float-progress-fill {
  height: 100%;
  background: linear-gradient(90deg, #3b82f6 0%, #2563eb 100%);
  border-radius: 3px;
  transition: width 0.3s ease;
}

.float-status {
  font-size: 12px;
  color: #64748b;
  line-height: 1.4;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* 浮窗动画 */
.slide-up-enter-active {
  animation: slideUpIn 0.3s ease-out;
}

.slide-up-leave-active {
  animation: slideUpOut 0.2s ease-in;
}

@keyframes slideUpIn {
  from {
    opacity: 0;
    transform: translateY(20px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

@keyframes slideUpOut {
  from {
    opacity: 1;
    transform: translateY(0);
  }
  to {
    opacity: 0;
    transform: translateY(20px);
  }
}

/* 全局底部状态栏 */
.global-status-bar {
  width: 100%;
  height: 28px;
  padding: 0;
  background: linear-gradient(180deg, #f8fafc 0%, #f1f5f9 100%);
  border-top: 1px solid #e2e8f0;
  display: flex;
  align-items: center;
  font-size: 11px;
  color: #64748b;
  flex-shrink: 0;
  user-select: none;
  -webkit-user-select: none;
}

/* 左侧区域：文件名 + 统计信息（对应侧边栏+字幕列表宽度） */
.status-left-section {
  width: 448px; /* 48px 侧边栏 + 400px 字幕列表 */
  flex-shrink: 0;
  display: flex;
  align-items: center;
  padding: 0 12px;
  height: 100%;
}

.file-name {
  color: #64748b;
  font-weight: 500;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  flex-shrink: 0;
}

.status-center {
  flex: 1;
  display: flex;
  justify-content: center;
}

.status-item {
  display: flex;
  align-items: center;
  gap: 4px;
  flex-shrink: 0;
}

.status-label {
  color: #64748b;
  font-weight: 500;
}

.status-value {
  color: #64748b;
  font-weight: 500;
  font-variant-numeric: tabular-nums;
}

.status-right {
  flex: 1;
  display: flex;
  justify-content: flex-end;
  padding: 0 12px;
}

.save-time {
  display: flex;
  align-items: center;
  gap: 4px;
  color: #64748b;
  font-weight: 500;
  font-variant-numeric: tabular-nums;
}

.save-icon {
  opacity: 0.7;
}
</style>

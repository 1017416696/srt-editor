<script setup lang="ts">
import { ref, computed, onMounted, onBeforeUnmount, watch, nextTick } from 'vue'
import { useRouter } from 'vue-router'
import { open } from '@tauri-apps/plugin-dialog'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'
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
import { DocumentCopy, VideoPlay, Delete, PriceTag, Document, Setting, DocumentAdd, Scissor, Search, ArrowDown, Switch } from '@element-plus/icons-vue'
import { ElMessage, ElMessageBox } from 'element-plus'

// Debounce helper function
function debounce<T extends (...args: any[]) => any>(func: T, wait: number) {
  let timeout: ReturnType<typeof setTimeout> | null = null

  return function executedFunction(...args: Parameters<T>) {
    const later = () => {
      timeout = null
      func(...args)
    }

    if (timeout) {
      clearTimeout(timeout)
    }
    timeout = setTimeout(later, wait)
  }
}

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
const editingText = ref('')
const editingStartTime = ref('')
const editingEndTime = ref('')
const subtitleListContainer = ref<HTMLElement | null>(null)
const searchInputRef = ref<any>(null) // el-input 组件
const replaceInputRef = ref<any>(null) // el-input 组件
const textareaInputRef = ref<any>(null) // el-input 的 ref
const subtitleItemRefs: Record<number, HTMLElement | null> = {}
const isUserEditing = ref(false) // 标记是否是用户在编辑
const isUserSelectingEntry = ref(false) // 标记用户是否在手动选择字幕
const isScissorMode = ref(false) // 剪刀模式：分割字幕
const showSearchPanel = ref(false) // 是否显示搜索面板
const activePanel = ref<'list' | 'search'>('list') // 当前激活的面板
const showSettingsDialog = ref(false) // 是否显示设置弹窗

// 切换搜索面板
const toggleSearchPanel = () => {
  showSearchPanel.value = !showSearchPanel.value
  if (showSearchPanel.value) {
    nextTick(() => {
      searchInputRef.value?.focus()
    })
  } else {
    // 关闭时清空搜索
    searchText.value = ''
    replaceText.value = ''
    showReplace.value = false
  }
}
let autoSaveTimer: ReturnType<typeof setTimeout> | null = null // 用于记录防抖计时器
let userSelectionTimer: ReturnType<typeof setTimeout> | null = null // 用于记录用户选择的计时器
let isSaving = false // 防止保存重复触发

// 计算属性
const hasContent = computed(() => subtitleStore.entries.length > 0)
const hasAudio = computed(() => audioStore.currentAudio !== null)
const canUndo = computed(() => subtitleStore.canUndo)
const canRedo = computed(() => subtitleStore.canRedo)

// 当前选中的字幕
const currentEntry = computed(() => {
  if (!selectedEntryId.value) return null
  return subtitleStore.entries.find((e) => e.id === selectedEntryId.value) || null
})

// 监听选中字幕变化，更新编辑文本和时间
watch(currentEntry, (entry) => {
  if (entry) {
    isUserEditing.value = false // 标记为非用户编辑
    editingText.value = entry.text
    editingStartTime.value = subtitleStore.formatTimeStamp(entry.startTime)
    editingEndTime.value = subtitleStore.formatTimeStamp(entry.endTime)
  }
})

// 监听 tab 切换，更新选中的字幕
watch(() => tabManager.activeTabId, () => {
  // 切换 tab 时，选中第一条字幕
  if (subtitleStore.entries.length > 0) {
    selectedEntryId.value = subtitleStore.entries[0]?.id ?? null
  } else {
    selectedEntryId.value = null
  }
  // 清空搜索
  searchText.value = ''
  showSearchPanel.value = false
})

// 搜索字幕文本
const handleSearch = (query: string) => {
  subtitleStore.search(query)

  // 如果有搜索结果，选中第一个
  if (subtitleStore.searchResults.length > 0) {
    selectedEntryId.value = subtitleStore.searchResults[0] ?? null
  }
}

// 监听搜索文本变化
watch(searchText, (query) => {
  handleSearch(query)
})

// 计算显示的字幕列表（根据搜索结果过滤）
const filteredEntries = computed(() => {
  if (!searchText.value) {
    // 未搜索时显示全部
    return subtitleStore.entries
  }

  // 搜索时只显示匹配的
  return subtitleStore.entries.filter((entry) =>
    subtitleStore.searchResults.includes(entry.id)
  )
})

// 执行替换全部
const replaceAll = async () => {
  if (!searchText.value) {
    return
  }

  try {
    // 如果正在播放，暂停
    if (audioStore.playerState.isPlaying) {
      audioStore.pause()
    }

    let modifiedCount = 0

    subtitleStore.entries.forEach((entry) => {
      const newText = entry.text.replaceAll(searchText.value, replaceText.value)

      if (newText !== entry.text) {
        subtitleStore.updateEntryText(entry.id, newText)
        modifiedCount++
      }
    })

    // 保存文件
    if (modifiedCount > 0) {
      await subtitleStore.saveToFile()
    }
  } catch (error) {
    // 替换失败，静默处理
  }
}

// 替换当前搜索结果
const replaceOne = async () => {
  if (!currentEntry.value || !subtitleStore.searchResults.includes(currentEntry.value.id)) {
    return
  }

  const entry = currentEntry.value
  let newText = entry.text

  try {
    // 如果正在播放，暂停
    if (audioStore.playerState.isPlaying) {
      audioStore.pause()
    }

    // 只支持普通字符串替换
    newText = newText.replaceAll(searchText.value, replaceText.value)

    if (newText !== entry.text) {
      subtitleStore.updateEntryText(entry.id, newText)
      await subtitleStore.saveToFile()

      // 替换后自动跳到下一个搜索结果
      const currentIndex = subtitleStore.searchResults.indexOf(entry.id)
      if (currentIndex !== -1 && currentIndex < subtitleStore.searchResults.length - 1) {
        // 还有下一个，自动跳到下一个
        const nextId = subtitleStore.searchResults[currentIndex + 1]
        selectedEntryId.value = nextId ?? null
      }
    }
  } catch (error) {
    // 替换失败，静默处理
  }
}

// 自动保存函数
const autoSaveCurrentEntry = async () => {
  if (!currentEntry.value) return

  const hasChanges = editingText.value !== currentEntry.value.text
  if (!hasChanges) {
    // 如果没有变化，不保存也不显示消息
    return
  }

  // 如果正在播放，暂停
  if (audioStore.playerState.isPlaying) {
    audioStore.pause()
  }

  // 更新 store 中的数据
  subtitleStore.updateEntryText(currentEntry.value.id, editingText.value)

  // 保存当前字幕编辑后，也保存整个文件
  if (!subtitleStore.currentFilePath) {
    return
  }

  try {
    await subtitleStore.saveToFile()
    // 自动保存完成，不显示提示
  } catch (error) {
    // 自动保存失败，静默处理
  }
}

// 新的防抖逻辑：当用户离焦时立即保存，或者 1500ms 后自动保存
const handleTextareaBlur = async () => {
  isUserEditing.value = false

  // 清除未执行的防抖计时器
  if (autoSaveTimer) {
    clearTimeout(autoSaveTimer)
    autoSaveTimer = null
  }

  // 离焦时立即保存
  await autoSaveCurrentEntry()
}

// 监听文本编辑，设置防抖计时器
const handleTextInput = () => {
  // 如果正在播放，立即暂停
  if (audioStore.playerState.isPlaying) {
    audioStore.pause()
  }

  // 清除之前的计时器
  if (autoSaveTimer) {
    clearTimeout(autoSaveTimer)
  }

  // 设置新的计时器：1500ms 后保存
  autoSaveTimer = setTimeout(() => {
    if (isUserEditing.value) {
      autoSaveCurrentEntry()
    }
    autoSaveTimer = null
  }, 1500)
}

// 监听音频播放进度，自动更新当前字幕
watch(() => audioStore.playerState.currentTime, (currentTime) => {
  if (hasAudio.value && !isUserSelectingEntry.value) {
    const entry = subtitleStore.getCurrentEntryByTime(currentTime)
    if (entry && selectedEntryId.value !== entry.id) {
      selectedEntryId.value = entry.id

      // 自动滚动字幕列表，使当前字幕保持在可见范围内
      nextTick(() => {
        const itemElement = subtitleItemRefs[entry.id]
        const containerElement = subtitleListContainer.value
        if (itemElement && containerElement) {
          itemElement.scrollIntoView({ behavior: 'smooth', block: 'nearest' })
        }
      })
    }
  }
})

// 用于存储事件监听器的清理函数
let unlistenOpenFile: (() => void) | null = null

// 初始化时选中第一条字幕，设置菜单监听和快捷键
onMounted(async () => {
  // 如果没有打开的 tab，跳转到欢迎页
  if (!tabManager.hasTabs) {
    router.push('/')
    return
  }

  if (subtitleStore.entries.length > 0) {
    selectedEntryId.value = subtitleStore.entries[0]?.id ?? null
  }

  try {
    // 注册全局菜单处理函数（供 main.ts 中的全局监听器调用）
    ;(window as any).__handleMenuOpenFile = async () => {
      await handleOpenFile()
    }

    ;(window as any).__handleMenuSave = async () => {
      await handleSave()
    }

    // 注册全局菜单处理函数（供 main.ts 中的全局监听器调用）
    unlistenOpenFile = await listen<void>('menu:open-file', async () => {
      await handleOpenFile()
    })

    // 先移除可能存在的旧监听器（防止热重载时重复注册）
    document.removeEventListener('keydown', handleKeydown, true)
    // 添加键盘快捷键监听（添加到 document 而不是 window，确保捕获所有键盘事件）
    document.addEventListener('keydown', handleKeydown, true)
  } catch (error) {
    console.error('Error setting up menu handlers:', error)
  }
})

// 在组件卸载时清理所有监听器（必须在顶层调用）
onBeforeUnmount(() => {
  // 清理 Tauri 事件监听器
  if (unlistenOpenFile) {
    unlistenOpenFile()
    unlistenOpenFile = null
  }
  // 清除全局处理函数
  ;(window as any).__handleMenuOpenFile = null
  ;(window as any).__handleMenuSave = null
  // 移除键盘事件监听器
  document.removeEventListener('keydown', handleKeydown, true)
})

// 打开 SRT 文件
const handleOpenFile = async () => {
  try {
    const selected = await open({
      multiple: false,
      filters: [
        {
          name: 'SRT 字幕文件',
          extensions: ['srt'],
        },
      ],
    })

    if (selected) {
      const srtFile = await invoke<SRTFile>('read_srt', { filePath: selected })
      await subtitleStore.loadSRTFile(srtFile)

      // 添加到最近文件列表
      configStore.addRecentFile(selected as string)
      
      // 更新菜单
      if ((window as any).__updateRecentFilesMenu) {
        await (window as any).__updateRecentFilesMenu()
      }

      // 选中第一条字幕
      if (subtitleStore.entries.length > 0) {
        selectedEntryId.value = subtitleStore.entries[0]?.id ?? null
      }
    }
  } catch (error) {
    // 加载失败，静默处理
  }
}

// 打开音频文件
const handleOpenAudio = async () => {
  try {
    const selected = await open({
      multiple: false,
      filters: [
        {
          name: '音频文件',
          extensions: ['mp3', 'wav', 'ogg', 'flac', 'm4a', 'aac'],
        },
      ],
    })

    if (selected && typeof selected === 'string') {
      const fileName = selected.split('/').pop() || 'audio'
      const fileExtension = selected.split('.').pop()?.toLowerCase() || 'mp3'

      const audioFile: AudioFile = {
        name: fileName,
        path: selected,
        duration: 0,
        format: fileExtension,
      }
      await audioStore.loadAudio(audioFile)
    }
  } catch (error) {
    // 加载失败，静默处理
  }
}

// 删除音频文件
const handleRemoveAudio = async () => {
  if (!hasAudio) return

  audioStore.unloadAudio()
}

// 保存文件
const handleSave = async () => {
  // 防止重复保存
  if (isSaving) return

  if (!subtitleStore.currentFilePath) {
    return
  }

  isSaving = true
  try {
    await subtitleStore.saveToFile()
  } catch (error) {
    // 保存失败，静默处理
  } finally {
    // 100ms 后允许再次保存
    setTimeout(() => {
      isSaving = false
    }, 100)
  }
}

// 保存当前字幕编辑
const saveCurrentEntry = async () => {
  if (!currentEntry.value) return

  if (editingText.value !== currentEntry.value.text) {
    subtitleStore.updateEntryText(currentEntry.value.id, editingText.value)
  }

  // 保存当前字幕编辑后，也保存整个文件
  if (!subtitleStore.currentFilePath) {
    return
  }

  try {
    await subtitleStore.saveToFile()
  } catch (error) {
    // 保存失败，静默处理
  }
}

// 选择字幕
const selectEntry = (id: number) => {
  selectedEntryId.value = id

  // 标记用户正在选择字幕，300ms 内音频 watch 不会自动更新选择
  isUserSelectingEntry.value = true
  if (userSelectionTimer) {
    clearTimeout(userSelectionTimer)
  }
  userSelectionTimer = setTimeout(() => {
    isUserSelectingEntry.value = false
    userSelectionTimer = null
  }, 300)

  // 如果加载了音频，跳转音频到该字幕的开始时间
  if (hasAudio.value) {
    const entry = subtitleStore.entries.find((e) => e.id === id)
    if (entry) {
      // 将时间戳转换为毫秒，再转换为秒数
      const timeMs = timeStampToMs(entry.startTime)
      const timeSeconds = timeMs / 1000
      audioStore.seek(timeSeconds)
    }
  }
}

// 添加字幕
const handleAddEntry = () => {
  // 如果正在播放，暂停
  if (audioStore.playerState.isPlaying) {
    audioStore.pause()
  }

  subtitleStore.addEntry()

  // 选中新添加的字幕
  const newEntry = subtitleStore.entries[subtitleStore.entries.length - 1]
  if (newEntry) {
    selectedEntryId.value = newEntry.id
  }
}

// 删除字幕
const handleDeleteEntry = async () => {
  if (!currentEntry.value) return

  const currentId = currentEntry.value.id

  try {
    // 显示确认对话框
    await ElMessageBox.confirm(
      `删除后无法恢复，确定删除字幕 #${currentId} 吗？`,
      '删除确认',
      {
        confirmButtonText: '删除',
        cancelButtonText: '取消',
        type: 'warning',
      }
    )

    // 如果正在播放，暂停
    if (audioStore.playerState.isPlaying) {
      audioStore.pause()
    }

    const currentIndex = subtitleStore.entries.findIndex((e) => e.id === currentId)

    subtitleStore.deleteEntry(currentId)

    // 保存文件
    if (subtitleStore.currentFilePath) {
      await subtitleStore.saveToFile()
    }

    // 选中下一条或上一条字幕
    if (subtitleStore.entries.length > 0) {
      const nextEntry = subtitleStore.entries[currentIndex] || subtitleStore.entries[currentIndex - 1]
      if (nextEntry) {
        selectedEntryId.value = nextEntry.id
      }
    } else {
      selectedEntryId.value = null
    }

    ElMessage.success({
      message: '已删除',
      duration: 1500,
    })
  } catch {
    // 用户点击了取消
  }
}

// 复制字幕文本（带防抖，防止快捷键重复触发）
let lastCopyTime = 0
const copySubtitleText = async (id: number) => {
  // 防抖：300ms 内不重复触发
  const now = Date.now()
  if (now - lastCopyTime < 300) {
    return
  }
  lastCopyTime = now

  const entry = subtitleStore.entries.find((e) => e.id === id)
  if (!entry) return

  try {
    await navigator.clipboard.writeText(entry.text)
    ElMessage.success({
      message: '已复制',
      duration: 1500,
    })
  } catch (error) {
    ElMessage.error({
      message: '复制失败',
      duration: 1500,
    })
  }
}

// 播放字幕音频
const playSubtitleAudio = (id: number) => {
  if (!hasAudio.value) return

  const entry = subtitleStore.entries.find((e) => e.id === id)
  if (!entry) return

  // 将时间戳转换为毫秒，再转换为秒数
  const timeMs = timeStampToMs(entry.startTime)
  const timeSeconds = timeMs / 1000

  // 跳转到字幕的开始时间并播放
  audioStore.seek(timeSeconds)
  audioStore.play()
}

// 删除字幕项目（从列表中快速删除）
const deleteSubtitleItem = async (id: number) => {
  const entry = subtitleStore.entries.find((e) => e.id === id)
  if (!entry) return

  try {
    // 显示确认对话框
    await ElMessageBox.confirm(
      `删除后无法恢复，确定删除字幕 #${id} 吗？`,
      '删除确认',
      {
        confirmButtonText: '删除',
        cancelButtonText: '取消',
        type: 'warning',
      }
    )

    // 如果正在播放，暂停
    if (audioStore.playerState.isPlaying) {
      audioStore.pause()
    }

    // 用户点击了确认
    const currentIndex = subtitleStore.entries.findIndex((e) => e.id === id)

    subtitleStore.deleteEntry(id)

    // 保存文件
    if (subtitleStore.currentFilePath) {
      await subtitleStore.saveToFile()
    }

    // 如果删除的是当前选中的字幕，选中下一条或上一条
    if (selectedEntryId.value === id) {
      if (subtitleStore.entries.length > 0) {
        const nextEntry = subtitleStore.entries[currentIndex] || subtitleStore.entries[currentIndex - 1]
        if (nextEntry) {
          selectedEntryId.value = nextEntry.id
        }
      } else {
        selectedEntryId.value = null
      }
    }

    ElMessage.success({
      message: '已删除',
      duration: 1500,
    })
  } catch (error) {
    // 用户点击了取消，或其他错误
    if (error instanceof Error && error.message !== 'cancel') {
      ElMessage.error({
        message: '删除失败',
        duration: 1500,
      })
    }
  }
}

// 移除 HTML 标签
const handleRemoveHTML = () => {
  // 如果正在播放，暂停
  if (audioStore.playerState.isPlaying) {
    audioStore.pause()
  }

  subtitleStore.removeHTMLTags()
  if (currentEntry.value) {
    editingText.value = currentEntry.value.text
  }
}

// 为当前字幕添加中英文空格
const handleAddCJKSpaces = () => {
  if (!currentEntry.value) return

  // 如果正在播放，暂停
  if (audioStore.playerState.isPlaying) {
    audioStore.pause()
  }

  subtitleStore.addSpacesForEntry(currentEntry.value.id)
  editingText.value = currentEntry.value.text
}

// 为当前字幕删除标点符号
const handleRemovePunctuation = () => {
  if (!currentEntry.value) return

  // 如果正在播放，暂停
  if (audioStore.playerState.isPlaying) {
    audioStore.pause()
  }

  subtitleStore.removePunctuationForEntry(currentEntry.value.id)
  editingText.value = currentEntry.value.text
}

// 批量添加中英文空格（供菜单调用）
const handleBatchAddCJKSpaces = async () => {
  // 如果正在播放，暂停
  if (audioStore.playerState.isPlaying) {
    audioStore.pause()
  }

  subtitleStore.addSpacesBetweenCJKAndAlphanumeric()
  if (currentEntry.value) {
    editingText.value = currentEntry.value.text
  }

  // 保存文件
  if (subtitleStore.currentFilePath) {
    try {
      await subtitleStore.saveToFile()
      ElMessage.success({ message: '已批量添加中英文空格', duration: 1500 })
    } catch (error) {
      // 保存失败，静默处理
    }
  }
}

// 批量删除标点符号（供菜单调用）
const handleBatchRemovePunctuation = async () => {
  // 如果正在播放，暂停
  if (audioStore.playerState.isPlaying) {
    audioStore.pause()
  }

  subtitleStore.removePunctuation()
  if (currentEntry.value) {
    editingText.value = currentEntry.value.text
  }

  // 保存文件
  if (subtitleStore.currentFilePath) {
    try {
      await subtitleStore.saveToFile()
      ElMessage.success({ message: '已批量删除标点符号', duration: 1500 })
    } catch (error) {
      // 保存失败，静默处理
    }
  }
}

// 批量移除HTML标签（供菜单调用）
const handleBatchRemoveHTML = async () => {
  // 如果正在播放，暂停
  if (audioStore.playerState.isPlaying) {
    audioStore.pause()
  }

  subtitleStore.removeHTMLTags()
  if (currentEntry.value) {
    editingText.value = currentEntry.value.text
  }

  // 保存文件
  if (subtitleStore.currentFilePath) {
    try {
      await subtitleStore.saveToFile()
      ElMessage.success({ message: '已批量移除HTML标签', duration: 1500 })
    } catch (error) {
      // 保存失败，静默处理
    }
  }
}

// 处理时间输入变化
const handleTimeChange = async (type: 'start' | 'end') => {
  if (!currentEntry.value) return

  try {
    // 验证时间格式 (HH:MM:SS,mmm)
    const timeRegex = /^(\d{2}):(\d{2}):(\d{2}),(\d{3})$/
    const timeValue = type === 'start' ? editingStartTime.value : editingEndTime.value

    if (!timeRegex.test(timeValue)) {
      ElMessage.warning({
        message: '时间格式不正确，应为 HH:MM:SS,mmm',
        duration: 2000,
      })
      // 恢复原始值
      if (type === 'start') {
        editingStartTime.value = subtitleStore.formatTimeStamp(currentEntry.value.startTime)
      } else {
        editingEndTime.value = subtitleStore.formatTimeStamp(currentEntry.value.endTime)
      }
      return
    }

    // 解析时间字符串为 TimeStamp 对象
    const match = timeValue.match(timeRegex)!
    const newTime: TimeStamp = {
      hours: parseInt(match[1] || '0'),
      minutes: parseInt(match[2] || '0'),
      seconds: parseInt(match[3] || '0'),
      milliseconds: parseInt(match[4] || '0')
    }

    // 如果正在播放，暂停
    if (audioStore.playerState.isPlaying) {
      audioStore.pause()
    }

    // 更新时间（输入框修改，需要记录历史）
    if (type === 'start') {
      subtitleStore.updateEntryTime(currentEntry.value.id, newTime, undefined, true)
    } else {
      subtitleStore.updateEntryTime(currentEntry.value.id, undefined, newTime, true)
    }

    // 保存文件
    if (subtitleStore.currentFilePath) {
      await subtitleStore.saveToFile()
    }
  } catch (error) {
    ElMessage.error({
      message: '时间更新失败',
      duration: 2000,
    })
    // 恢复原始值
    if (type === 'start' && currentEntry.value) {
      editingStartTime.value = subtitleStore.formatTimeStamp(currentEntry.value.startTime)
    } else if (currentEntry.value) {
      editingEndTime.value = subtitleStore.formatTimeStamp(currentEntry.value.endTime)
    }
  }
}

// 微调时间（增加或减少指定毫秒数）
const adjustTime = async (type: 'start' | 'end', deltaMs: number) => {
  if (!currentEntry.value) return

  try {
    // 如果正在播放，暂停
    if (audioStore.playerState.isPlaying) {
      audioStore.pause()
    }

    // 获取当前时间并转换为毫秒
    const currentTime = type === 'start' ? currentEntry.value.startTime : currentEntry.value.endTime
    let totalMs = timeStampToMs(currentTime)

    // 添加增量
    totalMs += deltaMs

    // 确保时间不为负
    if (totalMs < 0) {
      totalMs = 0
    }

    // 转换回 TimeStamp 格式
    const hours = Math.floor(totalMs / 3600000)
    const minutes = Math.floor((totalMs % 3600000) / 60000)
    const seconds = Math.floor((totalMs % 60000) / 1000)
    const milliseconds = totalMs % 1000

    const newTime: TimeStamp = {
      hours,
      minutes,
      seconds,
      milliseconds
    }

    // 更新时间（微调按钮，需要记录历史）
    if (type === 'start') {
      subtitleStore.updateEntryTime(currentEntry.value.id, newTime, undefined, true)
      editingStartTime.value = subtitleStore.formatTimeStamp(newTime)
    } else {
      subtitleStore.updateEntryTime(currentEntry.value.id, undefined, newTime, true)
      editingEndTime.value = subtitleStore.formatTimeStamp(newTime)
    }

    // 保存文件
    if (subtitleStore.currentFilePath) {
      await subtitleStore.saveToFile()
    }
  } catch (error) {
    ElMessage.error({
      message: '时间调整失败',
      duration: 2000,
    })
  }
}

// 处理波形点击跳转
const handleWaveformSeek = (time: number) => {
  audioStore.seek(time)
}

// 处理字幕时间更新（从波形 Region 拖拽）
const handleSubtitleUpdate = (id: number, startTime: TimeStamp, endTime: TimeStamp) => {
  console.log(`📝 Updating subtitle #${id} from waveform:`, { startTime, endTime })

  const entry = subtitleStore.entries.find((e) => e.id === id)
  if (!entry) {
    console.warn(`⚠️ Subtitle #${id} not found`)
    return
  }

  // 如果正在播放，暂停
  if (audioStore.playerState.isPlaying) {
    audioStore.pause()
  }

  // 更新字幕时间
  subtitleStore.updateEntryTime(id, startTime, endTime)

  // 自动保存
  if (subtitleStore.currentFilePath) {
    subtitleStore.saveToFile().catch((error) => {
      // 保存失败，静默处理
    })
  }
}

// 处理批量字幕时间更新
const handleSubtitlesUpdate = (updates: Array<{ id: number; startTime: TimeStamp; endTime: TimeStamp }>) => {
  console.log(`📝 Batch updating ${updates.length} subtitles from waveform`)

  // 如果正在播放，暂停
  if (audioStore.playerState.isPlaying) {
    audioStore.pause()
  }

  // 批量更新字幕时间
  updates.forEach(({ id, startTime, endTime }) => {
    subtitleStore.updateEntryTime(id, startTime, endTime)
  })

  // 自动保存
  if (subtitleStore.currentFilePath) {
    subtitleStore.saveToFile().catch((error) => {
      // 保存失败，静默处理
    })
  }
}

// 处理字幕选择变化
const handleSubtitlesSelect = (ids: number[]) => {
  // 可以在这里处理选择变化，比如更新 UI
  // 目前主要用于多选状态同步
}

// 处理拖动开始（记录原始时间）
const handleDragStart = (ids: number[]) => {
  subtitleStore.startDragging(ids)
}

// 处理拖动结束（记录历史）
const handleDragEnd = () => {
  subtitleStore.endDragging()
}

// 聚焦到字幕编辑区的文本输入框
const focusSubtitleTextarea = async () => {
  // 延迟焦点设置，让 DOM 有时间更新
  await nextTick()

  // 获取 el-input 组件并聚焦
  if (textareaInputRef.value) {
    // el-input 组件提供了 focus 方法
    textareaInputRef.value.focus()

    // 延迟设置光标位置，确保获得焦点后再设置
    await nextTick()
    const textarea = textareaInputRef.value.textarea as HTMLTextAreaElement
    if (textarea) {
      // 将光标放在文字的末尾
      const textLength = textarea.value.length
      textarea.setSelectionRange(textLength, textLength)
    }
  }
}

// 处理波形下字幕块的双击 - 跳转到编辑区并聚焦
const handleWaveformDoubleClick = async (id: number) => {
  // 确保字幕已被选中
  selectEntry(id)
  await focusSubtitleTextarea()
}

// 处理字幕列表项的双击 - 聚焦到编辑区
const handleSubtitleDoubleClick = async (id: number) => {
  // 字幕已经在单击时被选中，直接聚焦到编辑区
  await focusSubtitleTextarea()
}

// WaveformViewer ref
const waveformViewerRef = ref<InstanceType<typeof WaveformViewer> | null>(null)

// 计算当前缩放百分比
const waveformZoomLevel = computed(() => {
  return waveformViewerRef.value ? Math.round(waveformViewerRef.value.zoomLevel * 100) : 100
})

// 判断缩放按钮是否禁用
const canZoomIn = computed(() => {
  return waveformViewerRef.value ? waveformViewerRef.value.zoomLevel < 1.0 : false
})

const canZoomOut = computed(() => {
  return waveformViewerRef.value ? waveformViewerRef.value.zoomLevel > 0.5 : false
})

// 缩放控制
const handleZoomIn = () => {
  if (canZoomIn.value) {
    waveformViewerRef.value?.zoomIn()
  }
}

const handleZoomOut = () => {
  if (canZoomOut.value) {
    waveformViewerRef.value?.zoomOut()
  }
}

// 打开设置
const openSettings = () => {
  showSettingsDialog.value = true
}

// 添加字幕
const openSubtitle = async () => {
  // 如果正在播放，暂停
  if (audioStore.playerState.isPlaying) {
    audioStore.pause()
  }

  // 如果有选中的字幕，在其后面插入；否则添加到末尾
  const afterId = selectedEntryId.value ?? undefined
  const newId = subtitleStore.addEntry(afterId)

  // 选中新添加的字幕
  selectedEntryId.value = newId

  // 保存文件
  if (subtitleStore.currentFilePath) {
    try {
      await subtitleStore.saveToFile()
    } catch (error) {
      // 保存失败，静默处理
    }
  }
}

// 剪刀模式：分割字幕
const handleScissor = () => {
  if (!hasAudio.value) {
    ElMessage.warning('请先加载音频文件')
    return
  }

  // 切换剪刀模式
  isScissorMode.value = !isScissorMode.value
}

// 对齐字幕到波形（同时调整开始和结束时间）
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

  // 如果正在播放，暂停
  if (audioStore.playerState.isPlaying) {
    audioStore.pause()
  }

  const entry = currentEntry.value
  const currentStartMs = timestampToMs(entry.startTime)
  const currentEndMs = timestampToMs(entry.endTime)
  const duration = audioStore.playerState.duration

  // 查找最近的语音区域
  const voiceRegion = findVoiceRegion(
    waveformData,
    duration,
    currentStartMs,
    currentEndMs,
    2000 // 搜索窗口 ±2秒
  )

  if (!voiceRegion) {
    ElMessage.warning('未找到附近的语音区域')
    return
  }

  // 更新字幕时间
  const newStartTime = msToTimestamp(voiceRegion.startMs)
  const newEndTime = msToTimestamp(voiceRegion.endMs)

  subtitleStore.updateEntryTime(entry.id, newStartTime, newEndTime, true)

  // 更新编辑区显示
  editingStartTime.value = subtitleStore.formatTimeStamp(newStartTime)
  editingEndTime.value = subtitleStore.formatTimeStamp(newEndTime)

  // 保存文件
  if (subtitleStore.currentFilePath) {
    await subtitleStore.saveToFile()
  }

  ElMessage.success('已对齐到波形')
}

// 处理字幕分割
const handleSplitSubtitle = async (id: number, splitTimeMs: number) => {
  // 如果正在播放，暂停
  if (audioStore.playerState.isPlaying) {
    audioStore.pause()
  }

  const newId = subtitleStore.splitEntry(id, splitTimeMs)

  if (newId) {
    // 选中新分割出的字幕
    selectedEntryId.value = newId

    // 保存文件
    if (subtitleStore.currentFilePath) {
      try {
        await subtitleStore.saveToFile()
      } catch (error) {
        // 保存失败，静默处理
      }
    }
  }

  // 退出剪刀模式
  isScissorMode.value = false
}

// 返回欢迎页
const goBack = async () => {
  // 清理音频状态
  if (audioStore.currentAudio) {
    audioStore.unloadAudio()
  }

  // 清理字幕状态
  subtitleStore.$reset()

  // 清理本地状态
  searchText.value = ''
  replaceText.value = ''
  showReplace.value = false
  selectedEntryId.value = null
  editingText.value = ''

  // 返回欢迎页
  router.push('/')
}

// 检测平台特定的快捷键修饰符
const getKeyModifier = (e: KeyboardEvent): string => {
  const isMac = /Mac|iPhone|iPad|iPod/.test(navigator.platform)
  if (isMac && e.metaKey) return 'Cmd+'
  if (e.ctrlKey) return 'Ctrl+'
  return ''
}

// 规范化键名（处理大小写和特殊键）
const normalizeKeyName = (key: string): string => {
  const keyMap: Record<string, string> = {
    'o': 'o',
    'O': 'o',
    's': 's',
    'S': 's',
    'z': 'z',
    'Z': 'z',
    'f': 'f',
    'F': 'f',
    'n': 'n',
    'N': 'n',
  }
  return keyMap[key] || key.toLowerCase()
}

// 构建快捷键字符串（考虑平台差异）
const buildKeyString = (e: KeyboardEvent): string => {
  const modifier = getKeyModifier(e)

  // 对于 Shift 的处理
  let baseKey = normalizeKeyName(e.key)
  if (e.shiftKey && modifier) {
    baseKey = `Shift+${baseKey}`
  }

  return `${modifier}${baseKey}`
}

// 高亮搜索结果中的匹配文本
const highlightSearchText = (text: string, searchQuery: string): string => {
  if (!searchQuery) return text

  try {
    // 使用全局忽略大小写的正则表达式来替换所有匹配的文本
    const regex = new RegExp(`(${searchQuery.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')})`, 'gi')
    return text.replace(regex, '<mark>$1</mark>')
  } catch {
    // 如果正则表达式失败，返回原始文本
    return text
  }
}

// 键盘导航字幕列表
const navigateSubtitleList = (direction: 'up' | 'down') => {
  if (filteredEntries.value.length === 0) return

  let targetIndex = -1

  if (selectedEntryId.value === null) {
    // 如果没有选中任何字幕，选中第一个（向下）或最后一个（向上）
    targetIndex = direction === 'down' ? 0 : filteredEntries.value.length - 1
  } else {
    // 找到当前选中字幕在过滤列表中的位置
    const currentIndex = filteredEntries.value.findIndex(e => e.id === selectedEntryId.value)

    if (currentIndex !== -1) {
      if (direction === 'down') {
        // 向下，移动到下一个（如果已经在最后，保持不变）
        targetIndex = Math.min(currentIndex + 1, filteredEntries.value.length - 1)
      } else {
        // 向上，移动到上一个（如果已经在最前，保持不变）
        targetIndex = Math.max(currentIndex - 1, 0)
      }
    } else {
      // 如果当前选中的字幕不在过滤列表中，选择第一个或最后一个
      targetIndex = direction === 'down' ? 0 : filteredEntries.value.length - 1
    }
  }

  if (targetIndex !== -1) {
    const targetEntry = filteredEntries.value[targetIndex]
    if (targetEntry) {
      selectEntry(targetEntry.id)

      // 自动滚动字幕列表，使目标字幕保持在可见范围内
      nextTick(() => {
        const itemElement = subtitleItemRefs[targetEntry.id]
        const containerElement = subtitleListContainer.value
        if (itemElement && containerElement) {
          itemElement.scrollIntoView({ behavior: 'smooth', block: 'nearest' })
        }
      })
    }
  }
}

// 键盘快捷键
const handleKeydown = (e: KeyboardEvent) => {
  // ESC 键退出剪刀模式
  if (e.key === 'Escape' && isScissorMode.value) {
    e.preventDefault()
    isScissorMode.value = false
    return
  }

  const target = e.target as HTMLElement

  // 检查是否在文本输入框内
  const isInTextInput =
    target instanceof HTMLTextAreaElement ||
    target instanceof HTMLInputElement

  // 检查是否在搜索框内
  const isInSearchInput = target === searchInputRef.value?.$el?.querySelector('input')

  // 检查是否在替换框内
  const isInReplaceInput = target === replaceInputRef.value?.$el?.querySelector('input')

  const shortcuts = configStore.keyboardShortcuts
  const pressedKey = buildKeyString(e)

  // 如果在文本框内，只处理保存和打开快捷键
  if (isInTextInput) {
    if (shortcuts.save === pressedKey) {
      e.preventDefault()
      handleSave()
    } else if (shortcuts.open === pressedKey) {
      e.preventDefault()
      handleOpenFile()
    } else if (shortcuts.find === pressedKey) {
      // 在任意输入框内按 Cmd+F/Ctrl+F，打开搜索面板并聚焦
      e.preventDefault()
      showSearchPanel.value = true
      nextTick(() => {
        searchInputRef.value?.focus()
      })
    } else if (e.key === 'Escape') {
      // 在搜索/替换输入框内按 ESC 时，关闭搜索面板
      e.preventDefault()
      if (isInSearchInput || isInReplaceInput) {
        searchText.value = ''
        replaceText.value = ''
        showReplace.value = false
        showSearchPanel.value = false
        searchInputRef.value?.blur()
        replaceInputRef.value?.blur()
      }
    } else if ((e.key === 'ArrowDown' || e.key === 'ArrowUp') && isInSearchInput) {
      // 在搜索框内按上下箭头时，失焦并导航字幕列表
      e.preventDefault()
      searchInputRef.value?.blur()
      navigateSubtitleList(e.key === 'ArrowDown' ? 'down' : 'up')
      return
    }
    // 不处理其他快捷键，允许正常输入（包括空格）
    return
  }

  // 不在文本框内，处理全局快捷键和导航
  if (shortcuts.save === pressedKey) {
    e.preventDefault()
    handleSave()
  } else if (shortcuts.open === pressedKey) {
    e.preventDefault()
    handleOpenFile()
  } else if (shortcuts.find === pressedKey) {
    // Command+F 或 Ctrl+F：打开搜索面板并聚焦搜索输入框
    e.preventDefault()
    showSearchPanel.value = true
    nextTick(() => {
      searchInputRef.value?.focus()
    })
  } else if (pressedKey === 'Cmd+r' || pressedKey === 'Ctrl+r') {
    // Command+R 或 Ctrl+R：打开搜索面板、展示替换框并聚焦搜索输入框
    e.preventDefault()
    showSearchPanel.value = true
    showReplace.value = true
    nextTick(() => {
      searchInputRef.value?.focus()
    })
  } else if (shortcuts.copy === pressedKey) {
    // Command+C 或 Ctrl+C：复制当前选中字幕
    e.preventDefault()
    if (currentEntry.value) {
      copySubtitleText(currentEntry.value.id)
    }
  } else if (shortcuts.playPause === pressedKey.toLowerCase()) {
    e.preventDefault()
    audioStore.togglePlay()
  } else if (shortcuts.addEntry === pressedKey) {
    e.preventDefault()
    openSubtitle()
  } else if (shortcuts.deleteEntry === pressedKey) {
    e.preventDefault()
    handleDeleteEntry()
  } else if (hasAudio.value && (pressedKey === 'Cmd+=' || pressedKey === 'Cmd++' || pressedKey === 'Ctrl+=')) {
    // macOS: Cmd+=, Windows/Linux: Ctrl+=
    e.preventDefault()
    handleZoomIn()
  } else if (hasAudio.value && (pressedKey === 'Cmd+-' || pressedKey === 'Ctrl+-')) {
    // macOS: Cmd+-, Windows/Linux: Ctrl+-
    e.preventDefault()
    handleZoomOut()
  } else if (e.key === 'ArrowDown') {
    // 向下箭头：在列表中向下导航
    e.preventDefault()
    navigateSubtitleList('down')
  } else if (e.key === 'ArrowUp') {
    // 向上箭头：在列表中向上导航
    e.preventDefault()
    navigateSubtitleList('up')
  } else if (e.key === 'x' || e.key === 'X') {
    // x 键：开启/关闭分割模式
    e.preventDefault()
    handleScissor()
  } else if (pressedKey === 'Cmd+,' || pressedKey === 'Ctrl+,') {
    // Command+逗号 或 Ctrl+逗号：打开设置
    e.preventDefault()
    openSettings()
  } else if (shortcuts.undo === pressedKey) {
    // 撤销
    e.preventDefault()
    subtitleStore.undo()
  } else if (shortcuts.redo === pressedKey) {
    // 重做
    e.preventDefault()
    subtitleStore.redo()
  }
}
</script>

<template>
  <div class="editor-page">
    <!-- 标题栏区域（含标签页） -->
    <TitleBar />

    <!-- 时间轴区域：顶部全宽 -->
    <div v-if="hasAudio || audioStore.isGeneratingWaveform" class="timeline-section">
      <!-- 一体化控制栏：音频名称 + 缩放 + 播放 + 时长 + 音量 + 速度 -->
      <div v-if="hasAudio" class="timeline-unified-controls">
        <!-- 左侧组 -->
        <div class="controls-left">
          <span class="audio-name-compact">{{ audioStore.currentAudio?.name }}</span>
          <el-button text size="small" type="danger" @click="handleRemoveAudio">删除</el-button>

          <div class="divider"></div>

          <!-- 缩放控制 -->
          <span class="control-label-mini">缩放</span>
          <el-button size="small" @click="handleZoomOut" class="zoom-btn" :disabled="!canZoomOut">−</el-button>
          <span class="zoom-display">{{ waveformZoomLevel }}%</span>
          <el-button size="small" @click="handleZoomIn" class="zoom-btn" :disabled="!canZoomIn">+</el-button>
        </div>

        <!-- 中间播放控制（居中）-->
        <div class="controls-center">
          <span class="time-display-mini">{{ audioStore.formatTime(audioStore.playerState.currentTime) }}</span>
          <el-button
            circle
            size="small"
            type="primary"
            @click="audioStore.togglePlay()"
            class="play-button-mini"
          >
            {{ audioStore.playerState.isPlaying ? '⏸' : '▶' }}
          </el-button>
          <span class="time-display-mini">{{ audioStore.formatTime(audioStore.playerState.duration) }}</span>
        </div>

        <!-- 右侧组 -->
        <div class="controls-right">
          <!-- 音量控制 -->
          <span class="control-label-mini">音量</span>
          <el-slider
            v-model="audioStore.playerState.volume"
            :max="1"
            :step="0.01"
            :show-tooltip="false"
            class="volume-slider-mini"
            @input="(val: number) => audioStore.setVolume(val)"
          />
          <span class="param-value-mini">{{ Math.round(audioStore.playerState.volume * 100) }}%</span>

          <div class="divider"></div>

          <!-- 速度控制 -->
          <span class="control-label-mini">速度</span>
          <el-button
            v-for="rate in [0.5, 1, 1.5, 2]"
            :key="rate"
            :type="audioStore.playerState.playbackRate === rate ? 'primary' : 'default'"
            size="small"
            @click="audioStore.setPlaybackRate(rate)"
            class="speed-btn-mini"
          >
            {{ rate }}x
          </el-button>
        </div>
      </div>
      <!-- 波形生成时的简化控制栏 -->
      <div v-else-if="audioStore.isGeneratingWaveform" class="timeline-unified-controls loading-controls">
        <span class="loading-audio-text">正在加载音频...</span>
      </div>

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
        @seek="handleWaveformSeek"
        @update-subtitle="handleSubtitleUpdate"
        @update-subtitles="handleSubtitlesUpdate"
        @select-subtitles="handleSubtitlesSelect"
        @double-click-subtitle="handleWaveformDoubleClick"
        @split-subtitle="handleSplitSubtitle"
        @drag-start="handleDragStart"
        @drag-end="handleDragEnd"
      />
    </div>

    <!-- 音频加载占位符 - 优化空状态 -->
    <div v-else class="timeline-placeholder">
      <div class="audio-empty-state">
        <div class="audio-empty-icon">
          <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
            <path d="M9 18V5l12-2v13" stroke-linecap="round" stroke-linejoin="round"/>
            <circle cx="6" cy="18" r="3"/>
            <circle cx="18" cy="16" r="3"/>
          </svg>
        </div>
        <div class="audio-empty-text">
          <span class="audio-empty-title">加载音频以启用波形预览</span>
          <span class="audio-empty-hint">支持 MP3、WAV、OGG、FLAC、M4A、AAC 格式</span>
        </div>
        <el-button type="primary" size="small" @click="handleOpenAudio" class="audio-load-btn">
          <el-icon style="margin-right: 4px;"><Plus /></el-icon>
          选择音频文件
        </el-button>
      </div>
    </div>

    <!-- 主内容区：左右分栏 -->
    <div class="content-area">
      <!-- 左侧侧边栏：图标导航 -->
      <div class="sidebar">
        <div class="sidebar-top">
          <button
            class="sidebar-btn"
            @click="openSubtitle"
            title="添加字幕"
          >
            <el-icon><DocumentAdd /></el-icon>
          </button>
          <button
            class="sidebar-btn"
            :class="{ active: showSearchPanel }"
            @click="toggleSearchPanel"
            title="搜索字幕"
          >
            <el-icon><Search /></el-icon>
          </button>
          <button
            class="sidebar-btn"
            :class="{ active: isScissorMode }"
            @click="handleScissor"
            title="分割字幕 (X)"
          >
            <el-icon><Scissor /></el-icon>
          </button>
          <button
            class="sidebar-btn"
            @click="handleAlignToWaveform"
            :disabled="!hasAudio || !currentEntry"
            title="对齐到波形"
          >
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M2 12h4l3-9 4 18 3-9h6"/>
            </svg>
          </button>
        </div>
        <div class="sidebar-bottom">
          <button
            class="sidebar-btn"
            @click="openSettings"
            title="设置"
          >
            <el-icon><Setting /></el-icon>
          </button>
        </div>
      </div>

      <!-- 左侧：字幕列表 -->
      <div class="subtitle-list-panel">
        <!-- 搜索和替换框 -->
        <div v-if="showSearchPanel" class="search-replace-container">
          <!-- 搜索框 -->
          <div class="search-row">
            <el-input
              ref="searchInputRef"
              v-model="searchText"
              placeholder="搜索字幕"
              clearable
              class="search-input"
              size="small"
            >
              <template #prefix>
                <el-icon class="search-icon"><Search /></el-icon>
              </template>
            </el-input>
            <span v-if="searchText && subtitleStore.searchResults.length > 0" class="match-count">
              {{ subtitleStore.searchResults.length }}
            </span>
            <button
              class="expand-btn"
              :class="{ expanded: showReplace }"
              @click="showReplace = !showReplace"
              :title="showReplace ? '收起替换' : '展开替换'"
            >
              <el-icon><ArrowDown /></el-icon>
            </button>
          </div>

          <!-- 替换框 -->
          <div v-if="showReplace" class="replace-row">
            <el-input
              ref="replaceInputRef"
              v-model="replaceText"
              placeholder="替换为..."
              clearable
              class="replace-input"
              size="small"
            >
              <template #prefix>
                <el-icon class="replace-icon"><Switch /></el-icon>
              </template>
            </el-input>
            <button
              class="replace-btn"
              @click="replaceOne"
              :disabled="!searchText || subtitleStore.searchResults.length === 0"
              title="替换当前"
            >
              替换
            </button>
            <button
              class="replace-btn replace-all-btn"
              @click="replaceAll"
              :disabled="!searchText"
              title="全部替换"
            >
              全部
            </button>
          </div>
        </div>

        <!-- 字幕列表 -->
        <div class="subtitle-list" ref="subtitleListContainer">
          <div
            v-for="entry in filteredEntries"
            :key="entry.id"
            :ref="(el) => { if (el) subtitleItemRefs[entry.id] = el as HTMLElement }"
            class="subtitle-item"
            :class="{
              'is-selected': selectedEntryId === entry.id
            }"
            @click="selectEntry(entry.id)"
            @dblclick="handleSubtitleDoubleClick(entry.id)"
          >
            <div class="item-header">
              <span class="item-number">{{ entry.id }}</span>
              <span class="item-time">
                {{ subtitleStore.formatTimeStamp(entry.startTime).slice(0, 8) }}
                -
                {{ subtitleStore.formatTimeStamp(entry.endTime).slice(0, 8) }}
              </span>
            </div>

            <!-- 文本和操作按钮在同一行 -->
            <div class="item-content">
              <div class="item-text-wrapper">
                <div class="item-text" v-if="searchText" v-html="highlightSearchText(entry.text, searchText)"></div>
                <div class="item-text" v-else>{{ entry.text }}</div>
              </div>

              <!-- 操作按钮 -->
              <div class="item-actions">
                <el-button
                  link
                  type="primary"
                  size="small"
                  title="复制文本"
                  @click.stop="copySubtitleText(entry.id)"
                >
                  <template #icon>
                    <DocumentCopy />
                  </template>
                </el-button>
                <el-button
                  v-if="hasAudio"
                  link
                  type="primary"
                  size="small"
                  title="播放字幕音频"
                  @click.stop="playSubtitleAudio(entry.id)"
                >
                  <template #icon>
                    <VideoPlay />
                  </template>
                </el-button>
                <el-button
                  link
                  type="danger"
                  size="small"
                  title="删除字幕"
                  @click.stop="deleteSubtitleItem(entry.id)"
                >
                  <template #icon>
                    <Delete />
                  </template>
                </el-button>
              </div>
            </div>
          </div>

          <!-- 空状态 -->
          <div v-if="filteredEntries.length === 0 && hasContent" class="empty-state">
            <p class="text-gray-400">未找到匹配的字幕</p>
          </div>

          <div v-if="!hasContent" class="empty-state">
            <p class="text-gray-400">暂无字幕数据</p>
            <el-button type="text" @click="goBack">返回加载文件</el-button>
          </div>
        </div>

        <!-- 底部统计:字幕文件名 + 字幕数量 -->
        <div class="list-footer">
          <span class="file-info">
            {{ subtitleStore.currentFilePath ? subtitleStore.currentFilePath.split('/').pop()?.replace('.srt', '') : '豆包输入法' }}.srt
          </span>
          <span v-if="selectedEntryId" class="count-info">
            {{ filteredEntries.findIndex(e => e.id === selectedEntryId) + 1 }}/{{ filteredEntries.length }} 字幕
          </span>
          <span v-else class="count-info">
            0/{{ filteredEntries.length }} 字幕
          </span>
        </div>
      </div>

      <!-- 右侧：字幕编辑区 -->
      <div class="subtitle-edit-panel">
        <!-- 字幕编辑区 -->
        <div v-if="currentEntry" class="subtitle-edit-section">
          <!-- 编辑头部 -->
          <div class="edit-header">
            <div class="edit-header-left">
              <span class="edit-badge">#{{ currentEntry.id }}</span>
              <h3 class="edit-title">编辑字幕</h3>
            </div>
            <button class="delete-entry-btn" @click="handleDeleteEntry" title="删除此字幕">
              <el-icon><Delete /></el-icon>
            </button>
          </div>

          <!-- 时间设置 - 紧凑单行布局 -->
          <div class="time-row">
            <!-- 开始时间 -->
            <div class="time-block">
              <span class="time-label">开始</span>
              <div class="time-control">
                <button class="time-btn-sm" @click="adjustTime('start', -100)" title="-100ms">−</button>
                <el-input
                  v-model="editingStartTime"
                  class="time-input-sm"
                  size="small"
                  @blur="() => handleTimeChange('start')"
                  @keyup.enter="() => handleTimeChange('start')"
                />
                <button class="time-btn-sm" @click="adjustTime('start', 100)" title="+100ms">+</button>
              </div>
            </div>

            <span class="time-separator">→</span>

            <!-- 结束时间 -->
            <div class="time-block">
              <span class="time-label">结束</span>
              <div class="time-control">
                <button class="time-btn-sm" @click="adjustTime('end', -100)" title="-100ms">−</button>
                <el-input
                  v-model="editingEndTime"
                  class="time-input-sm"
                  size="small"
                  @blur="() => handleTimeChange('end')"
                  @keyup.enter="() => handleTimeChange('end')"
                />
                <button class="time-btn-sm" @click="adjustTime('end', 100)" title="+100ms">+</button>
              </div>
            </div>

            <!-- 时长 -->
            <div class="time-block duration-block">
              <span class="time-label">时长</span>
              <span class="duration-value-sm">
                {{ `00:${String(Math.floor((subtitleStore.formatTimeStamp(currentEntry.endTime).slice(6, 8) as any) - (subtitleStore.formatTimeStamp(currentEntry.startTime).slice(6, 8) as any))).padStart(2, '0')},000` }}
              </span>
            </div>
          </div>

          <!-- 文本编辑卡片 - 重新设计 -->
          <div class="text-edit-card">
            <div class="text-card-header">
              <div class="text-header-left">
                <svg class="text-icon" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/>
                  <polyline points="14,2 14,8 20,8"/>
                  <line x1="16" y1="13" x2="8" y2="13"/>
                  <line x1="16" y1="17" x2="8" y2="17"/>
                </svg>
                <span class="text-card-title">字幕内容</span>
              </div>
              <span class="char-count">{{ editingText.length }} 字符</span>
            </div>
            <div class="text-input-wrapper">
              <el-input
                ref="textareaInputRef"
                v-model="editingText"
                placeholder="在此输入字幕文本..."
                @focus="isUserEditing = true"
                @blur="handleTextareaBlur"
                @input="handleTextInput"
                class="text-input-new"
              />
            </div>
          </div>

          <!-- 快捷操作 - 重新设计 -->
          <div class="quick-actions">
            <span class="actions-label">快捷操作</span>
            <div class="actions-group">
              <button class="quick-action-btn" @click="handleRemoveHTML" title="移除HTML标签">
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <polyline points="4,7 4,4 20,4 20,7"/>
                  <line x1="9" y1="20" x2="15" y2="20"/>
                  <line x1="12" y1="4" x2="12" y2="20"/>
                </svg>
                <span>移除标签</span>
              </button>
              <button class="quick-action-btn" @click="handleAddCJKSpaces" title="中英文之间添加空格">
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <path d="M21 6H3M21 12H3M21 18H3"/>
                </svg>
                <span>添加空格</span>
              </button>
              <button class="quick-action-btn" @click="handleRemovePunctuation" title="删除标点符号">
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <circle cx="12" cy="12" r="10"/>
                  <line x1="15" y1="9" x2="9" y2="15"/>
                  <line x1="9" y1="9" x2="15" y2="15"/>
                </svg>
                <span>删除标点</span>
              </button>
            </div>
          </div>
        </div>

        <!-- 无选中状态 - 优化设计 -->
        <div v-else class="no-selection">
          <div class="no-selection-content">
            <div class="no-selection-icon-wrapper">
              <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
                <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/>
                <polyline points="14,2 14,8 20,8"/>
                <line x1="16" y1="13" x2="8" y2="13"/>
                <line x1="16" y1="17" x2="8" y2="17"/>
                <line x1="10" y1="9" x2="8" y2="9"/>
              </svg>
            </div>
            <div class="no-selection-text-group">
              <p class="no-selection-title">选择字幕开始编辑</p>
              <p class="no-selection-hint">从左侧列表中点击任意字幕条目</p>
            </div>
            <div class="no-selection-shortcuts">
              <div class="shortcut-item">
                <kbd>↑</kbd><kbd>↓</kbd>
                <span>切换字幕</span>
              </div>
              <div class="shortcut-item">
                <kbd>⌘</kbd><kbd>N</kbd>
                <span>新建字幕</span>
              </div>
            </div>
          </div>
        </div>
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

.timeline-placeholder {
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 0.75rem 1.5rem;
  background: linear-gradient(135deg, #f8fafc 0%, #f1f5f9 100%);
  border-bottom: 1px solid #e2e8f0;
}

.audio-empty-state {
  display: flex;
  align-items: center;
  gap: 1.25rem;
  padding: 0.625rem 1.25rem;
  background: white;
  border-radius: 10px;
  border: 1px dashed #cbd5e1;
  transition: all 0.2s ease;
}

.audio-empty-state:hover {
  border-color: #94a3b8;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.05);
}

.audio-empty-icon {
  width: 40px;
  height: 40px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: linear-gradient(135deg, #eff6ff 0%, #dbeafe 100%);
  border-radius: 10px;
  color: #3b82f6;
  flex-shrink: 0;
}

.audio-empty-icon svg {
  width: 24px;
  height: 24px;
}

.audio-empty-text {
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
}

.audio-empty-title {
  font-size: 0.875rem;
  font-weight: 600;
  color: #1e293b;
}

.audio-empty-hint {
  font-size: 0.6875rem;
  color: #94a3b8;
}

.audio-load-btn {
  padding: 0.375rem 0.875rem;
  border-radius: 8px;
  font-weight: 500;
  font-size: 0.8125rem;
}

/* 一体化控制栏：三栏布局（左、中、右）*/
.timeline-unified-controls {
  display: grid;
  grid-template-columns: 1fr auto 1fr;
  align-items: center;
  padding: 0.5rem 1rem;
  background: #fafafa;
  border-bottom: 1px solid #e5e7eb;
  gap: 1rem;
  font-size: 0.813rem;
}

.controls-left {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  justify-content: flex-start;
}

.controls-left > :nth-child(4) {
  margin-left: 2rem;
}

.controls-center {
  display: flex;
  align-items: center;
  gap: 1rem;
  justify-content: center;
}

.controls-right {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  justify-content: flex-end;
}

/* 分隔线 */
.divider {
  width: 1px;
  height: 20px;
  background: #d1d5db;
  margin: 0 0.25rem;
}

/* 音频名称 */
.audio-name-compact {
  font-size: 0.813rem;
  font-weight: 500;
  color: #333;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 150px;
}

/* 控制标签 */
.control-label-mini {
  font-size: 0.75rem;
  color: #666;
  white-space: nowrap;
  margin-right: 0.25rem;
}

/* 缩放按钮 */
.zoom-btn {
  min-width: 32px;
  height: 28px;
  padding: 0 0.5rem;
}

.zoom-display {
  font-size: 0.75rem;
  color: #666;
  min-width: 45px;
  text-align: center;
}

/* 时间显示 */
.time-display-mini {
  font-size: 0.75rem;
  color: #6b7280;
  font-family: 'SF Mono', 'Monaco', 'Consolas', monospace;
  font-weight: 500;
  min-width: 42px;
}

/* 播放按钮 */
.play-button-mini {
  font-size: 0.85rem;
  width: 32px;
  height: 32px;
}

/* 音量滑块 */
.volume-slider-mini {
  width: 100px;
}

.param-value-mini {
  font-size: 0.75rem;
  color: #999;
  min-width: 35px;
  text-align: right;
}

/* 速度按钮 */
.speed-btn-mini {
  min-width: 45px;
  height: 28px;
  font-size: 0.75rem;
}

/* 主内容区 */
.content-area {
  flex: 1;
  display: flex;
  overflow: hidden;
}

/* 左侧侧边栏 */
.sidebar {
  width: 52px;
  background: linear-gradient(180deg, #ffffff 0%, #f8fafc 100%);
  border-right: 1px solid #e2e8f0;
  display: flex;
  flex-direction: column;
  justify-content: space-between;
  padding: 0.75rem 0;
  flex-shrink: 0;
}

.sidebar-top,
.sidebar-bottom {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 0.25rem;
}

.sidebar-btn {
  width: 36px;
  height: 36px;
  margin: 0 auto;
  display: flex;
  align-items: center;
  justify-content: center;
  background: transparent;
  border: none;
  border-radius: 10px;
  cursor: pointer;
  transition: all 0.2s ease;
  position: relative;
}

.sidebar-btn .el-icon {
  font-size: 18px;
  color: #94a3b8;
  transition: all 0.2s ease;
}

.sidebar-btn:hover {
  background: #f1f5f9;
}

.sidebar-btn:hover .el-icon {
  color: #64748b;
}

.sidebar-btn.active {
  background: linear-gradient(135deg, #eff6ff 0%, #dbeafe 100%);
}

.sidebar-btn.active::before {
  content: '';
  position: absolute;
  left: 0;
  top: 50%;
  transform: translateY(-50%);
  width: 3px;
  height: 20px;
  background: linear-gradient(180deg, #3b82f6 0%, #2563eb 100%);
  border-radius: 0 3px 3px 0;
}

.sidebar-btn.active .el-icon {
  color: #3b82f6;
}

.sidebar-btn:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.sidebar-btn:disabled:hover {
  background: transparent;
}

.sidebar-btn svg {
  width: 18px;
  height: 18px;
  color: #94a3b8;
  transition: all 0.2s ease;
}

.sidebar-btn:hover svg {
  color: #64748b;
}

.sidebar-btn:disabled svg {
  color: #94a3b8;
}

/* 左侧字幕列表 */
.subtitle-list-panel {
  width: 400px;
  background: #f8fafc;
  border-right: 1px solid #e2e8f0;
  display: flex;
  flex-direction: column;
}

/* 搜索和替换框 */
.search-replace-container {
  padding: 0.5rem;
  border-bottom: 1px solid #e5e7eb;
  background: white;
}

.search-row {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  margin-bottom: 0.5rem;
}

.search-row:last-of-type {
  margin-bottom: 0;
}

/* 搜索图标 */
.search-icon,
.replace-icon {
  color: #9ca3af;
  font-size: 14px;
}

/* 展开/收起按钮 */
.expand-btn {
  width: 28px;
  height: 28px;
  padding: 0;
  border: 1px solid #e5e7eb;
  background: #f9fafb;
  color: #6b7280;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  border-radius: 0.375rem;
  transition: all 0.2s ease;
}

.expand-btn:hover {
  background: #f3f4f6;
  border-color: #d1d5db;
  color: #374151;
}

.expand-btn .el-icon {
  font-size: 14px;
  transition: transform 0.2s ease;
}

.expand-btn.expanded {
  background: #eff6ff;
  border-color: #3b82f6;
  color: #3b82f6;
}

.expand-btn.expanded .el-icon {
  transform: rotate(180deg);
}

.search-input {
  flex: 1;
  min-width: 0;
}

.search-input :deep(.el-input__wrapper) {
  padding: 0.4rem 0.6rem;
  background: white;
  border: 1px solid #dcdfe6;
  border-radius: 0.3rem;
  height: 2rem;
}

.search-input :deep(.el-input__wrapper:hover) {
  border-color: #b3d8ff;
  background: white;
}

.search-input :deep(.el-input__wrapper.is-focus) {
  border-color: #409eff;
  box-shadow: 0 0 0 2px rgba(64, 158, 255, 0.2);
}

.search-input :deep(.el-input__input) {
  font-size: 0.875rem;
}

.match-count {
  padding: 0.25rem 0.5rem;
  background: #f0f0f0;
  border-radius: 0.25rem;
  font-size: 0.75rem;
  color: #666;
  white-space: nowrap;
  flex-shrink: 0;
}

.replace-row {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  margin-top: 0.5rem;
}

.replace-input {
  flex: 1;
  min-width: 0;
}

.replace-input :deep(.el-input__wrapper) {
  padding: 0.4rem 0.6rem;
  background: white;
  border: 1px solid #dcdfe6;
  border-radius: 0.3rem;
  height: 2rem;
}

.replace-input :deep(.el-input__wrapper:hover) {
  border-color: #b3d8ff;
  background: white;
}

.replace-input :deep(.el-input__wrapper.is-focus) {
  border-color: #409eff;
  box-shadow: 0 0 0 2px rgba(64, 158, 255, 0.2);
}

.replace-input :deep(.el-input__input) {
  font-size: 0.875rem;
}

.replace-btn {
  padding: 0.4rem 0.8rem;
  background: white;
  border: 1px solid #dcdfe6;
  border-radius: 0.3rem;
  color: #606266;
  cursor: pointer;
  font-size: 0.875rem;
  transition: all 0.2s;
  white-space: nowrap;
  flex-shrink: 0;
}

.replace-btn:hover:not(:disabled) {
  border-color: #409eff;
  color: #409eff;
}

.replace-btn:disabled {
  color: #ccc;
  cursor: not-allowed;
}

.replace-all-btn:not(:disabled) {
  background: #409eff;
  border-color: #409eff;
  color: white;
}

.replace-all-btn:hover:not(:disabled) {
  background: #66b1ff;
  border-color: #66b1ff;
}

.subtitle-list {
  flex: 1;
  overflow-y: auto;
  padding: 0.5rem;
}

.subtitle-item {
  padding: 0.75rem;
  margin-bottom: 0.375rem;
  background: #ffffff;
  border: 1px solid #e2e8f0;
  border-radius: 10px;
  cursor: pointer;
  transition: all 0.2s ease;
}

.subtitle-item:hover {
  background: #f8fafc;
  border-color: #cbd5e1;
  box-shadow: 0 2px 4px rgba(0, 0, 0, 0.04);
}

.subtitle-item.is-selected {
  background: linear-gradient(135deg, #eff6ff 0%, #dbeafe 100%);
  border-color: #3b82f6;
  box-shadow: 0 0 0 1px rgba(59, 130, 246, 0.1);
}

.item-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 0.5rem;
}

.item-number {
  font-size: 0.6875rem;
  font-weight: 700;
  color: #64748b;
  background: #f1f5f9;
  padding: 0.125rem 0.5rem;
  border-radius: 4px;
}

.subtitle-item.is-selected .item-number {
  background: rgba(59, 130, 246, 0.15);
  color: #2563eb;
}

.item-time {
  font-size: 0.6875rem;
  color: #94a3b8;
  font-family: 'SF Mono', 'Monaco', 'Consolas', monospace;
}

.item-content {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  justify-content: space-between;
}

.item-text-wrapper {
  flex: 1;
  min-width: 0;
  overflow: hidden;
}

.item-text {
  color: #334155;
  font-size: 0.8125rem;
  line-height: 1.5;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.subtitle-item.is-selected .item-text {
  color: #1e40af;
}

.list-footer {
  padding: 0.625rem 1rem;
  border-top: 1px solid #e2e8f0;
  background: linear-gradient(135deg, #f8fafc 0%, #f1f5f9 100%);
  display: flex;
  align-items: center;
  justify-content: space-between;
  font-size: 0.75rem;
  color: #64748b;
  gap: 1rem;
}

.file-info {
  color: #334155;
  font-weight: 600;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  flex: 1;
  min-width: 0;
}

.count-info {
  color: #94a3b8;
  white-space: nowrap;
  flex-shrink: 0;
  font-weight: 500;
}

.empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  height: 100%;
  gap: 1rem;
  padding: 2rem;
  text-align: center;
}

/* 右侧字幕编辑区 */
.subtitle-edit-panel {
  flex: 1;
  background: linear-gradient(180deg, #f8fafc 0%, #f1f5f9 100%);
  overflow-y: auto;
  display: flex;
  flex-direction: column;
}

.audio-name {
  font-size: 0.875rem;
  font-weight: 500;
  color: #333;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 180px;
}

.subtitle-edit-section {
  padding: 1.5rem;
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 1.25rem;
}

/* 编辑头部 */
.edit-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.edit-header-left {
  display: flex;
  align-items: center;
  gap: 0.75rem;
}

.edit-badge {
  font-size: 0.75rem;
  font-weight: 600;
  color: #3b82f6;
  background: linear-gradient(135deg, #eff6ff 0%, #dbeafe 100%);
  padding: 0.25rem 0.625rem;
  border-radius: 6px;
  border: 1px solid #bfdbfe;
}

.edit-title {
  font-size: 1.125rem;
  font-weight: 600;
  color: #1e293b;
  margin: 0;
}

.delete-entry-btn {
  width: 32px;
  height: 32px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: transparent;
  border: 1px solid transparent;
  border-radius: 8px;
  cursor: pointer;
  color: #94a3b8;
  transition: all 0.2s ease;
}

.delete-entry-btn:hover {
  background: #fef2f2;
  border-color: #fecaca;
  color: #ef4444;
}

/* 时间设置 - 紧凑单行布局 */
.time-row {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  padding: 0.875rem 1rem;
  background: #ffffff;
  border: 1px solid #e2e8f0;
  border-radius: 10px;
}

.time-block {
  display: flex;
  align-items: center;
  gap: 0.5rem;
}

.time-block .time-label {
  font-size: 0.75rem;
  font-weight: 500;
  color: #64748b;
  white-space: nowrap;
}

.time-control {
  display: flex;
  align-items: center;
}

.time-btn-sm {
  width: 28px;
  height: 32px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: #f8fafc;
  border: 1px solid #e2e8f0;
  cursor: pointer;
  transition: all 0.15s ease;
  font-size: 0.9375rem;
  font-weight: 500;
  color: #64748b;
  padding: 0;
}

.time-btn-sm:first-child {
  border-radius: 6px 0 0 6px;
  border-right: none;
}

.time-btn-sm:last-child {
  border-radius: 0 6px 6px 0;
  border-left: none;
}

.time-btn-sm:hover {
  background: #eff6ff;
  color: #3b82f6;
}

.time-btn-sm:active {
  background: #dbeafe;
}

.time-input-sm {
  width: 115px;
}

.time-input-sm :deep(.el-input__wrapper) {
  font-family: 'SF Mono', 'Monaco', 'Consolas', monospace;
  font-size: 0.8125rem;
  padding: 0 0.5rem;
  background: #ffffff;
  border-radius: 0;
  border: 1px solid #e2e8f0;
  border-left: none;
  border-right: none;
  transition: all 0.2s ease;
  height: 32px;
  box-sizing: border-box;
  box-shadow: none;
}

.time-input-sm :deep(.el-input__wrapper:hover) {
  border-color: #e2e8f0;
}

.time-input-sm :deep(.el-input__wrapper.is-focus) {
  border-color: #3b82f6;
  box-shadow: none;
}

.time-input-sm :deep(.el-input__inner) {
  text-align: center;
  color: #1e293b;
  font-size: 0.8125rem;
}

.time-separator {
  font-size: 0.875rem;
  color: #94a3b8;
  font-weight: 400;
}

.duration-block {
  margin-left: auto;
  background: #f0f9ff;
  padding: 0.375rem 0.75rem;
  border-radius: 8px;
  border: 1px solid #bae6fd;
}

.duration-block .time-label {
  color: #0369a1;
}

.duration-value-sm {
  font-family: 'SF Mono', 'Monaco', 'Consolas', monospace;
  font-size: 0.8125rem;
  font-weight: 600;
  color: #0369a1;
}

/* 文本编辑卡片 - 新设计 */
.text-edit-card {
  background: #ffffff;
  border: 1px solid #e2e8f0;
  border-radius: 12px;
  overflow: hidden;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.04);
}

.text-card-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0.75rem 1rem;
  background: linear-gradient(135deg, #f8fafc 0%, #f1f5f9 100%);
  border-bottom: 1px solid #e2e8f0;
}

.text-header-left {
  display: flex;
  align-items: center;
  gap: 0.5rem;
}

.text-icon {
  color: #64748b;
}

.text-card-title {
  font-size: 0.8125rem;
  font-weight: 600;
  color: #475569;
}

.char-count {
  font-size: 0.75rem;
  color: #94a3b8;
  font-weight: 500;
  padding: 0.25rem 0.625rem;
  background: #ffffff;
  border-radius: 6px;
  border: 1px solid #e2e8f0;
}

.text-input-wrapper {
  padding: 1rem;
}

.text-input-new :deep(.el-input__wrapper) {
  border-radius: 8px;
  border: 1px solid #e2e8f0;
  padding: 0 0.875rem;
  font-size: 0.9375rem;
  line-height: 1.6;
  transition: all 0.2s ease;
  background: #ffffff;
  height: 44px;
  box-sizing: border-box;
}

.text-input-new :deep(.el-input__wrapper:hover) {
  border-color: #cbd5e1;
}

.text-input-new :deep(.el-input__wrapper.is-focus) {
  border-color: #3b82f6;
  box-shadow: 0 0 0 3px rgba(59, 130, 246, 0.1);
}

.text-input-new :deep(.el-input__inner) {
  color: #1e293b;
  font-size: 0.9375rem;
}

.text-input-new :deep(.el-input__inner::placeholder) {
  color: #94a3b8;
}

/* 快捷操作 - 新设计 */
.quick-actions {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  padding: 0.75rem 1rem;
  background: #f8fafc;
  border-radius: 10px;
  border: 1px solid #e2e8f0;
}

.actions-label {
  font-size: 0.75rem;
  font-weight: 500;
  color: #94a3b8;
  white-space: nowrap;
}

.actions-group {
  display: flex;
  gap: 0.5rem;
  flex: 1;
}

.quick-action-btn {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 0.375rem;
  padding: 0.5rem 0.75rem;
  background: #ffffff;
  border: 1px solid #e2e8f0;
  border-radius: 8px;
  cursor: pointer;
  transition: all 0.2s ease;
  font-size: 0.8125rem;
  font-weight: 500;
  color: #475569;
}

.quick-action-btn:hover {
  background: #f1f5f9;
  border-color: #cbd5e1;
  color: #3b82f6;
  transform: translateY(-1px);
  box-shadow: 0 2px 4px rgba(0, 0, 0, 0.04);
}

.quick-action-btn:active {
  transform: translateY(0);
}

.quick-action-btn svg {
  flex-shrink: 0;
}

/* 无选中状态 - 新设计 */
.no-selection {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 3rem;
  background: linear-gradient(135deg, #f8fafc 0%, #f1f5f9 100%);
}

.no-selection-content {
  text-align: center;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 1.25rem;
  max-width: 280px;
}

.no-selection-icon-wrapper {
  width: 80px;
  height: 80px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: linear-gradient(135deg, #ffffff 0%, #f8fafc 100%);
  border-radius: 20px;
  border: 1px solid #e2e8f0;
  color: #cbd5e1;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.04);
}

.no-selection-text-group {
  display: flex;
  flex-direction: column;
  gap: 0.375rem;
}

.no-selection-title {
  font-size: 1rem;
  font-weight: 600;
  color: #475569;
  margin: 0;
}

.no-selection-hint {
  font-size: 0.8125rem;
  color: #94a3b8;
  margin: 0;
}

.no-selection-shortcuts {
  display: flex;
  gap: 1.5rem;
  padding-top: 0.5rem;
}

.shortcut-item {
  display: flex;
  align-items: center;
  gap: 0.375rem;
  font-size: 0.75rem;
  color: #94a3b8;
}

.shortcut-item kbd {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 20px;
  height: 20px;
  padding: 0 0.375rem;
  background: #ffffff;
  border: 1px solid #e2e8f0;
  border-radius: 4px;
  font-family: inherit;
  font-size: 0.6875rem;
  font-weight: 500;
  color: #64748b;
  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.04);
}

/* 搜索高亮样式 */
mark {
  background-color: #ffd700;
  color: #333;
  padding: 0.1rem 0.2rem;
  border-radius: 0.2rem;
  font-weight: 500;
  box-shadow: 0 0 0 1px rgba(255, 215, 0, 0.3);
}

/* 字幕项目操作按钮 */
.item-actions {
  display: flex;
  gap: 0.25rem;
  flex-shrink: 0;
  align-items: center;
  margin-left: 0.5rem;
}

.item-actions :deep(.el-button) {
  padding: 0;
  font-size: 0.875rem;
  line-height: 1;
  min-width: auto;
  height: auto;
}

.item-actions :deep(.el-button[type='primary']) {
  color: #409eff;
}

.item-actions :deep(.el-button[type='primary']:hover) {
  color: #66b1ff;
}

.item-actions :deep(.el-button[type='danger']) {
  color: #f56c6c;
}

.item-actions :deep(.el-button[type='danger']:hover) {
  color: #f85e5e;
}

.item-actions :deep(.el-icon) {
  width: 1em;
  height: 1em;
}





</style>

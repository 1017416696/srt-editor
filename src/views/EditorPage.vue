<script setup lang="ts">
import { ref, computed, onMounted, onBeforeUnmount, watch, nextTick } from 'vue'
import { useRouter } from 'vue-router'
import { ElMessage, ElMessageBox } from 'element-plus'
import { open } from '@tauri-apps/plugin-dialog'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { useSubtitleStore } from '@/stores/subtitle'
import { useAudioStore } from '@/stores/audio'
import { useConfigStore } from '@/stores/config'
import { timeStampToMs } from '@/utils/time'
import type { SRTFile, AudioFile } from '@/types/subtitle'
import WaveformViewer from '@/components/WaveformViewer.vue'

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

// UI 状态
const searchText = ref('')
const replaceText = ref('')
const showReplace = ref(false)
const selectedEntryId = ref<number | null>(null)
const editingText = ref('')
const subtitleListContainer = ref<HTMLElement | null>(null)
const subtitleItemRefs: Record<number, HTMLElement | null> = {}
const isUserEditing = ref(false) // 标记是否是用户在编辑
const isUserSelectingEntry = ref(false) // 标记用户是否在手动选择字幕
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

// 监听选中字幕变化，更新编辑文本
watch(currentEntry, (entry) => {
  if (entry) {
    isUserEditing.value = false // 标记为非用户编辑
    editingText.value = entry.text
  }
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
    ElMessage.warning('请输入要查找的文本')
    return
  }

  try {
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
      ElMessage.success(`成功修改 ${modifiedCount} 条字幕`)
    } else {
      ElMessage.info('未找到匹配的文本')
    }
  } catch (error) {
    ElMessage.error(`替换失败: ${error}`)
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
        selectedEntryId.value = nextId
      } else if (currentIndex === subtitleStore.searchResults.length - 1) {
        // 已经是最后一个，提示
        ElMessage.success('已替换，没有更多结果了')
      }
    }
  } catch (error) {
    ElMessage.error(`替换失败: ${error}`)
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
    ElMessage.error(`自动保存失败: ${error}`)
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

// 初始化时选中第一条字幕，设置菜单监听和快捷键
onMounted(async () => {
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
    const unlistenOpenFile = await listen<void>('menu:open-file', async () => {
      await handleOpenFile()
    })

    // 添加键盘快捷键监听（添加到 document 而不是 window，确保捕获所有键盘事件）
    document.addEventListener('keydown', handleKeydown, true)

    // 在组件卸载时清理所有监听器
    onBeforeUnmount(() => {
      unlistenOpenFile()
      // 清除全局处理函数
      ;(window as any).__handleMenuOpenFile = null
      ;(window as any).__handleMenuSave = null
      document.removeEventListener('keydown', handleKeydown, true)
    })
  } catch (error) {
    console.error('Error setting up menu handlers:', error)
  }
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
      ElMessage.success('SRT 文件加载成功')

      // 选中第一条字幕
      if (subtitleStore.entries.length > 0) {
        selectedEntryId.value = subtitleStore.entries[0]?.id ?? null
      }
    }
  } catch (error) {
    ElMessage.error(`加载失败: ${error}`)
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
      ElMessage.success('音频文件加载成功')
    }
  } catch (error) {
    ElMessage.error(`加载失败: ${error}`)
  }
}

// 删除音频文件
const handleRemoveAudio = async () => {
  if (!hasAudio) return

  try {
    await ElMessageBox.confirm('确定要删除当前加载的音频文件吗？', '确认', {
      confirmButtonText: '删除',
      cancelButtonText: '取消',
      type: 'warning',
    })

    audioStore.unloadAudio()
    ElMessage.success('已删除音频文件')
  } catch {
    // 用户取消
  }
}

// 保存文件
const handleSave = async () => {
  // 防止重复保存
  if (isSaving) return

  if (!subtitleStore.currentFilePath) {
    ElMessage.warning('没有可保存的文件')
    return
  }

  isSaving = true
  try {
    await subtitleStore.saveToFile()
    ElMessage.success('保存成功')
  } catch (error) {
    ElMessage.error(`保存失败: ${error}`)
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
    ElMessage.warning('没有可保存的文件')
    return
  }

  try {
    await subtitleStore.saveToFile()
    ElMessage.success('已保存')
  } catch (error) {
    ElMessage.error(`保存失败: ${error}`)
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
  subtitleStore.addEntry()
  ElMessage.success('已添加新字幕')

  // 选中新添加的字幕
  const newEntry = subtitleStore.entries[subtitleStore.entries.length - 1]
  if (newEntry) {
    selectedEntryId.value = newEntry.id
  }
}

// 删除字幕
const handleDeleteEntry = async () => {
  if (!currentEntry.value) return

  try {
    await ElMessageBox.confirm('确定要删除这条字幕吗？', '确认删除', {
      confirmButtonText: '删除',
      cancelButtonText: '取消',
      type: 'warning',
    })

    const currentId = currentEntry.value.id
    const currentIndex = subtitleStore.entries.findIndex((e) => e.id === currentId)

    subtitleStore.deleteEntry(currentId)
    ElMessage.success('已删除字幕')

    // 选中下一条或上一条字幕
    if (subtitleStore.entries.length > 0) {
      const nextEntry = subtitleStore.entries[currentIndex] || subtitleStore.entries[currentIndex - 1]
      if (nextEntry) {
        selectedEntryId.value = nextEntry.id
      }
    } else {
      selectedEntryId.value = null
    }
  } catch {
    // 用户取消
  }
}

// 移除 HTML 标签
const handleRemoveHTML = () => {
  subtitleStore.removeHTMLTags()
  if (currentEntry.value) {
    editingText.value = currentEntry.value.text
  }
  ElMessage.success('已移除所有 HTML 标签')
}

// 处理波形点击跳转
const handleWaveformSeek = (time: number) => {
  audioStore.seek(time)
}

// 返回欢迎页
const goBack = async () => {
  if (subtitleStore.hasUnsavedChanges) {
    try {
      await ElMessageBox.confirm('有未保存的更改，确定要离开吗？', '确认', {
        confirmButtonText: '离开',
        cancelButtonText: '取消',
        type: 'warning',
      })
    } catch {
      return
    }
  }

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

// 键盘快捷键
const handleKeydown = (e: KeyboardEvent) => {
  const target = e.target as HTMLElement

  // 检查是否在文本输入框内
  const isInTextInput =
    target instanceof HTMLTextAreaElement ||
    target instanceof HTMLInputElement

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
    }
    // 不处理其他快捷键，允许正常输入（包括空格）
    return
  }

  // 不在文本框内，处理全局快捷键
  if (shortcuts.save === pressedKey) {
    e.preventDefault()
    handleSave()
  } else if (shortcuts.open === pressedKey) {
    e.preventDefault()
    handleOpenFile()
  } else if (shortcuts.playPause === pressedKey.toLowerCase()) {
    e.preventDefault()
    audioStore.togglePlay()
  } else if (shortcuts.addEntry === pressedKey) {
    e.preventDefault()
    handleAddEntry()
  } else if (shortcuts.deleteEntry === pressedKey) {
    e.preventDefault()
    handleDeleteEntry()
  }
}
</script>

<template>
  <div class="editor-page">
    <!-- 顶部工具栏 -->
    <div class="toolbar">
      <div class="toolbar-left">
        <span class="app-title">SRT 编辑工具</span>
        <span v-if="subtitleStore.currentFilePath" class="file-name">
          {{ subtitleStore.currentFilePath.split('/').pop() }}
        </span>
      </div>

      <div class="toolbar-right">
        <el-button type="primary" :disabled="!hasContent" @click="handleSave">
          保存
        </el-button>
      </div>
    </div>

    <!-- 主内容区 -->
    <div class="content-area">
      <!-- 左侧：字幕列表 -->
      <div class="subtitle-list-panel">
        <!-- 搜索和替换框 -->
        <div class="search-replace-container">
          <!-- 搜索框 -->
          <div class="search-row">
            <button
              class="toggle-btn"
              @click="showReplace = !showReplace"
              :title="showReplace ? '隐藏替换' : '显示替换'"
            >
              {{ showReplace ? '▼' : '▶' }}
            </button>
            <el-input
              v-model="searchText"
              placeholder="搜索字幕"
              clearable
              class="search-input"
              size="small"
            />
            <span v-if="searchText && subtitleStore.searchResults.length > 0" class="match-count">
              {{ subtitleStore.searchResults.length }}
            </span>
          </div>

          <!-- 替换框 -->
          <div v-if="showReplace" class="replace-row">
            <div class="replace-spacer"></div>
            <el-input
              v-model="replaceText"
              placeholder="替换为..."
              clearable
              class="replace-input"
              size="small"
            />
            <button
              class="replace-btn"
              @click="replaceOne"
              :disabled="!searchText || subtitleStore.searchResults.length === 0"
              title="替换当前项，然后跳到下一个"
            >
              替换
            </button>
            <button
              class="replace-btn replace-all-btn"
              @click="replaceAll"
              :disabled="!searchText"
              title="全部替换"
            >
              全部替换
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
          >
            <div class="item-header">
              <span class="item-number">{{ entry.id }}</span>
              <span class="item-time">
                {{ subtitleStore.formatTimeStamp(entry.startTime).slice(0, 8) }}
                -
                {{ subtitleStore.formatTimeStamp(entry.endTime).slice(0, 8) }}
              </span>
            </div>
            <div class="item-text">{{ entry.text }}</div>
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

        <!-- 底部统计 -->
        <div class="list-footer">
          <span v-if="searchText">
            {{ filteredEntries.length }}/{{ subtitleStore.entries.length }} 字幕
          </span>
          <span v-else>
            {{ subtitleStore.entries.length }}/{{ subtitleStore.entries.length }} 字幕
          </span>
        </div>
      </div>

      <!-- 右侧：编辑区域 -->
      <div class="edit-panel">
        <!-- 音频控制区 -->
        <div class="audio-section">
          <div v-if="!hasAudio" class="audio-placeholder">
            <span class="text-gray-500">未加载音频</span>
            <el-button size="small" @click="handleOpenAudio">加载音频</el-button>
          </div>

          <div v-else class="audio-controls">
            <div class="audio-header">
              <span class="audio-name">{{ audioStore.currentAudio?.name }}</span>
              <el-button text size="small" type="danger" @click="handleRemoveAudio">删除</el-button>
            </div>

            <div class="audio-player">
              <el-button
                circle
                type="primary"
                size="large"
                @click="audioStore.togglePlay()"
                class="play-button"
              >
                {{ audioStore.playerState.isPlaying ? '⏸' : '▶' }}
              </el-button>
              <span class="current-time">{{ audioStore.formatTime(audioStore.playerState.currentTime) }}</span>
              <div class="progress-slider">
                <el-slider
                  v-model="audioStore.playerState.currentTime"
                  :max="audioStore.playerState.duration"
                  :step="0.1"
                  :show-tooltip="false"
                  @input="(val: number) => audioStore.seek(val)"
                />
              </div>
              <span class="duration-time">{{ audioStore.formatTime(audioStore.playerState.duration) }}</span>
            </div>

            <div class="audio-controls-footer">
              <div class="volume-section">
                <div class="control-label">音量</div>
                <div class="volume-control">
                  <el-slider
                    v-model="audioStore.playerState.volume"
                    :max="1"
                    :step="0.01"
                    :show-tooltip="false"
                    class="volume-slider"
                    @input="(val: number) => audioStore.setVolume(val)"
                  />
                  <span class="volume-percentage">{{ Math.round(audioStore.playerState.volume * 100) }}%</span>
                </div>
              </div>

              <div class="playback-rate-section">
                <div class="control-label">速度</div>
                <div class="speed-buttons">
                  <el-button
                    v-for="rate in [0.5, 1, 1.5, 2]"
                    :key="rate"
                    :type="audioStore.playerState.playbackRate === rate ? 'primary' : 'default'"
                    size="small"
                    @click="audioStore.setPlaybackRate(rate)"
                  >
                    {{ rate }}x
                  </el-button>
                </div>
              </div>
            </div>
          </div>
        </div>

        <!-- 波形显示区 -->
        <div v-if="hasAudio" class="waveform-section">
          <WaveformViewer
            :waveform-data="audioStore.audioFile?.waveform"
            :current-time="audioStore.playerState.currentTime"
            :duration="audioStore.playerState.duration"
            :subtitles="subtitleStore.entries"
            :height="120"
            @seek="handleWaveformSeek"
          />
        </div>

        <!-- 字幕编辑区 -->
        <div v-if="currentEntry" class="subtitle-edit-section">
          <div class="edit-header">
            <h3 class="edit-title">字幕 #{{ currentEntry.id }}</h3>
          </div>

          <!-- 时间编辑 -->
          <div class="time-edit-row">
            <div class="time-field">
              <label>开始</label>
              <el-input
                :model-value="subtitleStore.formatTimeStamp(currentEntry.startTime)"
                size="small"
                readonly
              />
            </div>

            <div class="time-arrow">→</div>

            <div class="time-field">
              <label>结束</label>
              <el-input
                :model-value="subtitleStore.formatTimeStamp(currentEntry.endTime)"
                size="small"
                readonly
              />
            </div>

            <div class="time-field">
              <label>时长</label>
              <el-input
                :model-value="`00:${String(Math.floor((subtitleStore.formatTimeStamp(currentEntry.endTime).slice(6, 8) as any) - (subtitleStore.formatTimeStamp(currentEntry.startTime).slice(6, 8) as any))).padStart(2, '0')},000`"
                size="small"
                readonly
              />
            </div>
          </div>

          <!-- 文本编辑 -->
          <div class="text-edit-section">
            <label class="text-label">字幕文本</label>
            <el-input
              v-model="editingText"
              type="textarea"
              :rows="6"
              placeholder="支持拖动时间调整时间，点击时间精确编辑"
              @focus="isUserEditing = true"
              @blur="handleTextareaBlur"
              @input="handleTextInput"
            />
            <div class="text-meta">
              <span>{{ editingText.length }} 字</span>
            </div>
          </div>

          <!-- 底部操作 -->
          <div class="bottom-actions">
            <el-button text @click="handleRemoveHTML">移除HTML</el-button>
            <el-button text type="danger" @click="handleDeleteEntry">删除字幕</el-button>
          </div>
        </div>

        <!-- 无选中状态 -->
        <div v-else class="no-selection">
          <p class="text-gray-400">请从左侧选择一条字幕进行编辑</p>
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

/* 工具栏 */
.toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0.75rem 1.5rem;
  background: white;
  border-bottom: 1px solid #e5e7eb;
}

.toolbar-left {
  display: flex;
  align-items: center;
  gap: 1rem;
}

.app-title {
  font-size: 1.1rem;
  font-weight: 600;
  color: #333;
}

.file-name {
  color: #666;
  font-size: 0.9rem;
}

.toolbar-right {
  display: flex;
  gap: 0.5rem;
}

/* 主内容区 */
.content-area {
  flex: 1;
  display: flex;
  overflow: hidden;
}

/* 左侧字幕列表 */
.subtitle-list-panel {
  width: 450px;
  background: white;
  border-right: 1px solid #e5e7eb;
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

.toggle-btn {
  width: 2rem;
  height: 2rem;
  padding: 0;
  border: none;
  background: transparent;
  color: #606266;
  cursor: pointer;
  font-size: 1rem;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  border-radius: 0.25rem;
  transition: all 0.2s;
}

.toggle-btn:hover {
  color: #409eff;
  background: #f0f9ff;
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

.replace-spacer {
  width: 2rem;
  flex-shrink: 0;
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
  margin-bottom: 0.5rem;
  background: #f9fafb;
  border: 1px solid #e5e7eb;
  border-radius: 0.375rem;
  cursor: pointer;
  transition: all 0.2s;
}

.subtitle-item:hover {
  background: #f3f4f6;
  border-color: #d1d5db;
}

.subtitle-item.is-selected {
  background: #eff6ff;
  border-color: #3b82f6;
}

.item-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 0.5rem;
}

.item-number {
  font-size: 0.75rem;
  font-weight: 600;
  color: #6b7280;
}

.item-time {
  font-size: 0.75rem;
  color: #9ca3af;
  font-family: monospace;
}

.item-text {
  color: #333;
  font-size: 0.875rem;
  line-height: 1.5;
  overflow: hidden;
  text-overflow: ellipsis;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
}

.list-footer {
  padding: 0.75rem 1rem;
  border-top: 1px solid #e5e7eb;
  background: #f9fafb;
  text-align: center;
  font-size: 0.875rem;
  color: #6b7280;
}

.empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  height: 100%;
  gap: 1rem;
}

/* 右侧编辑区 */
.edit-panel {
  flex: 1;
  background: white;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
}

.audio-section {
  padding: 1rem 1.5rem;
  border-bottom: 1px solid #e5e7eb;
}

.audio-placeholder {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 1rem;
  background: #f9fafb;
  border-radius: 0.5rem;
}

.audio-controls {
  display: flex;
  flex-direction: column;
  gap: 1rem;
}

.audio-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.audio-name {
  font-size: 0.95rem;
  font-weight: 500;
  color: #333;
}

.audio-player {
  display: flex;
  align-items: center;
  gap: 0.75rem;
}

.play-button {
  flex-shrink: 0;
  font-size: 1.2rem;
}

.current-time,
.duration-time {
  font-size: 0.8rem;
  color: #666;
  font-family: monospace;
  min-width: 35px;
  text-align: center;
}

.progress-slider {
  flex: 1;
  min-width: 0;
}

.audio-controls-footer {
  display: grid;
  grid-template-columns: 1.5fr 1fr;
  gap: 1.5rem;
  padding-top: 0.75rem;
  border-top: 1px solid #e5e7eb;
}

.volume-section {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}

.playback-rate-section {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}

.control-label {
  font-size: 0.9rem;
  color: #333;
  font-weight: 500;
}

.volume-control {
  display: flex;
  align-items: center;
  gap: 0.75rem;
}

.volume-slider {
  flex: 1;
  min-width: 0;
}

.volume-percentage {
  font-size: 0.85rem;
  color: #999;
  min-width: 35px;
  text-align: right;
}

.speed-buttons {
  display: flex;
  gap: 0.5rem;
}

.waveform-section {
  padding: 1rem 1.5rem;
  border-bottom: 1px solid #e5e7eb;
  background: white;
}

.timeline-section {
  padding: 1rem 1.5rem;
  border-bottom: 1px solid #e5e7eb;
}

.timeline-bar {
  display: flex;
  align-items: center;
  gap: 1rem;
}

.time-label {
  font-size: 0.875rem;
  color: #6b7280;
  font-family: monospace;
  min-width: 50px;
  text-align: center;
}

.timeline-progress {
  flex: 1;
  height: 4px;
  background: #e5e7eb;
  border-radius: 2px;
  position: relative;
}

.progress-bar-bg {
  height: 100%;
  background: #3b82f6;
  border-radius: 2px;
  width: 0%;
}

.subtitle-edit-section {
  padding: 1.5rem;
  flex: 1;
}

.edit-header {
  margin-bottom: 1.5rem;
}

.edit-title {
  font-size: 1.25rem;
  font-weight: 600;
  color: #333;
}

.time-edit-row {
  display: flex;
  align-items: flex-end;
  gap: 1rem;
  margin-bottom: 1.5rem;
}

.time-field {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
  flex: 1;
}

.time-field label {
  font-size: 0.875rem;
  color: #6b7280;
}

.time-arrow {
  padding-bottom: 0.5rem;
  color: #9ca3af;
}

.text-edit-section {
  margin-bottom: 1.5rem;
}

.text-label {
  display: block;
  font-size: 0.875rem;
  color: #6b7280;
  margin-bottom: 0.5rem;
}

.text-meta {
  margin-top: 0.5rem;
  text-align: right;
  font-size: 0.75rem;
  color: #9ca3af;
}

.edit-actions {
  margin-bottom: 2rem;
}

.bottom-actions {
  display: flex;
  gap: 1rem;
  padding-top: 1rem;
  border-top: 1px solid #e5e7eb;
}

.no-selection {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
}

</style>

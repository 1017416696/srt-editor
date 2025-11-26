<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, watch, nextTick } from 'vue'
import { useRouter } from 'vue-router'
import { ElMessage, ElMessageBox } from 'element-plus'
import { open } from '@tauri-apps/plugin-dialog'
import { invoke } from '@tauri-apps/api/core'
import { useSubtitleStore } from '@/stores/subtitle'
import { useAudioStore } from '@/stores/audio'
import { useConfigStore } from '@/stores/config'
import { timeStampToMs } from '@/utils/time'
import type { SRTFile, AudioFile } from '@/types/subtitle'

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
const showSearchPanel = ref(false)
const selectedEntryId = ref<number | null>(null)
const editingText = ref('')
const subtitleListContainer = ref<HTMLElement | null>(null)
const subtitleItemRefs: Record<number, HTMLElement | null> = {}
const isUserEditing = ref(false) // 标记是否是用户在编辑
let autoSaveTimer: ReturnType<typeof setTimeout> | null = null // 用于记录防抖计时器

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
  if (hasAudio.value) {
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

// 初始化时选中第一条字幕
onMounted(() => {
  if (subtitleStore.entries.length > 0) {
    selectedEntryId.value = subtitleStore.entries[0]?.id ?? null
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
  if (!subtitleStore.currentFilePath) {
    ElMessage.warning('没有可保存的文件')
    return
  }

  try {
    await subtitleStore.saveToFile()
    ElMessage.success('保存成功')
  } catch (error) {
    ElMessage.error(`保存失败: ${error}`)
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

// 键盘快捷键
const handleKeydown = (e: KeyboardEvent) => {
  const target = e.target as HTMLElement

  // 检查是否在文本输入框内
  const isInTextInput =
    target instanceof HTMLTextAreaElement ||
    target instanceof HTMLInputElement

  // 如果在文本框内，不处理快捷键（除了保存）
  if (isInTextInput) {
    const shortcuts = configStore.keyboardShortcuts
    const key = `${e.ctrlKey ? 'Ctrl+' : ''}${e.key}`

    // 只允许在文本框内使用 Ctrl+S 保存
    if (shortcuts.save === key) {
      e.preventDefault()
      handleSave()
    }
    // 不处理其他快捷键，允许正常输入（包括空格）
    return
  }

  // 不在文本框内，处理全局快捷键
  const shortcuts = configStore.keyboardShortcuts
  const key = `${e.ctrlKey ? 'Ctrl+' : ''}${e.key}`

  if (shortcuts.save === key) {
    e.preventDefault()
    handleSave()
  } else if (shortcuts.playPause === key.toLowerCase()) {
    e.preventDefault()
    audioStore.togglePlay()
  }
}

onMounted(() => {
  window.addEventListener('keydown', handleKeydown)
})

onUnmounted(() => {
  window.removeEventListener('keydown', handleKeydown)
})
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
        <el-button @click="handleOpenFile">打开文件</el-button>
        <el-button>导出</el-button>
        <el-button type="primary" :disabled="!hasContent" @click="handleSave">
          保存
        </el-button>
      </div>
    </div>

    <!-- 主内容区 -->
    <div class="content-area">
      <!-- 左侧：字幕列表 -->
      <div class="subtitle-list-panel">
        <!-- 搜索和添加 -->
        <div class="list-header">
          <el-input
            v-model="searchText"
            placeholder="搜索字幕..."
            size="small"
            clearable
          >
            <template #prefix>
              <span class="i-mdi-magnify"></span>
            </template>
          </el-input>
          <el-button
            type="primary"
            size="small"
            circle
            class="ml-2"
            @click="handleAddEntry"
          >
            <span class="i-mdi-plus"></span>
          </el-button>
        </div>

        <!-- 字幕列表 -->
        <div class="subtitle-list" ref="subtitleListContainer">
          <div
            v-for="entry in filteredEntries"
            :key="entry.id"
            :ref="(el) => { if (el) subtitleItemRefs[entry.id] = el as HTMLElement }"
            class="subtitle-item"
            :class="{ 'is-selected': selectedEntryId === entry.id }"
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

.list-header {
  padding: 1rem;
  border-bottom: 1px solid #e5e7eb;
  display: flex;
  align-items: center;
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

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { useRouter } from 'vue-router'
import { open } from '@tauri-apps/plugin-dialog'
import { invoke } from '@tauri-apps/api/core'
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { ElMessage, ElMessageBox } from 'element-plus'
import { Document, Headset } from '@element-plus/icons-vue'
import { useSubtitleStore } from '@/stores/subtitle'
import { useAudioStore } from '@/stores/audio'
import { useConfigStore } from '@/stores/config'
import type { SRTFile, AudioFile } from '@/types/subtitle'

const router = useRouter()
const subtitleStore = useSubtitleStore()
const audioStore = useAudioStore()
const configStore = useConfigStore()

// 检测操作系统
const isMac = computed(() => {
  return typeof navigator !== 'undefined' && /Mac|iPhone|iPad|iPod/.test(navigator.platform)
})

// 将快捷键拆分为单独的按键数组
const splitShortcut = (shortcut: string): string[] => {
  let formatted = shortcut
  if (isMac.value) {
    formatted = formatted
      .replace('Ctrl', '⌘')
      .replace('Shift', '⇧')
      .replace('Alt', '⌥')
  }
  return formatted.split('+').map(k => k.trim()).filter(k => k)
}

const isDragging = ref(false)
const isLoading = ref(false)
const loadingMessage = ref('')

// 拖放的文件预览
const droppedFiles = ref<{ srt: string | null; audio: string | null }>({
  srt: null,
  audio: null,
})

// 监听 Tauri 的文件拖放事件
let unlistenFileDrop: (() => void) | null = null

onMounted(async () => {
  const appWindow = getCurrentWebviewWindow()

  const unlistenHover = await appWindow.onDragDropEvent((event) => {
    if (event.payload.type === 'over') {
      isDragging.value = true
    } else if (event.payload.type === 'leave') {
      isDragging.value = false
    } else if (event.payload.type === 'drop') {
      isDragging.value = false
      const paths = event.payload.paths
      handleFileDrop(paths)
    }
  })

  unlistenFileDrop = unlistenHover
})

onUnmounted(() => {
  if (unlistenFileDrop) {
    unlistenFileDrop()
  }
})

// 处理拖放的文件
const handleFileDrop = async (paths: string[]) => {
  if (!paths || paths.length === 0) {
    return
  }

  const srtFile = paths.find((p) => p.toLowerCase().endsWith('.srt'))
  const audioFile = paths.find((p) => /\.(mp3|wav|ogg|flac|m4a|aac)$/i.test(p.toLowerCase()))

  if (!srtFile && !audioFile) {
    await ElMessageBox.alert('请拖放有效的 SRT 字幕文件或音频文件', '无效文件', {
      confirmButtonText: '确定',
      type: 'warning',
    })
    return
  }

  await processFiles({ srtPath: srtFile, audioPath: audioFile })
}

// 打开文件选择对话框
const openSRTFile = async () => {
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
      await processFiles({ srtPath: selected as string })
    }
  } catch (error) {
    await ElMessageBox.alert('无法打开文件选择器', '错误', {
      confirmButtonText: '确定',
      type: 'error',
    })
  }
}

const openAudioFile = async () => {
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
      await processFiles({ audioPath: selected })
    }
  } catch (error) {
    await ElMessageBox.alert('无法打开文件选择器', '错误', {
      confirmButtonText: '确定',
      type: 'error',
    })
  }
}

// 处理文件加载
const processFiles = async ({
  srtPath,
  audioPath,
}: {
  srtPath?: string
  audioPath?: string
}) => {
  isLoading.value = true
  let srtLoaded = false
  let audioLoaded = false

  try {
    if (srtPath) {
      loadingMessage.value = '正在加载字幕文件...'
      try {
        const srtFile = await invoke<SRTFile>('read_srt', { filePath: srtPath })
        await subtitleStore.loadSRTFile(srtFile)
        droppedFiles.value.srt = srtPath.split('/').pop() || srtPath
        srtLoaded = true
        
        // 添加到最近文件列表
        configStore.addRecentFile(srtPath)
        
        // 更新菜单
        if ((window as any).__updateRecentFilesMenu) {
          await (window as any).__updateRecentFilesMenu()
        }
      } catch (error) {
        await ElMessageBox.alert(
          `加载 SRT 文件失败：${error instanceof Error ? error.message : '未知错误'}`,
          '加载失败',
          {
            confirmButtonText: '确定',
            type: 'error',
          }
        )
      }
    }

    if (audioPath) {
      loadingMessage.value = '正在加载音频文件...'
      try {
        const fileName = audioPath.split('/').pop() || 'audio'
        const fileExtension = audioPath.split('.').pop()?.toLowerCase() || 'mp3'

        const audioFile: AudioFile = {
          name: fileName,
          path: audioPath,
          duration: 0,
          format: fileExtension,
        }
        await audioStore.loadAudio(audioFile)
        droppedFiles.value.audio = fileName
        audioLoaded = true
      } catch (error) {
        await ElMessageBox.alert(
          `加载音频文件失败：${error instanceof Error ? error.message : '未知错误'}`,
          '加载失败',
          {
            confirmButtonText: '确定',
            type: 'error',
          }
        )
      }
    }

    if (srtLoaded) {
      loadingMessage.value = '即将进入编辑器...'
      setTimeout(() => {
        router.push('/editor')
      }, 500)
    }
  } finally {
    if (!srtLoaded) {
      isLoading.value = false
      loadingMessage.value = ''
    }
  }
}

// 记录点击时间用于检测双击
let lastClickTime = 0

// 标题栏鼠标按下事件 - 开始拖拽窗口
const onTitlebarMousedown = async (e: MouseEvent) => {
  if (e.button === 0) {
    const now = Date.now()
    const timeDiff = now - lastClickTime
    lastClickTime = now

    // 检测双击（300ms 内的两次点击）
    if (timeDiff < 300) {
      await onTitlebarDoubleClick()
      return
    }

    e.preventDefault()
    try {
      await getCurrentWindow().startDragging()
    } catch (err) {
      // 拖拽失败，静默处理
    }
  }
}

// 双击标题栏切换最大化/还原
const onTitlebarDoubleClick = async () => {
  const window = getCurrentWindow()
  const isMaximized = await window.isMaximized()
  if (isMaximized) {
    await window.unmaximize()
  } else {
    await window.maximize()
  }
}


</script>

<template>
  <div class="welcome-page">
    <!-- 标题栏区域（可拖拽） -->
    <div class="titlebar" @mousedown.left="onTitlebarMousedown" @dblclick="onTitlebarDoubleClick">
      <span class="titlebar-title">SRT 字幕编辑器</span>
    </div>

    <div class="welcome-content">
      <!-- 主区域 -->
      <div class="main-section">
        <!-- 品牌区域 -->
        <div class="brand-area">
          <div class="brand-icon">
            <i class="i-mdi-subtitles-outline"></i>
          </div>
          <p class="brand-desc">专业的字幕编辑工具，支持音频同步和批量操作</p>
        </div>

        <!-- 拖放区域 -->
        <div
          class="drop-zone"
          :class="{ 'is-dragging': isDragging, 'is-loading': isLoading }"
        >
          <div v-if="!isLoading" class="drop-zone-content">
            <div class="drop-icon">
              <i class="i-mdi-file-upload-outline"></i>
            </div>
            <p class="drop-title">拖放文件到此处</p>
            <p class="drop-hint">
              支持 <span class="file-type srt">.srt</span> 字幕文件和
              <span class="file-type audio">音频文件</span>
            </p>
          </div>

          <div v-else class="loading-content">
            <div class="loading-spinner"></div>
            <p class="loading-text">{{ loadingMessage }}</p>
          </div>
        </div>

        <!-- 操作按钮 -->
        <div class="action-buttons">
          <button
            class="action-btn primary"
            :disabled="isLoading"
            @click="openSRTFile"
          >
            <el-icon><Document /></el-icon>
            <span>选择 SRT 文件</span>
          </button>
          <button
            class="action-btn secondary"
            :disabled="isLoading"
            @click="openAudioFile"
          >
            <el-icon><Headset /></el-icon>
            <span>选择音频文件</span>
          </button>
        </div>

        <!-- 提示信息 -->
        <p class="tip-text">
          提示：可单独加载 SRT 文件编辑，或同时加载音频以同步调整时间轴
        </p>
      </div>

      <!-- 底部快捷键提示 -->
      <div class="shortcuts-bar">
        <div class="shortcut-item">
          <span class="shortcut-label">打开文件</span>
          <div class="shortcut-keys">
            <kbd v-for="(k, i) in splitShortcut('Ctrl+O')" :key="i" class="key-cap">{{ k }}</kbd>
          </div>
        </div>
        <div class="shortcut-item">
          <span class="shortcut-label">保存</span>
          <div class="shortcut-keys">
            <kbd v-for="(k, i) in splitShortcut('Ctrl+S')" :key="i" class="key-cap">{{ k }}</kbd>
          </div>
        </div>
        <div class="shortcut-item">
          <span class="shortcut-label">播放/暂停</span>
          <div class="shortcut-keys">
            <kbd class="key-cap">Space</kbd>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.welcome-page {
  width: 100%;
  height: 100vh;
  display: flex;
  flex-direction: column;
  background: #f5f5f5;
  overflow: hidden;
}

/* 标题栏 */
.titlebar {
  height: 38px;
  background: #ffffff;
  border-bottom: 1px solid #e5e7eb;
  display: flex;
  align-items: center;
  justify-content: center;
  position: relative;
  flex-shrink: 0;
  -webkit-app-region: drag;
  -webkit-user-select: none;
  user-select: none;
  cursor: default;
}

.titlebar-title {
  font-size: 13px;
  font-weight: 500;
  color: #333;
  pointer-events: none;
}



/* 主内容区 */
.welcome-content {
  flex: 1;
  display: flex;
  flex-direction: column;
  justify-content: center;
  align-items: center;
  padding: 2rem;
  gap: 1.5rem;
}

.main-section {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 1.5rem;
  max-width: 480px;
  width: 100%;
}

/* 品牌区域 */
.brand-area {
  text-align: center;
}

.brand-icon {
  width: 64px;
  height: 64px;
  margin: 0 auto 0.75rem;
  background: linear-gradient(135deg, #3b82f6 0%, #6366f1 100%);
  border-radius: 16px;
  display: flex;
  align-items: center;
  justify-content: center;
  box-shadow: 0 4px 12px rgba(59, 130, 246, 0.3);
}

.brand-icon i {
  font-size: 32px;
  color: white;
}

.brand-desc {
  font-size: 14px;
  color: #6b7280;
  margin: 0;
}

/* 拖放区域 */
.drop-zone {
  width: 100%;
  padding: 2.5rem 2rem;
  background: #ffffff;
  border: 2px dashed #d1d5db;
  border-radius: 12px;
  text-align: center;
  transition: all 0.2s ease;
}

.drop-zone:hover {
  border-color: #3b82f6;
  background: #fafbff;
}

.drop-zone.is-dragging {
  border-color: #3b82f6;
  border-style: solid;
  background: #eff6ff;
  transform: scale(1.01);
  box-shadow: 0 0 0 4px rgba(59, 130, 246, 0.1);
}

.drop-zone.is-loading {
  border-color: #93c5fd;
  background: #f0f7ff;
}

.drop-zone-content {
  pointer-events: none;
}

.drop-icon {
  margin-bottom: 0.75rem;
}

.drop-icon i {
  font-size: 48px;
  color: #9ca3af;
}

.drop-zone:hover .drop-icon i,
.drop-zone.is-dragging .drop-icon i {
  color: #3b82f6;
}

.drop-title {
  font-size: 16px;
  font-weight: 600;
  color: #374151;
  margin: 0 0 0.5rem;
}

.drop-hint {
  font-size: 13px;
  color: #9ca3af;
  margin: 0;
}

.file-type {
  font-weight: 500;
}

.file-type.srt {
  color: #3b82f6;
}

.file-type.audio {
  color: #10b981;
}

/* 加载状态 */
.loading-content {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 1rem;
}

.loading-spinner {
  width: 36px;
  height: 36px;
  border: 3px solid #e5e7eb;
  border-top-color: #3b82f6;
  border-radius: 50%;
  animation: spin 0.8s linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

.loading-text {
  font-size: 14px;
  color: #6b7280;
  margin: 0;
}

/* 操作按钮 */
.action-buttons {
  display: flex;
  gap: 0.75rem;
  width: 100%;
}

.action-btn {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 0.5rem;
  padding: 0.75rem 1rem;
  font-size: 14px;
  font-weight: 500;
  border-radius: 8px;
  cursor: pointer;
  transition: all 0.2s ease;
  border: none;
}

.action-btn:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.action-btn.primary {
  background: #3b82f6;
  color: white;
}

.action-btn.primary:hover:not(:disabled) {
  background: #2563eb;
}

.action-btn.secondary {
  background: #ffffff;
  color: #374151;
  border: 1px solid #d1d5db;
}

.action-btn.secondary:hover:not(:disabled) {
  background: #f9fafb;
  border-color: #9ca3af;
}

.action-btn .el-icon {
  font-size: 18px;
}

/* 提示文字 */
.tip-text {
  font-size: 12px;
  color: #9ca3af;
  margin: 0;
  text-align: center;
}

/* 快捷键栏 */
.shortcuts-bar {
  display: flex;
  gap: 2rem;
  padding: 0.875rem 1.5rem;
  background: #ffffff;
  border: 1px solid #e5e7eb;
  border-radius: 10px;
}

.shortcut-item {
  display: flex;
  align-items: center;
  gap: 0.625rem;
}

.shortcut-label {
  font-size: 13px;
  color: #666;
}

.shortcut-keys {
  display: flex;
  align-items: center;
  gap: 4px;
}

.key-cap {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 28px;
  height: 28px;
  padding: 0 8px;
  background: #f5f5f5;
  border: 1px solid #e0e0e0;
  border-radius: 6px;
  font-size: 13px;
  font-family: -apple-system, BlinkMacSystemFont, 'SF Pro Text', sans-serif;
  color: #555;
  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.05);
}
</style>

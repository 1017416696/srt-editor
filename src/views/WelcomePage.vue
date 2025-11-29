<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { useRouter } from 'vue-router'
import { open } from '@tauri-apps/plugin-dialog'
import { invoke } from '@tauri-apps/api/core'
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow'
import { ElMessage, ElMessageBox } from 'element-plus'
import { useSubtitleStore } from '@/stores/subtitle'
import { useAudioStore } from '@/stores/audio'
import type { SRTFile, AudioFile } from '@/types/subtitle'

const router = useRouter()
const subtitleStore = useSubtitleStore()
const audioStore = useAudioStore()

// 检测操作系统
const isMac = computed(() => {
  return typeof navigator !== 'undefined' && /Mac|iPhone|iPad|iPod/.test(navigator.platform)
})

// 根据操作系统返回快捷键字符串
const getShortcutKey = (shortcut: string) => {
  if (isMac.value) {
    return shortcut
      .replace(/Ctrl\+/g, '⌘ ')
      .replace(/Shift\+/g, '⇧ ')
      .replace(/\+/g, ' ')
  }
  return shortcut
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

  // 监听拖放悬停
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

  // 识别文件类型
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
    // 加载 SRT 文件
    if (srtPath) {
      loadingMessage.value = '正在加载字幕文件...'
      try {
        const srtFile = await invoke<SRTFile>('read_srt', { filePath: srtPath })
        await subtitleStore.loadSRTFile(srtFile)
        droppedFiles.value.srt = srtPath.split('/').pop() || srtPath
        srtLoaded = true
        ElMessage.success(`字幕文件加载成功：${droppedFiles.value.srt}`)
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

    // 加载音频文件
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
        ElMessage.success(`音频文件加载成功：${fileName}`)
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

    // 如果至少加载了字幕文件，跳转到编辑器
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
</script>

<template>
  <div class="welcome-page">
    <div class="welcome-container">
      <!-- 顶部标题区域 -->
      <div class="header-section">
        <h1 class="title">SRT 字幕编辑器</h1>
        <p class="subtitle">专业的字幕编辑工具，支持音频同步和批量操作</p>
      </div>

      <!-- 中部拖放区域 -->
      <div
        class="drop-section"
        :class="{ 'is-dragging': isDragging, 'is-loading': isLoading }"
      >
        <div v-if="!isLoading" class="drop-zone-content">
          <i class="i-mdi-file-document-edit text-8xl mb-6 text-indigo-400"></i>
          <p class="text-2xl mb-3 text-gray-700 font-semibold">拖放文件到此处</p>
          <p class="text-base text-gray-500 mb-6">
            支持 <span class="text-indigo-600 font-medium">.srt</span> 字幕文件和
            <span class="text-green-600 font-medium">音频文件</span>
          </p>
          <p class="text-sm text-gray-400">可以同时拖放字幕和音频文件</p>
        </div>

        <div v-else class="loading-content">
          <!-- 现代化的加载动画 -->
          <div class="loading-animation">
            <div class="loading-spinner">
              <div class="spinner-ring"></div>
              <div class="spinner-ring"></div>
              <div class="spinner-ring"></div>
            </div>
            <div class="loading-icon">
              <i class="i-mdi-file-document-edit text-4xl text-indigo-500"></i>
            </div>
          </div>
          <p class="text-xl mt-8 text-gray-700 font-medium loading-text">{{ loadingMessage }}</p>
          <div class="loading-dots">
            <span></span>
            <span></span>
            <span></span>
          </div>
        </div>
      </div>

      <!-- 底部操作区域 -->
      <div class="action-section">
        <div class="action-buttons">
          <el-button
            type="primary"
            size="large"
            :loading="isLoading"
            :disabled="isLoading"
            @click="openSRTFile"
          >
            <i class="i-mdi-file-document mr-2"></i>
            选择 SRT 文件
          </el-button>
          <el-button
            type="success"
            size="large"
            :loading="isLoading"
            :disabled="isLoading"
            @click="openAudioFile"
          >
            <i class="i-mdi-headset mr-2"></i>
            选择音频文件
          </el-button>
        </div>

        <div class="tips-container">
          <el-alert type="info" :closable="false" show-icon>
            <template #title>
              <span class="text-sm"
                >提示：您可以单独加载 SRT 文件进行编辑，也可以同时加载音频文件以便同步调整时间轴</span
              >
            </template>
          </el-alert>
        </div>

        <div class="shortcuts-hint">
          <p class="text-xs text-gray-400">
            快捷键：<kbd>{{ getShortcutKey('Ctrl+O') }}</kbd> 打开文件 ·
            <kbd>{{ getShortcutKey('Ctrl+S') }}</kbd> 保存 · <kbd>Space</kbd> 播放/暂停
          </p>
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
  align-items: center;
  justify-content: center;
  background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
  overflow: hidden;
}

.welcome-container {
  width: 90%;
  max-width: 700px;
  background: white;
  border-radius: 1.5rem;
  box-shadow: 0 25px 70px rgba(0, 0, 0, 0.35);
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

/* 顶部标题区域 */
.header-section {
  text-align: center;
  padding: 2.5rem 2rem 1.5rem;
  background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
  color: white;
}

.title {
  font-size: 2.25rem;
  font-weight: 800;
  margin-bottom: 0.5rem;
  letter-spacing: -0.5px;
}

.subtitle {
  font-size: 1rem;
  opacity: 0.95;
  font-weight: 300;
}

/* 中部拖放区域 */
.drop-section {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 3rem 2rem;
  margin: 2rem;
  border: 3px dashed #d1d5db;
  border-radius: 1rem;
  background-color: #f9fafb;
  transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
  min-height: 280px;
}

.drop-section:hover {
  border-color: #667eea;
  background-color: #f3f4f6;
  transform: translateY(-2px);
  box-shadow: 0 8px 16px rgba(102, 126, 234, 0.1);
}

.drop-section.is-dragging {
  border-color: #667eea;
  border-width: 4px;
  background: linear-gradient(135deg, #eef2ff 0%, #f3e8ff 100%);
  border-style: solid;
  transform: scale(1.02);
  box-shadow: 0 12px 24px rgba(102, 126, 234, 0.2);
}

.drop-section.is-loading {
  border-color: #a5b4fc;
  background: linear-gradient(135deg, #eef2ff 0%, #e0e7ff 100%);
}

.drop-zone-content {
  display: flex;
  flex-direction: column;
  align-items: center;
  text-align: center;
  pointer-events: none;
}

.loading-content {
  display: flex;
  flex-direction: column;
  align-items: center;
  text-align: center;
}

/* 底部操作区域 */
.action-section {
  padding: 1.5rem 2rem 2.5rem;
  background-color: #fafafa;
}

.action-buttons {
  display: flex;
  gap: 1rem;
  justify-content: center;
  margin-bottom: 1.5rem;
}

.action-buttons .el-button {
  flex: 1;
  max-width: 220px;
  height: 48px;
  font-size: 15px;
  font-weight: 600;
}

.tips-container {
  margin-bottom: 1.25rem;
}

.shortcuts-hint {
  text-align: center;
  padding-top: 1rem;
  border-top: 1px solid #e5e7eb;
}

.shortcuts-hint kbd {
  display: inline-block;
  padding: 0.2rem 0.5rem;
  font-size: 0.75rem;
  color: #333;
  background-color: #f3f4f6;
  border: 1px solid #d1d5db;
  border-radius: 0.3rem;
  box-shadow: 0 2px 3px rgba(0, 0, 0, 0.08);
  font-family: 'SF Mono', 'Monaco', 'Consolas', monospace;
}

/* 加载动画 */
.loading-animation {
  position: relative;
  width: 120px;
  height: 120px;
}

.loading-spinner {
  position: absolute;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
}

.spinner-ring {
  position: absolute;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  border: 4px solid transparent;
  border-radius: 50%;
  animation: spin-ring 2s cubic-bezier(0.5, 0, 0.5, 1) infinite;
}

.spinner-ring:nth-child(1) {
  border-top-color: #667eea;
  animation-delay: -0.45s;
}

.spinner-ring:nth-child(2) {
  border-top-color: #764ba2;
  animation-delay: -0.3s;
}

.spinner-ring:nth-child(3) {
  border-top-color: #f093fb;
  animation-delay: -0.15s;
}

.loading-icon {
  position: absolute;
  top: 50%;
  left: 50%;
  transform: translate(-50%, -50%);
  animation: pulse 2s ease-in-out infinite;
}

.loading-text {
  animation: fade-in-out 2s ease-in-out infinite;
}

.loading-dots {
  display: flex;
  gap: 8px;
  justify-content: center;
  margin-top: 12px;
}

.loading-dots span {
  width: 8px;
  height: 8px;
  background-color: #667eea;
  border-radius: 50%;
  animation: bounce-dot 1.4s infinite ease-in-out both;
}

.loading-dots span:nth-child(1) {
  animation-delay: -0.32s;
}

.loading-dots span:nth-child(2) {
  animation-delay: -0.16s;
}

/* 动画关键帧 */
@keyframes spin-ring {
  0% {
    transform: rotate(0deg);
  }
  100% {
    transform: rotate(360deg);
  }
}

@keyframes pulse {
  0%,
  100% {
    transform: translate(-50%, -50%) scale(1);
  }
  50% {
    transform: translate(-50%, -50%) scale(1.1);
  }
}

@keyframes fade-in-out {
  0%,
  100% {
    opacity: 1;
  }
  50% {
    opacity: 0.6;
  }
}

@keyframes bounce-dot {
  0%,
  80%,
  100% {
    transform: scale(0);
    opacity: 0.5;
  }
  40% {
    transform: scale(1);
    opacity: 1;
  }
}
</style>

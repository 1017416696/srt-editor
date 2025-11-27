<script setup lang="ts">
import { ref, computed } from 'vue'
import { useRouter } from 'vue-router'
import { open } from '@tauri-apps/plugin-dialog'
import { invoke } from '@tauri-apps/api/core'
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
    // 在 macOS 上转换为符号格式，并添加空格
    return shortcut
      .replace(/Ctrl\+/g, '⌘ ')
      .replace(/Shift\+/g, '⇧ ')
      .replace(/\+/g, ' ')
  }
  return shortcut
}

const isDragging = ref(false)
const isLoading = ref(false)

// 处理文件拖放
const handleDragOver = (e: DragEvent) => {
  e.preventDefault()
  isDragging.value = true
}

const handleDragLeave = () => {
  isDragging.value = false
}

const handleDrop = async (e: DragEvent) => {
  e.preventDefault()
  isDragging.value = false

  const files = Array.from(e.dataTransfer?.files || [])
  await processFiles(files)
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
      isLoading.value = true
      // 调用 Tauri 后端读取 SRT 文件
      const srtFile = await invoke<SRTFile>('read_srt', { filePath: selected })
      await subtitleStore.loadSRTFile(srtFile)
      isLoading.value = false

      // 加载成功后直接进入编辑器
      router.push('/editor')
    }
  } catch (error) {
    isLoading.value = false
    // 加载失败，静默处理
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
      isLoading.value = true
      try {
        // 创建音频文件对象
        const fileName = selected.split('/').pop() || 'audio'
        const fileExtension = selected.split('.').pop()?.toLowerCase() || 'mp3'

        const audioFile: AudioFile = {
          name: fileName,
          path: selected,
          duration: 0,
          format: fileExtension,
        }
        await audioStore.loadAudio(audioFile)
        isLoading.value = false

        // 如果字幕已加载，直接进入编辑器
        if (subtitleStore.entries.length > 0) {
          router.push('/editor')
        }
      } catch (error) {
        isLoading.value = false
        // 加载失败，静默处理
      }
    }
  } catch (error) {
    isLoading.value = false
    // 加载失败，静默处理
  }
}

// 处理文件
const processFiles = async (files: File[]) => {
  const srtFile = files.find((f) => f.name.toLowerCase().endsWith('.srt'))
  const audioFile = files.find((f) =>
    /\.(mp3|wav|ogg|flac|m4a|aac)$/i.test(f.name.toLowerCase())
  )

  isLoading.value = true

  try {
    // 在 Tauri 中，我们需要通过路径加载文件
    // 拖放功能暂不支持，需要用户使用"选择文件"按钮
  } finally {
    isLoading.value = false
  }
}

// 跳过文件加载，直接进入编辑器（用于测试）
const skipToEditor = () => {
  router.push('/editor')
}
</script>

<template>
  <div class="welcome-page">
    <div class="welcome-container">
      <!-- 标题区域 -->
      <div class="header">
        <h1 class="title">SRT 字幕编辑器</h1>
        <p class="subtitle">专业的字幕编辑工具，支持音频同步和批量操作</p>
      </div>

      <!-- 拖放区域 -->
      <div
        class="drop-zone"
        :class="{ 'is-dragging': isDragging }"
        @dragover="handleDragOver"
        @dragleave="handleDragLeave"
        @drop="handleDrop"
      >
        <div class="drop-zone-content">
          <i class="i-mdi-file-document-edit text-6xl mb-4 text-gray-400"></i>
          <p class="text-xl mb-2 text-gray-700">拖放文件到此处</p>
          <p class="text-sm text-gray-500">支持 .srt 字幕文件和音频文件</p>
        </div>
      </div>

      <!-- 操作按钮 -->
      <div class="actions">
        <el-button
          type="primary"
          size="large"
          :loading="isLoading"
          :icon="'Document'"
          @click="openSRTFile"
        >
          选择 SRT 文件
        </el-button>
        <el-button
          type="success"
          size="large"
          :loading="isLoading"
          :icon="'Headset'"
          @click="openAudioFile"
        >
          选择音频文件
        </el-button>
      </div>

      <!-- 提示信息 -->
      <div class="tips">
        <el-alert type="info" :closable="false" show-icon>
          <template #title>
            <span class="text-sm"
              >提示：您可以单独加载 SRT 文件进行编辑，也可以同时加载音频文件以便同步调整时间轴</span
            >
          </template>
        </el-alert>
      </div>

      <!-- 快捷键说明（可选） -->
      <div class="shortcuts-hint">
        <p class="text-xs text-gray-400">
          快捷键：<kbd>{{ getShortcutKey('Ctrl+O') }}</kbd> 打开文件 · <kbd>{{ getShortcutKey('Ctrl+S') }}</kbd> 保存 · <kbd>Space</kbd> 播放/暂停
        </p>
      </div>

      <!-- 开发调试按钮 -->
      <div v-if="false" class="debug-actions">
        <el-button text @click="skipToEditor">跳过（测试）</el-button>
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
}

.welcome-container {
  width: 90%;
  max-width: 600px;
  padding: 3rem;
  background: white;
  border-radius: 1rem;
  box-shadow: 0 20px 60px rgba(0, 0, 0, 0.3);
}

.header {
  text-align: center;
  margin-bottom: 2rem;
}

.title {
  font-size: 2rem;
  font-weight: 700;
  color: #333;
  margin-bottom: 0.5rem;
}

.subtitle {
  font-size: 0.95rem;
  color: #666;
}

.drop-zone {
  border: 2px dashed #d1d5db;
  border-radius: 0.75rem;
  padding: 3rem 2rem;
  text-align: center;
  transition: all 0.3s ease;
  background-color: #f9fafb;
  margin-bottom: 2rem;
}

.drop-zone:hover {
  border-color: #667eea;
  background-color: #f3f4f6;
}

.drop-zone.is-dragging {
  border-color: #667eea;
  background-color: #eef2ff;
  border-style: solid;
  transform: scale(1.02);
}

.drop-zone-content {
  display: flex;
  flex-direction: column;
  align-items: center;
  pointer-events: none;
}

.actions {
  display: flex;
  gap: 1rem;
  justify-content: center;
  margin-bottom: 2rem;
}

.tips {
  margin-bottom: 1.5rem;
}

.shortcuts-hint {
  text-align: center;
  padding-top: 1rem;
  border-top: 1px solid #e5e7eb;
}

.shortcuts-hint kbd {
  display: inline-block;
  padding: 0.15rem 0.4rem;
  font-size: 0.75rem;
  color: #333;
  background-color: #f3f4f6;
  border: 1px solid #d1d5db;
  border-radius: 0.25rem;
  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.05);
  font-family: monospace;
}

.debug-actions {
  text-align: center;
  margin-top: 1rem;
}
</style>

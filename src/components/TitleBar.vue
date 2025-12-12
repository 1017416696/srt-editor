<script setup lang="ts">
import { ref } from 'vue'
import { useRouter } from 'vue-router'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { useTabManagerStore } from '@/stores/tabManager'
import { useAudioStore } from '@/stores/audio'
import { Close, Plus } from '@element-plus/icons-vue'

const router = useRouter()
const tabManager = useTabManagerStore()
const audioStore = useAudioStore()

// 是否为 macOS（需要为红绿灯按钮预留空间）
const isMacOS = ref(/Mac|iPhone|iPad|iPod/.test(navigator.userAgent))

// 拖拽状态
const draggedTabId = ref<string | null>(null)
const dragOverTabId = ref<string | null>(null)

// 记录点击时间用于检测双击
let lastClickTime = 0

// 开始拖拽窗口
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
    await getCurrentWindow().startDragging()
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

// 切换标签页
const handleTabClick = (tabId: string) => {
  tabManager.setActiveTab(tabId)
}

// 关闭标签页
const handleCloseTab = (e: MouseEvent, tabId: string) => {
  e.stopPropagation()
  // 清理该 tab 的音频缓存
  audioStore.removeTabCache(tabId)
  const shouldGoWelcome = tabManager.closeTab(tabId)
  if (shouldGoWelcome) {
    router.push('/')
  }
}

// 新建标签页（跳转欢迎页）
const handleNewTab = () => {
  router.push('/')
}

// 拖拽开始
const handleDragStart = (e: DragEvent, tabId: string) => {
  draggedTabId.value = tabId
  if (e.dataTransfer) {
    e.dataTransfer.effectAllowed = 'move'
    e.dataTransfer.setData('text/plain', tabId)
  }
}

// 拖拽经过
const handleDragOver = (e: DragEvent, tabId: string) => {
  e.preventDefault()
  if (draggedTabId.value && draggedTabId.value !== tabId) {
    dragOverTabId.value = tabId
  }
}

// 拖拽离开
const handleDragLeave = () => {
  dragOverTabId.value = null
}

// 拖拽放下
const handleDrop = (e: DragEvent, targetTabId: string) => {
  e.preventDefault()
  if (draggedTabId.value && draggedTabId.value !== targetTabId) {
    const fromIndex = tabManager.tabs.findIndex(t => t.id === draggedTabId.value)
    const toIndex = tabManager.tabs.findIndex(t => t.id === targetTabId)
    if (fromIndex !== -1 && toIndex !== -1) {
      tabManager.reorderTabs(fromIndex, toIndex)
    }
  }
  draggedTabId.value = null
  dragOverTabId.value = null
}

// 拖拽结束
const handleDragEnd = () => {
  draggedTabId.value = null
  dragOverTabId.value = null
}
</script>

<template>
  <div class="titlebar">
    <!-- 左侧拖拽区域（macOS 红绿灯右边） -->
    <div 
      class="drag-region drag-region-left" 
      :class="{ 'macos': isMacOS }"
      @mousedown.left="onTitlebarMousedown" 
      @dblclick="onTitlebarDoubleClick"
    ></div>
    
    <!-- 标签页区域 -->
    <div class="tabs-container">
      <div
        v-for="tab in tabManager.tabs"
        :key="tab.id"
        class="tab-item"
        :class="{
          active: tab.id === tabManager.activeTabId,
          'drag-over': tab.id === dragOverTabId
        }"
        draggable="true"
        @click="handleTabClick(tab.id)"
        @dragstart="handleDragStart($event, tab.id)"
        @dragover="handleDragOver($event, tab.id)"
        @dragleave="handleDragLeave"
        @drop="handleDrop($event, tab.id)"
        @dragend="handleDragEnd"
      >
        <span class="tab-title" :title="tab.fileName">{{ tab.fileName }}</span>
        <button
          class="tab-close-btn"
          @click="handleCloseTab($event, tab.id)"
          @mousedown.stop
        >
          <el-icon :size="12"><Close /></el-icon>
        </button>
      </div>
      
      <!-- 新建标签页按钮 -->
      <button class="new-tab-btn" @click="handleNewTab" @mousedown.stop title="打开新文件">
        <el-icon :size="14"><Plus /></el-icon>
      </button>
    </div>
    
    <!-- 右侧拖拽区域 -->
    <div class="drag-region drag-region-right" @mousedown.left="onTitlebarMousedown" @dblclick="onTitlebarDoubleClick"></div>
  </div>
</template>

<style scoped>
.titlebar {
  height: 38px;
  background: linear-gradient(180deg, #f8fafc 0%, #f1f5f9 100%);
  border-bottom: 1px solid #e2e8f0;
  display: flex;
  align-items: center;
  user-select: none;
}

/* 拖拽区域 */
.drag-region {
  height: 100%;
  -webkit-app-region: drag;
  app-region: drag;
}

.drag-region-left {
  width: 12px; /* Windows/Linux 默认小间距 */
  flex-shrink: 0;
}

.drag-region-left.macos {
  width: 80px; /* macOS 红绿灯区域 */
}

.drag-region-right {
  flex: 1;
  min-width: 100px;
}

.tabs-container {
  display: flex;
  align-items: center;
  gap: 2px;
  height: 100%;
  overflow-x: auto;
  overflow-y: hidden;
  flex-shrink: 0;
  max-width: calc(100% - 200px); /* 留出两侧拖拽区域 */
}

.tabs-container::-webkit-scrollbar {
  display: none;
}

.tab-item {
  display: flex;
  align-items: center;
  gap: 6px;
  height: 28px;
  padding: 0 8px 0 12px;
  background: transparent;
  border: none;
  border-radius: 6px;
  cursor: pointer;
  transition: all 0.15s ease;
  max-width: 180px;
  min-width: 100px;
  flex-shrink: 0;
}

.tab-item:hover {
  background: rgba(255, 255, 255, 0.6);
}

.tab-item.active {
  background: #fff;
  box-shadow: 0 -1px 3px rgba(0, 0, 0, 0.05);
}

.tab-item.drag-over {
  background: rgba(59, 130, 246, 0.1);
  border-left: 2px solid #3b82f6;
}

.tab-title {
  flex: 1;
  font-size: 12px;
  color: #64748b;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.tab-item.active .tab-title {
  color: #1e293b;
  font-weight: 500;
}

.tab-close-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 18px;
  height: 18px;
  border: none;
  background: transparent;
  border-radius: 4px;
  cursor: pointer;
  opacity: 0;
  transition: all 0.15s ease;
  color: #94a3b8;
}

.tab-item:hover .tab-close-btn {
  opacity: 1;
}

.tab-close-btn:hover {
  background: rgba(0, 0, 0, 0.1);
  color: #64748b;
}

.new-tab-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 26px;
  height: 26px;
  border: none;
  background: transparent;
  border-radius: 6px;
  cursor: pointer;
  color: #64748b;
  transition: all 0.15s ease;
  flex-shrink: 0;
}

.new-tab-btn:hover {
  background: rgba(0, 0, 0, 0.05);
  color: #3b82f6;
}
</style>

<script setup lang="ts">
import { DocumentAdd, Search, Scissor, Guide, Magnet, Setting } from '@element-plus/icons-vue'

const props = defineProps<{
  hasAudio: boolean
  hasCurrentEntry: boolean
  isScissorMode: boolean
  isSnapEnabled: boolean
  isAltPressed: boolean
  showSearchPanel: boolean
  canMerge: boolean
}>()

const emit = defineEmits<{
  (e: 'add-subtitle'): void
  (e: 'toggle-search'): void
  (e: 'toggle-scissor'): void
  (e: 'merge-subtitles'): void
  (e: 'align-to-waveform'): void
  (e: 'toggle-snap'): void
  (e: 'open-settings'): void
}>()
</script>

<template>
  <div class="sidebar">
    <div class="sidebar-top">
      <button
        class="sidebar-btn"
        @click="emit('add-subtitle')"
        title="添加字幕 (N)"
      >
        <el-icon><DocumentAdd /></el-icon>
      </button>
      <button
        class="sidebar-btn"
        :class="{ active: showSearchPanel }"
        @click="emit('toggle-search')"
        title="搜索字幕"
      >
        <el-icon><Search /></el-icon>
      </button>
      <button
        class="sidebar-btn"
        :class="{ active: isScissorMode }"
        @click="emit('toggle-scissor')"
        :disabled="!hasAudio"
        title="分割字幕 (X)"
      >
        <el-icon><Scissor /></el-icon>
      </button>
      <button
        class="sidebar-btn"
        @click="emit('merge-subtitles')"
        :disabled="!canMerge"
        title="合并字幕 (M)"
      >
        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d="M4 4v16M4 4h4M4 20h4"/>
          <path d="M20 4v16M20 4h-4M20 20h-4"/>
          <path d="M9 12h2M13 12h2"/>
          <path d="M9 12l2-2M9 12l2 2"/>
          <path d="M15 12l-2-2M15 12l-2 2"/>
        </svg>
      </button>
      <button
        class="sidebar-btn"
        @click="emit('align-to-waveform')"
        :disabled="!hasAudio || !hasCurrentEntry"
        title="波形对齐 (A)"
      >
        <el-icon><Guide /></el-icon>
      </button>
      <button
        class="sidebar-btn"
        :class="{ active: isSnapEnabled, 'snap-paused': isSnapEnabled && isAltPressed }"
        @click="emit('toggle-snap')"
        :disabled="!hasAudio"
        :title="isSnapEnabled && isAltPressed ? '吸附已暂停 (松开Alt恢复)' : '拖拽吸附 (S) 按住Alt临时禁用'"
      >
        <el-icon><Magnet /></el-icon>
      </button>
    </div>
    <div class="sidebar-bottom">
      <button
        class="sidebar-btn"
        @click="emit('open-settings')"
        title="设置"
      >
        <el-icon><Setting /></el-icon>
      </button>
    </div>
  </div>
</template>

<style scoped>
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

.sidebar-btn.active svg {
  color: #3b82f6;
}

/* 吸附暂停状态（Alt 键按下时） */
.sidebar-btn.snap-paused {
  background: linear-gradient(135deg, #fef3c7 0%, #fde68a 100%);
}

.sidebar-btn.snap-paused::before {
  background: linear-gradient(180deg, #f59e0b 0%, #d97706 100%);
}

.sidebar-btn.snap-paused svg {
  color: #f59e0b;
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
</style>

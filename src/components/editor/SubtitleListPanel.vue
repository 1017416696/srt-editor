<script setup lang="ts">
import { ref, computed, watch, nextTick, onMounted, onBeforeUnmount } from 'vue'
import { DocumentCopy, VideoPlay, Delete, WarningFilled } from '@element-plus/icons-vue'
import SearchReplaceBar from './SearchReplaceBar.vue'
import type { SubtitleEntry } from '@/types/subtitle'

const props = defineProps<{
  entries: SubtitleEntry[]
  selectedEntryId: number | null
  searchText: string
  replaceText: string
  showSearchPanel: boolean
  showReplace: boolean
  searchResults: number[]
  hasAudio: boolean
  currentFilePath: string | null
  formatTimeStamp: (ts: any) => string
  showOnlyNeedsCorrection?: boolean // 只显示需要校正的字幕
}>()

const emit = defineEmits<{
  (e: 'update:searchText', value: string): void
  (e: 'update:replaceText', value: string): void
  (e: 'update:showReplace', value: boolean): void
  (e: 'select-entry', id: number): void
  (e: 'double-click-entry', id: number): void
  (e: 'copy-text', id: number): void
  (e: 'play-audio', id: number): void
  (e: 'delete-entry', id: number): void
  (e: 'replace-one'): void
  (e: 'replace-all'): void
  (e: 'close-search'): void
  (e: 'go-back'): void
  (e: 'toggle-correction-mark', id: number): void
}>()

const subtitleListContainer = ref<HTMLElement | null>(null)
const subtitleItemRefs: Record<number, HTMLElement | null> = {}
const searchReplaceRef = ref<InstanceType<typeof SearchReplaceBar> | null>(null)

// 虚拟滚动配置
const SUBTITLE_ITEM_HEIGHT = 76
const VIRTUAL_OVERSCAN = 5

// 滚动状态
const scrollTop = ref(0)
const containerHeight = ref(400)

// 计算显示的字幕列表（根据搜索结果和校正筛选过滤）
const filteredEntries = computed(() => {
  let result = props.entries
  
  // 搜索过滤
  if (props.searchText) {
    result = result.filter((entry) => props.searchResults.includes(entry.id))
  }
  
  // 只显示需要校正的字幕
  if (props.showOnlyNeedsCorrection) {
    result = result.filter((entry) => entry.needsCorrection)
  }
  
  return result
})

// 需要校正的字幕数量
const needsCorrectionCount = computed(() => {
  return props.entries.filter(e => e.needsCorrection).length
})

// 计算可见范围
const visibleRange = computed(() => {
  const start = Math.max(0, Math.floor(scrollTop.value / SUBTITLE_ITEM_HEIGHT) - VIRTUAL_OVERSCAN)
  const visibleCount = Math.ceil(containerHeight.value / SUBTITLE_ITEM_HEIGHT) + VIRTUAL_OVERSCAN * 2
  const end = Math.min(filteredEntries.value.length, start + visibleCount)
  return { start, end }
})

// 虚拟列表数据
const virtualList = computed(() => {
  const { start, end } = visibleRange.value
  return filteredEntries.value.slice(start, end).map((entry, index) => ({
    data: entry,
    index: start + index,
  }))
})

// 总高度（用于滚动条）
const totalHeight = computed(() => filteredEntries.value.length * SUBTITLE_ITEM_HEIGHT)

// 偏移量（用于定位可见项）
const offsetY = computed(() => visibleRange.value.start * SUBTITLE_ITEM_HEIGHT)

// 处理滚动
const handleVirtualScroll = (e: Event) => {
  const target = e.target as HTMLElement
  scrollTop.value = target.scrollTop
}

// 滚动到指定索引
const virtualScrollTo = (index: number) => {
  const container = subtitleListContainer.value
  if (container) {
    const targetScrollTop = index * SUBTITLE_ITEM_HEIGHT - containerHeight.value / 2 + SUBTITLE_ITEM_HEIGHT / 2
    container.scrollTop = Math.max(0, targetScrollTop)
  }
}

// 更新容器高度
const updateContainerHeight = () => {
  const container = subtitleListContainer.value
  if (container) {
    containerHeight.value = container.clientHeight
  }
}

// 滚动到指定字幕项
const scrollToEntry = (entryId: number) => {
  const index = filteredEntries.value.findIndex(e => e.id === entryId)
  if (index !== -1) {
    virtualScrollTo(index)
  }
}

// 高亮搜索结果中的匹配文本
const highlightSearchText = (text: string, searchQuery: string): string => {
  if (!searchQuery) return text

  try {
    const regex = new RegExp(`(${searchQuery.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')})`, 'gi')
    return text.replace(regex, '<mark>$1</mark>')
  } catch {
    return text
  }
}

// 键盘导航字幕列表
const navigateSubtitleList = (direction: 'up' | 'down') => {
  if (filteredEntries.value.length === 0) return

  let targetIndex = -1

  if (props.selectedEntryId === null) {
    targetIndex = direction === 'down' ? 0 : filteredEntries.value.length - 1
  } else {
    const currentIndex = filteredEntries.value.findIndex(e => e.id === props.selectedEntryId)

    if (currentIndex !== -1) {
      if (direction === 'down') {
        targetIndex = Math.min(currentIndex + 1, filteredEntries.value.length - 1)
      } else {
        targetIndex = Math.max(currentIndex - 1, 0)
      }
    } else {
      targetIndex = direction === 'down' ? 0 : filteredEntries.value.length - 1
    }
  }

  if (targetIndex !== -1) {
    const targetEntry = filteredEntries.value[targetIndex]
    if (targetEntry) {
      emit('select-entry', targetEntry.id)
      nextTick(() => {
        scrollToEntry(targetEntry.id)
      })
    }
  }
}

// ResizeObserver
let resizeObserver: ResizeObserver | null = null

onMounted(() => {
  nextTick(() => {
    updateContainerHeight()
    if (subtitleListContainer.value) {
      resizeObserver = new ResizeObserver(() => {
        updateContainerHeight()
      })
      resizeObserver.observe(subtitleListContainer.value)
    }
  })
})

onBeforeUnmount(() => {
  if (resizeObserver) {
    resizeObserver.disconnect()
    resizeObserver = null
  }
})

// 暴露给父组件
defineExpose({
  scrollToEntry,
  navigateSubtitleList,
  focusSearch: () => searchReplaceRef.value?.focus(),
  blurSearch: () => searchReplaceRef.value?.blur(),
  getSearchInput: () => searchReplaceRef.value?.getSearchInput(),
  getReplaceInput: () => searchReplaceRef.value?.getReplaceInput(),
  getFilteredEntries: () => filteredEntries.value,
  getNeedsCorrectionCount: () => needsCorrectionCount.value
})
</script>

<template>
  <div class="subtitle-list-panel">
    <!-- 搜索和替换框 -->
    <SearchReplaceBar
      v-if="showSearchPanel"
      ref="searchReplaceRef"
      :model-value="searchText"
      :replace-value="replaceText"
      :match-count="searchResults.length"
      :show-replace="showReplace"
      @update:model-value="emit('update:searchText', $event)"
      @update:replace-value="emit('update:replaceText', $event)"
      @update:show-replace="emit('update:showReplace', $event)"
      @replace-one="emit('replace-one')"
      @replace-all="emit('replace-all')"
      @close="emit('close-search')"
      @navigate="navigateSubtitleList"
    />

    <!-- 字幕列表（虚拟滚动） -->
    <div class="subtitle-list" ref="subtitleListContainer" @scroll="handleVirtualScroll">
      <div class="virtual-list-wrapper" :style="{ height: totalHeight + 'px', paddingTop: offsetY + 'px' }">
        <div
          v-for="{ data: entry } in virtualList"
          :key="entry.id"
          :ref="(el) => { if (el) subtitleItemRefs[entry.id] = el as HTMLElement }"
          class="subtitle-item"
          :class="{
            'is-selected': selectedEntryId === entry.id
          }"
          @click="emit('select-entry', entry.id)"
          @dblclick="emit('double-click-entry', entry.id)"
        >
          <div class="item-header">
            <div class="item-header-left">
              <span class="item-number">{{ entry.id }}</span>
              <!-- 需要校正标记 -->
              <el-tooltip v-if="entry.needsCorrection" content="需要二次校正" placement="top">
                <span class="correction-mark" @click.stop="emit('toggle-correction-mark', entry.id)">
                  <WarningFilled />
                </span>
              </el-tooltip>
            </div>
            <span class="item-time">
              {{ formatTimeStamp(entry.startTime).slice(0, 8) }}
              -
              {{ formatTimeStamp(entry.endTime).slice(0, 8) }}
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
                @click.stop="emit('copy-text', entry.id)"
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
                @click.stop="emit('play-audio', entry.id)"
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
                @click.stop="emit('delete-entry', entry.id)"
              >
                <template #icon>
                  <Delete />
                </template>
              </el-button>
            </div>
          </div>
        </div>
      </div>

      <!-- 空状态 -->
      <div v-if="filteredEntries.length === 0 && entries.length > 0" class="empty-state empty-state-absolute">
        <p class="text-gray-400">未找到匹配的字幕</p>
      </div>

      <div v-if="entries.length === 0" class="empty-state empty-state-absolute">
        <p class="text-gray-400">暂无字幕数据</p>
        <el-button type="text" @click="emit('go-back')">返回加载文件</el-button>
      </div>
    </div>

    <!-- 底部统计:字幕文件名 + 字幕数量 -->
    <div class="list-footer">
      <span class="file-info">
        {{ currentFilePath ? currentFilePath.split('/').pop()?.replace('.srt', '') : '豆包输入法' }}.srt
      </span>
      <span v-if="selectedEntryId" class="count-info">
        {{ filteredEntries.findIndex(e => e.id === selectedEntryId) + 1 }}/{{ filteredEntries.length }} 字幕
      </span>
      <span v-else class="count-info">
        0/{{ filteredEntries.length }} 字幕
      </span>
    </div>
  </div>
</template>

<style scoped>
/* 左侧字幕列表 */
.subtitle-list-panel {
  width: 400px;
  background: #f8fafc;
  border-right: 1px solid #e2e8f0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  min-height: 0;
}

.subtitle-list {
  flex: 1;
  overflow-y: auto;
  padding: 0.5rem;
  position: relative;
  min-height: 0;
}

.virtual-list-wrapper {
  position: relative;
  box-sizing: border-box;
}

/* 虚拟滚动空状态需要绝对定位 */
.empty-state-absolute {
  position: absolute;
  top: 50%;
  left: 50%;
  transform: translate(-50%, -50%);
  width: 100%;
}

.subtitle-item {
  padding: 0.75rem;
  margin-bottom: 0.375rem;
  background: #ffffff;
  border: 1px solid #e2e8f0;
  border-radius: 10px;
  cursor: pointer;
  transition: all 0.2s ease;
  user-select: none;
  -webkit-user-select: none;
  height: 70px;
  box-sizing: border-box;
  overflow: hidden;
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

.item-header-left {
  display: flex;
  align-items: center;
  gap: 0.375rem;
}

/* 需要校正标记 */
.correction-mark {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  color: #f59e0b;
  font-size: 0.875rem;
  cursor: pointer;
  transition: all 0.2s;
}

.correction-mark:hover {
  color: #d97706;
  transform: scale(1.1);
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
  user-select: none;
  -webkit-user-select: none;
}

.count-info {
  color: #94a3b8;
  white-space: nowrap;
  flex-shrink: 0;
  font-weight: 500;
  user-select: none;
  -webkit-user-select: none;
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

/* 搜索高亮样式 */
:deep(mark) {
  background-color: #ffd700;
  color: #333;
  padding: 0.1rem 0.2rem;
  border-radius: 0.2rem;
  font-weight: 500;
  box-shadow: 0 0 0 1px rgba(255, 215, 0, 0.3);
}
</style>

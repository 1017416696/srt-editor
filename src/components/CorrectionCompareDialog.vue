<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import type { CorrectionEntry, CorrectionEntryWithChoice, CorrectionChoice } from '@/types/correction'
import type { TimeStamp } from '@/types/subtitle'

const props = defineProps<{
  visible: boolean
  entries: CorrectionEntry[]
  audioPath?: string
}>()

const emit = defineEmits<{
  (e: 'update:visible', value: boolean): void
  (e: 'confirm', entries: CorrectionEntryWithChoice[]): void
  (e: 'cancel'): void
}>()

// 带选择状态的条目列表
const entriesWithChoice = ref<CorrectionEntryWithChoice[]>([])

// 只显示有差异的条目
const showOnlyDiff = ref(true)

// 初始化选择状态
watch(() => props.entries, (newEntries) => {
  entriesWithChoice.value = newEntries.map(entry => ({
    ...entry,
    choice: entry.has_diff ? 'corrected' : 'original' as CorrectionChoice
  }))
}, { immediate: true })

// 过滤后的条目
const filteredEntries = computed(() => {
  if (showOnlyDiff.value) {
    return entriesWithChoice.value.filter(e => e.has_diff)
  }
  return entriesWithChoice.value
})

// 统计信息
const stats = computed(() => {
  const total = entriesWithChoice.value.length
  const diffCount = entriesWithChoice.value.filter(e => e.has_diff).length
  const chosenCorrected = entriesWithChoice.value.filter(e => e.choice === 'corrected' && e.has_diff).length
  return { total, diffCount, chosenCorrected }
})

// 格式化时间戳
function formatTime(ts: TimeStamp): string {
  const pad = (n: number, len = 2) => n.toString().padStart(len, '0')
  return `${pad(ts.hours)}:${pad(ts.minutes)}:${pad(ts.seconds)},${pad(ts.milliseconds, 3)}`
}

// 切换单条选择
function toggleChoice(entry: CorrectionEntryWithChoice) {
  entry.choice = entry.choice === 'original' ? 'corrected' : 'original'
}

// 全部采用原文
function useAllOriginal() {
  entriesWithChoice.value.forEach(e => {
    if (e.has_diff) e.choice = 'original'
  })
}

// 全部采用校正
function useAllCorrected() {
  entriesWithChoice.value.forEach(e => {
    if (e.has_diff) e.choice = 'corrected'
  })
}

// 确认应用
function handleConfirm() {
  emit('confirm', entriesWithChoice.value)
  emit('update:visible', false)
}

// 取消
function handleCancel() {
  emit('cancel')
  emit('update:visible', false)
}
</script>

<template>
  <div v-if="visible" class="correction-dialog-overlay" @click.self="handleCancel">
    <div class="correction-dialog">
      <!-- 头部 -->
      <div class="dialog-header">
        <h3>字幕校正对比</h3>
        <div class="header-stats">
          共 {{ stats.total }} 条，{{ stats.diffCount }} 处差异，已选择 {{ stats.chosenCorrected }} 处校正
        </div>
      </div>

      <!-- 工具栏 -->
      <div class="dialog-toolbar">
        <label class="filter-checkbox">
          <input type="checkbox" v-model="showOnlyDiff" />
          只显示有差异的条目
        </label>
        <div class="toolbar-actions">
          <button class="btn-secondary" @click="useAllOriginal">全部采用原文</button>
          <button class="btn-secondary" @click="useAllCorrected">全部采用校正</button>
        </div>
      </div>

      <!-- 对比列表 -->
      <div class="compare-list">
        <div
          v-for="entry in filteredEntries"
          :key="entry.id"
          class="compare-item"
          :class="{ 'has-diff': entry.has_diff }"
        >
          <div class="item-header">
            <span class="item-id">#{{ entry.id }}</span>
            <span class="item-time">
              {{ formatTime(entry.start_time) }} → {{ formatTime(entry.end_time) }}
            </span>
          </div>

          <div class="item-content">
            <!-- 原文 -->
            <div
              class="text-box original"
              :class="{ selected: entry.choice === 'original' }"
              @click="entry.has_diff && (entry.choice = 'original')"
            >
              <div class="text-label">原文</div>
              <div class="text-content">{{ entry.original }}</div>
            </div>

            <!-- 校正 -->
            <div
              class="text-box corrected"
              :class="{ selected: entry.choice === 'corrected' }"
              @click="entry.has_diff && (entry.choice = 'corrected')"
            >
              <div class="text-label">校正</div>
              <div class="text-content">{{ entry.corrected }}</div>
            </div>
          </div>

          <!-- 状态标签 -->
          <div class="item-status">
            <span v-if="!entry.has_diff" class="status-tag same">无差异</span>
            <span v-else-if="entry.choice === 'corrected'" class="status-tag use-corrected">
              采用校正
            </span>
            <span v-else class="status-tag use-original">采用原文</span>
          </div>
        </div>

        <div v-if="filteredEntries.length === 0" class="empty-state">
          {{ showOnlyDiff ? '没有发现差异' : '没有校正结果' }}
        </div>
      </div>

      <!-- 底部按钮 -->
      <div class="dialog-footer">
        <button class="btn-cancel" @click="handleCancel">取消</button>
        <button class="btn-confirm" @click="handleConfirm">确认应用</button>
      </div>
    </div>
  </div>
</template>


<style scoped>
.correction-dialog-overlay {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: rgba(0, 0, 0, 0.5);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
}

.correction-dialog {
  background: var(--bg-color, #fff);
  border-radius: 12px;
  width: 90%;
  max-width: 900px;
  max-height: 80vh;
  display: flex;
  flex-direction: column;
  box-shadow: 0 20px 60px rgba(0, 0, 0, 0.3);
}

.dialog-header {
  padding: 20px 24px;
  border-bottom: 1px solid var(--border-color, #e0e0e0);
}

.dialog-header h3 {
  margin: 0 0 8px 0;
  font-size: 18px;
  font-weight: 600;
}

.header-stats {
  font-size: 13px;
  color: var(--text-secondary, #666);
}

.dialog-toolbar {
  padding: 12px 24px;
  display: flex;
  justify-content: space-between;
  align-items: center;
  border-bottom: 1px solid var(--border-color, #e0e0e0);
  background: var(--bg-secondary, #f5f5f5);
}

.filter-checkbox {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 13px;
  cursor: pointer;
}

.toolbar-actions {
  display: flex;
  gap: 8px;
}

.btn-secondary {
  padding: 6px 12px;
  border: 1px solid var(--border-color, #ddd);
  background: var(--bg-color, #fff);
  border-radius: 6px;
  font-size: 13px;
  cursor: pointer;
  transition: all 0.2s;
}

.btn-secondary:hover {
  background: var(--bg-hover, #f0f0f0);
}

.compare-list {
  flex: 1;
  overflow-y: auto;
  padding: 16px 24px;
}

.compare-item {
  padding: 16px;
  margin-bottom: 12px;
  border: 1px solid var(--border-color, #e0e0e0);
  border-radius: 8px;
  background: var(--bg-color, #fff);
}

.compare-item.has-diff {
  border-color: var(--warning-color, #f0ad4e);
}

.item-header {
  display: flex;
  gap: 12px;
  margin-bottom: 12px;
  font-size: 13px;
}

.item-id {
  font-weight: 600;
  color: var(--primary-color, #007aff);
}

.item-time {
  color: var(--text-secondary, #666);
  font-family: monospace;
}

.item-content {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 12px;
}

.text-box {
  padding: 12px;
  border: 2px solid var(--border-color, #e0e0e0);
  border-radius: 6px;
  cursor: pointer;
  transition: all 0.2s;
}

.text-box:hover {
  border-color: var(--primary-color, #007aff);
}

.text-box.selected {
  border-color: var(--success-color, #28a745);
  background: rgba(40, 167, 69, 0.05);
}

.text-label {
  font-size: 11px;
  font-weight: 600;
  text-transform: uppercase;
  color: var(--text-secondary, #666);
  margin-bottom: 6px;
}

.text-content {
  font-size: 14px;
  line-height: 1.5;
  white-space: pre-wrap;
  word-break: break-word;
}

.item-status {
  margin-top: 12px;
  text-align: right;
}

.status-tag {
  display: inline-block;
  padding: 4px 10px;
  border-radius: 4px;
  font-size: 12px;
  font-weight: 500;
}

.status-tag.same {
  background: var(--bg-secondary, #f0f0f0);
  color: var(--text-secondary, #666);
}

.status-tag.use-corrected {
  background: rgba(40, 167, 69, 0.1);
  color: var(--success-color, #28a745);
}

.status-tag.use-original {
  background: rgba(0, 122, 255, 0.1);
  color: var(--primary-color, #007aff);
}

.empty-state {
  text-align: center;
  padding: 40px;
  color: var(--text-secondary, #666);
}

.dialog-footer {
  padding: 16px 24px;
  border-top: 1px solid var(--border-color, #e0e0e0);
  display: flex;
  justify-content: flex-end;
  gap: 12px;
}

.btn-cancel,
.btn-confirm {
  padding: 10px 20px;
  border-radius: 8px;
  font-size: 14px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s;
}

.btn-cancel {
  background: var(--bg-secondary, #f0f0f0);
  border: none;
  color: var(--text-color, #333);
}

.btn-cancel:hover {
  background: var(--bg-hover, #e0e0e0);
}

.btn-confirm {
  background: var(--primary-color, #007aff);
  border: none;
  color: #fff;
}

.btn-confirm:hover {
  background: var(--primary-hover, #0056b3);
}

/* 暗色主题适配 */
:root.dark .correction-dialog {
  --bg-color: #1e1e1e;
  --bg-secondary: #2d2d2d;
  --bg-hover: #3d3d3d;
  --border-color: #404040;
  --text-color: #e0e0e0;
  --text-secondary: #999;
}
</style>

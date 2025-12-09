<script setup lang="ts">
import { ref, computed, watch, nextTick } from 'vue'
import { Delete } from '@element-plus/icons-vue'
import type { SubtitleEntry, TimeStamp } from '@/types/subtitle'

const props = defineProps<{
  entry: SubtitleEntry | null
  formatTimeStamp: (ts: TimeStamp) => string
  fireredReady?: boolean
  isCorrecting?: boolean
  correctionResult?: { original: string; corrected: string; has_diff: boolean } | null
  needsCorrectionCount?: number
}>()

const emit = defineEmits<{
  (e: 'update-text', text: string): void
  (e: 'update-time', type: 'start' | 'end', value: string): void
  (e: 'adjust-time', type: 'start' | 'end', deltaMs: number): void
  (e: 'delete-entry'): void
  (e: 'remove-html'): void
  (e: 'add-cjk-spaces'): void
  (e: 'remove-punctuation'): void
  (e: 'text-focus'): void
  (e: 'text-blur'): void
  (e: 'text-input'): void
  (e: 'correct-entry'): void
  (e: 'apply-correction'): void
  (e: 'dismiss-correction'): void
  (e: 'toggle-correction-mark'): void
  (e: 'apply-suggestion'): void
  (e: 'dismiss-suggestion'): void
}>()

const editingText = ref('')
const editingStartTime = ref('')
const editingEndTime = ref('')
const textareaInputRef = ref<any>(null)

// 监听选中字幕变化，更新编辑文本和时间
watch(() => props.entry, (entry) => {
  if (entry) {
    editingText.value = entry.text
    editingStartTime.value = props.formatTimeStamp(entry.startTime)
    editingEndTime.value = props.formatTimeStamp(entry.endTime)
  }
}, { immediate: true, deep: true })

// 计算时长显示
const durationDisplay = computed(() => {
  if (!props.entry) return '00:00,000'
  
  const startMs = props.entry.startTime.hours * 3600000 +
    props.entry.startTime.minutes * 60000 +
    props.entry.startTime.seconds * 1000 +
    props.entry.startTime.milliseconds
  
  const endMs = props.entry.endTime.hours * 3600000 +
    props.entry.endTime.minutes * 60000 +
    props.entry.endTime.seconds * 1000 +
    props.entry.endTime.milliseconds
  
  const durationMs = Math.max(0, endMs - startMs)
  const seconds = Math.floor(durationMs / 1000)
  const ms = durationMs % 1000
  
  return `00:${String(seconds).padStart(2, '0')},${String(ms).padStart(3, '0')}`
})

const handleTextInput = () => {
  emit('update-text', editingText.value)
  emit('text-input')
}

const handleTimeChange = (type: 'start' | 'end') => {
  const value = type === 'start' ? editingStartTime.value : editingEndTime.value
  emit('update-time', type, value)
}

// 聚焦到文本输入框
const focusTextarea = async () => {
  await nextTick()
  if (textareaInputRef.value) {
    textareaInputRef.value.focus()
    await nextTick()
    // el-input 组件的内部 input 元素
    const inputEl = textareaInputRef.value.input as HTMLInputElement
    if (inputEl) {
      const textLength = inputEl.value.length
      inputEl.setSelectionRange(textLength, textLength)
    }
  }
}

// 计算文本差异，返回带标记的 HTML
const computeDiff = (original: string, corrected: string) => {
  // 简单的字符级别差异对比
  const result = {
    originalHtml: '',
    correctedHtml: ''
  }
  
  // 找出两个字符串的最长公共子序列
  const lcs = (a: string, b: string): string => {
    const m = a.length, n = b.length
    const dp: number[][] = Array(m + 1).fill(null).map(() => Array(n + 1).fill(0))
    
    for (let i = 1; i <= m; i++) {
      for (let j = 1; j <= n; j++) {
        if (a[i - 1] === b[j - 1]) {
          dp[i][j] = dp[i - 1][j - 1] + 1
        } else {
          dp[i][j] = Math.max(dp[i - 1][j], dp[i][j - 1])
        }
      }
    }
    
    // 回溯找出 LCS
    let i = m, j = n
    let result = ''
    while (i > 0 && j > 0) {
      if (a[i - 1] === b[j - 1]) {
        result = a[i - 1] + result
        i--; j--
      } else if (dp[i - 1][j] > dp[i][j - 1]) {
        i--
      } else {
        j--
      }
    }
    return result
  }
  
  const common = lcs(original, corrected)
  
  // 标记原文中被删除的字符
  let commonIdx = 0
  let origHtml = ''
  for (const char of original) {
    if (commonIdx < common.length && char === common[commonIdx]) {
      origHtml += char
      commonIdx++
    } else {
      origHtml += `<span class="diff-del">${char}</span>`
    }
  }
  
  // 标记校正文本中新增的字符
  commonIdx = 0
  let corrHtml = ''
  for (const char of corrected) {
    if (commonIdx < common.length && char === common[commonIdx]) {
      corrHtml += char
      commonIdx++
    } else {
      corrHtml += `<span class="diff-add">${char}</span>`
    }
  }
  
  result.originalHtml = origHtml
  result.correctedHtml = corrHtml
  return result
}

// 计算差异结果
const diffResult = computed(() => {
  if (!props.correctionResult) return null
  return computeDiff(props.correctionResult.original, props.correctionResult.corrected)
})

// 暴露给父组件
defineExpose({
  focusTextarea,
  getTextareaRef: () => textareaInputRef.value
})
</script>

<template>
  <!-- 字幕编辑区 -->
  <div v-if="entry" class="subtitle-edit-section">
    <!-- 编辑头部 -->
    <div class="edit-header">
      <div class="edit-header-left">
        <span class="edit-badge">#{{ entry.id }}</span>
        <h3 class="edit-title">编辑字幕</h3>
        <!-- 需要校正标记 -->
        <button 
          class="correction-mark-btn"
          :class="{ marked: entry.needsCorrection }"
          @click="emit('toggle-correction-mark')"
          :title="entry.needsCorrection ? '取消校正标记' : '标记为需要校正'"
        >
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"/>
            <line x1="12" y1="9" x2="12" y2="13"/>
            <line x1="12" y1="17" x2="12.01" y2="17"/>
          </svg>
          <span>{{ entry.needsCorrection ? '已标记' : '标记校正' }}</span>
        </button>
      </div>
      <button class="delete-entry-btn" @click="emit('delete-entry')" title="删除此字幕">
        <el-icon><Delete /></el-icon>
      </button>
    </div>

    <!-- 时间设置 - 紧凑单行布局 -->
    <div class="time-row">
      <!-- 开始时间 -->
      <div class="time-block">
        <span class="time-label">开始</span>
        <div class="time-control">
          <button class="time-btn-sm" @click="emit('adjust-time', 'start', -100)" title="-100ms">−</button>
          <el-input
            v-model="editingStartTime"
            class="time-input-sm"
            size="small"
            @blur="() => handleTimeChange('start')"
            @keyup.enter="() => handleTimeChange('start')"
          />
          <button class="time-btn-sm" @click="emit('adjust-time', 'start', 100)" title="+100ms">+</button>
        </div>
      </div>

      <span class="time-separator">→</span>

      <!-- 结束时间 -->
      <div class="time-block">
        <span class="time-label">结束</span>
        <div class="time-control">
          <button class="time-btn-sm" @click="emit('adjust-time', 'end', -100)" title="-100ms">−</button>
          <el-input
            v-model="editingEndTime"
            class="time-input-sm"
            size="small"
            @blur="() => handleTimeChange('end')"
            @keyup.enter="() => handleTimeChange('end')"
          />
          <button class="time-btn-sm" @click="emit('adjust-time', 'end', 100)" title="+100ms">+</button>
        </div>
      </div>

      <!-- 时长 -->
      <div class="time-block duration-block">
        <span class="time-label">时长</span>
        <span class="duration-value-sm">{{ durationDisplay }}</span>
      </div>
    </div>

    <!-- 文本编辑卡片 -->
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
          @focus="emit('text-focus')"
          @blur="emit('text-blur')"
          @input="handleTextInput"
          class="text-input-new"
        />
      </div>
    </div>

    <!-- 批量校正建议（存储在 entry 中的） -->
    <div v-if="entry?.correctionSuggestion" class="correction-result suggestion-result">
      <div class="correction-header">
        <div class="correction-header-left">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"/>
            <path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"/>
          </svg>
          <span>AI 校正建议（待确认）</span>
        </div>
      </div>
      <div class="correction-compare">
        <div class="correction-item original">
          <span class="correction-label">原</span>
          <span class="correction-text" v-html="computeDiff(entry.text, entry.correctionSuggestion).originalHtml"></span>
        </div>
        <div class="correction-item corrected">
          <span class="correction-label">新</span>
          <span class="correction-text" v-html="computeDiff(entry.text, entry.correctionSuggestion).correctedHtml"></span>
        </div>
      </div>
      <div class="correction-actions">
        <button class="correction-btn dismiss" @click="emit('dismiss-suggestion')">忽略</button>
        <button class="correction-btn apply" @click="emit('apply-suggestion')">采用</button>
      </div>
    </div>

    <!-- 单条校正结果对比（内联显示） -->
    <div v-else-if="correctionResult && correctionResult.has_diff" class="correction-result">
      <div class="correction-header">
        <div class="correction-header-left">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"/>
            <path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"/>
          </svg>
          <span>校正建议</span>
        </div>
      </div>
      <div class="correction-compare">
        <div class="correction-item original">
          <span class="correction-label">原</span>
          <span class="correction-text" v-html="diffResult?.originalHtml"></span>
        </div>
        <div class="correction-item corrected">
          <span class="correction-label">新</span>
          <span class="correction-text" v-html="diffResult?.correctedHtml"></span>
        </div>
      </div>
      <div class="correction-actions">
        <button class="correction-btn dismiss" @click="emit('dismiss-correction')">忽略</button>
        <button class="correction-btn apply" @click="emit('apply-correction')">采用</button>
      </div>
    </div>

    <!-- 校正结果无差异提示 -->
    <div v-else-if="correctionResult && !correctionResult.has_diff" class="correction-no-diff">
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <path d="M22 11.08V12a10 10 0 1 1-5.93-9.14"/>
        <polyline points="22 4 12 14.01 9 11.01"/>
      </svg>
      <span>识别结果与原文一致</span>
      <button class="dismiss-btn" @click="emit('dismiss-correction')">×</button>
    </div>

    <!-- 快捷操作 -->
    <div class="quick-actions">
      <span class="actions-label">快捷操作</span>
      <div class="actions-group">
        <button class="quick-action-btn" @click="emit('remove-html')" title="移除HTML标签">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <polyline points="4,7 4,4 20,4 20,7"/>
            <line x1="9" y1="20" x2="15" y2="20"/>
            <line x1="12" y1="4" x2="12" y2="20"/>
          </svg>
          <span>移除标签</span>
        </button>
        <button class="quick-action-btn" @click="emit('add-cjk-spaces')" title="中英文之间添加空格">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M21 6H3M21 12H3M21 18H3"/>
          </svg>
          <span>添加空格</span>
        </button>
        <button class="quick-action-btn" @click="emit('remove-punctuation')" title="删除标点符号">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <circle cx="12" cy="12" r="10"/>
            <line x1="15" y1="9" x2="9" y2="15"/>
            <line x1="9" y1="9" x2="15" y2="15"/>
          </svg>
          <span>删除标点</span>
        </button>
        <button 
          class="quick-action-btn correct-btn" 
          :class="{ loading: isCorrecting }"
          :disabled="!fireredReady || isCorrecting"
          @click="emit('correct-entry')" 
          :title="fireredReady ? '使用 FireRedASR 校正此条字幕' : '请先在设置中安装 FireRedASR'"
        >
          <svg v-if="!isCorrecting" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"/>
            <path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"/>
          </svg>
          <span v-if="isCorrecting" class="loading-spinner"></span>
          <span>{{ isCorrecting ? '校正中...' : 'AI校正' }}</span>
        </button>
      </div>
    </div>
  </div>

  <!-- 无选中状态 -->
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
</template>

<style scoped>
/* 右侧字幕编辑区 */
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
  user-select: none;
  -webkit-user-select: none;
}

.edit-title {
  font-size: 1.125rem;
  font-weight: 600;
  color: #1e293b;
  margin: 0;
  user-select: none;
  -webkit-user-select: none;
}

/* 校正标记按钮 */
.correction-mark-btn {
  display: flex;
  align-items: center;
  gap: 0.375rem;
  padding: 0.25rem 0.625rem;
  background: #f8fafc;
  border: 1px solid #e2e8f0;
  border-radius: 6px;
  cursor: pointer;
  transition: all 0.2s ease;
  font-size: 0.75rem;
  font-weight: 500;
  color: #64748b;
}

.correction-mark-btn:hover {
  background: #fef3c7;
  border-color: #fcd34d;
  color: #d97706;
}

.correction-mark-btn.marked {
  background: linear-gradient(135deg, #fef3c7 0%, #fde68a 100%);
  border-color: #f59e0b;
  color: #d97706;
}

.correction-mark-btn.marked svg {
  fill: #f59e0b;
  stroke: #d97706;
}

.correction-mark-btn svg {
  flex-shrink: 0;
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
  user-select: none;
  -webkit-user-select: none;
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
  user-select: none;
  -webkit-user-select: none;
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
  user-select: none;
  -webkit-user-select: none;
}

/* 文本编辑卡片 */
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
  user-select: none;
  -webkit-user-select: none;
}

.char-count {
  font-size: 0.75rem;
  color: #94a3b8;
  font-weight: 500;
  padding: 0.25rem 0.625rem;
  background: #ffffff;
  border-radius: 6px;
  border: 1px solid #e2e8f0;
  user-select: none;
  -webkit-user-select: none;
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

/* 快捷操作 */
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
  user-select: none;
  -webkit-user-select: none;
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
  user-select: none;
  -webkit-user-select: none;
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

.quick-action-btn.correct-btn {
  background: linear-gradient(135deg, #eff6ff 0%, #dbeafe 100%);
  border-color: #93c5fd;
  color: #2563eb;
}

.quick-action-btn.correct-btn:hover:not(:disabled) {
  background: linear-gradient(135deg, #dbeafe 0%, #bfdbfe 100%);
  border-color: #60a5fa;
}

.quick-action-btn.correct-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.quick-action-btn.correct-btn.loading {
  pointer-events: none;
}

.loading-spinner {
  width: 14px;
  height: 14px;
  border: 2px solid #93c5fd;
  border-top-color: #2563eb;
  border-radius: 50%;
  animation: spin 0.8s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

/* 校正结果样式 - 简洁现代风格 */
.correction-result {
  background: #ffffff;
  border: 1px solid #e2e8f0;
  border-radius: 10px;
  overflow: hidden;
}

.correction-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0.625rem 1rem;
  background: #f8fafc;
  border-bottom: 1px solid #e2e8f0;
}

.correction-header-left {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  font-size: 0.75rem;
  font-weight: 600;
  color: #64748b;
}

.correction-header svg {
  color: #3b82f6;
}

.correction-compare {
  display: flex;
  flex-direction: column;
}

.correction-item {
  display: flex;
  align-items: center;
  padding: 0.75rem 1rem;
  gap: 1rem;
  border-bottom: 1px solid #f1f5f9;
}

.correction-item:last-child {
  border-bottom: none;
}

.correction-item.original {
  background: #fafafa;
}

.correction-item.corrected {
  background: #f0fdf4;
}

.correction-label {
  font-size: 0.6875rem;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.5px;
  padding: 0.25rem 0.5rem;
  border-radius: 4px;
  min-width: 36px;
  text-align: center;
}

.correction-item.original .correction-label {
  background: #f1f5f9;
  color: #64748b;
}

.correction-item.corrected .correction-label {
  background: #dcfce7;
  color: #16a34a;
}

.correction-text {
  flex: 1;
  font-size: 0.875rem;
  color: #1e293b;
  line-height: 1.5;
}

/* 差异高亮 */
.correction-text :deep(.diff-del) {
  background: #fecaca;
  color: #dc2626;
  text-decoration: line-through;
  padding: 0 2px;
  border-radius: 2px;
}

.correction-text :deep(.diff-add) {
  background: #bbf7d0;
  color: #16a34a;
  font-weight: 500;
  padding: 0 2px;
  border-radius: 2px;
}

.correction-actions {
  display: flex;
  justify-content: flex-end;
  gap: 0.5rem;
  padding: 0.75rem 1rem;
  background: #f8fafc;
  border-top: 1px solid #e2e8f0;
}

.correction-btn {
  padding: 0.5rem 1rem;
  border-radius: 6px;
  font-size: 0.8125rem;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.15s;
}

.correction-btn.dismiss {
  background: #fff;
  border: 1px solid #e2e8f0;
  color: #64748b;
}

.correction-btn.dismiss:hover {
  background: #f1f5f9;
  border-color: #cbd5e1;
}

.correction-btn.apply {
  background: #3b82f6;
  border: 1px solid #2563eb;
  color: #fff;
}

.correction-btn.apply:hover {
  background: #2563eb;
}

/* 无差异提示 */
.correction-no-diff {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  padding: 0.75rem 1rem;
  background: #ffffff;
  border: 1px solid #e2e8f0;
  border-radius: 8px;
  font-size: 0.8125rem;
  color: #16a34a;
}

.correction-no-diff svg {
  flex-shrink: 0;
  color: #22c55e;
}

.correction-no-diff .dismiss-btn {
  margin-left: auto;
  width: 20px;
  height: 20px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: transparent;
  border: none;
  border-radius: 4px;
  cursor: pointer;
  color: #94a3b8;
  font-size: 1rem;
}

.correction-no-diff .dismiss-btn:hover {
  background: #f1f5f9;
  color: #64748b;
}

/* 无选中状态 */
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
</style>

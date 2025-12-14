<script setup lang="ts">
import { ref, computed, watch, nextTick } from 'vue'
import { useAudioStore } from '@/stores/audio'

interface SplitSegment {
  id: number
  startTimeMs: number
  endTimeMs: number
  text: string
}

const props = defineProps<{
  visible: boolean
  subtitleId: number
  originalText: string
  startTimeMs: number
  endTimeMs: number
  initialSplitTimeMs?: number
  waveformData?: number[]
  audioDuration?: number
}>()

const emit = defineEmits<{
  (e: 'update:visible', value: boolean): void
  (e: 'confirm', segments: SplitSegment[]): void
  (e: 'cancel'): void
}>()

const audioStore = useAudioStore()

const segmentCount = ref(2)
const segments = ref<SplitSegment[]>([])
const splitPoints = ref<number[]>([])
const waveformCanvasRef = ref<HTMLCanvasElement | null>(null)
const segmentsListRef = ref<HTMLElement | null>(null)
const segmentItemRefs = ref<(HTMLElement | null)[]>([])
const isFirstInit = ref(true)
const isPlaying = ref(false)
const playheadPosition = ref<number | null>(null)
const isNearSplitLine = ref(false) // 鼠标是否靠近分割线
const lastPlayingIndex = ref(-1) // 上一次播放的段落索引，用于避免重复滚动

watch(() => props.visible, (visible) => {
  if (visible && props.originalText) {
    isFirstInit.value = true
    segmentCount.value = 2
    segments.value = []
    splitPoints.value = []
    isPlaying.value = false
    playheadPosition.value = null
    lastPlayingIndex.value = -1
    segmentItemRefs.value = []
    nextTick(() => {
      initializeSegments()
    })
  } else if (!visible) {
    stopPlayback()
  }
})

watch(() => props.originalText, (text) => {
  if (props.visible && text && segments.value.length === 0) {
    isFirstInit.value = true
    segmentCount.value = 2
    splitPoints.value = []
    initializeSegments()
  }
})

watch(() => segmentCount.value, (newCount, oldCount) => {
  if (!props.visible || isFirstInit.value || segments.value.length === 0) return
  if (newCount > oldCount) addSegment()
  else if (newCount < oldCount) removeSegment()
  nextTick(() => renderWaveform())
})

watch(splitPoints, () => {
  // segments 为空时跳过（初始化期间）
  if (segments.value.length === 0) return
  updateSegmentsFromSplitPoints()
  nextTick(() => renderWaveform())
}, { deep: true })

// 当前播放的段落索引
const currentPlayingIndex = computed(() => {
  if (!isPlaying.value) return -1
  const currentMs = audioStore.playerState.currentTime * 1000
  return segments.value.findIndex(seg => 
    currentMs >= seg.startTimeMs && currentMs < seg.endTimeMs
  )
})

// 监听当前播放索引变化，自动滚动到对应段落
watch(currentPlayingIndex, (newIndex) => {
  if (newIndex !== -1 && newIndex !== lastPlayingIndex.value) {
    lastPlayingIndex.value = newIndex
    nextTick(() => {
      scrollToSegment(newIndex)
    })
  }
})

// 滚动到指定段落
function scrollToSegment(index: number) {
  const container = segmentsListRef.value
  const item = segmentItemRefs.value[index]
  if (!container || !item) return
  
  const containerRect = container.getBoundingClientRect()
  const itemRect = item.getBoundingClientRect()
  
  // 检查元素是否在可视区域内
  const isAbove = itemRect.top < containerRect.top
  const isBelow = itemRect.bottom > containerRect.bottom
  
  if (isAbove || isBelow) {
    // 滚动到元素位置，使其居中显示
    const scrollTop = item.offsetTop - container.offsetTop - (containerRect.height - itemRect.height) / 2
    container.scrollTo({
      top: Math.max(0, scrollTop),
      behavior: 'smooth'
    })
  }
}

watch(() => audioStore.playerState.currentTime, (currentTime) => {
  if (!isPlaying.value || !waveformCanvasRef.value) return
  const currentMs = currentTime * 1000
  if (currentMs < props.startTimeMs || currentMs > props.endTimeMs) {
    stopPlayback()
    return
  }
  const canvas = waveformCanvasRef.value
  const rect = canvas.getBoundingClientRect()
  const subtitleDuration = props.endTimeMs - props.startTimeMs
  playheadPosition.value = ((currentMs - props.startTimeMs) / subtitleDuration) * rect.width
})

function initializeSegments() {
  const totalDuration = props.endTimeMs - props.startTimeMs
  const text = props.originalText
  if (!text || totalDuration <= 0) return
  
  const newSegments: SplitSegment[] = []
  const count = segmentCount.value
  
  if (count === 2 && props.initialSplitTimeMs) {
    // 有初始分割点时，使用该点分割
    const splitTime = props.initialSplitTimeMs
    const ratio = (splitTime - props.startTimeMs) / totalDuration
    const textSplitIndex = Math.round(text.length * ratio)
    splitPoints.value = [splitTime]
    newSegments.push({ id: 1, startTimeMs: props.startTimeMs, endTimeMs: splitTime, text: text.substring(0, textSplitIndex) })
    newSegments.push({ id: 2, startTimeMs: splitTime, endTimeMs: props.endTimeMs, text: text.substring(textSplitIndex) })
  } else {
    // 均匀分割
    const segmentDuration = totalDuration / count
    const charsPerSegment = Math.ceil(text.length / count)
    const points: number[] = []
    
    // 创建分割点
    for (let i = 1; i < count; i++) {
      points.push(Math.round(props.startTimeMs + i * segmentDuration))
    }
    splitPoints.value = points
    
    // 创建段落
    for (let i = 0; i < count; i++) {
      const segStartTime = i === 0 ? props.startTimeMs : points[i - 1]!
      const segEndTime = i === count - 1 ? props.endTimeMs : points[i]!
      const textStart = i * charsPerSegment
      const textEnd = i === count - 1 ? text.length : (i + 1) * charsPerSegment
      newSegments.push({ 
        id: i + 1, 
        startTimeMs: segStartTime, 
        endTimeMs: segEndTime, 
        text: text.substring(textStart, textEnd) 
      })
    }
  }
  
  segments.value = newSegments
  isFirstInit.value = false
  nextTick(() => renderWaveform())
}

function updateSegmentsFromSplitPoints() {
  if (segments.value.length === 0) return
  const sortedPoints = [...splitPoints.value].sort((a, b) => a - b)
  segments.value.forEach((seg, i) => {
    seg.startTimeMs = i === 0 ? props.startTimeMs : sortedPoints[i - 1]!
    seg.endTimeMs = i === segments.value.length - 1 ? props.endTimeMs : sortedPoints[i]!
  })
}

function addSegment() {
  const lastSegment = segments.value[segments.value.length - 1]
  if (!lastSegment) return
  
  const midTime = Math.round((lastSegment.startTimeMs + lastSegment.endTimeMs) / 2)
  const duration = lastSegment.endTimeMs - lastSegment.startTimeMs
  const ratio = (midTime - lastSegment.startTimeMs) / duration
  const textSplitIndex = Math.round(lastSegment.text.length * ratio)
  
  // 保存最后一段的后半部分文本
  const secondPartText = lastSegment.text.substring(textSplitIndex)
  
  // 更新最后一段
  lastSegment.endTimeMs = midTime
  lastSegment.text = lastSegment.text.substring(0, textSplitIndex)
  
  // 添加新分割点
  splitPoints.value.push(midTime)
  splitPoints.value.sort((a, b) => a - b)
  
  // 添加新段，使用从最后一段分割出的后半部分文本
  segments.value.push({ 
    id: segments.value.length + 1, 
    startTimeMs: midTime, 
    endTimeMs: props.endTimeMs, 
    text: secondPartText 
  })
  
  segments.value.forEach((seg, i) => { seg.id = i + 1 })
}

function removeSegment() {
  if (segments.value.length < 2) return
  const lastSegment = segments.value.pop()
  const secondLastSegment = segments.value[segments.value.length - 1]
  if (lastSegment && secondLastSegment) {
    secondLastSegment.text = secondLastSegment.text + lastSegment.text
    secondLastSegment.endTimeMs = lastSegment.endTimeMs
  }
  splitPoints.value.pop()
  segments.value.forEach((seg, i) => { seg.id = i + 1 })
}

// 在指定位置分割某个段落（通过回车键触发）
function splitSegmentAtCursor(segmentIndex: number, cursorPosition: number) {
  if (segmentCount.value >= 5) return // 最多5段
  
  const segment = segments.value[segmentIndex]
  if (!segment || cursorPosition <= 0 || cursorPosition >= segment.text.length) return
  
  // 计算分割点时间（按文本位置比例）
  const textRatio = cursorPosition / segment.text.length
  const segmentDuration = segment.endTimeMs - segment.startTimeMs
  const splitTimeMs = Math.round(segment.startTimeMs + segmentDuration * textRatio)
  
  // 分割文本
  const firstPartText = segment.text.substring(0, cursorPosition)
  const secondPartText = segment.text.substring(cursorPosition)
  
  // 更新当前段
  const originalEndTime = segment.endTimeMs
  segment.endTimeMs = splitTimeMs
  segment.text = firstPartText
  
  // 在当前段后面插入新段
  const newSegment: SplitSegment = {
    id: segmentIndex + 2,
    startTimeMs: splitTimeMs,
    endTimeMs: originalEndTime,
    text: secondPartText
  }
  segments.value.splice(segmentIndex + 1, 0, newSegment)
  
  // 添加新分割点
  splitPoints.value.push(splitTimeMs)
  splitPoints.value.sort((a, b) => a - b)
  
  // 重新编号
  segments.value.forEach((seg, i) => { seg.id = i + 1 })
  
  // 更新段数
  segmentCount.value = segments.value.length
  
  // 重新渲染波形
  nextTick(() => renderWaveform())
}

// 处理文本框按键事件
function handleTextKeyDown(e: KeyboardEvent, index: number) {
  if (e.key === 'Enter') {
    e.preventDefault()
    const textarea = e.target as HTMLTextAreaElement
    const cursorPosition = textarea.selectionStart
    splitSegmentAtCursor(index, cursorPosition)
  }
}

function renderWaveform() {
  const canvas = waveformCanvasRef.value
  if (!canvas || !props.waveformData || !props.audioDuration) return
  const ctx = canvas.getContext('2d')
  if (!ctx) return
  
  const dpr = window.devicePixelRatio || 1
  const rect = canvas.getBoundingClientRect()
  canvas.width = rect.width * dpr
  canvas.height = rect.height * dpr
  ctx.scale(dpr, dpr)
  const width = rect.width, height = rect.height
  
  const totalDurationMs = props.audioDuration * 1000
  const startRatio = props.startTimeMs / totalDurationMs
  const endRatio = props.endTimeMs / totalDurationMs
  const dataLength = props.waveformData.length / 2
  const startIndex = Math.floor(startRatio * dataLength)
  const endIndex = Math.ceil(endRatio * dataLength)
  const segmentLength = endIndex - startIndex
  if (segmentLength <= 0) return
  
  const amplitudes: number[] = []
  for (let i = startIndex; i < endIndex; i++) amplitudes.push(Math.abs(props.waveformData[i * 2 + 1] ?? 0))
  
  const smoothedData: number[] = []
  for (let i = 0; i < amplitudes.length; i++) {
    let sum = 0, count = 0
    for (let j = Math.max(0, i - 3); j <= Math.min(amplitudes.length - 1, i + 3); j++) { sum += amplitudes[j] ?? 0; count++ }
    smoothedData.push(sum / count)
  }
  
  const bgGradient = ctx.createLinearGradient(0, 0, 0, height)
  bgGradient.addColorStop(0, 'rgba(59, 130, 246, 0.08)')
  bgGradient.addColorStop(1, 'rgba(59, 130, 246, 0.15)')
  ctx.fillStyle = bgGradient
  ctx.fillRect(0, 0, width, height)
  
  const maxWaveHeight = height - 8, baseY = height - 4
  const pointsPerPixel = smoothedData.length / width
  const pixelAmplitudes: number[] = []
  for (let x = 0; x < width; x++) {
    const startIdx = Math.floor(x * pointsPerPixel)
    const endIdx = Math.min(Math.ceil((x + 1) * pointsPerPixel), smoothedData.length)
    let amp = 0
    for (let j = startIdx; j < endIdx; j++) if ((smoothedData[j] ?? 0) > amp) amp = smoothedData[j]!
    pixelAmplitudes.push(amp)
  }
  
  ctx.beginPath()
  ctx.moveTo(0, baseY)
  for (let i = 0; i < pixelAmplitudes.length; i++) ctx.lineTo(i, baseY - pixelAmplitudes[i]! * maxWaveHeight)
  ctx.lineTo(width - 1, baseY)
  ctx.closePath()
  
  const waveGradient = ctx.createLinearGradient(0, 0, 0, height)
  waveGradient.addColorStop(0, 'rgba(59, 130, 246, 0.9)')
  waveGradient.addColorStop(0.5, 'rgba(96, 165, 250, 0.7)')
  waveGradient.addColorStop(1, 'rgba(147, 197, 253, 0.5)')
  ctx.fillStyle = waveGradient
  ctx.fill()
  
  const subtitleDuration = props.endTimeMs - props.startTimeMs
  ctx.strokeStyle = '#ef4444'
  ctx.lineWidth = 2
  splitPoints.value.forEach(point => {
    const x = ((point - props.startTimeMs) / subtitleDuration) * width
    ctx.beginPath()
    ctx.setLineDash([5, 3])
    ctx.moveTo(x, 0)
    ctx.lineTo(x, height)
    ctx.stroke()
  })
  ctx.setLineDash([])
}

// 检测鼠标是否靠近分割线
function handleWaveformMouseMove(e: MouseEvent) {
  const canvas = waveformCanvasRef.value
  if (!canvas || splitPoints.value.length === 0) {
    isNearSplitLine.value = false
    return
  }
  
  const rect = canvas.getBoundingClientRect()
  const x = e.clientX - rect.left
  const ratio = x / rect.width
  const subtitleDuration = props.endTimeMs - props.startTimeMs
  const mouseTimeMs = props.startTimeMs + ratio * subtitleDuration
  
  const threshold = (12 / rect.width) * subtitleDuration
  const nearIndex = splitPoints.value.findIndex(p => Math.abs(p - mouseTimeMs) < threshold)
  isNearSplitLine.value = nearIndex !== -1
}

function handleWaveformMouseLeave() {
  isNearSplitLine.value = false
}

function handleWaveformMouseDown(e: MouseEvent) {
  const canvas = waveformCanvasRef.value
  if (!canvas) return
  const rect = canvas.getBoundingClientRect()
  const x = e.clientX - rect.left
  const ratio = x / rect.width
  const subtitleDuration = props.endTimeMs - props.startTimeMs
  const clickTimeMs = props.startTimeMs + ratio * subtitleDuration
  
  const threshold = (15 / rect.width) * subtitleDuration
  const dragIndex = splitPoints.value.findIndex(p => Math.abs(p - clickTimeMs) < threshold)
  
  if (dragIndex !== -1) {
    // 拖动分割线
    const onMove = (moveE: MouseEvent) => {
      const moveX = moveE.clientX - rect.left
      const moveRatio = Math.max(0, Math.min(1, moveX / rect.width))
      let newTimeMs = Math.round(props.startTimeMs + moveRatio * subtitleDuration)
      const minTime = dragIndex === 0 ? props.startTimeMs + 100 : splitPoints.value[dragIndex - 1]! + 100
      const maxTime = dragIndex === splitPoints.value.length - 1 ? props.endTimeMs - 100 : splitPoints.value[dragIndex + 1]! - 100
      newTimeMs = Math.max(minTime, Math.min(maxTime, newTimeMs))
      splitPoints.value[dragIndex] = newTimeMs
    }
    const onUp = () => {
      document.removeEventListener('mousemove', onMove)
      document.removeEventListener('mouseup', onUp)
      // 只更新时间，不重新分配文本（保留用户编辑的内容）
      updateSegmentsFromSplitPoints()
    }
    document.addEventListener('mousemove', onMove)
    document.addEventListener('mouseup', onUp)
    e.preventDefault()
  } else {
    // 点击播放
    isPlaying.value = true
    playheadPosition.value = x
    audioStore.seek(clickTimeMs / 1000)
    audioStore.play()
  }
}

function redistributeText() {
  const totalText = props.originalText
  const totalDuration = props.endTimeMs - props.startTimeMs
  const sortedPoints = [props.startTimeMs, ...splitPoints.value.sort((a, b) => a - b), props.endTimeMs]
  let textIndex = 0
  segments.value.forEach((seg, i) => {
    const segDuration = sortedPoints[i + 1]! - sortedPoints[i]!
    const ratio = segDuration / totalDuration
    const charCount = Math.round(totalText.length * ratio)
    const endIndex = i === segments.value.length - 1 ? totalText.length : textIndex + charCount
    seg.text = totalText.substring(textIndex, endIndex)
    seg.startTimeMs = sortedPoints[i]!
    seg.endTimeMs = sortedPoints[i + 1]!
    textIndex = endIndex
  })
}

function stopPlayback() {
  isPlaying.value = false
  playheadPosition.value = null
  audioStore.pause()
}

function formatTime(ms: number): string {
  const totalSeconds = Math.floor(ms / 1000)
  const minutes = Math.floor(totalSeconds / 60)
  const seconds = totalSeconds % 60
  const milliseconds = ms % 1000
  return `${minutes.toString().padStart(2, '0')}:${seconds.toString().padStart(2, '0')},${milliseconds.toString().padStart(3, '0')}`
}

function getDuration(startMs: number, endMs: number): string {
  return `${((endMs - startMs) / 1000).toFixed(1)}s`
}

function updateSegmentText(index: number, text: string) {
  if (segments.value[index]) segments.value[index].text = text
}

const isValid = computed(() => !segments.value.some(s => s.text.trim() === '') && segments.value.every(s => s.endTimeMs > s.startTimeMs))

function handleConfirm() {
  if (!isValid.value) return
  stopPlayback()
  emit('confirm', segments.value)
  emit('update:visible', false)
}

function handleCancel() {
  stopPlayback()
  emit('cancel')
  emit('update:visible', false)
}

function increaseSegments() { if (segmentCount.value < 5) segmentCount.value++ }
function decreaseSegments() { if (segmentCount.value > 2) segmentCount.value-- }
</script>

<template>
  <div v-if="visible" class="split-dialog-overlay" @click.self="handleCancel">
    <div class="split-dialog">
      <!-- 头部 -->
      <div class="dialog-header">
        <h3>分割字幕 #{{ subtitleId }}</h3>
        <span class="header-time">{{ formatTime(startTimeMs) }} → {{ formatTime(endTimeMs) }}</span>
        <button class="close-btn" @click="handleCancel">×</button>
      </div>

      <!-- 分割设置 -->
      <div class="split-settings">
        <span>分割为</span>
        <div class="segment-counter">
          <button @click="decreaseSegments" :disabled="segmentCount <= 2">−</button>
          <span>{{ segmentCount }}</span>
          <button @click="increaseSegments" :disabled="segmentCount >= 5">+</button>
        </div>
        <span>段</span>
      </div>

      <!-- 波形区域 -->
      <div class="waveform-section" v-if="waveformData && waveformData.length > 0">
        <div class="waveform-container">
          <canvas 
            ref="waveformCanvasRef" 
            class="waveform-canvas" 
            :class="{ 'near-split-line': isNearSplitLine }"
            @mousedown="handleWaveformMouseDown"
            @mousemove="handleWaveformMouseMove"
            @mouseleave="handleWaveformMouseLeave"
          />
          <div v-if="playheadPosition !== null" class="playhead" :style="{ left: playheadPosition + 'px' }">
            <div class="playhead-handle"></div>
            <div class="playhead-line"></div>
          </div>
        </div>
        <div class="waveform-footer">
          <span>{{ formatTime(startTimeMs) }}</span>
          <span class="hint">点击播放 · 拖动红线调整</span>
          <span>{{ formatTime(endTimeMs) }}</span>
        </div>
      </div>

      <!-- 分割预览 - 纵向排列 -->
      <div class="segments-list" ref="segmentsListRef">
        <div 
          v-for="(segment, index) in segments" 
          :key="segment.id" 
          :ref="(el) => segmentItemRefs[index] = el as HTMLElement"
          class="segment-item" 
          :class="{ 'is-playing': currentPlayingIndex === index }"
        >
          <div class="segment-header">
            <span class="segment-id">#{{ subtitleId + index }}</span>
            <span class="segment-time">{{ formatTime(segment.startTimeMs) }} → {{ formatTime(segment.endTimeMs) }}</span>
            <span class="segment-duration">{{ getDuration(segment.startTimeMs, segment.endTimeMs) }}</span>
          </div>
          <textarea
            class="segment-text"
            :value="segment.text"
            @input="updateSegmentText(index, ($event.target as HTMLTextAreaElement).value)"
            @keydown="handleTextKeyDown($event, index)"
            rows="1"
            placeholder="输入文本... (按回车分割)"
          />
        </div>
      </div>

      <!-- 底部按钮 -->
      <div class="dialog-footer">
        <button class="btn-cancel" @click="handleCancel">取消</button>
        <button class="btn-confirm" @click="handleConfirm" :disabled="!isValid">确认分割</button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.split-dialog-overlay {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.5);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
}

.split-dialog {
  background: #fff;
  border-radius: 12px;
  width: 90%;
  max-width: 640px;
  box-shadow: 0 20px 60px rgba(0, 0, 0, 0.3);
  overflow: hidden;
}

.dialog-header {
  padding: 14px 20px;
  border-bottom: 1px solid #eee;
  display: flex;
  align-items: center;
  gap: 12px;
}

.dialog-header h3 {
  margin: 0;
  font-size: 15px;
  font-weight: 600;
  color: #1a1a1a;
}

.header-time {
  font-size: 12px;
  color: #666;
  font-family: monospace;
}

.close-btn {
  margin-left: auto;
  width: 28px;
  height: 28px;
  border: none;
  background: transparent;
  font-size: 20px;
  cursor: pointer;
  color: #999;
  border-radius: 6px;
  display: flex;
  align-items: center;
  justify-content: center;
}

.close-btn:hover {
  background: #f5f5f5;
  color: #333;
}

.split-settings {
  padding: 12px 20px;
  display: flex;
  align-items: center;
  gap: 10px;
  font-size: 14px;
  color: #333;
  border-bottom: 1px solid #eee;
}

.segment-counter {
  display: flex;
  align-items: center;
  gap: 2px;
  background: #f5f5f5;
  border-radius: 8px;
  padding: 3px;
}

.segment-counter button {
  width: 28px;
  height: 28px;
  border: none;
  background: #fff;
  border-radius: 6px;
  font-size: 16px;
  font-weight: 600;
  cursor: pointer;
  color: #333;
  transition: all 0.15s;
}

.segment-counter button:hover:not(:disabled) {
  background: #007aff;
  color: #fff;
}

.segment-counter button:disabled {
  opacity: 0.3;
  cursor: not-allowed;
}

.segment-counter > span {
  width: 28px;
  text-align: center;
  font-weight: 600;
  color: #007aff;
  font-size: 15px;
}

.waveform-section {
  padding: 12px 20px;
  border-bottom: 1px solid #eee;
}

.waveform-container {
  position: relative;
}

.waveform-canvas {
  width: 100%;
  height: 70px;
  border-radius: 8px;
  cursor: pointer;
}

.waveform-canvas.near-split-line {
  cursor: ew-resize;
}

.playhead {
  position: absolute;
  top: 0;
  bottom: 0;
  width: 2px;
  pointer-events: none;
  z-index: 10;
}

.playhead-line {
  width: 2px;
  height: 100%;
  background: #ef4444;
}

.playhead-handle {
  position: absolute;
  top: -4px;
  left: -5px;
  width: 12px;
  height: 12px;
  background: #ef4444;
  border-radius: 50%;
  border: 2px solid #ffffff;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.2);
}

.waveform-footer {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-top: 6px;
  font-size: 11px;
  color: #999;
  font-family: monospace;
}

.hint {
  font-family: system-ui, sans-serif;
  color: #888;
}

.segments-list {
  padding: 12px 20px;
  display: flex;
  flex-direction: column;
  gap: 10px;
  max-height: 240px;
  overflow-y: auto;
}

.segment-item {
  background: #fafafa;
  border: 1px solid #eee;
  border-radius: 8px;
  padding: 10px 12px;
  transition: all 0.15s;
}

.segment-item.is-playing {
  background: linear-gradient(135deg, #f0f9ff 0%, #e0f2fe 100%);
  border-color: #7dd3fc;
  box-shadow: inset 0 0 0 1px rgba(56, 189, 248, 0.1);
}

.segment-item.is-playing .segment-id {
  color: #0284c7;
  background: linear-gradient(135deg, #e0f2fe 0%, #bae6fd 100%);
}

.segment-item.is-playing .segment-time {
  color: #0369a1;
}

.segment-item.is-playing .segment-text {
  border-color: #7dd3fc;
  background: #fff;
  box-shadow: 0 0 0 2px rgba(56, 189, 248, 0.15);
}

.segment-header {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-bottom: 8px;
  font-size: 12px;
}

.segment-id {
  font-weight: 600;
  color: #007aff;
}

.segment-time {
  color: #666;
  font-family: monospace;
}

.segment-duration {
  margin-left: auto;
  color: #999;
  background: #f0f0f0;
  padding: 2px 6px;
  border-radius: 4px;
  font-size: 11px;
}

.segment-text {
  width: 100%;
  padding: 8px 10px;
  border: 1px solid #ddd;
  border-radius: 6px;
  font-size: 14px;
  line-height: 1.5;
  resize: none;
  background: #fff;
  color: #1a1a1a;
  transition: border-color 0.15s;
}

.segment-text:focus {
  outline: none;
  border-color: #007aff;
  box-shadow: 0 0 0 3px rgba(0, 122, 255, 0.1);
}

.segment-text::placeholder {
  color: #bbb;
}

.dialog-footer {
  padding: 14px 20px;
  border-top: 1px solid #eee;
  display: flex;
  justify-content: flex-end;
  gap: 10px;
}

.btn-cancel, .btn-confirm {
  padding: 9px 18px;
  border-radius: 8px;
  font-size: 14px;
  font-weight: 500;
  cursor: pointer;
  border: none;
  transition: all 0.15s;
}

.btn-cancel {
  background: #f5f5f5;
  color: #333;
}

.btn-cancel:hover {
  background: #eee;
}

.btn-confirm {
  background: #007aff;
  color: #fff;
}

.btn-confirm:hover:not(:disabled) {
  background: #0066dd;
}

.btn-confirm:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

/* 暗色主题 */
:root.dark .split-dialog {
  background: #1e1e1e;
}

:root.dark .dialog-header {
  border-color: #333;
}

:root.dark .dialog-header h3 {
  color: #e0e0e0;
}

:root.dark .header-time {
  color: #888;
}

:root.dark .close-btn {
  color: #888;
}

:root.dark .close-btn:hover {
  background: #333;
  color: #e0e0e0;
}

:root.dark .split-settings {
  border-color: #333;
  color: #e0e0e0;
}

:root.dark .segment-counter {
  background: #2a2a2a;
}

:root.dark .segment-counter button {
  background: #333;
  color: #e0e0e0;
}

:root.dark .waveform-section {
  border-color: #333;
}

:root.dark .segments-list {
  border-color: #333;
}

:root.dark .segment-item {
  background: #252525;
  border-color: #333;
}

:root.dark .segment-item.is-playing {
  background: linear-gradient(135deg, #1e3a5f 0%, #1e3a4f 100%);
  border-color: #3b82f6;
}

:root.dark .segment-item.is-playing .segment-id {
  color: #60a5fa;
  background: linear-gradient(135deg, #1e3a5f 0%, #2563eb33 100%);
}

:root.dark .segment-item.is-playing .segment-time {
  color: #93c5fd;
}

:root.dark .segment-item.is-playing .segment-text {
  border-color: #3b82f6;
  background: #1a1a2e;
  box-shadow: 0 0 0 2px rgba(59, 130, 246, 0.2);
}

:root.dark .segment-time {
  color: #888;
}

:root.dark .segment-duration {
  background: #333;
  color: #888;
}

:root.dark .segment-text {
  background: #1e1e1e;
  border-color: #444;
  color: #e0e0e0;
}

:root.dark .dialog-footer {
  border-color: #333;
}

:root.dark .btn-cancel {
  background: #333;
  color: #e0e0e0;
}

:root.dark .btn-cancel:hover {
  background: #444;
}
</style>

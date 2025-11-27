<template>
  <div class="timeline-editor">
    <!-- 时间轴主区域 -->
    <div class="timeline-container" ref="timelineContainerRef">
      <!-- 波形和字幕轨道 -->
      <div class="timeline-track-area" ref="trackAreaRef" @scroll="handleScroll" @wheel="handleWheel">
        <div class="timeline-content" :style="{ width: timelineWidth + 'px' }" @click="handleTimelineClick">
          <!-- 时间刻度尺 -->
          <div class="time-ruler" :style="{ width: timelineWidth + 'px' }">
            <div
              v-for="marker in timeMarkers"
              :key="marker.time"
              class="time-marker"
              :style="{ left: timeToPixel(marker.time) + 'px' }"
            >
              <span class="time-label">{{ formatTime(marker.time) }}</span>
            </div>
          </div>

          <!-- 波形图 -->
          <div class="waveform-layer" ref="waveformRef">
            <div v-if="loading" class="loading-overlay">
              <el-icon class="is-loading"><Loading /></el-icon>
              <span>生成波形中...</span>
            </div>
          </div>

          <!-- 字幕轨道 -->
          <div class="subtitle-track">
            <div
              v-for="subtitle in subtitles"
              :key="subtitle.id"
              class="subtitle-block"
              :class="{
                'is-dragging': draggingSubtitle?.id === subtitle.id,
                'is-active': currentSubtitleId === subtitle.id
              }"
              :style="getSubtitleStyle(subtitle)"
              @mousedown="handleSubtitleMouseDown($event, subtitle)"
            >
              <!-- 左调整手柄 -->
              <div
                class="resize-handle left"
                @mousedown.stop="handleResizeStart($event, subtitle, 'left')"
              ></div>

              <!-- 字幕内容 -->
              <div class="subtitle-content">
                <span class="subtitle-label">#{{ subtitle.id }}</span>
                <span class="subtitle-text">{{ truncateText(subtitle.text, 40) }}</span>
              </div>

              <!-- 右调整手柄 -->
              <div
                class="resize-handle right"
                @mousedown.stop="handleResizeStart($event, subtitle, 'right')"
              ></div>
            </div>
          </div>

          <!-- 播放指针 -->
          <div
            class="playhead"
            :style="{ left: timeToPixel(currentTime) + 'px' }"
          >
            <div class="playhead-line"></div>
            <div class="playhead-handle"></div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, watch, nextTick } from 'vue'
import { Loading } from '@element-plus/icons-vue'
import WaveSurfer from 'wavesurfer.js'
import type { SubtitleEntry, TimeStamp } from '@/types/subtitle'

interface Props {
  waveformData?: number[]
  currentTime?: number
  duration?: number
  subtitles?: SubtitleEntry[]
}

const props = withDefaults(defineProps<Props>(), {
  currentTime: 0,
  duration: 0,
  subtitles: () => []
})

const emit = defineEmits<{
  seek: [time: number]
  updateSubtitle: [id: number, startTime: TimeStamp, endTime: TimeStamp]
}>()

// Refs
const timelineContainerRef = ref<HTMLDivElement | null>(null)
const trackAreaRef = ref<HTMLDivElement | null>(null)
const waveformRef = ref<HTMLDivElement | null>(null)
const wavesurfer = ref<WaveSurfer | null>(null)
const loading = ref(false)

// Timeline state
const zoomLevel = ref(1) // 缩放级别：1 = 1秒占用 100px
const pixelsPerSecond = computed(() => 100 * zoomLevel.value)
const timelineWidth = computed(() => props.duration * pixelsPerSecond.value)

// Dragging state
const draggingSubtitle = ref<SubtitleEntry | null>(null)
const resizingSubtitle = ref<{ subtitle: SubtitleEntry; side: 'left' | 'right' } | null>(null)
const dragStartX = ref(0)
const dragStartTime = ref(0)
const currentSubtitleId = ref<number | null>(null)

// Helper: Time to pixel position
const timeToPixel = (time: number): number => {
  return time * pixelsPerSecond.value
}

// Helper: Pixel to time
const pixelToTime = (pixel: number): number => {
  return pixel / pixelsPerSecond.value
}

// Helper: Convert TimeStamp to seconds
const timestampToSeconds = (ts: TimeStamp): number => {
  return ts.hours * 3600 + ts.minutes * 60 + ts.seconds + ts.milliseconds / 1000
}

// Helper: Convert seconds to TimeStamp
const secondsToTimestamp = (seconds: number): TimeStamp => {
  const hours = Math.floor(seconds / 3600)
  const minutes = Math.floor((seconds % 3600) / 60)
  const secs = Math.floor(seconds % 60)
  const milliseconds = Math.floor((seconds % 1) * 1000)
  return { hours, minutes, seconds: secs, milliseconds }
}

// Helper: Truncate text
const truncateText = (text: string, maxLength: number): string => {
  if (text.length <= maxLength) return text
  return text.substring(0, maxLength) + '...'
}

// Helper: Format time for display
const formatTime = (seconds: number): string => {
  const mins = Math.floor(seconds / 60)
  const secs = Math.floor(seconds % 60)
  return `${String(mins).padStart(2, '0')}:${String(secs).padStart(2, '0')}`
}

// Generate time markers for ruler
const timeMarkers = computed(() => {
  const markers = []
  const interval = zoomLevel.value >= 2 ? 1 : zoomLevel.value >= 1 ? 5 : 10 // 根据缩放级别调整间隔
  for (let i = 0; i <= props.duration; i += interval) {
    markers.push({ time: i })
  }
  return markers
})

// Get subtitle style
const getSubtitleStyle = (subtitle: SubtitleEntry) => {
  const start = timestampToSeconds(subtitle.startTime)
  const end = timestampToSeconds(subtitle.endTime)
  const left = timeToPixel(start)
  const width = timeToPixel(end - start)

  // 生成颜色
  const hue = (subtitle.id * 137.5) % 360
  const color = `hsl(${hue}, 70%, 65%)`

  return {
    left: left + 'px',
    width: Math.max(width, 50) + 'px', // 最小宽度 50px
    backgroundColor: color
  }
}

// Zoom controls - zoom centered on playhead
const zoomIn = () => {
  zoomLevel.value = Math.min(zoomLevel.value * 1.5, 10)
  nextTick(() => {
    // 以播放指针为中心进行缩放
    if (trackAreaRef.value) {
      const newPlayheadPixel = timeToPixel(props.currentTime)
      const containerWidth = trackAreaRef.value.clientWidth
      trackAreaRef.value.scrollLeft = newPlayheadPixel - containerWidth / 2
    }
  })
}

const zoomOut = () => {
  zoomLevel.value = Math.max(zoomLevel.value / 1.5, 0.1)
  nextTick(() => {
    // 以播放指针为中心进行缩放
    if (trackAreaRef.value) {
      const newPlayheadPixel = timeToPixel(props.currentTime)
      const containerWidth = trackAreaRef.value.clientWidth
      trackAreaRef.value.scrollLeft = newPlayheadPixel - containerWidth / 2
    }
  })
}

// Scroll to specific time
const scrollToTime = (time: number) => {
  if (!trackAreaRef.value) return
  const pixel = timeToPixel(time)
  const containerWidth = trackAreaRef.value.clientWidth
  trackAreaRef.value.scrollLeft = pixel - containerWidth / 2
}

// Handle scroll
const handleScroll = () => {
  // 可以在这里添加滚动时的逻辑
}

// Handle timeline click to seek
const handleTimelineClick = (event: MouseEvent) => {
  // 忽略对字幕块的点击
  if ((event.target as HTMLElement).closest('.subtitle-block')) {
    return
  }

  if (!trackAreaRef.value) return

  // 获取点击相对于 timeline-content 的位置
  const timelineContent = trackAreaRef.value.querySelector('.timeline-content') as HTMLElement
  if (!timelineContent) return

  // 获取点击点相对于 timeline-content 的像素位置
  const rect = timelineContent.getBoundingClientRect()
  const trackRect = trackAreaRef.value.getBoundingClientRect()
  const clickX = event.clientX - trackRect.left + trackAreaRef.value.scrollLeft

  // 转换像素为时间
  const time = pixelToTime(clickX)

  // 确保时间在有效范围内
  const clampedTime = Math.max(0, Math.min(time, props.duration))

  // 发送 seek 事件
  emit('seek', clampedTime)
}

// Handle wheel zoom (mouse wheel or trackpad)
const handleWheel = (event: WheelEvent) => {
  // 检查是否是垂直滚动（缩放）或水平滚动（导航）
  // 如果 deltaY 的绝对值大于 deltaX，认为是垂直滚动（缩放）
  const isVerticalScroll = Math.abs(event.deltaY) > Math.abs(event.deltaX)

  // 如果是水平滚动，允许默认行为
  if (!isVerticalScroll) {
    return
  }

  event.preventDefault()

  // deltaY > 0 表示向下滚动（缩小），deltaY < 0 表示向上滚动（放大）
  // 对于触控板，deltaY 可能很大，所以需要归一化
  const isZoomingIn = event.deltaY < 0

  // 根据 deltaY 的大小调整缩放因子
  let zoomFactor = 1.0
  const absDelta = Math.abs(event.deltaY)
  if (absDelta > 0) {
    if (isZoomingIn) {
      zoomFactor = 1 + (Math.min(absDelta, 100) / 100) * 0.2
    } else {
      zoomFactor = 1 - (Math.min(absDelta, 100) / 100) * 0.2
    }
  }

  // 计算新的缩放级别
  const newZoomLevel = Math.max(0.1, Math.min(zoomLevel.value * zoomFactor, 10))

  // 以当前播放位置（红线）为基准进行缩放
  if (!trackAreaRef.value) return

  // 获取当前播放时间对应的像素位置
  const playheadPixel = timeToPixel(props.currentTime)

  // 更新缩放级别
  zoomLevel.value = newZoomLevel

  // 重新计算缩放后播放指针应在的像素位置，使其保持在视图中央
  nextTick(() => {
    const newPlayheadPixel = timeToPixel(props.currentTime)
    const containerWidth = trackAreaRef.value?.clientWidth || 0
    // 将播放指针保持在视图的中央位置
    trackAreaRef.value!.scrollLeft = newPlayheadPixel - containerWidth / 2
  })
}

// Subtitle dragging
const handleSubtitleMouseDown = (event: MouseEvent, subtitle: SubtitleEntry) => {
  draggingSubtitle.value = subtitle
  dragStartX.value = event.clientX
  dragStartTime.value = timestampToSeconds(subtitle.startTime)
  currentSubtitleId.value = subtitle.id

  document.addEventListener('mousemove', handleSubtitleDrag)
  document.addEventListener('mouseup', handleSubtitleDragEnd)
  event.preventDefault()
}

const handleSubtitleDrag = (event: MouseEvent) => {
  if (!draggingSubtitle.value) return

  const deltaX = event.clientX - dragStartX.value
  const deltaTime = pixelToTime(deltaX)
  const newStartTime = Math.max(0, dragStartTime.value + deltaTime)

  const subtitle = draggingSubtitle.value
  const duration = timestampToSeconds(subtitle.endTime) - timestampToSeconds(subtitle.startTime)
  const newEndTime = Math.min(props.duration, newStartTime + duration)

  if (newEndTime <= props.duration) {
    emit('updateSubtitle', subtitle.id, secondsToTimestamp(newStartTime), secondsToTimestamp(newEndTime))
  }
}

const handleSubtitleDragEnd = () => {
  draggingSubtitle.value = null
  currentSubtitleId.value = null
  document.removeEventListener('mousemove', handleSubtitleDrag)
  document.removeEventListener('mouseup', handleSubtitleDragEnd)
}

// Subtitle resizing
const handleResizeStart = (event: MouseEvent, subtitle: SubtitleEntry, side: 'left' | 'right') => {
  resizingSubtitle.value = { subtitle, side }
  dragStartX.value = event.clientX
  currentSubtitleId.value = subtitle.id

  document.addEventListener('mousemove', handleResize)
  document.addEventListener('mouseup', handleResizeEnd)
  event.preventDefault()
}

const handleResize = (event: MouseEvent) => {
  if (!resizingSubtitle.value) return

  const { subtitle, side } = resizingSubtitle.value
  const deltaX = event.clientX - dragStartX.value
  const deltaTime = pixelToTime(deltaX)

  let newStartTime = timestampToSeconds(subtitle.startTime)
  let newEndTime = timestampToSeconds(subtitle.endTime)

  if (side === 'left') {
    newStartTime = Math.max(0, newStartTime + deltaTime)
    newStartTime = Math.min(newStartTime, newEndTime - 0.1)
  } else {
    newEndTime = Math.min(props.duration, newEndTime + deltaTime)
    newEndTime = Math.max(newEndTime, newStartTime + 0.1)
  }

  emit('updateSubtitle', subtitle.id, secondsToTimestamp(newStartTime), secondsToTimestamp(newEndTime))
  dragStartX.value = event.clientX
}

const handleResizeEnd = () => {
  resizingSubtitle.value = null
  currentSubtitleId.value = null
  document.removeEventListener('mousemove', handleResize)
  document.removeEventListener('mouseup', handleResizeEnd)
}

// Initialize WaveSurfer
const initWaveSurfer = () => {
  if (!waveformRef.value) return

  try {
    wavesurfer.value = WaveSurfer.create({
      container: waveformRef.value,
      waveColor: '#4a9eff',
      progressColor: '#1e40af',
      cursorColor: 'transparent',
      barWidth: 2,
      barGap: 1,
      barRadius: 2,
      height: 120,
      normalize: true,
      interact: false, // 禁用交互，我们自己处理
    })

    wavesurfer.value.on('ready', () => {
      console.log('✅ WaveSurfer ready!')
    })
  } catch (error) {
    console.error('❌ Failed to initialize WaveSurfer:', error)
  }
}

// Load waveform data
const loadWaveformData = (data: number[]) => {
  if (!wavesurfer.value || !data || data.length === 0) return

  loading.value = true

  try {
    const peaks = new Float32Array(data.length)
    for (let i = 0; i < data.length; i++) {
      peaks[i] = data[i] || 0
    }
    wavesurfer.value.load('', [peaks], props.duration || 1)
  } catch (error) {
    console.error('❌ Failed to load waveform data:', error)
  } finally {
    loading.value = false
  }
}

// Update waveform width when zoom changes
watch(zoomLevel, () => {
  if (wavesurfer.value && waveformRef.value) {
    // 重新设置容器宽度
    waveformRef.value.style.width = timelineWidth.value + 'px'
    // WaveSurfer 会自动适应容器宽度
    nextTick(() => {
      wavesurfer.value?.drawer?.wrapper?.style && (wavesurfer.value.drawer.wrapper.style.width = '100%')
    })
  }
})

// Watch for waveform data changes
watch(() => props.waveformData, (data) => {
  if (data && data.length > 0) {
    nextTick(() => loadWaveformData(data))
  }
}, { immediate: false })

// Auto-scroll to current time
watch(() => props.currentTime, (time) => {
  if (trackAreaRef.value && props.duration > 0) {
    const pixel = timeToPixel(time)
    const scrollLeft = trackAreaRef.value.scrollLeft
    const containerWidth = trackAreaRef.value.clientWidth

    // 如果播放位置超出可视区域，自动滚动
    if (pixel < scrollLeft || pixel > scrollLeft + containerWidth) {
      scrollToTime(time)
    }
  }
})

onMounted(() => {
  initWaveSurfer()

  if (props.waveformData && props.waveformData.length > 0) {
    setTimeout(() => {
      loadWaveformData(props.waveformData!)
    }, 100)
  }
})

onUnmounted(() => {
  if (wavesurfer.value) {
    wavesurfer.value.destroy()
  }
  document.removeEventListener('mousemove', handleSubtitleDrag)
  document.removeEventListener('mouseup', handleSubtitleDragEnd)
  document.removeEventListener('mousemove', handleResize)
  document.removeEventListener('mouseup', handleResizeEnd)
})

// Expose methods to parent component
defineExpose({
  zoomIn,
  zoomOut,
  zoomLevel
})
</script>

<style scoped>
.timeline-editor {
  width: 100%;
  display: flex;
  flex-direction: column;
  background: #ffffff;
  border-radius: 8px;
  overflow: hidden;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
}

/* 控制栏 */
.timeline-controls {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 8px 16px;
  background: #f8fafc;
  border-bottom: 1px solid #e2e8f0;
}

.zoom-controls {
  display: flex;
  align-items: center;
  gap: 8px;
}

.zoom-level {
  min-width: 50px;
  text-align: center;
  font-size: 13px;
  color: #64748b;
  font-weight: 500;
}

/* 时间轴容器 */
.timeline-container {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-height: 0;
}

/* 时间刻度尺 */
.time-ruler {
  height: 30px;
  position: sticky;
  top: 0;
  background: #f8fafc;
  border-bottom: 1px solid #e2e8f0;
  overflow: visible;
  z-index: 50;
  flex-shrink: 0;
}

.time-marker {
  position: absolute;
  top: 0;
  height: 100%;
  border-left: 1px solid #cbd5e1;
  padding-left: 4px;
}

.time-label {
  font-size: 11px;
  color: #64748b;
  line-height: 30px;
  user-select: none;
}

/* 轨道区域 */
.timeline-track-area {
  flex: 1;
  overflow-x: auto;
  overflow-y: hidden;
  position: relative;
  min-height: 200px;
}

.timeline-content {
  position: relative;
  height: 100%;
  min-height: 200px;
}

/* 波形层 */
.waveform-layer {
  width: 100%;
  height: 120px;
  background: linear-gradient(to bottom, #f8fafc 0%, #f1f5f9 100%);
  position: relative;
}

.loading-overlay {
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: rgba(255, 255, 255, 0.9);
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 12px;
  font-size: 14px;
  color: #64748b;
}

/* 字幕轨道 */
.subtitle-track {
  width: 100%;
  height: 80px;
  position: relative;
  background: #f8fafc;
  border-top: 1px solid #e2e8f0;
}

/* 字幕块 */
.subtitle-block {
  position: absolute;
  top: 20px;
  height: 40px;
  border-radius: 4px;
  border: 2px solid transparent;
  cursor: move;
  transition: border-color 0.2s, box-shadow 0.2s;
  overflow: hidden;
  user-select: none;
}

.subtitle-block:hover {
  border-color: #3b82f6;
  box-shadow: 0 2px 8px rgba(59, 130, 246, 0.3);
  z-index: 10;
}

.subtitle-block.is-active {
  border-color: #3b82f6;
  box-shadow: 0 2px 12px rgba(59, 130, 246, 0.4);
  z-index: 20;
}

.subtitle-block.is-dragging {
  opacity: 0.8;
  cursor: grabbing;
}

.subtitle-content {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 0 8px;
  height: 100%;
  font-size: 12px;
  color: #1e293b;
  white-space: nowrap;
  overflow: hidden;
}

.subtitle-label {
  font-weight: 600;
  flex-shrink: 0;
}

.subtitle-text {
  overflow: hidden;
  text-overflow: ellipsis;
}

/* 调整手柄 */
.resize-handle {
  position: absolute;
  top: 0;
  bottom: 0;
  width: 8px;
  background: #3b82f6;
  opacity: 0;
  cursor: ew-resize;
  transition: opacity 0.2s;
  z-index: 30;
}

.resize-handle.left {
  left: -4px;
}

.resize-handle.right {
  right: -4px;
}

.subtitle-block:hover .resize-handle {
  opacity: 0.6;
}

.resize-handle:hover {
  opacity: 1 !important;
}

/* 播放指针 */
.playhead {
  position: absolute;
  top: 0;
  bottom: 0;
  width: 2px;
  z-index: 100;
  pointer-events: none;
}

.playhead-line {
  width: 2px;
  height: 100%;
  background: #ef4444;
}

.playhead-handle {
  position: absolute;
  top: 0;
  left: -6px;
  width: 14px;
  height: 14px;
  background: #ef4444;
  border-radius: 50%;
  border: 2px solid #ffffff;
}

/* 滚动条样式 */
.timeline-track-area::-webkit-scrollbar {
  height: 10px;
}

.timeline-track-area::-webkit-scrollbar-track {
  background: #f1f5f9;
}

.timeline-track-area::-webkit-scrollbar-thumb {
  background: #cbd5e1;
  border-radius: 5px;
}

.timeline-track-area::-webkit-scrollbar-thumb:hover {
  background: #94a3b8;
}
</style>

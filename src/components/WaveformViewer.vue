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
            <!-- 加载动画 - 只在生成波形时显示 -->
            <div v-if="props.isGeneratingWaveform" class="waveform-loading-overlay">
              <div class="waveform-loading-box">
                <!-- 纯 CSS 旋转动画 -->
                <div class="spinner"></div>
                <div class="loading-message">正在生成波形数据...</div>
                <!-- 纯 CSS 进度条 -->
                <div class="progress-bar-container">
                  <div class="progress-bar-bg">
                    <div class="progress-bar-fill" :style="{ width: props.waveformProgress + '%' }"></div>
                  </div>
                  <div class="progress-text">{{ props.waveformProgress }}%</div>
                </div>
              </div>
            </div>
          </div>

          <!-- 字幕轨道 -->
          <div class="subtitle-track" :style="{ height: subtitleTrackHeight + 'px' }" @mousedown="handleTrackMouseDown">
            <div
              v-for="subtitle in subtitles"
              :key="subtitle.id"
              class="subtitle-block"
              :class="{
                'is-dragging': draggingSubtitle?.id === subtitle.id || draggingSelectedSubtitles.some(s => s.id === subtitle.id),
                'is-active': currentSubtitleId === subtitle.id,
                'is-selected': selectedSubtitleIds.has(subtitle.id)
              }"
              :style="getSubtitleStyle(subtitle)"
              @mousedown="handleSubtitleMouseDown($event, subtitle)"
              @dblclick="handleSubtitleDoubleClick(subtitle)"
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

            <!-- 选择框 -->
            <div
              v-if="selectionBox && Math.abs(selectionBox.endX - selectionBox.startX) > 5"
              class="selection-box"
              :style="{
                left: Math.min(selectionBox.startX, selectionBox.endX) + 'px',
                top: '0px',
                width: Math.abs(selectionBox.endX - selectionBox.startX) + 'px',
                height: subtitleTrackHeight + 'px'
              }"
            ></div>
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
  isGeneratingWaveform?: boolean
  waveformProgress?: number
}

const props = withDefaults(defineProps<Props>(), {
  currentTime: 0,
  duration: 0,
  subtitles: () => [],
  isGeneratingWaveform: false,
  waveformProgress: 0
})

// 计算属性，用于调试
const emit = defineEmits<{
  seek: [time: number]
  updateSubtitle: [id: number, startTime: TimeStamp, endTime: TimeStamp]
  updateSubtitles: [updates: Array<{ id: number; startTime: TimeStamp; endTime: TimeStamp }>]
  selectSubtitles: [ids: number[]]
  doubleClickSubtitle: [id: number]
}>()

// Refs
const timelineContainerRef = ref<HTMLDivElement | null>(null)
const trackAreaRef = ref<HTMLDivElement | null>(null)
const waveformRef = ref<HTMLDivElement | null>(null)
const wavesurfer = ref<WaveSurfer | null>(null)
const loading = ref(false)
const waveformRebuildTimer = ref<ReturnType<typeof setTimeout> | null>(null)

// Timeline state
const zoomLevel = ref(1) // 缩放级别：1 = 1秒占用 100px
const pixelsPerSecond = computed(() => 100 * zoomLevel.value)
const timelineWidth = computed(() => props.duration * pixelsPerSecond.value)

// 计算字幕轨道的高度（基于轨道数量）
const subtitleTrackHeight = computed(() => {
  if (!props.subtitles || props.subtitles.length === 0) return 80

  // 找出最大的轨道号
  const maxTrackNumber = Math.max(
    ...props.subtitles.map(s => s.trackNumber ?? 0),
    0
  )

  // 轨道 0 从 top: 20px 开始，每条轨道高度 40px，间隙 2px
  // 最后一条轨道的下边界 = 20 + (maxTrackNumber + 1) * 40 + maxTrackNumber * 2 + 20(底部padding)
  const trackHeight = 40
  const trackGap = 2
  const totalHeight = 20 + (maxTrackNumber + 1) * trackHeight + maxTrackNumber * trackGap + 20

  return Math.max(totalHeight, 80)
})

// Selection state
const selectedSubtitleIds = ref<Set<number>>(new Set())
const isSelecting = ref(false)
const selectionBox = ref<{ startX: number; startY: number; endX: number; endY: number } | null>(null)

// Dragging state
const draggingSubtitle = ref<SubtitleEntry | null>(null)
const draggingSelectedSubtitles = ref<SubtitleEntry[]>([]) // 批量拖拽时选中的字幕
const resizingSubtitle = ref<{ subtitle: SubtitleEntry; side: 'left' | 'right' } | null>(null)
const dragStartX = ref(0)
const dragStartTime = ref(0)
const dragStartTimes = ref<Map<number, number>>(new Map()) // 批量拖拽时每个字幕的起始时间
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

  // 如果被选中，使用更亮的颜色作为基础色
  const isSelected = selectedSubtitleIds.value.has(subtitle.id)
  const baseColor = isSelected ? `hsl(${hue}, 75%, 70%)` : color

  // 根据缩放级别动态调整最小宽度
  // 缩放越小，最小宽度也越小，避免字幕块过长挤占空间
  const minWidth = Math.max(10, Math.round(20 * zoomLevel.value))

  // 根据轨道号计算垂直位置
  const trackNumber = subtitle.trackNumber ?? 0
  const trackHeight = 40  // 每条轨道高度
  const trackGap = 2      // 轨道间隙
  const top = 20 + (trackNumber * (trackHeight + trackGap))

  return {
    left: left + 'px',
    width: Math.max(width, minWidth) + 'px',
    top: top + 'px',
    backgroundColor: baseColor
  }
}

// Zoom controls - zoom centered on playhead
// 缩放范围限制：最小 50%（0.5），最大 100%（1.0）
const MIN_ZOOM = 0.5
const MAX_ZOOM = 1.0

const zoomIn = () => {
  zoomLevel.value = Math.min(zoomLevel.value * 1.5, MAX_ZOOM)
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
  zoomLevel.value = Math.max(zoomLevel.value / 1.5, MIN_ZOOM)
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
  // 忽略对字幕块的点击和选择框
  if ((event.target as HTMLElement).closest('.subtitle-block') || 
      (event.target as HTMLElement).closest('.subtitle-track')) {
    return
  }

  if (!trackAreaRef.value) return

  // 清除选择（如果点击空白区域）
  if (!event.shiftKey) {
    selectedSubtitleIds.value.clear()
    emit('selectSubtitles', [])
  }

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

// Handle track mouse down for selection box
const handleTrackMouseDown = (event: MouseEvent) => {
  // 如果点击在字幕块上，不处理（由字幕块的 mousedown 处理）
  if ((event.target as HTMLElement).closest('.subtitle-block') ||
      (event.target as HTMLElement).closest('.resize-handle')) {
    return
  }

  // 记录初始位置，用于判断是否是拖拽
  const initialX = event.clientX
  const initialY = event.clientY
  let hasMoved = false
  const isAccumulateMode = event.ctrlKey || event.metaKey || event.altKey || event.shiftKey

  // 立即初始化选择框，这样用户一开始拖拽就能看到
  const trackRect = trackAreaRef.value?.getBoundingClientRect()
  if (!trackAreaRef.value || !trackRect) return

  // 获取字幕轨道的位置
  const subtitleTrack = trackAreaRef.value.querySelector('.subtitle-track') as HTMLElement
  if (!subtitleTrack) return

  const trackElementRect = subtitleTrack.getBoundingClientRect()
  const startX = initialX - trackRect.left + trackAreaRef.value.scrollLeft
  const startY = initialY - trackElementRect.top

  // 立即创建选择框，但先设置为很小的尺寸
  isSelecting.value = true

  // 如果不是累加模式，清除之前的选择
  if (!isAccumulateMode) {
    selectedSubtitleIds.value.clear()
  }

  selectionBox.value = {
    startX,
    startY,
    endX: startX,
    endY: startY
  }

  // 记录初始时间（用于判断点击是否移动）
  const initialTime = pixelToTime(startX)

  const handleMove = (e: MouseEvent) => {
    const deltaX = Math.abs(e.clientX - initialX)
    const deltaY = Math.abs(e.clientY - initialY)
    if (deltaX > 3 || deltaY > 3) {
      hasMoved = true
    }
    
    // 更新选择框位置
    if (selectionBox.value && trackAreaRef.value) {
      const currentTrackRect = trackAreaRef.value.getBoundingClientRect()
      const endX = e.clientX - currentTrackRect.left + trackAreaRef.value.scrollLeft
      
      // 获取字幕轨道相对于 trackArea 的位置
      const subtitleTrack = trackAreaRef.value.querySelector('.subtitle-track') as HTMLElement
      if (subtitleTrack) {
        const trackRect = subtitleTrack.getBoundingClientRect()
        const containerRect = trackAreaRef.value.getBoundingClientRect()
        const endY = e.clientY - trackRect.top
        
        selectionBox.value.endX = endX
        selectionBox.value.endY = endY
        
        // 更新选中的字幕
        const isAccumulate = e.ctrlKey || e.metaKey || e.altKey || e.shiftKey
        updateSelectionFromBox(isAccumulate)
      }
    }
  }

  const handleUp = (e: MouseEvent) => {
    document.removeEventListener('mousemove', handleMove)
    document.removeEventListener('mouseup', handleUp)

    // 如果没有移动，只是点击，跳转到该时间点
    if (!hasMoved) {
      // 发送 seek 事件
      emit('seek', initialTime)

      // 清除选择（除非按住 Shift/Ctrl 等）
      if (!isAccumulateMode) {
        selectedSubtitleIds.value.clear()
        emit('selectSubtitles', [])
      }
    }

    // 结束框选
    isSelecting.value = false

    // 如果选择框太小，清除选择
    if (selectionBox.value) {
      const width = Math.abs(selectionBox.value.endX - selectionBox.value.startX)
      const height = Math.abs(selectionBox.value.endY - selectionBox.value.startY)

      if (width < 10 && height < 10 && !isAccumulateMode) {
        selectedSubtitleIds.value.clear()
      }
    }

    selectionBox.value = null
    emit('selectSubtitles', Array.from(selectedSubtitleIds.value))
  }

  document.addEventListener('mousemove', handleMove)
  document.addEventListener('mouseup', handleUp)
  event.preventDefault()
}

// Handle selection box move (保留此函数以防其他地方调用)
const handleSelectionMove = (event: MouseEvent) => {
  if (!isSelecting.value || !selectionBox.value || !trackAreaRef.value) return

  const trackRect = trackAreaRef.value.getBoundingClientRect()
  const endX = event.clientX - trackRect.left + trackAreaRef.value.scrollLeft
  const endY = event.clientY - trackRect.top

  selectionBox.value.endX = endX
  selectionBox.value.endY = endY

  // 更新选中的字幕
  // 如果按住 Ctrl/Cmd/Alt/Shift，累加选择；否则替换选择
  const isAccumulateMode = event.ctrlKey || event.metaKey || event.altKey || event.shiftKey
  updateSelectionFromBox(isAccumulateMode)
}

// Handle selection box end
const handleSelectionEnd = (event?: MouseEvent) => {
  isSelecting.value = false
  
  // 如果选择框太小（可能是误触），清除选择
  if (selectionBox.value) {
    const width = Math.abs(selectionBox.value.endX - selectionBox.value.startX)
    const height = Math.abs(selectionBox.value.endY - selectionBox.value.startY)
    
    // 如果选择框太小（小于10px），可能是误触，清除选择（除非按住修饰键）
    if (width < 10 && height < 10 && !(event?.ctrlKey || event?.metaKey || event?.altKey || event?.shiftKey)) {
      selectedSubtitleIds.value.clear()
    }
  }
  
  selectionBox.value = null
  document.removeEventListener('mousemove', handleSelectionMove)
  document.removeEventListener('mouseup', handleSelectionEnd)
  
  emit('selectSubtitles', Array.from(selectedSubtitleIds.value))
}

// Update selection from selection box
const updateSelectionFromBox = (accumulate: boolean = false) => {
  if (!selectionBox.value || !trackAreaRef.value) return

  const boxLeft = Math.min(selectionBox.value.startX, selectionBox.value.endX)
  const boxRight = Math.max(selectionBox.value.startX, selectionBox.value.endX)
  const boxTop = Math.min(selectionBox.value.startY, selectionBox.value.endY)
  const boxBottom = Math.max(selectionBox.value.startY, selectionBox.value.endY)

  // 临时存储当前框选范围内的字幕ID
  const boxSelectedIds = new Set<number>()

  // 检查每个字幕是否在选择框内
  // 注意：选择框的坐标是相对于字幕轨道的，字幕块的位置也是相对于字幕轨道的
  props.subtitles.forEach(subtitle => {
    const start = timestampToSeconds(subtitle.startTime)
    const end = timestampToSeconds(subtitle.endTime)
    const left = timeToPixel(start)
    const right = timeToPixel(end)

    // 根据轨道号计算字幕块的垂直位置
    const trackNumber = subtitle.trackNumber ?? 0
    const trackHeight = 40
    const trackGap = 2
    const top = 20 + (trackNumber * (trackHeight + trackGap))
    const bottom = top + trackHeight

    // 检查是否与选择框相交
    // 只要字幕块与选择框有任何重叠，就选中它
    if (right >= boxLeft && left <= boxRight && bottom >= boxTop && top <= boxBottom) {
      boxSelectedIds.add(subtitle.id)
    }
  })

  // 更新选择
  if (accumulate) {
    // 累加模式：添加框选范围内的字幕到现有选择
    boxSelectedIds.forEach(id => {
      selectedSubtitleIds.value.add(id)
    })
  } else {
    // 替换模式：只保留框选范围内的字幕
    selectedSubtitleIds.value.clear()
    boxSelectedIds.forEach(id => {
      selectedSubtitleIds.value.add(id)
    })
  }
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

  // 计算新的缩放级别（限制在 MIN_ZOOM 到 MAX_ZOOM 之间）
  const newZoomLevel = Math.max(MIN_ZOOM, Math.min(zoomLevel.value * zoomFactor, MAX_ZOOM))

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

// Handle subtitle double click - focus the text input
const handleSubtitleDoubleClick = (subtitle: SubtitleEntry) => {
  // 发送双击事件，告诉父组件选中此字幕并跳转到编辑区
  emit('doubleClickSubtitle', subtitle.id)
}

// Subtitle dragging
const handleSubtitleMouseDown = (event: MouseEvent, subtitle: SubtitleEntry) => {
  // 如果点击调整手柄，不处理（由 resize 处理）
  if ((event.target as HTMLElement).closest('.resize-handle')) {
    return
  }

  // 处理多选
  if (event.shiftKey) {
    // Shift+点击：切换选择状态
    if (selectedSubtitleIds.value.has(subtitle.id)) {
      selectedSubtitleIds.value.delete(subtitle.id)
    } else {
      selectedSubtitleIds.value.add(subtitle.id)
    }
    emit('selectSubtitles', Array.from(selectedSubtitleIds.value))
    return
  } else if (event.ctrlKey || event.metaKey) {
    // Ctrl/Cmd+点击：添加到选择
    selectedSubtitleIds.value.add(subtitle.id)
    emit('selectSubtitles', Array.from(selectedSubtitleIds.value))
    return
  }

  // 如果当前字幕已被选中，且还有其他选中项，则批量拖拽
  const hasMultipleSelected = selectedSubtitleIds.value.size > 1 && selectedSubtitleIds.value.has(subtitle.id)
  
  if (hasMultipleSelected) {
    // 批量拖拽：拖拽所有选中的字幕
    draggingSelectedSubtitles.value = props.subtitles.filter(s => selectedSubtitleIds.value.has(s.id))
    dragStartTimes.value = new Map()
    draggingSelectedSubtitles.value.forEach(s => {
      dragStartTimes.value.set(s.id, timestampToSeconds(s.startTime))
    })
  } else {
    // 单个拖拽
    selectedSubtitleIds.value.clear()
    selectedSubtitleIds.value.add(subtitle.id)
    emit('selectSubtitles', [subtitle.id])
    draggingSubtitle.value = subtitle
    dragStartTime.value = timestampToSeconds(subtitle.startTime)
  }

  dragStartX.value = event.clientX
  currentSubtitleId.value = subtitle.id

  document.addEventListener('mousemove', handleSubtitleDrag)
  document.addEventListener('mouseup', handleSubtitleDragEnd)
  event.preventDefault()
}

const handleSubtitleDrag = (event: MouseEvent) => {
  const deltaX = event.clientX - dragStartX.value
  const deltaTime = pixelToTime(deltaX)

  // 批量拖拽
  if (draggingSelectedSubtitles.value.length > 0) {
    const updates: Array<{ id: number; startTime: TimeStamp; endTime: TimeStamp }> = []
    
    draggingSelectedSubtitles.value.forEach(subtitle => {
      const originalStartTime = dragStartTimes.value.get(subtitle.id)
      if (originalStartTime === undefined) return

      const newStartTime = Math.max(0, originalStartTime + deltaTime)
      const duration = timestampToSeconds(subtitle.endTime) - timestampToSeconds(subtitle.startTime)
      const newEndTime = Math.min(props.duration, newStartTime + duration)

      if (newEndTime <= props.duration && newStartTime >= 0) {
        updates.push({
          id: subtitle.id,
          startTime: secondsToTimestamp(newStartTime),
          endTime: secondsToTimestamp(newEndTime)
        })
      }
    })

    if (updates.length > 0) {
      emit('updateSubtitles', updates)
    }
  } 
  // 单个拖拽
  else if (draggingSubtitle.value) {
    const newStartTime = Math.max(0, dragStartTime.value + deltaTime)
    const subtitle = draggingSubtitle.value
    const duration = timestampToSeconds(subtitle.endTime) - timestampToSeconds(subtitle.startTime)
    const newEndTime = Math.min(props.duration, newStartTime + duration)

    if (newEndTime <= props.duration) {
      emit('updateSubtitle', subtitle.id, secondsToTimestamp(newStartTime), secondsToTimestamp(newEndTime))
    }
  }
}

const handleSubtitleDragEnd = () => {
  draggingSubtitle.value = null
  draggingSelectedSubtitles.value = []
  dragStartTimes.value.clear()
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

// 根据缩放级别计算 barWidth 和 barGap
const getWaveformParams = () => {
  // 缩放范围：50% - 100%
  // 在这个范围内，使用标准参数即可
  
  if (zoomLevel.value <= 0.7) {
    // 50%-70%：标准宽度带小间隙
    return { barWidth: 2, barGap: 1 }
  } else {
    // 70%-100%：标准宽度无间隙，保持连贯
    return { barWidth: 2, barGap: 0 }
  }
}

// Initialize WaveSurfer
const initWaveSurfer = () => {
  if (!waveformRef.value) return

  // 销毁旧实例
  if (wavesurfer.value) {
    wavesurfer.value.destroy()
    wavesurfer.value = null
  }

  const { barWidth, barGap } = getWaveformParams()

  try {
    wavesurfer.value = WaveSurfer.create({
      container: waveformRef.value,
      waveColor: '#4a9eff',
      progressColor: '#1e40af',
      cursorColor: 'transparent',
      barWidth: barWidth,
      barGap: barGap,
      barRadius: Math.min(2, barWidth / 2),
      height: 80, // 减小波形高度，更紧凑
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
  if (waveformRef.value) {
    // 重新设置容器宽度
    waveformRef.value.style.width = timelineWidth.value + 'px'
    
    // 使用防抖：等待用户停止缩放后再重建波形，避免频繁重建
    if (waveformRebuildTimer.value) {
      clearTimeout(waveformRebuildTimer.value)
    }
    
    waveformRebuildTimer.value = setTimeout(() => {
      // 重新创建 WaveSurfer 实例以应用新的 barWidth 参数
      initWaveSurfer()
      // 重新加载波形数据
      if (props.waveformData && props.waveformData.length > 0) {
        loadWaveformData(props.waveformData)
      }
    }, 150) // 150ms 防抖延迟
  }
})

// Watch for waveform data changes
watch(() => props.waveformData, (data) => {
  if (data && data.length > 0 && !props.isGeneratingWaveform) {
    nextTick(() => {
      loadWaveformData(data)
    })
  }
}, { immediate: false })

// Watch for waveform generation status
watch(() => props.isGeneratingWaveform, (isGenerating) => {
  // 当波形生成完成时，如果数据存在则加载
  if (!isGenerating && props.waveformData && props.waveformData.length > 0) {
    nextTick(() => {
      loadWaveformData(props.waveformData!)
    })
  }
}, { immediate: true })

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

  // 只有在波形生成完成且数据存在时才加载
  if (props.waveformData && props.waveformData.length > 0 && !props.isGeneratingWaveform) {
    setTimeout(() => {
      loadWaveformData(props.waveformData!)
    }, 100)
  }
})

onUnmounted(() => {
  if (wavesurfer.value) {
    wavesurfer.value.destroy()
  }
  if (waveformRebuildTimer.value) {
    clearTimeout(waveformRebuildTimer.value)
  }
  document.removeEventListener('mousemove', handleSubtitleDrag)
  document.removeEventListener('mouseup', handleSubtitleDragEnd)
  document.removeEventListener('mousemove', handleResize)
  document.removeEventListener('mouseup', handleResizeEnd)
  document.removeEventListener('mousemove', handleSelectionMove)
  document.removeEventListener('mouseup', handleSelectionEnd)
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
  height: 80px;
  background: linear-gradient(to bottom, #f8fafc 0%, #f1f5f9 100%);
  position: relative;
  overflow: visible;
}

/* 波形加载动画 - 纯 CSS 实现 */
.waveform-loading-overlay {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: rgba(248, 250, 252, 0.95);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 9999;
}

.waveform-loading-box {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 16px;
  padding: 24px 40px;
  background: white;
  border-radius: 12px;
  box-shadow: 0 4px 20px rgba(0, 0, 0, 0.1);
}

/* 旋转加载动画 */
.spinner {
  width: 40px;
  height: 40px;
  border: 4px solid #e5e7eb;
  border-top-color: #3b82f6;
  border-radius: 50%;
  animation: spin 1s linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

.loading-message {
  font-size: 15px;
  font-weight: 500;
  color: #374151;
}

/* 进度条 */
.progress-bar-container {
  width: 200px;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 8px;
}

.progress-bar-bg {
  width: 100%;
  height: 8px;
  background: #e5e7eb;
  border-radius: 4px;
  overflow: hidden;
}

.progress-bar-fill {
  height: 100%;
  background: linear-gradient(90deg, #3b82f6, #60a5fa);
  border-radius: 4px;
  transition: width 0.3s ease;
}

.progress-text {
  font-size: 14px;
  font-weight: 600;
  color: #3b82f6;
}

/* 旋转动画 */
@keyframes rotate {
  from {
    transform: rotate(0deg);
  }
  to {
    transform: rotate(360deg);
  }
}

.is-loading {
  animation: rotate 1s linear infinite;
}

/* 字幕轨道 */
.subtitle-track {
  width: 100%;
  height: auto;
  min-height: 80px;
  position: relative;
  background: #f8fafc;
  border-top: 1px solid #e2e8f0;
  cursor: default;
}

.subtitle-track:active {
  cursor: crosshair;
}

/* 字幕块 */
.subtitle-block {
  position: absolute;
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

.subtitle-block.is-selected {
  outline: 3px solid #3b82f6 !important;
  outline-offset: -1px;
  z-index: 15;
}

.subtitle-block.is-selected:hover {
  outline-color: #2563eb !important;
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

/* 选择框 */
.selection-box {
  position: absolute;
  border: 2px dashed #3b82f6;
  background: rgba(59, 130, 246, 0.08);
  pointer-events: none;
  z-index: 5;
  border-radius: 4px;
}
</style>

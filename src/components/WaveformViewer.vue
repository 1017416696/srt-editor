<template>
  <div class="timeline-editor waveform-container">
    <!-- 时间轴主区域 -->
    <div class="timeline-container" ref="timelineContainerRef">
      <!-- 左侧轨道控制面板 -->
      <div class="track-controls">
        <!-- 时间刻度尺占位 + 交换按钮 -->
        <div class="track-control-header">
          <button 
            class="track-swap-btn"
            @click="toggleTrackOrder"
            title="交换波形和字幕位置"
          >
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M7 16V4M7 4L3 8M7 4l4 4M17 8v12M17 20l4-4M17 20l-4-4"/>
            </svg>
          </button>
        </div>
        <!-- 轨道控制区域 -->
        <div class="track-control-items">
          <!-- 波形轨道控制 -->
          <div 
            class="track-control-item waveform-control" 
            :class="{ disabled: !showWaveform, 'has-separator': waveformOnTop }"
            :style="{ order: waveformOnTop ? 1 : 2, height: '80px' }"
          >
            <button 
              class="track-visibility-btn"
              @click="showWaveform = !showWaveform"
              :title="showWaveform ? '隐藏波形' : '显示波形'"
            >
              <svg v-if="showWaveform" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z"/>
                <circle cx="12" cy="12" r="3"/>
              </svg>
              <svg v-else width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M17.94 17.94A10.07 10.07 0 0 1 12 20c-7 0-11-8-11-8a18.45 18.45 0 0 1 5.06-5.94M9.9 4.24A9.12 9.12 0 0 1 12 4c7 0 11 8 11 8a18.5 18.5 0 0 1-2.16 3.19m-6.72-1.07a3 3 0 1 1-4.24-4.24"/>
                <line x1="1" y1="1" x2="23" y2="23"/>
              </svg>
            </button>
          </div>
          <!-- 字幕轨道控制 -->
          <div 
            class="track-control-item subtitle-control" 
            :class="{ disabled: !showSubtitles, 'has-separator': !waveformOnTop }"
            :style="{ order: waveformOnTop ? 2 : 1, height: subtitleTrackHeight + 'px' }"
          >
            <button 
              class="track-visibility-btn"
              @click="showSubtitles = !showSubtitles"
              :title="showSubtitles ? '隐藏字幕' : '显示字幕'"
            >
              <svg v-if="showSubtitles" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z"/>
                <circle cx="12" cy="12" r="3"/>
              </svg>
              <svg v-else width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M17.94 17.94A10.07 10.07 0 0 1 12 20c-7 0-11-8-11-8a18.45 18.45 0 0 1 5.06-5.94M9.9 4.24A9.12 9.12 0 0 1 12 4c7 0 11 8 11 8a18.5 18.5 0 0 1-2.16 3.19m-6.72-1.07a3 3 0 1 1-4.24-4.24"/>
                <line x1="1" y1="1" x2="23" y2="23"/>
              </svg>
            </button>
          </div>
        </div>
      </div>

      <!-- 右侧主内容区域 -->
      <div class="timeline-main-area">
        <!-- 波形加载动画 - 相对于 timeline-container 定位 -->
        <div v-if="props.isGeneratingWaveform" class="waveform-loading-overlay">
          <!-- 波形动画 -->
          <div class="waveform-bars">
            <div class="bar"></div>
            <div class="bar"></div>
            <div class="bar"></div>
            <div class="bar"></div>
            <div class="bar"></div>
          </div>
          <!-- 进度信息 -->
          <div class="loading-info">
            <span class="loading-text">生成波形</span>
            <!-- Windows 上不显示百分比，因为 WebView2 渲染批处理导致进度显示不准确 -->
            <span v-if="!isWindows" class="loading-progress">{{ props.waveformProgress }}%</span>
          </div>
        </div>
        <!-- 波形和字幕轨道 -->
        <div class="timeline-track-area" ref="trackAreaRef" @scroll="handleScroll" @wheel="handleWheel">
        <div class="timeline-content" :style="{ width: timelineWidth + 'px' }" @click="handleTimelineClick">
          <!-- 时间刻度尺 -->
          <div class="time-ruler" :style="{ width: timelineWidth + 'px' }">
            <!-- 主刻度（带时间标签） -->
            <div
              v-for="marker in timeMarkers"
              :key="'main-' + marker.time"
              class="time-marker main"
              :style="{ left: timeToPixel(marker.time) + 'px' }"
            >
              <span class="time-label">{{ formatTimeLabel(marker.time) }}</span>
            </div>
            <!-- 次级刻度（只有短竖线，无标签） -->
            <div
              v-for="marker in subMarkers"
              :key="'sub-' + marker.time"
              class="time-marker sub"
              :style="{ left: timeToPixel(marker.time) + 'px' }"
            />
          </div>

          <!-- 轨道容器 - 支持位置交换 -->
          <div class="tracks-container" :class="{ 'reversed': !waveformOnTop }">
            <!-- 波形图 -->
            <div class="waveform-layer" :class="{ 'track-disabled': !showWaveform }" ref="waveformRef" :style="{ order: waveformOnTop ? 1 : 2 }">
              <!-- Canvas 波形渲染 -->
              <canvas ref="waveformCanvasRef" class="waveform-canvas"></canvas>
              <!-- 悬浮高亮区域 - 显示字幕对应的波形范围 -->
              <div
                v-if="hoveredSubtitle && showWaveform"
                class="subtitle-hover-highlight"
                :style="{
                  left: timeToPixel(timestampToSeconds(hoveredSubtitle.startTime)) + 'px',
                  width: timeToPixel(timestampToSeconds(hoveredSubtitle.endTime) - timestampToSeconds(hoveredSubtitle.startTime)) + 'px'
                }"
              ></div>
            </div>

            <!-- 字幕轨道 -->
            <div class="subtitle-track" :class="{ 'scissor-mode': props.scissorMode, 'track-disabled': !showSubtitles }" :style="{ height: subtitleTrackHeight + 'px', order: waveformOnTop ? 2 : 1 }" @mousedown="showSubtitles ? handleTrackMouseDown($event) : null">
            <div
              v-for="subtitle in visibleSubtitles"
              :key="subtitle.id"
              class="subtitle-block"
              :class="{
                'is-dragging': draggingSubtitle?.id === subtitle.id || draggingSelectedSubtitles.some(s => s.id === subtitle.id),
                'is-active': props.currentSubtitleId === subtitle.id,
                'is-selected': selectedSubtitleIds.has(subtitle.id),
                'scissor-mode': props.scissorMode
              }"
              :style="getSubtitleStyle(subtitle)"
              @mousedown="handleSubtitleMouseDown($event, subtitle)"
              @mouseenter="handleSubtitleMouseEnter(subtitle)"
              @mouseleave="handleSubtitleMouseLeave"
              @dblclick="handleSubtitleDoubleClick(subtitle)"
              @contextmenu.prevent="handleSubtitleContextMenu($event, subtitle)"
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
            >
              <!-- 选中数量提示 -->
              <div v-if="selectedSubtitleIds.size > 0" class="selection-count">
                {{ selectedSubtitleIds.size }}
              </div>
            </div>
          </div>
          </div><!-- 轨道容器结束 -->

          <!-- 播放指针 -->
          <div
            class="playhead"
            :style="{ left: timeToPixel(currentTime) + 'px' }"
          >
            <div class="playhead-line"></div>
            <div class="playhead-handle"></div>
          </div>

          <!-- 剪刀参考线 -->
          <div
            v-if="props.scissorMode && scissorLineX !== null"
            class="scissor-line"
            :style="{ left: scissorLineX + 'px' }"
          >
            <div class="scissor-line-bar"></div>
            <div class="scissor-icon">✂</div>
          </div>

          <!-- 吸附参考线 -->
          <div
            v-if="snapLineX !== null"
            class="snap-line"
            :class="{ 'snap-waveform': snapLineType === 'waveform' }"
            :style="{ left: snapLineX + 'px' }"
          >
            <div class="snap-line-bar"></div>
          </div>
        </div>
      </div>
      </div><!-- timeline-main-area 结束 -->
    </div>

    <!-- 右键菜单 -->
    <Teleport to="body">
      <div
        v-if="contextMenu.visible"
        class="context-menu"
        :style="{ left: contextMenu.x + 'px', top: contextMenu.y + 'px' }"
        @click.stop
      >
        <div class="context-menu-item" @click="handleContextMenuSplit">
          <span class="context-menu-label">分割字幕</span>
          <span class="context-menu-shortcut">X</span>
        </div>
        <div
          class="context-menu-item"
          :class="{ disabled: !canMergeSelected }"
          @click="handleContextMenuMerge"
        >
          <span class="context-menu-label">合并字幕</span>
          <span class="context-menu-shortcut">M</span>
        </div>
        <div class="context-menu-divider"></div>
        <div class="context-menu-item" @click="handleContextMenuAlign">
          <span class="context-menu-label">对齐波形</span>
          <span class="context-menu-shortcut">A</span>
        </div>
        <div class="context-menu-divider"></div>
        <div
          class="context-menu-item"
          :class="{ active: props.snapEnabled }"
          @click="handleContextMenuToggleSnap"
        >
          <span class="context-menu-label">拖拽吸附</span>
          <span class="context-menu-shortcut">S</span>
        </div>
      </div>
    </Teleport>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, watch, nextTick } from 'vue'
import type { SubtitleEntry, TimeStamp } from '@/types/subtitle'

interface Props {
  waveformData?: number[]
  currentTime?: number
  duration?: number
  subtitles?: SubtitleEntry[]
  isGeneratingWaveform?: boolean
  waveformProgress?: number
  currentSubtitleId?: number | null
  scissorMode?: boolean
  snapEnabled?: boolean
}

const props = withDefaults(defineProps<Props>(), {
  currentTime: 0,
  duration: 0,
  subtitles: () => [],
  isGeneratingWaveform: false,
  waveformProgress: 0,
  currentSubtitleId: null,
  scissorMode: false,
  snapEnabled: false
})

// Detect Windows platform (WebView2 has rendering batching issues with fast progress updates)
const isWindows = navigator.platform.toLowerCase().includes('win')

// 计算属性，用于调试
const emit = defineEmits<{
  seek: [time: number]
  updateSubtitle: [id: number, startTime: TimeStamp, endTime: TimeStamp]
  updateSubtitles: [updates: Array<{ id: number; startTime: TimeStamp; endTime: TimeStamp }>]
  selectSubtitles: [ids: number[]]
  doubleClickSubtitle: [id: number]
  splitSubtitle: [id: number, splitTimeMs: number]
  dragStart: [ids: number[]]
  dragEnd: []
  deleteSelectedSubtitles: [ids: number[]]
  mergeSubtitles: []
  alignToWaveform: []
  toggleSnap: []
  enterScissorMode: []
}>()

// Refs
const timelineContainerRef = ref<HTMLDivElement | null>(null)
const trackAreaRef = ref<HTMLDivElement | null>(null)
const waveformRef = ref<HTMLDivElement | null>(null)
const waveformCanvasRef = ref<HTMLCanvasElement | null>(null)
const waveformRebuildTimer = ref<ReturnType<typeof setTimeout> | null>(null)

// Timeline state
const zoomLevel = ref(1) // 缩放级别：1 = 1秒占用 100px
const pixelsPerSecond = computed(() => 100 * zoomLevel.value)
const timelineWidth = computed(() => props.duration * pixelsPerSecond.value)

// 缩放节流控制
const isZooming = ref(false)
const zoomRAF = ref<number | null>(null)

// 计算字幕轨道的高度（基于轨道数量）
const subtitleTrackHeight = computed(() => {
  if (!props.subtitles || props.subtitles.length === 0) return 36

  // 找出最大的轨道号
  const maxTrackNumber = Math.max(
    ...props.subtitles.map(s => s.trackNumber ?? 0),
    0
  )

  // 轨道 0 从 top: 2px 开始，每条轨道高度 32px，间隙 2px
  const trackHeight = 32
  const trackGap = 2
  const totalHeight = 2 + (maxTrackNumber + 1) * trackHeight + maxTrackNumber * trackGap

  return Math.max(totalHeight, 36)
})

// Selection state
const selectedSubtitleIds = ref<Set<number>>(new Set())
const lastSelectedSubtitleId = ref<number | null>(null) // 记录上次选中的字幕，用于 Shift 范围选择
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

// 拖拽节流控制
const dragRAF = ref<number | null>(null)
const pendingDragEvent = ref<MouseEvent | null>(null)

// 滚动位置追踪（用于虚拟渲染）
const scrollLeft = ref(0)
const viewportWidth = ref(800) // 默认值，会在 mounted 时更新

// 用户手动滚动后的自动跟随禁用控制
const isUserScrolling = ref(false)
const userScrollTimer = ref<ReturnType<typeof setTimeout> | null>(null)
const USER_SCROLL_COOLDOWN = 2000 // 用户滚动后 2 秒内禁用自动跟随

// 可见字幕过滤（虚拟渲染优化）
const visibleSubtitles = computed(() => {
  if (!props.subtitles || props.subtitles.length === 0) return []

  // 计算可见时间范围（加上缓冲区）
  const bufferPx = 200 // 左右各 200px 缓冲区
  const visibleStartTime = Math.max(0, pixelToTime(scrollLeft.value - bufferPx))
  const visibleEndTime = Math.min(props.duration, pixelToTime(scrollLeft.value + viewportWidth.value + bufferPx))

  // 过滤出在可见范围内的字幕
  return props.subtitles.filter(subtitle => {
    const startTime = timestampToSeconds(subtitle.startTime)
    const endTime = timestampToSeconds(subtitle.endTime)
    // 字幕与可见范围有交集
    return endTime >= visibleStartTime && startTime <= visibleEndTime
  })
})

// 剪刀模式参考线位置
const scissorLineX = ref<number | null>(null)

// 悬浮高亮状态
const hoveredSubtitle = ref<SubtitleEntry | null>(null)

// 轨道控制状态
const showWaveform = ref(true) // 波形轨道显示/隐藏
const showSubtitles = ref(true) // 字幕轨道显示/隐藏
const waveformOnTop = ref(true) // true: 波形在上，字幕在下；false: 字幕在上，波形在下

// 切换轨道位置
const toggleTrackOrder = () => {
  waveformOnTop.value = !waveformOnTop.value
}

// 右键菜单状态
const contextMenu = ref<{
  visible: boolean
  x: number
  y: number
  subtitleId: number | null
}>({
  visible: false,
  x: 0,
  y: 0,
  subtitleId: null
})

// 计算是否可以合并（选中了多个字幕）
const canMergeSelected = computed(() => {
  return selectedSubtitleIds.value.size >= 2
})

// 吸附功能相关
const SNAP_THRESHOLD_PX = 8 // 吸附阈值（像素）
const snapLineX = ref<number | null>(null) // 吸附参考线位置
const snapLineType = ref<'start' | 'end' | 'waveform' | null>(null) // 吸附类型

// 缓存波形边界点（避免重复计算）
const waveformEdgesCache = ref<number[]>([])
const waveformEdgesCacheKey = ref<string>('')

// 检测波形中的语音边界（静音到有声、有声到静音的转换点）
const detectWaveformEdges = (searchTimeStart: number, searchTimeEnd: number): number[] => {
  if (!props.waveformData || props.waveformData.length === 0 || props.duration <= 0) {
    return []
  }

  const data = props.waveformData
  const durationMs = props.duration * 1000
  const numPoints = data.length / 2
  const msPerPoint = durationMs / numPoints

  // 转换搜索范围到索引
  const searchStartMs = Math.max(0, searchTimeStart * 1000 - 500) // 扩展 500ms
  const searchEndMs = Math.min(durationMs, searchTimeEnd * 1000 + 500)
  const startIdx = Math.floor(searchStartMs / msPerPoint)
  const endIdx = Math.min(Math.ceil(searchEndMs / msPerPoint), numPoints)

  if (endIdx - startIdx < 10) return []

  // 提取振幅数据
  const amplitudes: number[] = []
  for (let i = startIdx; i < endIdx; i++) {
    const maxVal = Math.abs(data[i * 2 + 1] || 0)
    amplitudes.push(maxVal)
  }

  // 计算动态阈值
  const sortedAmps = [...amplitudes].sort((a, b) => a - b)
  const lowPercentile = sortedAmps[Math.floor(sortedAmps.length * 0.25)] ?? 0
  const highPercentile = sortedAmps[Math.floor(sortedAmps.length * 0.75)] ?? 0
  
  if (highPercentile - lowPercentile < 0.02) return []

  const threshold = lowPercentile + (highPercentile - lowPercentile) * 0.3

  // 平滑振幅
  const smoothed: number[] = []
  for (let i = 0; i < amplitudes.length; i++) {
    let sum = 0
    let count = 0
    for (let j = Math.max(0, i - 2); j <= Math.min(amplitudes.length - 1, i + 2); j++) {
      sum += amplitudes[j] ?? 0
      count++
    }
    smoothed.push(sum / count)
  }

  // 检测边界点（静音到有声、有声到静音的转换）
  const edges: number[] = []
  let wasVoice = (smoothed[0] ?? 0) >= threshold

  for (let i = 1; i < smoothed.length; i++) {
    const isVoice = (smoothed[i] ?? 0) >= threshold
    if (isVoice !== wasVoice) {
      // 转换点：转换为时间（秒）
      const timeMs = (startIdx + i) * msPerPoint
      edges.push(timeMs / 1000)
    }
    wasVoice = isVoice
  }

  return edges
}

// 计算吸附点（其他字幕的边缘时间 + 波形边界）
const getSnapPoints = (excludeIds: number[], searchTimeStart?: number, searchTimeEnd?: number): number[] => {
  const points: number[] = [0] // 始终包含 0 点
  
  // 添加其他字幕的边缘
  props.subtitles.forEach(subtitle => {
    if (excludeIds.includes(subtitle.id)) return
    points.push(timestampToSeconds(subtitle.startTime))
    points.push(timestampToSeconds(subtitle.endTime))
  })
  
  // 添加波形边界点（如果提供了搜索范围）
  if (searchTimeStart !== undefined && searchTimeEnd !== undefined) {
    const waveformEdges = detectWaveformEdges(searchTimeStart, searchTimeEnd)
    waveformEdges.forEach(edge => {
      if (!points.includes(edge)) {
        points.push(edge)
      }
    })
  }
  
  return points.sort((a, b) => a - b)
}

// 查找最近的吸附点
const findSnapPoint = (
  time: number, 
  excludeIds: number[], 
  side: 'start' | 'end' | 'both' = 'both',
  searchRange?: { start: number; end: number }
): { time: number; snapped: boolean; snapType: 'start' | 'end' | 'waveform' | null } => {
  const snapPoints = getSnapPoints(
    excludeIds, 
    searchRange?.start ?? time - 2, 
    searchRange?.end ?? time + 2
  )
  const thresholdTime = pixelToTime(SNAP_THRESHOLD_PX)
  
  let closestPoint = time
  let minDistance = Infinity
  let snapType: 'start' | 'end' | 'waveform' | null = null
  
  for (const point of snapPoints) {
    const distance = Math.abs(point - time)
    if (distance < minDistance && distance <= thresholdTime) {
      minDistance = distance
      closestPoint = point
      
      // 判断吸附类型：先检查是否是字幕边缘
      const matchingSubtitle = props.subtitles.find(s => {
        if (excludeIds.includes(s.id)) return false
        const start = timestampToSeconds(s.startTime)
        const end = timestampToSeconds(s.endTime)
        return Math.abs(start - point) < 0.001 || Math.abs(end - point) < 0.001
      })
      
      if (matchingSubtitle) {
        const start = timestampToSeconds(matchingSubtitle.startTime)
        snapType = Math.abs(start - point) < 0.001 ? 'start' : 'end'
      } else {
        // 不是字幕边缘，可能是波形边界
        snapType = 'waveform'
      }
    }
  }
  
  return {
    time: closestPoint,
    snapped: minDistance <= thresholdTime,
    snapType
  }
}

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
// 主刻度间隔（带时间标签）
const markerInterval = computed(() => {
  if (zoomLevel.value >= 1.5) return 1      // 很大：每1秒
  if (zoomLevel.value >= 0.5) return 5      // 正常：每5秒
  return 10                                  // 缩小：每10秒
})

// 次级刻度间隔（只有短竖线，无标签）
const subMarkerInterval = computed(() => {
  // 当主刻度是1秒时，不需要次级刻度
  if (markerInterval.value === 1) return 0
  return 1  // 其他情况：每1秒一个次级刻度
})

const timeMarkers = computed(() => {
  const markers = []
  const interval = markerInterval.value
  for (let i = 0; i <= props.duration; i += interval) {
    markers.push({ time: i })
  }
  return markers
})

// 次级刻度（主刻度之间的小竖线，无标签）
const subMarkers = computed(() => {
  const markers: { time: number }[] = []
  const mainInterval = markerInterval.value
  const subInterval = subMarkerInterval.value
  
  // 不需要次级刻度时返回空
  if (subInterval === 0) return markers
  
  for (let i = 0; i <= props.duration; i += subInterval) {
    // 跳过主刻度位置
    if (i % mainInterval !== 0) {
      markers.push({ time: i })
    }
  }
  return markers
})

// 智能时间格式（根据音频长度自适应）
const formatTimeLabel = (seconds: number): string => {
  if (props.duration < 60) {
    // 短音频（<1分钟）：直接显示秒数
    return seconds % 1 === 0 ? `${seconds}s` : `${seconds.toFixed(1)}s`
  }
  const mins = Math.floor(seconds / 60)
  const secs = Math.floor(seconds % 60)
  if (props.duration < 600) {
    // 中等长度（<10分钟）：简化格式 m:ss
    return `${mins}:${String(secs).padStart(2, '0')}`
  }
  // 长音频：完整格式 mm:ss
  return `${String(mins).padStart(2, '0')}:${String(secs).padStart(2, '0')}`
}

// 颜色缓存：根据 subtitle.id 缓存 hue 值，避免重复计算
const subtitleHueCache = new Map<number, number>()

const getSubtitleHue = (id: number): number => {
  let hue = subtitleHueCache.get(id)
  if (hue === undefined) {
    hue = (id * 137.5) % 360
    subtitleHueCache.set(id, hue)
  }
  return hue
}

// Get subtitle style
const getSubtitleStyle = (subtitle: SubtitleEntry) => {
  const start = timestampToSeconds(subtitle.startTime)
  const end = timestampToSeconds(subtitle.endTime)
  const left = timeToPixel(start)
  const width = timeToPixel(end - start)

  // 🎄 圣诞季节判断
  const isChristmasSeason = () => {
    const now = new Date()
    const month = now.getMonth() + 1
    const day = now.getDate()
    return month === 12 && day >= 20 && day <= 31
  }

  // 如果被选中，使用更亮的颜色作为基础色
  const isSelected = selectedSubtitleIds.value.has(subtitle.id)
  const isActive = props.currentSubtitleId === subtitle.id

  let baseColor: string
  if (isChristmasSeason()) {
    // 🎄 圣诞配色：选中/激活用红色，普通用绿色
    if (isSelected || isActive) {
      baseColor = 'linear-gradient(135deg, rgba(220, 38, 38, 0.9), rgba(185, 28, 28, 0.9))'
    } else {
      baseColor = 'linear-gradient(135deg, rgba(34, 197, 94, 0.85), rgba(22, 163, 74, 0.85))'
    }
  } else {
    // 使用缓存的 hue 值
    const hue = getSubtitleHue(subtitle.id)
    baseColor = isSelected ? `hsl(${hue}, 75%, 70%)` : `hsl(${hue}, 70%, 65%)`
  }

  // 根据缩放级别动态调整最小宽度
  // 缩放越小，最小宽度也越小，避免字幕块过长挤占空间
  const minWidth = Math.max(10, Math.round(20 * zoomLevel.value))

  // 根据轨道号计算垂直位置
  const trackNumber = subtitle.trackNumber ?? 0
  const trackHeight = 32  // 每条轨道高度
  const trackGap = 2      // 轨道间隙
  const top = 2 + (trackNumber * (trackHeight + trackGap))

  return {
    left: left + 'px',
    width: Math.max(width, minWidth) + 'px',
    top: top + 'px',
    background: baseColor
  }
}

// Zoom controls - zoom centered on playhead
// 缩放范围限制：最小 25%（0.25），最大 200%（2.0）
const MIN_ZOOM = 0.25
const MAX_ZOOM = 2.0

// 内部缩放实现（带节流）
const applyZoom = (newLevel: number, centerOnPlayhead = true) => {
  // 取消之前的 RAF
  if (zoomRAF.value) {
    cancelAnimationFrame(zoomRAF.value)
  }

  zoomRAF.value = requestAnimationFrame(() => {
    zoomLevel.value = Math.max(MIN_ZOOM, Math.min(newLevel, MAX_ZOOM))

    if (centerOnPlayhead && trackAreaRef.value) {
      const newPlayheadPixel = timeToPixel(props.currentTime)
      const containerWidth = trackAreaRef.value.clientWidth
      trackAreaRef.value.scrollLeft = newPlayheadPixel - containerWidth / 2
    }

    zoomRAF.value = null
  })
}

const zoomIn = () => {
  applyZoom(zoomLevel.value * 1.5)
}

const zoomOut = () => {
  applyZoom(zoomLevel.value / 1.5)
}

// 设置缩放级别
const setZoom = (level: number) => {
  applyZoom(level)
}

// 适应屏幕宽度（双击重置）
const fitToWidth = () => {
  if (!trackAreaRef.value || props.duration <= 0) return

  const containerWidth = trackAreaRef.value.clientWidth
  const fitZoom = containerWidth / (props.duration * 100)

  // 限制在有效范围内
  zoomLevel.value = Math.max(MIN_ZOOM, Math.min(fitZoom, MAX_ZOOM))

  nextTick(() => {
    if (trackAreaRef.value) {
      trackAreaRef.value.scrollLeft = 0
    }
  })
}

// Scroll to specific time
const scrollToTime = (time: number) => {
  if (!trackAreaRef.value) return
  const pixel = timeToPixel(time)
  // 将播放线定位到页面左侧（留 3% 的边距），减少跳转频率
  const containerWidth = trackAreaRef.value.clientWidth
  trackAreaRef.value.scrollLeft = pixel - containerWidth * 0.03
}

// Handle scroll
const handleScroll = () => {
  if (trackAreaRef.value) {
    scrollLeft.value = trackAreaRef.value.scrollLeft
    
    // 标记用户正在手动滚动，暂时禁用自动跟随
    isUserScrolling.value = true
    
    // 清除之前的定时器
    if (userScrollTimer.value) {
      clearTimeout(userScrollTimer.value)
    }
    
    // 设置冷却时间，2秒后恢复自动跟随
    userScrollTimer.value = setTimeout(() => {
      isUserScrolling.value = false
      userScrollTimer.value = null
    }, USER_SCROLL_COOLDOWN)
    
    // 滚动时更新波形分段渲染
    updateWaveformOnScroll()
  }
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
  const isVerticalScroll = Math.abs(event.deltaY) > Math.abs(event.deltaX)

  // 如果是水平滚动，允许默认行为
  if (!isVerticalScroll) {
    return
  }

  // 增加最小阈值，避免触控板水平滚动时的微小垂直分量误触发缩放
  // 只有当垂直滚动量足够大时才触发缩放
  const MIN_ZOOM_DELTA = 10
  if (Math.abs(event.deltaY) < MIN_ZOOM_DELTA) {
    return
  }

  event.preventDefault()

  // deltaY > 0 表示向下滚动（缩小），deltaY < 0 表示向上滚动（放大）
  const isZoomingInDir = event.deltaY < 0

  // 根据 deltaY 的大小调整缩放因子（降低灵敏度）
  const absDelta = Math.abs(event.deltaY)
  let zoomFactor = 1.0
  if (absDelta > 0) {
    // 降低缩放灵敏度，从 0.2 改为 0.15
    const sensitivity = 0.15
    if (isZoomingInDir) {
      zoomFactor = 1 + (Math.min(absDelta, 100) / 100) * sensitivity
    } else {
      zoomFactor = 1 - (Math.min(absDelta, 100) / 100) * sensitivity
    }
  }

  // 计算新的缩放级别
  const newZoomLevel = Math.max(MIN_ZOOM, Math.min(zoomLevel.value * zoomFactor, MAX_ZOOM))

  // 使用 applyZoom 进行节流，但不自动居中播放头（保持当前滚动位置）
  applyZoom(newZoomLevel, false)
}

// Handle subtitle mouse enter - show hover highlight
const handleSubtitleMouseEnter = (subtitle: SubtitleEntry) => {
  hoveredSubtitle.value = subtitle
}

// Handle subtitle mouse leave - hide hover highlight
const handleSubtitleMouseLeave = () => {
  hoveredSubtitle.value = null
}

// Handle subtitle double click - focus the text input
const handleSubtitleDoubleClick = (subtitle: SubtitleEntry) => {
  // 发送双击事件，告诉父组件选中此字幕并跳转到编辑区
  emit('doubleClickSubtitle', subtitle.id)
}

// Handle subtitle right click - show context menu
const handleSubtitleContextMenu = (event: MouseEvent, subtitle: SubtitleEntry) => {
  event.preventDefault()
  event.stopPropagation()
  
  // 如果右键点击的字幕不在选中列表中，先选中它
  if (!selectedSubtitleIds.value.has(subtitle.id)) {
    selectedSubtitleIds.value.clear()
    selectedSubtitleIds.value.add(subtitle.id)
    emit('selectSubtitles', [subtitle.id])
  }
  
  // 显示右键菜单
  contextMenu.value = {
    visible: true,
    x: event.clientX,
    y: event.clientY,
    subtitleId: subtitle.id
  }
  
  // 添加全局点击监听，点击其他地方关闭菜单
  setTimeout(() => {
    document.addEventListener('click', closeContextMenu)
    document.addEventListener('contextmenu', closeContextMenu)
  }, 0)
}

// Close context menu
const closeContextMenu = () => {
  contextMenu.value.visible = false
  contextMenu.value.subtitleId = null
  document.removeEventListener('click', closeContextMenu)
  document.removeEventListener('contextmenu', closeContextMenu)
}

// Context menu actions
const handleContextMenuSplit = () => {
  closeContextMenu()
  // 进入剪刀模式
  emit('enterScissorMode')
}

const handleContextMenuMerge = () => {
  if (!canMergeSelected.value) return
  closeContextMenu()
  emit('mergeSubtitles')
}

const handleContextMenuAlign = () => {
  closeContextMenu()
  emit('alignToWaveform')
}

const handleContextMenuToggleSnap = () => {
  closeContextMenu()
  emit('toggleSnap')
}

// Subtitle dragging
const handleSubtitleMouseDown = (event: MouseEvent, subtitle: SubtitleEntry) => {
  // 如果点击调整手柄，不处理（由 resize 处理）
  if ((event.target as HTMLElement).closest('.resize-handle')) {
    return
  }

  // 剪刀模式：分割字幕
  if (props.scissorMode) {
    event.preventDefault()
    event.stopPropagation()

    // 计算点击位置对应的时间
    if (!trackAreaRef.value) return

    const trackRect = trackAreaRef.value.getBoundingClientRect()
    const clickX = event.clientX - trackRect.left + trackAreaRef.value.scrollLeft
    const clickTime = pixelToTime(clickX)
    const clickTimeMs = clickTime * 1000

    // 发送分割事件
    emit('splitSubtitle', subtitle.id, clickTimeMs)
    return
  }

  // 处理多选
  if (event.shiftKey) {
    // Shift+点击：范围选择（选中两个字幕之间的所有字幕）
    if (lastSelectedSubtitleId.value !== null && lastSelectedSubtitleId.value !== subtitle.id) {
      // 找到上次选中和当前点击的字幕在列表中的索引
      const lastIdx = props.subtitles.findIndex(s => s.id === lastSelectedSubtitleId.value)
      const currentIdx = props.subtitles.findIndex(s => s.id === subtitle.id)
      
      if (lastIdx !== -1 && currentIdx !== -1) {
        const startIdx = Math.min(lastIdx, currentIdx)
        const endIdx = Math.max(lastIdx, currentIdx)
        
        // 选中范围内的所有字幕
        for (let i = startIdx; i <= endIdx; i++) {
          selectedSubtitleIds.value.add(props.subtitles[i].id)
        }
      }
    } else {
      // 没有上次选中的字幕，直接添加当前字幕
      selectedSubtitleIds.value.add(subtitle.id)
      lastSelectedSubtitleId.value = subtitle.id
    }
    emit('selectSubtitles', Array.from(selectedSubtitleIds.value))
    return
  } else if (event.ctrlKey || event.metaKey) {
    // Ctrl/Cmd+点击：添加到选择
    selectedSubtitleIds.value.add(subtitle.id)
    lastSelectedSubtitleId.value = subtitle.id
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
    // 通知开始拖动，记录原始时间
    emit('dragStart', draggingSelectedSubtitles.value.map(s => s.id))
  } else {
    // 单个拖拽
    selectedSubtitleIds.value.clear()
    selectedSubtitleIds.value.add(subtitle.id)
    lastSelectedSubtitleId.value = subtitle.id // 记录最后选中的字幕
    emit('selectSubtitles', [subtitle.id])
    draggingSubtitle.value = subtitle
    dragStartTime.value = timestampToSeconds(subtitle.startTime)
    // 通知开始拖动，记录原始时间
    emit('dragStart', [subtitle.id])
  }

  dragStartX.value = event.clientX
  currentSubtitleId.value = subtitle.id

  document.addEventListener('mousemove', handleSubtitleDrag)
  document.addEventListener('mouseup', handleSubtitleDragEnd)
  event.preventDefault()
}

// 实际执行拖拽更新的函数
const executeDragUpdate = (event: MouseEvent) => {
  const deltaX = event.clientX - dragStartX.value
  const deltaTime = pixelToTime(deltaX)
  
  // 吸附开关：需要开启吸附模式，且没有按住 Alt 键
  const snapActive = props.snapEnabled && !event.altKey

  // 批量拖拽
  if (draggingSelectedSubtitles.value.length > 0) {
    const updates: Array<{ id: number; startTime: TimeStamp; endTime: TimeStamp }> = []
    const excludeIds = draggingSelectedSubtitles.value.map(s => s.id)
    
    // 使用第一个字幕作为吸附参考
    const firstSubtitle = draggingSelectedSubtitles.value[0]
    const firstOriginalStart = dragStartTimes.value.get(firstSubtitle.id)
    if (firstOriginalStart === undefined) return
    
    const firstDuration = timestampToSeconds(firstSubtitle.endTime) - timestampToSeconds(firstSubtitle.startTime)
    let rawNewStart = Math.max(0, firstOriginalStart + deltaTime)
    let rawNewEnd = rawNewStart + firstDuration
    
    // 选择更近的吸附点
    let snapOffset = 0
    
    if (snapActive) {
      // 检查开始和结束边缘的吸附（包含波形边界搜索范围）
      const searchRange = { start: rawNewStart - 1, end: rawNewEnd + 1 }
      const startSnap = findSnapPoint(rawNewStart, excludeIds, 'start', searchRange)
      const endSnap = findSnapPoint(rawNewEnd, excludeIds, 'end', searchRange)
      
      if (startSnap.snapped && endSnap.snapped) {
        // 两边都能吸附，选择更近的
        const startDist = Math.abs(startSnap.time - rawNewStart)
        const endDist = Math.abs(endSnap.time - rawNewEnd)
        if (startDist <= endDist) {
          snapOffset = startSnap.time - rawNewStart
          snapLineX.value = timeToPixel(startSnap.time)
          snapLineType.value = startSnap.snapType
        } else {
          snapOffset = endSnap.time - rawNewEnd
          snapLineX.value = timeToPixel(endSnap.time)
          snapLineType.value = endSnap.snapType
        }
      } else if (startSnap.snapped) {
        snapOffset = startSnap.time - rawNewStart
        snapLineX.value = timeToPixel(startSnap.time)
        snapLineType.value = startSnap.snapType
      } else if (endSnap.snapped) {
        snapOffset = endSnap.time - rawNewEnd
        snapLineX.value = timeToPixel(endSnap.time)
        snapLineType.value = endSnap.snapType
      } else {
        snapLineX.value = null
        snapLineType.value = null
      }
    } else {
      // 禁用吸附时清除参考线
      snapLineX.value = null
      snapLineType.value = null
    }
    
    draggingSelectedSubtitles.value.forEach(subtitle => {
      const originalStartTime = dragStartTimes.value.get(subtitle.id)
      if (originalStartTime === undefined) return

      let newStartTime = Math.max(0, originalStartTime + deltaTime + snapOffset)
      const duration = timestampToSeconds(subtitle.endTime) - timestampToSeconds(subtitle.startTime)
      let newEndTime = Math.min(props.duration, newStartTime + duration)

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
    const subtitle = draggingSubtitle.value
    const duration = timestampToSeconds(subtitle.endTime) - timestampToSeconds(subtitle.startTime)
    let rawNewStart = Math.max(0, dragStartTime.value + deltaTime)
    let rawNewEnd = rawNewStart + duration
    
    let newStartTime = rawNewStart
    let newEndTime = rawNewEnd
    
    if (snapActive) {
      // 检查开始和结束边缘的吸附（包含波形边界搜索范围）
      const excludeIds = [subtitle.id]
      const searchRange = { start: rawNewStart - 1, end: rawNewEnd + 1 }
      const startSnap = findSnapPoint(rawNewStart, excludeIds, 'start', searchRange)
      const endSnap = findSnapPoint(rawNewEnd, excludeIds, 'end', searchRange)
      
      if (startSnap.snapped && endSnap.snapped) {
        const startDist = Math.abs(startSnap.time - rawNewStart)
        const endDist = Math.abs(endSnap.time - rawNewEnd)
        if (startDist <= endDist) {
          newStartTime = startSnap.time
          newEndTime = newStartTime + duration
          snapLineX.value = timeToPixel(startSnap.time)
          snapLineType.value = startSnap.snapType
        } else {
          newEndTime = endSnap.time
          newStartTime = newEndTime - duration
          snapLineX.value = timeToPixel(endSnap.time)
          snapLineType.value = endSnap.snapType
        }
      } else if (startSnap.snapped) {
        newStartTime = startSnap.time
        newEndTime = newStartTime + duration
        snapLineX.value = timeToPixel(startSnap.time)
        snapLineType.value = startSnap.snapType
      } else if (endSnap.snapped) {
        newEndTime = endSnap.time
        newStartTime = newEndTime - duration
        snapLineX.value = timeToPixel(endSnap.time)
        snapLineType.value = endSnap.snapType
      } else {
        snapLineX.value = null
        snapLineType.value = null
      }
    } else {
      // 禁用吸附时清除参考线
      snapLineX.value = null
      snapLineType.value = null
    }
    
    // 边界检查
    newStartTime = Math.max(0, newStartTime)
    newEndTime = Math.min(props.duration, newEndTime)

    if (newEndTime <= props.duration) {
      emit('updateSubtitle', subtitle.id, secondsToTimestamp(newStartTime), secondsToTimestamp(newEndTime))
    }
  }
}

// 使用 requestAnimationFrame 节流的拖拽处理函数
const handleSubtitleDrag = (event: MouseEvent) => {
  // 保存最新的事件
  pendingDragEvent.value = event
  
  // 如果已经有 RAF 在等待，跳过
  if (dragRAF.value !== null) return
  
  // 请求下一帧执行更新
  dragRAF.value = requestAnimationFrame(() => {
    if (pendingDragEvent.value) {
      executeDragUpdate(pendingDragEvent.value)
      pendingDragEvent.value = null
    }
    dragRAF.value = null
  })
}

const handleSubtitleDragEnd = () => {
  // 取消待执行的 RAF
  if (dragRAF.value !== null) {
    cancelAnimationFrame(dragRAF.value)
    dragRAF.value = null
  }
  pendingDragEvent.value = null
  // 通知拖动结束，记录历史
  emit('dragEnd')
  
  draggingSubtitle.value = null
  draggingSelectedSubtitles.value = []
  dragStartTimes.value.clear()
  currentSubtitleId.value = null
  // 清除吸附线
  snapLineX.value = null
  snapLineType.value = null
  document.removeEventListener('mousemove', handleSubtitleDrag)
  document.removeEventListener('mouseup', handleSubtitleDragEnd)
}

// Subtitle resizing
// resize 节流控制
const resizeRAF = ref<number | null>(null)
const pendingResizeEvent = ref<MouseEvent | null>(null)

const handleResizeStart = (event: MouseEvent, subtitle: SubtitleEntry, side: 'left' | 'right') => {
  resizingSubtitle.value = { subtitle, side }
  dragStartX.value = event.clientX
  currentSubtitleId.value = subtitle.id
  // 通知开始拖动，记录原始时间
  emit('dragStart', [subtitle.id])

  document.addEventListener('mousemove', handleResize)
  document.addEventListener('mouseup', handleResizeEnd)
  event.preventDefault()
}

// 实际执行 resize 更新的函数
const executeResizeUpdate = (event: MouseEvent) => {
  if (!resizingSubtitle.value) return

  const { subtitle, side } = resizingSubtitle.value
  const deltaX = event.clientX - dragStartX.value
  const deltaTime = pixelToTime(deltaX)
  const excludeIds = [subtitle.id]
  
  // 吸附开关：需要开启吸附模式，且没有按住 Alt 键
  const snapActive = props.snapEnabled && !event.altKey

  let newStartTime = timestampToSeconds(subtitle.startTime)
  let newEndTime = timestampToSeconds(subtitle.endTime)

  if (side === 'left') {
    let rawNewStart = Math.max(0, newStartTime + deltaTime)
    rawNewStart = Math.min(rawNewStart, newEndTime - 0.1)
    
    if (snapActive) {
      // 吸附检查（包含波形边界）
      const searchRange = { start: rawNewStart - 1, end: newEndTime }
      const snap = findSnapPoint(rawNewStart, excludeIds, 'start', searchRange)
      if (snap.snapped && snap.time < newEndTime - 0.1) {
        newStartTime = snap.time
        snapLineX.value = timeToPixel(snap.time)
        snapLineType.value = snap.snapType
      } else {
        newStartTime = rawNewStart
        snapLineX.value = null
        snapLineType.value = null
      }
    } else {
      newStartTime = rawNewStart
      snapLineX.value = null
      snapLineType.value = null
    }
  } else {
    let rawNewEnd = Math.min(props.duration, newEndTime + deltaTime)
    rawNewEnd = Math.max(rawNewEnd, newStartTime + 0.1)
    
    if (snapActive) {
      // 吸附检查（包含波形边界）
      const searchRange = { start: newStartTime, end: rawNewEnd + 1 }
      const snap = findSnapPoint(rawNewEnd, excludeIds, 'end', searchRange)
      if (snap.snapped && snap.time > newStartTime + 0.1) {
        newEndTime = snap.time
        snapLineX.value = timeToPixel(snap.time)
        snapLineType.value = snap.snapType
      } else {
        newEndTime = rawNewEnd
        snapLineX.value = null
        snapLineType.value = null
      }
    } else {
      newEndTime = rawNewEnd
      snapLineX.value = null
      snapLineType.value = null
    }
  }

  emit('updateSubtitle', subtitle.id, secondsToTimestamp(newStartTime), secondsToTimestamp(newEndTime))
  dragStartX.value = event.clientX
}

// 使用 requestAnimationFrame 节流的 resize 处理函数
const handleResize = (event: MouseEvent) => {
  // 保存最新的事件
  pendingResizeEvent.value = event
  
  // 如果已经有 RAF 在等待，跳过
  if (resizeRAF.value !== null) return
  
  // 请求下一帧执行更新
  resizeRAF.value = requestAnimationFrame(() => {
    if (pendingResizeEvent.value) {
      executeResizeUpdate(pendingResizeEvent.value)
      pendingResizeEvent.value = null
    }
    resizeRAF.value = null
  })
}

const handleResizeEnd = () => {
  // 取消待执行的 RAF
  if (resizeRAF.value !== null) {
    cancelAnimationFrame(resizeRAF.value)
    resizeRAF.value = null
  }
  pendingResizeEvent.value = null
  
  // 通知拖动结束，记录历史
  emit('dragEnd')
  
  resizingSubtitle.value = null
  currentSubtitleId.value = null
  // 清除吸附线
  snapLineX.value = null
  snapLineType.value = null
  document.removeEventListener('mousemove', handleResize)
  document.removeEventListener('mouseup', handleResizeEnd)
}

// 波形配置
const WAVEFORM_HEIGHT = 80
const WAVEFORM_BUFFER_PX = 500 // 左右缓冲区像素

// 缓存预处理后的波形数据（5点移动平均平滑处理）
const smoothedWaveformCache = ref<number[]>([])
const waveformDataCacheKey = ref<string>('')

// 分段渲染状态
const lastRenderedRange = ref<{ start: number; end: number } | null>(null)
const waveformScrollRAF = ref<number | null>(null)

// 预处理波形数据：提取振幅并应用5点移动平均平滑
const preprocessWaveformData = (data: number[]): number[] => {
  if (!data || data.length === 0) return []

  const isMinMaxFormat = data.length % 2 === 0 && data.length > 2
  const numPoints = isMinMaxFormat ? data.length / 2 : data.length

  // 提取每个采样点的振幅
  const amplitudes: number[] = []
  for (let i = 0; i < numPoints; i++) {
    let amp = 0
    if (isMinMaxFormat) {
      const min = Math.abs(data[i * 2] ?? 0)
      const max = Math.abs(data[i * 2 + 1] ?? 0)
      amp = Math.max(min, max)
    } else {
      amp = Math.abs(data[i] ?? 0)
    }
    amplitudes.push(amp)
  }

  // 5点移动平均平滑处理（只计算一次）
  const smoothed: number[] = []
  for (let i = 0; i < amplitudes.length; i++) {
    let sum = 0
    let count = 0
    for (let j = -2; j <= 2; j++) {
      const idx = i + j
      if (idx >= 0 && idx < amplitudes.length) {
        sum += amplitudes[idx]
        count++
      }
    }
    smoothed.push(sum / count)
  }

  return smoothed
}

// 计算当前需要渲染的范围
const getVisibleWaveformRange = () => {
  const bufferPx = WAVEFORM_BUFFER_PX
  const start = Math.max(0, scrollLeft.value - bufferPx)
  const end = Math.min(timelineWidth.value, scrollLeft.value + viewportWidth.value + bufferPx)
  return { start: Math.floor(start), end: Math.ceil(end) }
}

// 检查是否需要重新渲染（滚动超出缓冲区）
const needsRerender = (newRange: { start: number; end: number }) => {
  if (!lastRenderedRange.value) return true
  const { start: lastStart, end: lastEnd } = lastRenderedRange.value
  // 如果新范围超出了上次渲染范围的 50%，需要重新渲染
  const threshold = WAVEFORM_BUFFER_PX * 0.5
  return newRange.start < lastStart + threshold || newRange.end > lastEnd - threshold
}

// 分段渲染波形 - 只渲染可见区域
const renderWaveformSegment = (data: number[], forceFullRender = false) => {
  const canvas = waveformCanvasRef.value
  if (!canvas || !data || data.length === 0) return

  const ctx = canvas.getContext('2d')
  if (!ctx) return

  if (timelineWidth.value <= 0) {
    setTimeout(() => renderWaveformSegment(data, forceFullRender), 100)
    return
  }

  // 检查缓存是否有效
  const cacheKey = `${data.length}-${data[0]}-${data[data.length - 1]}`
  if (waveformDataCacheKey.value !== cacheKey) {
    smoothedWaveformCache.value = preprocessWaveformData(data)
    waveformDataCacheKey.value = cacheKey
    forceFullRender = true
  }

  const smoothedData = smoothedWaveformCache.value
  if (smoothedData.length === 0) return

  const totalWidth = timelineWidth.value
  const height = WAVEFORM_HEIGHT
  const dpr = window.devicePixelRatio || 1

  // 计算可见范围
  const visibleRange = getVisibleWaveformRange()
  
  // 短音频（< 3000px）直接全量渲染，避免分段开销
  // 注意：某些浏览器对 canvas 尺寸有限制（如 Chrome 最大约 16384px）
  // 如果 canvas 实际像素宽度太大，强制使用分段渲染
  const maxCanvasWidth = 16000 // 安全的最大宽度
  const shouldFullRender =
    (totalWidth < 3000 || forceFullRender) && totalWidth * dpr <= maxCanvasWidth

  if (shouldFullRender) {
    // 全量渲染
    canvas.width = totalWidth * dpr
    canvas.height = height * dpr
    canvas.style.width = totalWidth + 'px'
    canvas.style.height = height + 'px'
    canvas.style.left = '0px'
    ctx.scale(dpr, dpr)

    renderWaveformToContext(ctx, smoothedData, 0, totalWidth, totalWidth, height)
    lastRenderedRange.value = { start: 0, end: totalWidth }
    return
  }

  // 检查是否需要重新渲染
  if (!needsRerender(visibleRange) && !forceFullRender) {
    return
  }

  // 分段渲染：只渲染可见区域 + 缓冲区
  const renderStart = visibleRange.start
  const renderEnd = visibleRange.end
  const renderWidth = renderEnd - renderStart

  canvas.width = renderWidth * dpr
  canvas.height = height * dpr
  canvas.style.width = renderWidth + 'px'
  canvas.style.height = height + 'px'
  canvas.style.left = renderStart + 'px'
  ctx.scale(dpr, dpr)

  renderWaveformToContext(ctx, smoothedData, renderStart, renderEnd, totalWidth, height)
  lastRenderedRange.value = { start: renderStart, end: renderEnd }
}

// 核心渲染函数：将波形绘制到指定 context
const renderWaveformToContext = (
  ctx: CanvasRenderingContext2D,
  smoothedData: number[],
  startPx: number,
  endPx: number,
  totalWidth: number,
  height: number
) => {
  const renderWidth = endPx - startPx
  const numPoints = smoothedData.length
  const pointsPerPixel = numPoints / totalWidth

  // 清空画布
  ctx.clearRect(0, 0, renderWidth, height)

  // 绘制背景渐变
  const bgGradient = ctx.createLinearGradient(0, 0, 0, height)
  bgGradient.addColorStop(0, 'rgba(59, 130, 246, 0.08)')
  bgGradient.addColorStop(1, 'rgba(59, 130, 246, 0.15)')
  ctx.fillStyle = bgGradient
  ctx.fillRect(0, 0, renderWidth, height)

  // 波形参数
  const maxWaveHeight = height - 8
  const baseY = height - 4

  // 为渲染范围内的每个像素采样
  const pixelAmplitudes: number[] = []
  for (let x = startPx; x < endPx; x++) {
    const startIdx = Math.floor(x * pointsPerPixel)
    const endIdx = Math.min(Math.ceil((x + 1) * pointsPerPixel), numPoints)

    let amp = 0
    for (let j = startIdx; j < endIdx; j++) {
      const val = smoothedData[j] ?? 0
      if (val > amp) amp = val
    }
    pixelAmplitudes.push(amp)
  }

  // 绘制波形填充
  ctx.beginPath()
  ctx.moveTo(0, baseY)

  for (let i = 0; i < pixelAmplitudes.length; i++) {
    const waveHeight = pixelAmplitudes[i] * maxWaveHeight
    ctx.lineTo(i, baseY - waveHeight)
  }

  ctx.lineTo(renderWidth - 1, baseY)
  ctx.closePath()

  // 波形渐变填充
  const waveGradient = ctx.createLinearGradient(0, 0, 0, height)
  waveGradient.addColorStop(0, 'rgba(59, 130, 246, 0.9)')
  waveGradient.addColorStop(0.5, 'rgba(96, 165, 250, 0.7)')
  waveGradient.addColorStop(1, 'rgba(147, 197, 253, 0.5)')
  ctx.fillStyle = waveGradient
  ctx.fill()
}

// 滚动时更新波形渲染（节流）
const updateWaveformOnScroll = () => {
  if (!props.waveformData || props.waveformData.length === 0) return
  
  // 取消之前的 RAF
  if (waveformScrollRAF.value !== null) {
    cancelAnimationFrame(waveformScrollRAF.value)
  }
  
  waveformScrollRAF.value = requestAnimationFrame(() => {
    renderWaveformSegment(props.waveformData!)
    waveformScrollRAF.value = null
  })
}

// 兼容旧 API
const renderWaveform = (data: number[]) => {
  renderWaveformSegment(data, true)
}

// Load waveform data
const loadWaveformData = (data: number[]) => {
  if (!data || data.length === 0) return
  renderWaveform(data)
}

// Update waveform width when zoom changes
watch(zoomLevel, () => {
  const canvas = waveformCanvasRef.value
  const waveform = waveformRef.value
  if (!waveform) return

  // 标记正在缩放
  isZooming.value = true

  // 重新设置容器宽度
  waveform.style.width = timelineWidth.value + 'px'

  // 缩放时用 CSS transform 缩放 canvas（GPU 加速，非常流畅）
  if (canvas) {
    const currentCanvasWidth = canvas.width / (window.devicePixelRatio || 1)
    if (currentCanvasWidth > 0) {
      // 计算相对于当前渲染位置的缩放
      const lastStart = lastRenderedRange.value?.start ?? 0
      const scale = timelineWidth.value / (currentCanvasWidth + lastStart) * (currentCanvasWidth / (currentCanvasWidth))
      canvas.style.transform = `scaleX(${scale})`
      canvas.style.transformOrigin = 'left top'
    }
  }

  // 使用防抖：等待用户停止缩放后再重新渲染波形
  if (waveformRebuildTimer.value) {
    clearTimeout(waveformRebuildTimer.value)
  }

  waveformRebuildTimer.value = setTimeout(() => {
    isZooming.value = false
    // 重置 transform 和渲染范围缓存，强制重新渲染
    if (canvas) {
      canvas.style.transform = ''
      canvas.style.left = '0px'
    }
    lastRenderedRange.value = null
    if (props.waveformData && props.waveformData.length > 0) {
      loadWaveformData(props.waveformData)
    }
  }, 200) // 200ms 防抖延迟（从 400ms 优化到 200ms）
})

// Watch for waveform data changes
watch(
  () => props.waveformData,
  (data) => {
    if (data && data.length > 0 && !props.isGeneratingWaveform && props.duration > 0) {
      nextTick(() => {
        loadWaveformData(data)
      })
    }
  },
  { immediate: false }
)

// Watch for duration changes - re-render waveform when duration becomes available
watch(
  () => props.duration,
  (duration) => {
    if (
      duration > 0 &&
      props.waveformData &&
      props.waveformData.length > 0 &&
      !props.isGeneratingWaveform
    ) {
      nextTick(() => {
        loadWaveformData(props.waveformData!)
      })
    }
  }
)

// Watch for waveform generation status
watch(
  () => props.isGeneratingWaveform,
  (isGenerating) => {
    // 当波形生成完成时，如果数据存在且 duration 有效则加载
    if (!isGenerating && props.waveformData && props.waveformData.length > 0 && props.duration > 0) {
      nextTick(() => {
        loadWaveformData(props.waveformData!)
      })
    }
  },
  { immediate: true }
)

// Auto-scroll to current time
watch(() => props.currentTime, (time) => {
  // 如果用户正在手动滚动，不自动跟随播放指针
  if (isUserScrolling.value) return
  
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

// 剪刀模式鼠标移动处理
const handleScissorMouseMove = (event: MouseEvent) => {
  if (!props.scissorMode || !trackAreaRef.value) {
    scissorLineX.value = null
    return
  }

  const trackRect = trackAreaRef.value.getBoundingClientRect()
  const mouseX = event.clientX - trackRect.left + trackAreaRef.value.scrollLeft

  // 确保在有效范围内
  if (mouseX >= 0 && mouseX <= timelineWidth.value) {
    scissorLineX.value = mouseX
  } else {
    scissorLineX.value = null
  }
}

// 剪刀模式鼠标离开处理
const handleScissorMouseLeave = () => {
  scissorLineX.value = null
}

// 更新视口宽度（用于虚拟渲染）
const updateViewportWidth = () => {
  if (trackAreaRef.value) {
    viewportWidth.value = trackAreaRef.value.clientWidth
    scrollLeft.value = trackAreaRef.value.scrollLeft
  }
}

// ResizeObserver 用于监听视口大小变化
let viewportResizeObserver: ResizeObserver | null = null

onMounted(() => {
  // 初始化视口宽度
  updateViewportWidth()

  // 监听视口大小变化
  if (trackAreaRef.value) {
    viewportResizeObserver = new ResizeObserver(() => {
      updateViewportWidth()
    })
    viewportResizeObserver.observe(trackAreaRef.value)
  }

  // 只有在波形生成完成且数据存在时才加载
  if (props.waveformData && props.waveformData.length > 0 && !props.isGeneratingWaveform) {
    setTimeout(() => {
      loadWaveformData(props.waveformData!)
    }, 100)
  }
})

onUnmounted(() => {
  if (waveformRebuildTimer.value) {
    clearTimeout(waveformRebuildTimer.value)
  }
  // 清理视口 ResizeObserver
  if (viewportResizeObserver) {
    viewportResizeObserver.disconnect()
    viewportResizeObserver = null
  }
  // 清理拖拽相关的 RAF
  if (dragRAF.value !== null) {
    cancelAnimationFrame(dragRAF.value)
    dragRAF.value = null
  }
  if (resizeRAF.value !== null) {
    cancelAnimationFrame(resizeRAF.value)
    resizeRAF.value = null
  }
  if (zoomRAF.value !== null) {
    cancelAnimationFrame(zoomRAF.value)
    zoomRAF.value = null
  }
  // 清理波形滚动 RAF
  if (waveformScrollRAF.value !== null) {
    cancelAnimationFrame(waveformScrollRAF.value)
    waveformScrollRAF.value = null
  }
  document.removeEventListener('mousemove', handleSubtitleDrag)
  document.removeEventListener('mouseup', handleSubtitleDragEnd)
  document.removeEventListener('mousemove', handleResize)
  document.removeEventListener('mouseup', handleResizeEnd)
  document.removeEventListener('mousemove', handleSelectionMove)
  document.removeEventListener('mouseup', handleSelectionEnd)
})

// 监听剪刀模式变化
watch(() => props.scissorMode, (isScissorMode) => {
  if (isScissorMode) {
    // 进入剪刀模式，添加鼠标移动监听
    trackAreaRef.value?.addEventListener('mousemove', handleScissorMouseMove)
    trackAreaRef.value?.addEventListener('mouseleave', handleScissorMouseLeave)
  } else {
    // 退出剪刀模式，移除监听并隐藏参考线
    trackAreaRef.value?.removeEventListener('mousemove', handleScissorMouseMove)
    trackAreaRef.value?.removeEventListener('mouseleave', handleScissorMouseLeave)
    scissorLineX.value = null
  }
})

// 请求删除选中的字幕
const requestDeleteSelectedSubtitles = () => {
  if (selectedSubtitleIds.value.size === 0) return
  const ids = Array.from(selectedSubtitleIds.value)
  emit('deleteSelectedSubtitles', ids)
}

// 清除选择
const clearSelection = () => {
  selectedSubtitleIds.value.clear()
  emit('selectSubtitles', [])
}

// 获取选中的字幕 ID
const getSelectedSubtitleIds = () => {
  return Array.from(selectedSubtitleIds.value)
}

// Expose methods to parent component
defineExpose({
  zoomIn,
  zoomOut,
  setZoom,
  fitToWidth,
  zoomLevel,
  requestDeleteSelectedSubtitles,
  clearSelection,
  getSelectedSubtitleIds
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
  flex-direction: row;
  min-height: 0;
  position: relative;
}

/* 左侧轨道控制面板 */
.track-controls {
  width: 52px;
  flex-shrink: 0;
  background: linear-gradient(180deg, #ffffff 0%, #f8fafc 100%);
  border-right: 1px solid #e2e8f0;
  display: flex;
  flex-direction: column;
  z-index: 60;
}

.track-control-header {
  height: 24px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-bottom: 1px solid #e2e8f0;
}

.track-control-items {
  display: flex;
  flex-direction: column;
  flex: 1;
}

.track-control-item {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 4px;
}

/* 在视觉上第一个的控制项下方添加分隔线 */
.track-control-item.has-separator {
  border-bottom: 1px solid #e2e8f0;
}

/* 轨道控制按钮 - 与侧边栏按钮风格一致 */
.track-visibility-btn {
  width: 36px;
  height: 36px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: transparent;
  border: none;
  border-radius: 10px;
  cursor: pointer;
  color: #94a3b8;
  transition: all 0.2s ease;
}

.track-visibility-btn svg {
  transition: all 0.2s ease;
}

.track-visibility-btn:hover {
  background: #f1f5f9;
}

.track-visibility-btn:hover svg {
  color: #64748b;
}

.track-control-item.disabled {
  opacity: 0.6;
}

/* 交换按钮 - 与侧边栏按钮风格一致 */
.track-swap-btn {
  width: 36px;
  height: 36px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: transparent;
  border: none;
  border-radius: 10px;
  cursor: pointer;
  color: #94a3b8;
  transition: all 0.2s ease;
}

.track-swap-btn svg {
  transition: all 0.2s ease;
}

.track-swap-btn:hover {
  background: #f1f5f9;
}

.track-swap-btn:hover svg {
  color: #64748b;
}

/* 轨道容器 - 支持 flexbox 排序 */
.tracks-container {
  display: flex;
  flex-direction: column;
}

/* 右侧主内容区域 */
.timeline-main-area {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-width: 0;
  position: relative;
}

/* 时间刻度尺 */
.time-ruler {
  height: 24px;
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

/* 主刻度 - 更明显的样式 */
.time-marker.main {
  border-left: 1px solid #94a3b8;
}

/* 次级刻度 - 只有短竖线 */
.time-marker.sub {
  border-left: 1px solid #d1d5db;
  height: 10px;
  top: auto;
  bottom: 0;
  padding-left: 0;
}

.time-label {
  font-size: 10px;
  color: #64748b;
  line-height: 24px;
  user-select: none;
}

/* 轨道区域 */
.timeline-track-area {
  flex: 1;
  overflow-x: overlay;
  overflow-y: hidden;
  position: relative;
  background: #f8fafc;
}

.timeline-content {
  position: relative;
}

/* 波形层 */
.waveform-layer {
  width: 100%;
  height: 80px;
  background: transparent;
  position: relative;
  overflow: visible;
}

/* 轨道禁用状态 - 覆盖灰色遮罩 */
.waveform-layer.track-disabled {
  pointer-events: none;
}

.waveform-layer.track-disabled::after {
  content: '';
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: rgba(100, 116, 139, 0.4);
  z-index: 10;
}

/* Canvas 波形 - 支持分段渲染动态定位 */
.waveform-canvas {
  position: absolute;
  top: 0;
  left: 0;
  height: 100%;
  /* width 和 left 由 JS 动态设置 */
}

/* 波形加载动画 - 只遮罩时间轴区域 */
.waveform-loading-overlay {
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: rgba(248, 250, 252, 0.85);
  backdrop-filter: blur(2px);
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 12px;
  z-index: 100;
  pointer-events: auto;
}

/* 波形条动画 */
.waveform-bars {
  display: flex;
  align-items: center;
  gap: 3px;
  height: 32px;
}

.waveform-bars .bar {
  width: 4px;
  background: linear-gradient(180deg, #3b82f6 0%, #60a5fa 100%);
  border-radius: 2px;
  animation: waveform-bounce 1s ease-in-out infinite;
}

.waveform-bars .bar:nth-child(1) { height: 12px; animation-delay: 0s; }
.waveform-bars .bar:nth-child(2) { height: 20px; animation-delay: 0.1s; }
.waveform-bars .bar:nth-child(3) { height: 28px; animation-delay: 0.2s; }
.waveform-bars .bar:nth-child(4) { height: 20px; animation-delay: 0.3s; }
.waveform-bars .bar:nth-child(5) { height: 12px; animation-delay: 0.4s; }

@keyframes waveform-bounce {
  0%, 100% { transform: scaleY(0.4); opacity: 0.6; }
  50% { transform: scaleY(1); opacity: 1; }
}

/* 进度信息 */
.loading-info {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 13px;
  color: #64748b;
}

.loading-progress {
  font-weight: 600;
  color: #3b82f6;
}

/* 悬浮高亮区域 - 显示字幕对应的波形范围 */
.subtitle-hover-highlight {
  position: absolute;
  top: 0;
  height: 100%; /* 填满波形层 */
  background: linear-gradient(
    180deg,
    rgba(59, 130, 246, 0.12) 0%,
    rgba(59, 130, 246, 0.18) 50%,
    rgba(59, 130, 246, 0.08) 100%
  );
  pointer-events: none;
  z-index: 5;
}

/* 字幕轨道 */
.subtitle-track {
  width: 100%;
  position: relative;
  background: #f8fafc;
  cursor: default;
  user-select: none;
  -webkit-user-select: none;
  transition: opacity 0.2s ease, filter 0.2s ease;
}

.subtitle-track:active {
  cursor: crosshair;
}

/* 字幕轨道禁用状态 - 覆盖灰色遮罩 */
.subtitle-track.track-disabled {
  pointer-events: none;
}

.subtitle-track.track-disabled::after {
  content: '';
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: rgba(100, 116, 139, 0.4);
  z-index: 10;
}

/* 字幕块 */
.subtitle-block {
  position: absolute;
  height: 32px;
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
  user-select: none;
  -webkit-user-select: none;
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
  width: 16px;
  background: #3b82f6;
  opacity: 0;
  cursor: col-resize;
  transition: opacity 0.2s;
  z-index: 30;
}

.resize-handle.left {
  left: -8px;
}

.resize-handle.right {
  right: -8px;
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

/* 滚动条样式 - 覆盖在内容上方 */
.timeline-track-area::-webkit-scrollbar {
  height: 6px;
  background: transparent;
}

.timeline-track-area::-webkit-scrollbar-track {
  background: transparent;
}

.timeline-track-area::-webkit-scrollbar-thumb {
  background: rgba(0, 0, 0, 0.25);
  border-radius: 3px;
}

.timeline-track-area::-webkit-scrollbar-thumb:hover {
  background: rgba(0, 0, 0, 0.4);
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

/* 选中数量提示 */
.selection-count {
  position: absolute;
  top: -8px;
  right: -8px;
  min-width: 18px;
  height: 18px;
  line-height: 18px;
  text-align: center;
  background: #3b82f6;
  color: white;
  font-size: 11px;
  font-weight: 500;
  padding: 0 5px;
  border-radius: 9px;
  box-shadow: 0 1px 4px rgba(0, 0, 0, 0.2);
  z-index: 10;
}

/* 剪刀模式 - 使用竖着的剪刀光标（通过 g transform 旋转） */
.subtitle-track.scissor-mode {
  cursor: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='24' height='24' viewBox='0 0 24 24'%3E%3Cg transform='rotate(90 12 12)'%3E%3Cpath fill='%23f56c6c' d='M9.64 7.64c.23-.5.36-1.05.36-1.64c0-2.21-1.79-4-4-4S2 3.79 2 6s1.79 4 4 4c.59 0 1.14-.13 1.64-.36L10 12l-2.36 2.36C7.14 14.13 6.59 14 6 14c-2.21 0-4 1.79-4 4s1.79 4 4 4s4-1.79 4-4c0-.59-.13-1.14-.36-1.64L12 14l7 7h3v-1L9.64 7.64zM6 8c-1.1 0-2-.89-2-2s.9-2 2-2s2 .89 2 2s-.9 2-2 2zm0 12c-1.1 0-2-.89-2-2s.9-2 2-2s2 .89 2 2s-.9 2-2 2zm6-7.5c-.28 0-.5-.22-.5-.5s.22-.5.5-.5s.5.22.5.5s-.22.5-.5.5zM19 3l-6 6l2 2l7-7V3h-3z'/%3E%3C/g%3E%3C/svg%3E") 12 12, crosshair;
}

.subtitle-block.scissor-mode {
  cursor: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='24' height='24' viewBox='0 0 24 24'%3E%3Cg transform='rotate(90 12 12)'%3E%3Cpath fill='%23f56c6c' d='M9.64 7.64c.23-.5.36-1.05.36-1.64c0-2.21-1.79-4-4-4S2 3.79 2 6s1.79 4 4 4c.59 0 1.14-.13 1.64-.36L10 12l-2.36 2.36C7.14 14.13 6.59 14 6 14c-2.21 0-4 1.79-4 4s1.79 4 4 4s4-1.79 4-4c0-.59-.13-1.14-.36-1.64L12 14l7 7h3v-1L9.64 7.64zM6 8c-1.1 0-2-.89-2-2s.9-2 2-2s2 .89 2 2s-.9 2-2 2zm0 12c-1.1 0-2-.89-2-2s.9-2 2-2s2 .89 2 2s-.9 2-2 2zm6-7.5c-.28 0-.5-.22-.5-.5s.22-.5.5-.5s.5.22.5.5s-.22.5-.5.5zM19 3l-6 6l2 2l7-7V3h-3z'/%3E%3C/g%3E%3C/svg%3E") 12 12, crosshair;
}

.subtitle-block.scissor-mode:hover {
  border-color: #f56c6c;
  box-shadow: 0 2px 8px rgba(245, 108, 108, 0.4);
}

/* 剪刀参考线 */
.scissor-line {
  position: absolute;
  top: 0;
  bottom: 0;
  width: 2px;
  pointer-events: none;
  z-index: 100;
  transform: translateX(-1px);
}

.scissor-line-bar {
  position: absolute;
  top: 30px;
  bottom: 0;
  width: 2px;
  background: repeating-linear-gradient(
    to bottom,
    #f56c6c 0px,
    #f56c6c 4px,
    transparent 4px,
    transparent 8px
  );
  box-shadow: 0 0 4px rgba(245, 108, 108, 0.4);
}

.scissor-icon {
  position: absolute;
  top: 8px;
  left: 50%;
  transform: translateX(-50%) rotate(90deg);
  font-size: 18px;
  color: #f56c6c;
  text-shadow: 0 0 4px rgba(245, 108, 108, 0.6);
}

/* 吸附参考线 */
.snap-line {
  position: absolute;
  top: 0;
  bottom: 0;
  width: 2px;
  pointer-events: none;
  z-index: 99;
  transform: translateX(-1px);
}

.snap-line-bar {
  position: absolute;
  top: 30px;
  bottom: 0;
  width: 2px;
  background: #10b981;
  box-shadow: 0 0 6px rgba(16, 185, 129, 0.6);
  animation: snap-pulse 0.6s ease-in-out infinite;
}

/* 波形吸附使用橙色 */
.snap-line.snap-waveform .snap-line-bar {
  background: #f59e0b;
  box-shadow: 0 0 6px rgba(245, 158, 11, 0.6);
  animation: snap-pulse-waveform 0.6s ease-in-out infinite;
}

@keyframes snap-pulse {
  0%, 100% {
    opacity: 1;
    box-shadow: 0 0 6px rgba(16, 185, 129, 0.6);
  }
  50% {
    opacity: 0.7;
    box-shadow: 0 0 10px rgba(16, 185, 129, 0.8);
  }
}

@keyframes snap-pulse-waveform {
  0%, 100% {
    opacity: 1;
    box-shadow: 0 0 6px rgba(245, 158, 11, 0.6);
  }
  50% {
    opacity: 0.7;
    box-shadow: 0 0 10px rgba(245, 158, 11, 0.8);
  }
}

/* 右键菜单样式 */
.context-menu {
  position: fixed;
  min-width: 140px;
  background: #ffffff;
  border: 1px solid #e2e8f0;
  border-radius: 8px;
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.12), 0 2px 4px rgba(0, 0, 0, 0.08);
  padding: 6px 0;
  z-index: 10000;
  font-size: 13px;
}

.context-menu-item {
  display: flex;
  align-items: center;
  padding: 8px 12px;
  cursor: pointer;
  transition: background-color 0.15s ease;
  color: #334155;
}

.context-menu-item:hover {
  background: #f1f5f9;
}

.context-menu-item.disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.context-menu-item.disabled:hover {
  background: transparent;
}

.context-menu-item.active {
  background: linear-gradient(135deg, #eff6ff 0%, #dbeafe 100%);
  color: #3b82f6;
}

.context-menu-label {
  flex: 1;
}

.context-menu-shortcut {
  margin-left: 16px;
  font-size: 11px;
  color: #94a3b8;
}

.context-menu-divider {
  height: 1px;
  background: #e2e8f0;
  margin: 4px 8px;
}

/* 🎄 圣诞节字幕块样式 */
:global(.christmas-season) .subtitle-block {
  background: linear-gradient(135deg, rgba(34, 197, 94, 0.85), rgba(22, 163, 74, 0.85)) !important;
  border-color: #16a34a !important;
}

:global(.christmas-season) .subtitle-block::after {
  content: '❄';
  position: absolute;
  right: 6px;
  top: 50%;
  transform: translateY(-50%);
  font-size: 12px;
  opacity: 0.8;
}

:global(.christmas-season) .subtitle-block:hover {
  border-color: #15803d !important;
  box-shadow: 0 2px 8px rgba(34, 197, 94, 0.4) !important;
}

:global(.christmas-season) .subtitle-block.is-active,
:global(.christmas-season) .subtitle-block.is-selected {
  background: linear-gradient(135deg, rgba(220, 38, 38, 0.9), rgba(185, 28, 28, 0.9)) !important;
  border-color: #dc2626 !important;
  outline-color: #dc2626 !important;
}

:global(.christmas-season) .subtitle-block.is-active::after,
:global(.christmas-season) .subtitle-block.is-selected::after {
  content: '🎄';
}

:global(.christmas-season) .subtitle-block.is-selected:hover {
  outline-color: #b91c1c !important;
}
</style>

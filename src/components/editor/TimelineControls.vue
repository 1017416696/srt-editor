<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useAudioStore } from '@/stores/audio'

const props = defineProps<{
  zoomLevel: number
}>()

const emit = defineEmits<{
  (e: 'zoom-change', value: number): void
  (e: 'zoom-reset'): void
  (e: 'remove-audio'): void
}>()

const audioStore = useAudioStore()

// 缩放滑块值
const zoomSliderValue = ref(props.zoomLevel)

// 同步滑块值与实际缩放级别
watch(() => props.zoomLevel, (newVal) => {
  zoomSliderValue.value = newVal
})

// 滑块变化时更新缩放
const handleZoomSliderChange = (value: number) => {
  emit('zoom-change', value)
}

// 双击缩放显示区域重置到适合屏幕宽度
const handleZoomReset = () => {
  emit('zoom-reset')
}

const hasAudio = computed(() => audioStore.currentAudio !== null)
</script>

<template>
  <!-- 一体化控制栏：音频名称 + 缩放 + 播放 + 时长 + 音量 + 速度 -->
  <div v-if="hasAudio" class="timeline-unified-controls timeline-controls">
    <!-- 左侧组：文件信息 + 缩放 -->
    <div class="controls-left">
      <div class="audio-file-info">
        <svg class="audio-icon" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M9 18V5l12-2v13" stroke-linecap="round" stroke-linejoin="round"/>
          <circle cx="6" cy="18" r="3"/>
          <circle cx="18" cy="16" r="3"/>
        </svg>
        <span class="audio-name-compact">{{ audioStore.currentAudio?.name }}</span>
        <button class="remove-audio-btn" @click="emit('remove-audio')" title="移除音频">
          <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M18 6L6 18M6 6l12 12" stroke-linecap="round"/>
          </svg>
        </button>
      </div>

      <div class="control-group">
        <svg class="control-icon" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <circle cx="11" cy="11" r="8"/>
          <path d="M21 21l-4.35-4.35" stroke-linecap="round"/>
          <path d="M8 11h6M11 8v6" stroke-linecap="round"/>
        </svg>
        <div class="zoom-slider-container" @dblclick.prevent="handleZoomReset" title="双击重置缩放">
          <el-slider
            v-model="zoomSliderValue"
            :min="25"
            :max="200"
            :step="5"
            :show-tooltip="false"
            class="zoom-slider-mini"
            @input="handleZoomSliderChange"
          />
        </div>
        <span class="param-value">{{ zoomLevel }}%</span>
      </div>
    </div>

    <!-- 中间播放控制（居中）-->
    <div class="controls-center">
      <span class="time-current">{{ audioStore.formatTime(audioStore.playerState.currentTime) }}</span>
      <button
        class="play-btn"
        :class="{ playing: audioStore.playerState.isPlaying }"
        @click="audioStore.togglePlay()"
      >
        <svg v-if="audioStore.playerState.isPlaying" width="16" height="16" viewBox="0 0 24 24" fill="currentColor">
          <rect x="6" y="4" width="4" height="16" rx="1" />
          <rect x="14" y="4" width="4" height="16" rx="1" />
        </svg>
        <svg v-else width="16" height="16" viewBox="0 0 24 24" fill="currentColor">
          <path d="M8 5v14l11-7z" />
        </svg>
      </button>
      <span class="time-total">{{ audioStore.formatTime(audioStore.playerState.duration) }}</span>
    </div>

    <!-- 右侧组：音量 + 速度 -->
    <div class="controls-right">
      <div class="control-group">
        <svg class="control-icon" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5"/>
          <path d="M15.54 8.46a5 5 0 0 1 0 7.07M19.07 4.93a10 10 0 0 1 0 14.14" stroke-linecap="round"/>
        </svg>
        <el-slider
          v-model="audioStore.playerState.volume"
          :max="1"
          :step="0.01"
          :show-tooltip="false"
          class="volume-slider-mini"
          @input="(val: number) => audioStore.setVolume(val)"
        />
      </div>

      <div class="speed-group">
        <button
          v-for="rate in [0.5, 1, 1.5, 2]"
          :key="rate"
          class="speed-btn"
          :class="{ active: audioStore.playerState.playbackRate === rate }"
          @click="audioStore.setPlaybackRate(rate)"
        >
          {{ rate }}x
        </button>
      </div>
    </div>
  </div>

  <!-- 波形生成时的简化控制栏 -->
  <div v-else-if="audioStore.isGeneratingWaveform" class="timeline-unified-controls loading-controls">
    <span class="loading-audio-text">正在加载音频...</span>
  </div>
</template>

<style scoped>
/* 一体化控制栏：三栏布局（左、中、右）*/
.timeline-unified-controls {
  display: grid;
  grid-template-columns: 1fr auto 1fr;
  align-items: center;
  padding: 0.5rem 1.25rem;
  background: linear-gradient(180deg, #ffffff 0%, #f8fafc 100%);
  border-bottom: 1px solid #e2e8f0;
  gap: 2rem;
  min-height: 44px;
}

.controls-left {
  display: flex;
  align-items: center;
  gap: 1.5rem;
  justify-content: flex-start;
}

.controls-center {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  justify-content: center;
}

.controls-right {
  display: flex;
  align-items: center;
  gap: 1.25rem;
  justify-content: flex-end;
}

/* 音频文件信息 */
.audio-file-info {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  padding: 0.25rem 0.5rem 0.25rem 0.625rem;
  background: #f1f5f9;
  border-radius: 6px;
  max-width: 180px;
}

.audio-icon {
  color: #64748b;
  flex-shrink: 0;
}

.audio-name-compact {
  font-size: 0.8125rem;
  font-weight: 500;
  color: #334155;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  user-select: none;
  -webkit-user-select: none;
}

.remove-audio-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 20px;
  height: 20px;
  background: transparent;
  border: none;
  border-radius: 4px;
  cursor: pointer;
  color: #94a3b8;
  transition: all 0.15s ease;
  flex-shrink: 0;
}

.remove-audio-btn:hover {
  background: #fee2e2;
  color: #ef4444;
}

/* 控制组 */
.control-group {
  display: flex;
  align-items: center;
  gap: 0.5rem;
}

.control-icon {
  color: #94a3b8;
  flex-shrink: 0;
}

/* 缩放滑块容器 */
.zoom-slider-container {
  display: flex;
  align-items: center;
  cursor: pointer;
}

.zoom-slider-mini {
  width: 80px;
}

/* 统一滑块样式 */
.zoom-slider-mini :deep(.el-slider__button),
.volume-slider-mini :deep(.el-slider__button) {
  width: 12px;
  height: 12px;
  border-width: 2px;
  border-color: #3b82f6;
  background: #fff;
}

.zoom-slider-mini :deep(.el-slider__runway),
.volume-slider-mini :deep(.el-slider__runway) {
  height: 4px;
  background: #e2e8f0;
}

.zoom-slider-mini :deep(.el-slider__bar),
.volume-slider-mini :deep(.el-slider__bar) {
  background: #3b82f6;
}

.param-value {
  font-size: 0.75rem;
  font-weight: 500;
  color: #64748b;
  min-width: 32px;
  text-align: right;
  user-select: none;
  font-family: 'SF Mono', 'Monaco', 'Consolas', monospace;
}

/* 播放控制 */
.time-current,
.time-total {
  font-size: 0.8125rem;
  font-family: 'SF Mono', 'Monaco', 'Consolas', monospace;
  font-weight: 500;
  min-width: 48px;
  user-select: none;
  -webkit-user-select: none;
}

.time-current {
  color: #3b82f6;
  text-align: right;
}

.time-total {
  color: #94a3b8;
  text-align: left;
}

.play-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 36px;
  height: 36px;
  background: linear-gradient(135deg, #3b82f6 0%, #2563eb 100%);
  border: none;
  border-radius: 50%;
  cursor: pointer;
  color: #fff;
  transition: all 0.2s ease;
  box-shadow: 0 2px 8px rgba(59, 130, 246, 0.3);
}

.play-btn:hover {
  transform: scale(1.05);
  box-shadow: 0 4px 12px rgba(59, 130, 246, 0.4);
}

.play-btn:active {
  transform: scale(0.98);
}

.play-btn.playing {
  background: linear-gradient(135deg, #6366f1 0%, #4f46e5 100%);
  box-shadow: 0 2px 8px rgba(99, 102, 241, 0.3);
}

/* 音量滑块 */
.volume-slider-mini {
  width: 80px;
}

/* 速度按钮组 */
.speed-group {
  display: flex;
  align-items: center;
  background: #f1f5f9;
  border-radius: 6px;
  padding: 2px;
  gap: 2px;
}

.speed-btn {
  padding: 0.25rem 0.5rem;
  font-size: 0.75rem;
  font-weight: 500;
  color: #64748b;
  background: transparent;
  border: none;
  border-radius: 4px;
  cursor: pointer;
  transition: all 0.15s ease;
  white-space: nowrap;
}

.speed-btn:hover {
  color: #334155;
  background: #e2e8f0;
}

.speed-btn.active {
  color: #fff;
  background: linear-gradient(135deg, #3b82f6 0%, #2563eb 100%);
  box-shadow: 0 1px 3px rgba(59, 130, 246, 0.3);
}

.loading-controls {
  display: flex;
  justify-content: center;
}

.loading-audio-text {
  font-size: 0.875rem;
  color: #64748b;
}
</style>

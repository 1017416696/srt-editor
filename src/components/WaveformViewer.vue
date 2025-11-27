<template>
  <div class="waveform-container">
    <div ref="waveformRef" class="waveform"></div>
    <div v-if="loading" class="loading-overlay">
      <el-icon class="is-loading">
        <Loading />
      </el-icon>
      <span>生成波形中...</span>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch } from 'vue'
import { Loading } from '@element-plus/icons-vue'
import WaveSurfer from 'wavesurfer.js'
import type { SubtitleEntry } from '@/types/subtitle'

interface Props {
  audioUrl?: string
  waveformData?: number[]
  currentTime?: number
  duration?: number
  subtitles?: SubtitleEntry[]
  height?: number
}

const props = withDefaults(defineProps<Props>(), {
  height: 128,
  currentTime: 0,
  duration: 0,
  subtitles: () => []
})

const emit = defineEmits<{
  seek: [time: number]
}>()

const waveformRef = ref<HTMLDivElement | null>(null)
const wavesurfer = ref<WaveSurfer | null>(null)
const loading = ref(false)

// Initialize WaveSurfer
const initWaveSurfer = () => {
  if (!waveformRef.value) {
    console.error('❌ Waveform container ref not found!')
    return
  }

  console.log('🎨 Initializing WaveSurfer...')

  try {
    wavesurfer.value = WaveSurfer.create({
      container: waveformRef.value,
      waveColor: '#4a9eff',
      progressColor: '#1e40af',
      cursorColor: '#ef4444',
      barWidth: 2,
      barGap: 1,
      barRadius: 2,
      height: props.height,
      normalize: true,
      interact: true,
    })

    console.log('✅ WaveSurfer initialized successfully!')

    // Handle seeking
    wavesurfer.value.on('click', (relativeX) => {
      const time = relativeX * props.duration
      console.log('🖱️ Waveform clicked, seeking to:', time)
      emit('seek', time)
    })

    wavesurfer.value.on('ready', () => {
      console.log('✅ WaveSurfer ready!')
    })

    wavesurfer.value.on('error', (error) => {
      console.error('❌ WaveSurfer error:', error)
    })
  } catch (error) {
    console.error('❌ Failed to initialize WaveSurfer:', error)
  }
}

// Load waveform from pre-computed data
const loadWaveformData = (data: number[]) => {
  if (!wavesurfer.value || !data || data.length === 0) {
    console.warn('⚠️ Cannot load waveform:', {
      hasWavesurfer: !!wavesurfer.value,
      hasData: !!data,
      dataLength: data?.length
    })
    return
  }

  loading.value = true
  console.log('📥 Loading waveform data...', { dataPoints: data.length })

  try {
    // WaveSurfer v7 expects peaks data in a specific format
    // We need to convert our amplitude data to peaks format
    const peaks = new Float32Array(data.length)
    for (let i = 0; i < data.length; i++) {
      peaks[i] = data[i]
    }

    // Load peaks directly (WaveSurfer v7 API)
    wavesurfer.value.load('', [peaks], props.duration || 1)
    console.log('✅ Waveform loaded successfully!')
  } catch (error) {
    console.error('❌ Failed to load waveform data:', error)
  } finally {
    loading.value = false
  }
}

// Load audio from URL (fallback)
const loadAudioUrl = async (url: string) => {
  if (!wavesurfer.value || !url) return

  loading.value = true

  try {
    await wavesurfer.value.load(url)
  } catch (error) {
    console.error('Failed to load audio URL:', error)
  } finally {
    loading.value = false
  }
}

// Update playback position
const updatePosition = (time: number) => {
  if (!wavesurfer.value || !props.duration) return
  const progress = time / props.duration
  wavesurfer.value.seekTo(progress)
}

// Watch for waveform data changes
watch(() => props.waveformData, (data) => {
  if (wavesurfer.value && data && data.length > 0) {
    loadWaveformData(data)
  }
}, { immediate: false })

// Watch for audio URL changes (fallback)
watch(() => props.audioUrl, (url) => {
  if (wavesurfer.value && url && (!props.waveformData || props.waveformData.length === 0)) {
    loadAudioUrl(url)
  }
})

// Watch for current time changes
watch(() => props.currentTime, (time) => {
  updatePosition(time)
})

onMounted(() => {
  initWaveSurfer()

  // Load waveform data if available after initialization
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
})
</script>

<style scoped>
.waveform-container {
  position: relative;
  width: 100%;
  background: linear-gradient(to bottom, #f8fafc 0%, #f1f5f9 100%);
  border-radius: 8px;
  overflow: hidden;
  box-shadow: inset 0 2px 4px rgba(0, 0, 0, 0.06);
}

.waveform {
  width: 100%;
  cursor: pointer;
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

.loading-overlay .el-icon {
  font-size: 24px;
}

/* Dark mode support */
@media (prefers-color-scheme: dark) {
  .waveform-container {
    background: linear-gradient(to bottom, #1e293b 0%, #0f172a 100%);
  }

  .loading-overlay {
    background: rgba(15, 23, 42, 0.9);
    color: #94a3b8;
  }
}
</style>

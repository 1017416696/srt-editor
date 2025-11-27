import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import type { AudioFile, PlayerState } from '@/types/subtitle'
import { Howl } from 'howler'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'

export const useAudioStore = defineStore('audio', () => {
  // 状态
  const audioFile = ref<AudioFile | null>(null)
  const howl = ref<Howl | null>(null)
  const isGeneratingWaveform = ref(false)
  const waveformProgress = ref(0) // 0-100

  const playerState = ref<PlayerState>({
    isPlaying: false,
    currentTime: 0,
    duration: 0,
    volume: 1,
    playbackRate: 1,
  })

  // 计算属性
  const isLoaded = computed(() => audioFile.value !== null && howl.value !== null)
  const progress = computed(() => {
    if (playerState.value.duration === 0) return 0
    return (playerState.value.currentTime / playerState.value.duration) * 100
  })

  // 全局事件监听器（在模块级别设置，避免重复设置）
  let waveformProgressUnlisten: (() => void) | null = null

  // 加载音频文件
  const loadAudio = async (file: AudioFile) => {
    // 清理现有音频
    if (howl.value) {
      howl.value.unload()
    }

    // 设置波形生成状态
    isGeneratingWaveform.value = true
    waveformProgress.value = 0
    
    // 设置事件监听器
    try {
      if (waveformProgressUnlisten) {
        waveformProgressUnlisten()
        waveformProgressUnlisten = null
      }
      
      waveformProgressUnlisten = await listen<number>('waveform-progress', (event) => {
        const progress = Math.round(event.payload * 100)
        waveformProgress.value = progress
      })
    } catch (error) {
      console.error('Failed to set up waveform progress listener:', error)
    }
    
    // 设置音频文件（先清空波形数据）
    audioFile.value = {
      ...file,
      waveform: undefined
    }

    return new Promise<void>(async (resolve, reject) => {
      try {
        // 调用 Tauri 后端读取文件为 base64，避免路径编码问题
        const fileBase64 = await invoke<string>('read_audio_file', { filePath: file.path })

        // 在后台异步生成波形数据（不阻塞音频加载）
        // 注意：isGeneratingWaveform 已经在函数开始时设置为 true
        generateWaveform(file.path).catch((error) => {
          console.error('❌ Waveform generation failed:', error)
          isGeneratingWaveform.value = false
        })

        // 将 base64 转换为 Blob
        const binaryString = atob(fileBase64)
        const bytes = new Uint8Array(binaryString.length)
        for (let i = 0; i < binaryString.length; i++) {
          bytes[i] = binaryString.charCodeAt(i)
        }
        const blob = new Blob([bytes], { type: `audio/${file.format}` })

        // 创建 Object URL
        const audioUrl = URL.createObjectURL(blob)

        howl.value = new Howl({
          src: [audioUrl],
          html5: true,
          format: [file.format],
          volume: playerState.value.volume,
          rate: playerState.value.playbackRate,
          preload: true,
          onload: () => {
            if (howl.value) {
              playerState.value.duration = howl.value.duration()
            }
            resolve()
          },
          onloaderror: (_id, error) => {
            console.error('Audio load error:', error)
            URL.revokeObjectURL(audioUrl)
            reject(new Error(`Failed to load audio: ${error}`))
          },
          onplay: () => {
            playerState.value.isPlaying = true
            updateProgress()
          },
          onpause: () => {
            playerState.value.isPlaying = false
          },
          onstop: () => {
            playerState.value.isPlaying = false
            playerState.value.currentTime = 0
          },
          onend: () => {
            playerState.value.isPlaying = false
            playerState.value.currentTime = 0
          },
        })
      } catch (error) {
        console.error('Failed to read file:', error)
        reject(new Error(`Failed to read audio file: ${error}`))
      }
    })
  }

  // 更新播放进度
  let progressInterval: number | null = null
  const updateProgress = () => {
    if (progressInterval) {
      clearInterval(progressInterval)
    }

    progressInterval = window.setInterval(() => {
      if (howl.value && playerState.value.isPlaying) {
        playerState.value.currentTime = howl.value.seek() as number
      }
    }, 100)
  }

  // 播放/暂停
  const togglePlay = () => {
    if (!howl.value) return

    if (playerState.value.isPlaying) {
      howl.value.pause()
    } else {
      howl.value.play()
    }
  }

  // 播放
  const play = () => {
    if (!howl.value) return
    howl.value.play()
  }

  // 暂停
  const pause = () => {
    if (!howl.value) return
    howl.value.pause()
  }

  // 停止
  const stop = () => {
    if (!howl.value) return
    howl.value.stop()
  }

  // 跳转到指定时间
  const seek = (time: number) => {
    if (!howl.value) return
    howl.value.seek(time)
    playerState.value.currentTime = time
  }

  // 设置音量
  const setVolume = (volume: number) => {
    playerState.value.volume = Math.max(0, Math.min(1, volume))
    if (howl.value) {
      howl.value.volume(playerState.value.volume)
    }
  }

  // 增加音量
  const increaseVolume = () => {
    setVolume(playerState.value.volume + 0.1)
  }

  // 减少音量
  const decreaseVolume = () => {
    setVolume(playerState.value.volume - 0.1)
  }

  // 设置播放速率
  const setPlaybackRate = (rate: number) => {
    playerState.value.playbackRate = Math.max(0.5, Math.min(2, rate))
    if (howl.value) {
      howl.value.rate(playerState.value.playbackRate)
    }
  }

    // 卸载音频文件
  const unloadAudio = () => {
    if (progressInterval) {
      clearInterval(progressInterval)
      progressInterval = null
    }

    if (howl.value) {
      howl.value.unload()
      howl.value = null
    }

    // 清理事件监听器
    if (waveformProgressUnlisten) {
      waveformProgressUnlisten()
      waveformProgressUnlisten = null
    }

    audioFile.value = null
    isGeneratingWaveform.value = false
    waveformProgress.value = 0
    playerState.value = {
      isPlaying: false,
      currentTime: 0,
      duration: 0,
      volume: 1,
      playbackRate: 1,
    }
  }

  // 清理
  const cleanup = () => {
    unloadAudio()
  }

  // 格式化时间为字符串
  const formatTime = (seconds: number): string => {
    const mins = Math.floor(seconds / 60)
    const secs = Math.floor(seconds % 60)
    return `${mins.toString().padStart(2, '0')}:${secs.toString().padStart(2, '0')}`
  }

  // 生成波形数据
  const generateWaveform = async (filePath: string) => {
    // 确保状态已设置
    if (!isGeneratingWaveform.value) {
      isGeneratingWaveform.value = true
    }
    if (waveformProgress.value !== 0) {
      waveformProgress.value = 0
    }
    
    try {
      // 使用固定的高密度数据点，确保任何缩放级别下都有足够的密度
      // 20000 个数据点对于大多数音频足够了
      const targetSamples = 20000
      
      const waveform = await invoke<number[]>('generate_audio_waveform', {
        filePath,
        targetSamples
      })
      
      if (audioFile.value && waveform) {
        audioFile.value.waveform = waveform
      }
    } catch (error) {
      console.error('Waveform generation failed:', error)
    } finally {
      isGeneratingWaveform.value = false
      waveformProgress.value = 100
      if (waveformProgressUnlisten) {
        waveformProgressUnlisten()
        waveformProgressUnlisten = null
      }
    }
  }

  // 当前音频文件的引用（为了兼容性）
  const currentAudio = computed(() => audioFile.value)

  return {
    // 状态
    audioFile,
    playerState,
    isLoaded,
    progress,
    currentAudio,
    isGeneratingWaveform,
    waveformProgress,

    // 方法
    loadAudio,
    unloadAudio,
    togglePlay,
    play,
    pause,
    stop,
    seek,
    setVolume,
    increaseVolume,
    decreaseVolume,
    setPlaybackRate,
    cleanup,
    formatTime,
    generateWaveform,
  }
})

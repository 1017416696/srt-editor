import { defineStore } from 'pinia'
import { ref, computed, watch } from 'vue'
import type { AudioFile, PlayerState } from '@/types/subtitle'
import { Howl } from 'howler'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { useTabManagerStore } from '@/stores/tabManager'
import logger from '@/utils/logger'

// Howl 缓存项
interface HowlCacheItem {
  howl: Howl
  audioUrl: string
  lastUsed: number
}

// 最大缓存数量
const MAX_CACHED_HOWLS = 3

export const useAudioStore = defineStore('audio', () => {
  const tabManager = useTabManagerStore()

  // Howl 实例缓存：key 是 tabId
  const howlCache = new Map<string, HowlCacheItem>()
  
  // 当前使用的 Howl 实例
  const currentHowl = ref<Howl | null>(null)
  const currentTabId = ref<string | null>(null)
  
  const isGeneratingWaveform = ref(false)
  const waveformProgress = ref(0)

  // 播放状态
  const playerState = ref<PlayerState>({
    isPlaying: false,
    currentTime: 0,
    duration: 0,
    volume: 1,
    playbackRate: 1,
  })

  // 从当前 tab 获取音频信息
  const audioFile = computed(() => {
    const tab = tabManager.activeTab
    if (!tab || !tab.audio.filePath) return null
    return {
      name: tab.audio.fileName || '',
      path: tab.audio.filePath,
      duration: tab.audio.duration,
      format: tab.audio.format || 'mp3',
      waveform: tab.audio.waveform || undefined,
    } as AudioFile
  })

  const isLoaded = computed(() => audioFile.value !== null && currentHowl.value !== null)
  const progress = computed(() => {
    if (playerState.value.duration === 0) return 0
    return (playerState.value.currentTime / playerState.value.duration) * 100
  })
  const currentAudio = computed(() => audioFile.value)

  // 事件监听器
  let waveformProgressUnlisten: (() => void) | null = null
  let progressSimulationTimer: number | null = null
  let progressInterval: number | null = null

  // 清理超出限制的缓存
  const cleanupCache = () => {
    if (howlCache.size <= MAX_CACHED_HOWLS) return

    // 找出最久未使用的缓存项（排除当前使用的）
    let oldestKey: string | null = null
    let oldestTime = Infinity

    howlCache.forEach((item, key) => {
      if (key !== currentTabId.value && item.lastUsed < oldestTime) {
        oldestTime = item.lastUsed
        oldestKey = key
      }
    })

    if (oldestKey) {
      const item = howlCache.get(oldestKey)
      if (item) {
        logger.info('释放音频缓存', { tabId: oldestKey })
        item.howl.unload()
        URL.revokeObjectURL(item.audioUrl)
        howlCache.delete(oldestKey)
      }
    }
  }

  // 从缓存获取或创建 Howl 实例
  const getOrCreateHowl = async (tabId: string, filePath: string, format: string): Promise<Howl> => {
    // 检查缓存
    const cached = howlCache.get(tabId)
    if (cached) {
      cached.lastUsed = Date.now()
      logger.info('使用缓存的音频实例', { tabId })
      return cached.howl
    }

    // 创建新实例
    logger.info('创建新的音频实例', { tabId, filePath })
    const fileBase64 = await invoke<string>('read_audio_file', { filePath })
    
    const binaryString = atob(fileBase64)
    const bytes = new Uint8Array(binaryString.length)
    for (let i = 0; i < binaryString.length; i++) {
      bytes[i] = binaryString.charCodeAt(i)
    }
    const blob = new Blob([bytes], { type: `audio/${format}` })
    const audioUrl = URL.createObjectURL(blob)

    return new Promise((resolve, reject) => {
      const howl = new Howl({
        src: [audioUrl],
        html5: true,
        format: [format],
        volume: playerState.value.volume,
        rate: playerState.value.playbackRate,
        preload: true,
        onload: () => {
          // 加入缓存
          howlCache.set(tabId, {
            howl,
            audioUrl,
            lastUsed: Date.now(),
          })
          // 清理超出限制的缓存
          cleanupCache()
          resolve(howl)
        },
        onloaderror: (_id, error) => {
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
        },
        onend: () => {
          playerState.value.isPlaying = false
        },
      })
    })
  }

  // 切换到指定 tab 的音频
  const switchToTabAudio = async (tabId: string) => {
    const tab = tabManager.tabs.find(t => t.id === tabId)
    if (!tab || !tab.audio.filePath) {
      // 该 tab 没有音频
      if (currentHowl.value) {
        currentHowl.value.stop()
      }
      currentHowl.value = null
      currentTabId.value = null
      playerState.value.isPlaying = false
      playerState.value.currentTime = 0
      playerState.value.duration = 0
      return
    }

    // 停止当前播放
    if (currentHowl.value) {
      currentHowl.value.stop()
    }

    try {
      const howl = await getOrCreateHowl(tabId, tab.audio.filePath, tab.audio.format || 'mp3')
      
      currentHowl.value = howl
      currentTabId.value = tabId
      
      // 恢复状态
      playerState.value.volume = tab.audio.volume
      playerState.value.playbackRate = tab.audio.playbackRate
      playerState.value.duration = howl.duration()
      playerState.value.currentTime = tab.audio.currentTime || 0
      playerState.value.isPlaying = false
      
      // 应用音量和播放速度
      howl.volume(playerState.value.volume)
      howl.rate(playerState.value.playbackRate)
      
      // 恢复播放位置
      if (tab.audio.currentTime > 0) {
        howl.seek(tab.audio.currentTime)
      }
    } catch (error) {
      logger.error('切换音频失败', { tabId, error: String(error) })
      currentHowl.value = null
      currentTabId.value = null
    }
  }

  // 监听 tab 切换
  watch(() => tabManager.activeTabId, async (newTabId, oldTabId) => {
    if (newTabId === oldTabId || !newTabId) return
    
    // 保存旧 tab 的播放位置
    if (oldTabId && currentHowl.value) {
      const oldTab = tabManager.tabs.find(t => t.id === oldTabId)
      if (oldTab) {
        oldTab.audio.currentTime = currentHowl.value.seek() as number
      }
    }
    
    await switchToTabAudio(newTabId)
  })


  // 加载音频文件
  const loadAudio = async (file: AudioFile) => {
    const loadStartTime = Date.now()
    const tabId = tabManager.activeTabId
    if (!tabId) return

    // 如果该 tab 已有缓存，先清除
    const existingCache = howlCache.get(tabId)
    if (existingCache) {
      existingCache.howl.unload()
      URL.revokeObjectURL(existingCache.audioUrl)
      howlCache.delete(tabId)
    }

    // 停止当前播放
    if (currentHowl.value) {
      currentHowl.value.stop()
    }

    // 设置波形生成状态
    isGeneratingWaveform.value = true
    waveformProgress.value = 0
    
    let lastRealProgress = 0
    
    const startProgressSimulation = () => {
      if (progressSimulationTimer) {
        clearInterval(progressSimulationTimer)
      }
      progressSimulationTimer = window.setInterval(() => {
        if (waveformProgress.value < 99 && waveformProgress.value >= lastRealProgress) {
          waveformProgress.value = Math.min(99, waveformProgress.value + 0.5)
        }
      }, 200)
    }
    
    try {
      if (waveformProgressUnlisten) {
        waveformProgressUnlisten()
        waveformProgressUnlisten = null
      }
      
      waveformProgressUnlisten = await listen<number>('waveform-progress', (event) => {
        const progress = Math.round(event.payload * 100)
        lastRealProgress = progress
        if (progress > waveformProgress.value) {
          waveformProgress.value = progress
        }
      })
      
      startProgressSimulation()
    } catch (error) {
      logger.error('设置波形进度监听器失败', { error: String(error) })
    }
    
    // 更新当前 tab 的音频状态
    if (tabManager.activeTab) {
      tabManager.activeTab.audio.filePath = file.path
      tabManager.activeTab.audio.fileName = file.name
      tabManager.activeTab.audio.format = file.format
      tabManager.activeTab.audio.waveform = null
    }

    try {
      // 异步生成波形
      generateWaveform(file.path).catch((error) => {
        console.error('❌ Waveform generation failed:', error)
        isGeneratingWaveform.value = false
      })

      const howl = await getOrCreateHowl(tabId, file.path, file.format)
      
      currentHowl.value = howl
      currentTabId.value = tabId
      
      playerState.value.duration = howl.duration()
      playerState.value.currentTime = 0
      playerState.value.isPlaying = false
      
      // 同步到 tab
      if (tabManager.activeTab) {
        tabManager.activeTab.audio.duration = howl.duration()
      }
      
      const loadDuration = Date.now() - loadStartTime
      logger.info('音频加载完成', {
        path: file.path,
        duration: howl.duration(),
        loadTime: `${loadDuration}ms`,
      })
    } catch (error) {
      logger.error('读取音频文件失败', { path: file.path, error: String(error) })
      throw error
    }
  }

  // 更新播放进度
  const updateProgress = () => {
    if (progressInterval) {
      clearInterval(progressInterval)
    }

    progressInterval = window.setInterval(() => {
      if (currentHowl.value && playerState.value.isPlaying) {
        playerState.value.currentTime = currentHowl.value.seek() as number
        // 同步到 tab
        if (tabManager.activeTab) {
          tabManager.activeTab.audio.currentTime = playerState.value.currentTime
        }
      }
    }, 100)
  }

  // 播放控制
  const togglePlay = () => {
    if (!currentHowl.value) return
    if (playerState.value.isPlaying) {
      currentHowl.value.pause()
    } else {
      currentHowl.value.play()
    }
  }

  const play = () => {
    if (!currentHowl.value) return
    currentHowl.value.play()
  }

  const pause = () => {
    if (!currentHowl.value) return
    currentHowl.value.pause()
  }

  const stop = () => {
    if (!currentHowl.value) return
    currentHowl.value.stop()
  }

  const seek = (time: number) => {
    if (!currentHowl.value) return
    currentHowl.value.seek(time)
    playerState.value.currentTime = time
  }

  const setVolume = (volume: number) => {
    playerState.value.volume = Math.max(0, Math.min(1, volume))
    if (currentHowl.value) {
      currentHowl.value.volume(playerState.value.volume)
    }
    // 同步到 tab
    if (tabManager.activeTab) {
      tabManager.activeTab.audio.volume = playerState.value.volume
    }
  }

  const increaseVolume = () => {
    setVolume(playerState.value.volume + 0.1)
  }

  const decreaseVolume = () => {
    setVolume(playerState.value.volume - 0.1)
  }

  const setPlaybackRate = (rate: number) => {
    playerState.value.playbackRate = Math.max(0.5, Math.min(2, rate))
    if (currentHowl.value) {
      currentHowl.value.rate(playerState.value.playbackRate)
    }
    // 同步到 tab
    if (tabManager.activeTab) {
      tabManager.activeTab.audio.playbackRate = playerState.value.playbackRate
    }
  }

  // 卸载当前 tab 的音频
  const unloadAudio = () => {
    if (progressInterval) {
      clearInterval(progressInterval)
      progressInterval = null
    }

    if (progressSimulationTimer) {
      clearInterval(progressSimulationTimer)
      progressSimulationTimer = null
    }

    // 从缓存中移除当前 tab 的音频
    if (currentTabId.value) {
      const cached = howlCache.get(currentTabId.value)
      if (cached) {
        cached.howl.unload()
        URL.revokeObjectURL(cached.audioUrl)
        howlCache.delete(currentTabId.value)
      }
    }

    currentHowl.value = null
    currentTabId.value = null

    if (waveformProgressUnlisten) {
      waveformProgressUnlisten()
      waveformProgressUnlisten = null
    }

    // 清除当前 tab 的音频状态
    if (tabManager.activeTab) {
      tabManager.activeTab.audio.filePath = null
      tabManager.activeTab.audio.fileName = null
      tabManager.activeTab.audio.format = null
      tabManager.activeTab.audio.waveform = null
      tabManager.activeTab.audio.duration = 0
      tabManager.activeTab.audio.currentTime = 0
    }

    isGeneratingWaveform.value = false
    waveformProgress.value = 0
    playerState.value = {
      isPlaying: false,
      currentTime: 0,
      duration: 0,
      volume: playerState.value.volume,
      playbackRate: playerState.value.playbackRate,
    }
  }

  // 清理所有缓存
  const cleanup = () => {
    howlCache.forEach((item) => {
      item.howl.unload()
      URL.revokeObjectURL(item.audioUrl)
    })
    howlCache.clear()
    unloadAudio()
  }

  // 当 tab 关闭时，清理对应的缓存
  const removeTabCache = (tabId: string) => {
    const cached = howlCache.get(tabId)
    if (cached) {
      cached.howl.unload()
      URL.revokeObjectURL(cached.audioUrl)
      howlCache.delete(tabId)
      logger.info('清理已关闭 tab 的音频缓存', { tabId })
    }
  }

  const formatTime = (seconds: number): string => {
    const mins = Math.floor(seconds / 60)
    const secs = Math.floor(seconds % 60)
    return `${mins.toString().padStart(2, '0')}:${secs.toString().padStart(2, '0')}`
  }


  // 生成波形数据
  const generateWaveform = async (filePath: string) => {
    const waveformStartTime = Date.now()
    
    if (!isGeneratingWaveform.value) {
      isGeneratingWaveform.value = true
    }
    if (waveformProgress.value !== 0) {
      waveformProgress.value = 0
    }
    
    try {
      const targetSamples = 20000
      
      const waveform = await invoke<number[]>('generate_audio_waveform', {
        filePath,
        targetSamples
      })
      
      // 保存到当前 tab
      if (tabManager.activeTab && waveform) {
        tabManager.activeTab.audio.waveform = waveform
        const waveformDuration = Date.now() - waveformStartTime
        logger.info('波形生成完成', {
          path: filePath,
          samples: waveform.length,
          generateTime: `${waveformDuration}ms`,
        })
      }
    } catch (error) {
      const waveformDuration = Date.now() - waveformStartTime
      logger.error('波形生成失败', {
        path: filePath,
        error: String(error),
        generateTime: `${waveformDuration}ms`,
      })
    } finally {
      if (progressSimulationTimer) {
        clearInterval(progressSimulationTimer)
        progressSimulationTimer = null
      }
      
      waveformProgress.value = 100
      
      await new Promise(resolve => setTimeout(resolve, 300))
      
      isGeneratingWaveform.value = false
      if (waveformProgressUnlisten) {
        waveformProgressUnlisten()
        waveformProgressUnlisten = null
      }
    }
  }

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
    removeTabCache,
  }
})

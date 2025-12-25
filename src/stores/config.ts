import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import type { EditorConfig, KeyBinding } from '@/types/subtitle'
import logger from '@/utils/logger'

// 最近文件项
export interface RecentFile {
  path: string
  name: string
  lastOpened: number // timestamp
}

// 最大最近文件数量
const MAX_RECENT_FILES = 10

// 默认删除的标点符号
export const DEFAULT_PUNCTUATION = `，。！？、；：""''（）《》【】…—,.!?;:'"()[]{}·~～@#$%^&*_+=|\\//<>`

export const useConfigStore = defineStore('config', () => {
  // 编辑器配置
  const config = ref<EditorConfig>({
    autoSave: true,
    autoscroll: true,
    showWaveform: true,
    showKeyboardHints: true,
    theme: 'light',
    language: 'zh-CN',
    newSubtitleDuration: 3,
  })

  // 要删除的标点符号（用户可自定义）
  const punctuationToRemove = ref<string>(DEFAULT_PUNCTUATION)

  // 语音转录设置
  const transcriptionEngine = ref<'whisper' | 'sensevoice'>('whisper')
  const whisperModel = ref<string>('base')
  const whisperLanguage = ref<string>('zh')

  // FireRedASR 校正设置
  const fireredPreserveCase = ref<boolean>(true) // 保留原始英文大小写

  // 导出设置
  const defaultExportFormat = ref<string>('txt') // 默认导出格式
  const defaultFcpxmlFps = ref<number>(30) // FCPXML 默认帧率

  // 🎄 圣诞彩蛋设置
  const showChristmasSnow = ref<boolean>(true) // 是否显示飘雪效果

  // 更新检测设置
  const skippedVersion = ref<string>('') // 用户跳过的版本
  const lastUpdateCheck = ref<number>(0) // 上次检查更新的时间戳

  // 重置标点符号为默认值
  const resetPunctuation = () => {
    punctuationToRemove.value = DEFAULT_PUNCTUATION
    savePunctuation()
  }

  // 保存标点符号配置
  const savePunctuation = () => {
    localStorage.setItem('vosub-punctuation', punctuationToRemove.value)
  }

  // 加载标点符号配置
  const loadPunctuation = () => {
    const saved = localStorage.getItem('vosub-punctuation')
    if (saved !== null) {
      punctuationToRemove.value = saved
    }
  }

  // 保存转录设置
  const saveWhisperSettings = () => {
    localStorage.setItem('vosub-whisper', JSON.stringify({
      engine: transcriptionEngine.value,
      model: whisperModel.value,
      language: whisperLanguage.value,
      fireredPreserveCase: fireredPreserveCase.value,
    }))
  }

  // 加载转录设置
  const loadWhisperSettings = () => {
    const saved = localStorage.getItem('vosub-whisper')
    if (saved) {
      try {
        const parsed = JSON.parse(saved)
        if (parsed.engine) transcriptionEngine.value = parsed.engine
        if (parsed.model) whisperModel.value = parsed.model
        if (parsed.language) whisperLanguage.value = parsed.language
        if (typeof parsed.fireredPreserveCase === 'boolean') fireredPreserveCase.value = parsed.fireredPreserveCase
      } catch (e) {
        // ignore
      }
    }
  }

  // 保存导出设置
  const saveExportSettings = () => {
    localStorage.setItem('vosub-export', JSON.stringify({
      format: defaultExportFormat.value,
      fcpxmlFps: defaultFcpxmlFps.value,
      showChristmasSnow: showChristmasSnow.value,
    }))
  }

  // 加载导出设置
  const loadExportSettings = () => {
    const saved = localStorage.getItem('vosub-export')
    if (saved) {
      try {
        const parsed = JSON.parse(saved)
        if (parsed.format) defaultExportFormat.value = parsed.format
        if (parsed.fcpxmlFps) defaultFcpxmlFps.value = parsed.fcpxmlFps
        if (typeof parsed.showChristmasSnow === 'boolean') showChristmasSnow.value = parsed.showChristmasSnow
      } catch (e) {
        // ignore
      }
    }
  }

  // 保存更新设置
  const saveUpdateSettings = () => {
    localStorage.setItem('vosub-update', JSON.stringify({
      skippedVersion: skippedVersion.value,
      lastUpdateCheck: lastUpdateCheck.value,
    }))
  }

  // 加载更新设置
  const loadUpdateSettings = () => {
    const saved = localStorage.getItem('vosub-update')
    if (saved) {
      try {
        const parsed = JSON.parse(saved)
        if (parsed.skippedVersion) skippedVersion.value = parsed.skippedVersion
        if (parsed.lastUpdateCheck) lastUpdateCheck.value = parsed.lastUpdateCheck
      } catch (e) {
        // ignore
      }
    }
  }

  // 跳过某个版本
  const skipVersion = (version: string) => {
    skippedVersion.value = version
    saveUpdateSettings()
  }

  // 记录检查更新时间
  const recordUpdateCheck = () => {
    lastUpdateCheck.value = Date.now()
    saveUpdateSettings()
  }

  // 最近打开的文件列表
  const recentFiles = ref<RecentFile[]>([])

  // 快捷键绑定
  const keyBindings = ref<KeyBinding[]>([
    { key: 'Space', description: '播放/暂停', action: 'toggle-play' },
    { key: 'L', description: '倍速播放', action: 'speed-up' },
    { key: 'K', description: '正常速度', action: 'speed-reset' },
    { key: 'ArrowUp', description: '上一条字幕', action: 'prev-subtitle' },
    { key: 'ArrowDown', description: '下一条字幕', action: 'next-subtitle' },
    { key: 'ArrowLeft', description: '字幕前移100ms', action: 'move-subtitle-left' },
    { key: 'ArrowRight', description: '字幕后移100ms', action: 'move-subtitle-right' },
    { key: 'Enter', description: '编辑字幕', action: 'edit-subtitle' },
    { key: 'Escape', description: '退出编辑', action: 'exit-edit' },
    { key: 'Delete/Backspace', description: '删除字幕（支持多选）', action: 'delete-subtitle' },
    { key: 'Tab', description: '保存并下一条', action: 'save-and-next' },
    { key: 'Shift+Tab', description: '保存并上一条', action: 'save-and-prev' },
    { key: 'Ctrl+S', description: '保存文件', action: 'save-file' },
    { key: 'Ctrl+O', description: '打开文件', action: 'open-file' },
    { key: 'Ctrl+E', description: '导出', action: 'export-dialog' },
    { key: 'Ctrl+W', description: '关闭标签页', action: 'close-tab' },
    { key: 'Ctrl+Q', description: '关闭窗口', action: 'close-window' },
    { key: 'Ctrl+F', description: '查找', action: 'find' },
    { key: 'Ctrl+R', description: '替换', action: 'replace' },
    { key: 'N', description: '新增字幕', action: 'new-subtitle' },
    { key: 'Ctrl+Z', description: '撤销', action: 'undo' },
    { key: 'Ctrl+Shift+Z', description: '重做', action: 'redo' },
    { key: 'Ctrl+,', description: '设置', action: 'settings' },
    { key: 'X', description: '分割字幕', action: 'split-subtitle' },
    { key: 'M', description: '合并字幕', action: 'merge-subtitles' },
    { key: 'S', description: '拖拽吸附', action: 'toggle-snap' },
    { key: 'A', description: '对齐波形', action: 'align-to-waveform' },
    { key: 'Ctrl+=', description: '放大波形', action: 'zoom-in' },
    { key: 'Ctrl+-', description: '缩小波形', action: 'zoom-out' },
    { key: 'Ctrl+0', description: '重置缩放', action: 'zoom-reset' },
    { key: 'Ctrl+D', description: '添加词条', action: 'add-to-dictionary' },
  ])

  // 更新配置
  const updateConfig = (partial: Partial<EditorConfig>) => {
    config.value = { ...config.value, ...partial }
    saveConfig()
  }

  // 保存配置到本地
  const saveConfig = () => {
    localStorage.setItem('vosub-config', JSON.stringify(config.value))
  }

  // 加载配置
  const loadConfig = () => {
    const saved = localStorage.getItem('vosub-config')
    if (saved) {
      try {
        const parsed = JSON.parse(saved)
        config.value = { ...config.value, ...parsed }
        logger.debug('配置加载完成')
      } catch (error) {
        logger.error('配置加载失败', { error: String(error) })
      }
    }
    // 加载最近文件列表
    const savedRecentFiles = localStorage.getItem('vosub-recent-files')
    if (savedRecentFiles) {
      try {
        recentFiles.value = JSON.parse(savedRecentFiles)
        logger.debug('最近文件列表加载完成', { count: recentFiles.value.length })
      } catch (error) {
        logger.error('最近文件列表加载失败', { error: String(error) })
      }
    }
  }

  // 添加最近文件
  const addRecentFile = (filePath: string) => {
    const fileName = filePath.split('/').pop() || filePath.split('\\').pop() || filePath
    
    // 移除已存在的相同路径
    recentFiles.value = recentFiles.value.filter(f => f.path !== filePath)
    
    // 添加到列表开头
    recentFiles.value.unshift({
      path: filePath,
      name: fileName,
      lastOpened: Date.now(),
    })
    
    // 限制数量
    if (recentFiles.value.length > MAX_RECENT_FILES) {
      recentFiles.value = recentFiles.value.slice(0, MAX_RECENT_FILES)
    }
    
    // 保存到本地存储
    localStorage.setItem('vosub-recent-files', JSON.stringify(recentFiles.value))
  }

  // 清空最近文件
  const clearRecentFiles = () => {
    recentFiles.value = []
    localStorage.removeItem('vosub-recent-files')
  }

  // 删除单个最近文件
  const removeRecentFile = (filePath: string) => {
    recentFiles.value = recentFiles.value.filter(f => f.path !== filePath)
    localStorage.setItem('vosub-recent-files', JSON.stringify(recentFiles.value))
  }

  // 初始化时加载配置
  loadConfig()
  loadPunctuation()
  loadWhisperSettings()
  loadExportSettings()
  loadUpdateSettings()

  // 检测平台
  const isMac = () => typeof navigator !== 'undefined' && /Mac|iPhone|iPad|iPod/.test(navigator.platform)

  // 创建快捷键映射对象（支持平台特定快捷键）
  const keyboardShortcuts = computed(() => {
    const isApple = isMac()
    const shortcuts: Record<string, string> = {}
    keyBindings.value.forEach((binding) => {
      shortcuts[binding.action.replace(/-([a-z])/g, (g) => g[1]?.toUpperCase() || g)] = binding.key
    })
    return {
      // macOS 使用 Cmd，Windows/Linux 使用 Ctrl
      save: isApple ? 'Cmd+s' : 'Ctrl+s',
      open: isApple ? 'Cmd+o' : 'Ctrl+o',
      undo: isApple ? 'Cmd+z' : 'Ctrl+z',
      redo: isApple ? 'Cmd+Shift+z' : 'Ctrl+Shift+z',
      playPause: ' ',
      find: isApple ? 'Cmd+f' : 'Ctrl+f',
      addEntry: 'n',
      deleteEntry: 'Delete',
      copy: isApple ? 'Cmd+c' : 'Ctrl+c',
      zoomIn: isApple ? 'Cmd+=' : 'Ctrl+=',
      zoomOut: isApple ? 'Cmd+-' : 'Ctrl+-',
      zoomReset: isApple ? 'Cmd+0' : 'Ctrl+0',
    }
  })

  return {
    config,
    keyBindings,
    keyboardShortcuts,
    recentFiles,
    punctuationToRemove,
    transcriptionEngine,
    whisperModel,
    whisperLanguage,
    fireredPreserveCase,
    defaultExportFormat,
    defaultFcpxmlFps,
    showChristmasSnow,
    skippedVersion,
    lastUpdateCheck,
    updateConfig,
    saveConfig,
    loadConfig,
    addRecentFile,
    clearRecentFiles,
    removeRecentFile,
    savePunctuation,
    resetPunctuation,
    saveWhisperSettings,
    saveExportSettings,
    loadExportSettings,
    skipVersion,
    recordUpdateCheck,
  }
})

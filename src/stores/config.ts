import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import type { EditorConfig, KeyBinding } from '@/types/subtitle'

// 最近文件项
export interface RecentFile {
  path: string
  name: string
  lastOpened: number // timestamp
}

// 最大最近文件数量
const MAX_RECENT_FILES = 10

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

  // 最近打开的文件列表
  const recentFiles = ref<RecentFile[]>([])

  // 快捷键绑定
  const keyBindings = ref<KeyBinding[]>([
    { key: 'Space', description: '播放/暂停', action: 'toggle-play' },
    { key: 'ArrowLeft', description: '上一条字幕', action: 'prev-subtitle' },
    { key: 'ArrowRight', description: '下一条字幕', action: 'next-subtitle' },
    { key: 'ArrowUp', description: '增加音量', action: 'volume-up' },
    { key: 'ArrowDown', description: '减少音量', action: 'volume-down' },
    { key: 'Enter', description: '编辑字幕', action: 'edit-subtitle' },
    { key: 'Escape', description: '退出编辑', action: 'exit-edit' },
    { key: 'Delete', description: '删除字幕', action: 'delete-subtitle' },
    { key: 'Tab', description: '保存并下一条', action: 'save-and-next' },
    { key: 'Shift+Tab', description: '保存并上一条', action: 'save-and-prev' },
    { key: 'Ctrl+S', description: '保存文件', action: 'save-file' },
    { key: 'Ctrl+O', description: '打开文件', action: 'open-file' },
    { key: 'Ctrl+F', description: '查找', action: 'find' },
    { key: 'Ctrl+R', description: '替换', action: 'replace' },
    { key: 'Ctrl+N', description: '新增字幕', action: 'new-subtitle' },
    { key: 'Ctrl+Z', description: '撤销', action: 'undo' },
    { key: 'Ctrl+Shift+Z', description: '重做', action: 'redo' },
    { key: 'Ctrl+,', description: '设置', action: 'settings' },
    { key: 'X', description: '分割字幕', action: 'split-subtitle' },
  ])

  // 更新配置
  const updateConfig = (partial: Partial<EditorConfig>) => {
    config.value = { ...config.value, ...partial }
    saveConfig()
  }

  // 保存配置到本地
  const saveConfig = () => {
    localStorage.setItem('srt-editor-config', JSON.stringify(config.value))
  }

  // 加载配置
  const loadConfig = () => {
    const saved = localStorage.getItem('srt-editor-config')
    if (saved) {
      try {
        const parsed = JSON.parse(saved)
        config.value = { ...config.value, ...parsed }
      } catch (error) {
        console.error('Failed to load config:', error)
      }
    }
    // 加载最近文件列表
    const savedRecentFiles = localStorage.getItem('srt-editor-recent-files')
    if (savedRecentFiles) {
      try {
        recentFiles.value = JSON.parse(savedRecentFiles)
      } catch (error) {
        console.error('Failed to load recent files:', error)
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
    localStorage.setItem('srt-editor-recent-files', JSON.stringify(recentFiles.value))
  }

  // 清空最近文件
  const clearRecentFiles = () => {
    recentFiles.value = []
    localStorage.removeItem('srt-editor-recent-files')
  }

  // 初始化时加载配置
  loadConfig()

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
      addEntry: isApple ? 'Cmd+n' : 'Ctrl+n',
      deleteEntry: 'Delete',
      copy: isApple ? 'Cmd+c' : 'Ctrl+c',
    }
  })

  return {
    config,
    keyBindings,
    keyboardShortcuts,
    recentFiles,
    updateConfig,
    saveConfig,
    loadConfig,
    addRecentFile,
    clearRecentFiles,
  }
})

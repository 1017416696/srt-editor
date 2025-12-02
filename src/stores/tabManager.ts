import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import type { EditorTab } from '@/types/tab'
import { createDefaultSubtitleState, createDefaultAudioState } from '@/types/tab'
import logger from '@/utils/logger'

export const useTabManagerStore = defineStore('tabManager', () => {
  // 所有打开的标签页
  const tabs = ref<EditorTab[]>([])
  
  // 当前激活的标签页 ID
  const activeTabId = ref<string | null>(null)

  // 计算属性：当前激活的标签页
  const activeTab = computed(() => {
    if (!activeTabId.value) return null
    return tabs.value.find(tab => tab.id === activeTabId.value) || null
  })

  // 计算属性：是否有标签页
  const hasTabs = computed(() => tabs.value.length > 0)

  // 生成唯一 ID
  const generateTabId = (): string => {
    return `tab-${Date.now()}-${Math.random().toString(36).substring(2, 9)}`
  }

  // 从文件路径提取文件名
  const extractFileName = (filePath: string): string => {
    return filePath.split('/').pop() || filePath.split('\\').pop() || 'Untitled'
  }

  // 创建新标签页
  const createTab = (subtitleFilePath: string, entries: any[]): EditorTab => {
    const id = generateTabId()
    const fileName = extractFileName(subtitleFilePath)
    
    const newTab: EditorTab = {
      id,
      fileName,
      subtitle: {
        ...createDefaultSubtitleState(),
        filePath: subtitleFilePath,
        entries,
        currentEntryId: entries.length > 0 ? entries[0]?.id ?? null : null,
      },
      audio: createDefaultAudioState(),
    }

    tabs.value.push(newTab)
    activeTabId.value = id
    
    logger.info('创建新标签页', { id, fileName, entriesCount: entries.length })
    
    return newTab
  }

  // 关闭标签页
  const closeTab = (tabId: string): boolean => {
    const index = tabs.value.findIndex(tab => tab.id === tabId)
    if (index === -1) return false

    const closedTab = tabs.value[index]
    tabs.value.splice(index, 1)
    
    logger.info('关闭标签页', { id: tabId, fileName: closedTab?.fileName })

    // 如果关闭的是当前激活的标签页，切换到相邻的标签页
    if (activeTabId.value === tabId) {
      if (tabs.value.length > 0) {
        // 优先切换到右边的标签页，如果没有则切换到左边
        const newIndex = Math.min(index, tabs.value.length - 1)
        activeTabId.value = tabs.value[newIndex]?.id || null
      } else {
        activeTabId.value = null
      }
    }

    return tabs.value.length === 0
  }

  // 切换标签页
  const setActiveTab = (tabId: string) => {
    const tab = tabs.value.find(t => t.id === tabId)
    if (tab) {
      activeTabId.value = tabId
      logger.info('切换标签页', { id: tabId, fileName: tab.fileName })
    }
  }

  // 重新排序标签页（拖拽排序）
  const reorderTabs = (fromIndex: number, toIndex: number) => {
    if (fromIndex < 0 || fromIndex >= tabs.value.length) return
    if (toIndex < 0 || toIndex >= tabs.value.length) return
    
    const [movedTab] = tabs.value.splice(fromIndex, 1)
    if (movedTab) {
      tabs.value.splice(toIndex, 0, movedTab)
    }
  }

  // 更新标签页的字幕状态
  const updateSubtitleState = (tabId: string, updates: Partial<EditorTab['subtitle']>) => {
    const tab = tabs.value.find(t => t.id === tabId)
    if (tab) {
      Object.assign(tab.subtitle, updates)
    }
  }

  // 更新标签页的音频状态
  const updateAudioState = (tabId: string, updates: Partial<EditorTab['audio']>) => {
    const tab = tabs.value.find(t => t.id === tabId)
    if (tab) {
      Object.assign(tab.audio, updates)
    }
  }

  // 检查文件是否已经打开
  const findTabByFilePath = (filePath: string): EditorTab | undefined => {
    return tabs.value.find(tab => tab.subtitle.filePath === filePath)
  }

  // 清空所有标签页
  const clearAllTabs = () => {
    tabs.value = []
    activeTabId.value = null
  }

  return {
    // 状态
    tabs,
    activeTabId,
    
    // 计算属性
    activeTab,
    hasTabs,
    
    // 方法
    createTab,
    closeTab,
    setActiveTab,
    reorderTabs,
    updateSubtitleState,
    updateAudioState,
    findTabByFilePath,
    clearAllTabs,
  }
})

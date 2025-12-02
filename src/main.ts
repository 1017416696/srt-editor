import { devtools } from '@vue/devtools'
import { createPinia } from 'pinia'
import { createApp } from 'vue'
import ElementPlus from 'element-plus'
import 'element-plus/dist/index.css'
import App from './App.vue'
import router from './router'
import './assets/main.css'
import { listen } from '@tauri-apps/api/event'
import { open } from '@tauri-apps/plugin-dialog'
import { invoke } from '@tauri-apps/api/core'
import logger, { initLogger } from './utils/logger'

if (process.env.NODE_ENV === 'development') {
  devtools.connect('http://localhost', 8098)
}

// 初始化日志系统
initLogger().then(() => {
  logger.info('前端应用初始化')
})

const app = createApp(App)
const pinia = createPinia()

app.use(pinia)
app.use(router)
app.use(ElementPlus)

// 获取 store 的引用
const subtitleStore = pinia.state.value.subtitle
const audioStore = pinia.state.value.audio

// 全局打开文件函数
const globalOpenFile = async () => {
  try {
    const selected = await open({
      multiple: false,
      filters: [
        {
          name: 'SRT 字幕文件',
          extensions: ['srt'],
        },
      ],
    })

    if (selected) {
      const loadStartTime = Date.now()
      const { useSubtitleStore } = await import('./stores/subtitle')
      const { useConfigStore } = await import('./stores/config')
      const store = useSubtitleStore()
      const configStore = useConfigStore()
      const srtFile = (await invoke('read_srt', { filePath: selected })) as any
      await store.loadSRTFile(srtFile)

      const loadDuration = Date.now() - loadStartTime
      logger.info('打开 SRT 文件', {
        path: selected,
        entries: srtFile.entries?.length,
        loadTime: `${loadDuration}ms`,
      })

      // 添加到最近文件列表
      configStore.addRecentFile(selected as string)
      
      // 更新菜单
      await updateRecentFilesMenu()

      // 如果当前不在编辑器页面，导航到编辑器
      if (router.currentRoute.value.path !== '/editor') {
        router.push('/editor')
      }
    }
  } catch (error) {
    logger.error('打开文件失败', { error: String(error) })
  }
}

// 全局保存文件函数
const globalSaveFile = async () => {
  try {
    const { useSubtitleStore } = await import('./stores/subtitle')
    const store = useSubtitleStore()
    if (!store.currentFilePath) {
      return
    }
    await store.saveToFile()
    logger.info('保存文件成功', { path: store.currentFilePath })
  } catch (error) {
    logger.error('保存文件失败', { error: String(error) })
  }
}

// 全局批量添加中英文空格函数
const globalBatchAddCJKSpaces = async () => {
  try {
    const { useSubtitleStore } = await import('./stores/subtitle')
    const store = useSubtitleStore()
    if (store.entries.length === 0) {
      return
    }
    store.addSpacesBetweenCJKAndAlphanumeric()
    logger.info('批量添加中英文空格', { entries: store.entries.length })
    if (store.currentFilePath) {
      await store.saveToFile()
    }
  } catch (error) {
    logger.error('批量添加中英文空格失败', { error: String(error) })
  }
}

// 全局批量移除HTML标签函数
const globalBatchRemoveHTML = async () => {
  try {
    const { useSubtitleStore } = await import('./stores/subtitle')
    const store = useSubtitleStore()
    if (store.entries.length === 0) {
      return
    }
    store.removeHTMLTags()
    logger.info('批量移除 HTML 标签', { entries: store.entries.length })
    if (store.currentFilePath) {
      await store.saveToFile()
    }
  } catch (error) {
    logger.error('批量移除 HTML 标签失败', { error: String(error) })
  }
}

// 全局批量删除标点符号函数
const globalBatchRemovePunctuation = async () => {
  try {
    const { useSubtitleStore } = await import('./stores/subtitle')
    const store = useSubtitleStore()
    if (store.entries.length === 0) {
      return
    }
    store.removePunctuation()
    logger.info('批量删除标点符号', { entries: store.entries.length })
    if (store.currentFilePath) {
      await store.saveToFile()
    }
  } catch (error) {
    logger.error('批量删除标点符号失败', { error: String(error) })
  }
}

// 全局清除最近文件函数
const globalClearRecentFiles = async () => {
  try {
    const { useConfigStore } = await import('./stores/config')
    const configStore = useConfigStore()
    configStore.clearRecentFiles()
    // 更新菜单
    await invoke('update_recent_files_menu', { files: [] })
  } catch (error) {
    // 处理失败，静默处理
  }
}

// 全局关闭当前标签页函数
const globalCloseCurrentTab = async () => {
  try {
    const { useTabManagerStore } = await import('./stores/tabManager')
    const { useAudioStore } = await import('./stores/audio')
    const tabManager = useTabManagerStore()
    const audioStore = useAudioStore()
    
    if (!tabManager.activeTabId) {
      return
    }
    
    // 清理该 tab 的音频缓存
    audioStore.removeTabCache(tabManager.activeTabId)
    const shouldGoWelcome = tabManager.closeTab(tabManager.activeTabId)
    
    if (shouldGoWelcome) {
      router.push('/')
    }
  } catch (error) {
    logger.error('关闭标签页失败', { error: String(error) })
  }
}

// 全局打开最近文件函数
const globalOpenRecentFile = async (index: number) => {
  try {
    const loadStartTime = Date.now()
    const { useConfigStore } = await import('./stores/config')
    const { useSubtitleStore } = await import('./stores/subtitle')
    const configStore = useConfigStore()
    const subtitleStore = useSubtitleStore()

    const recentFile = configStore.recentFiles[index]
    if (!recentFile) return

    const srtFile = (await invoke('read_srt', { filePath: recentFile.path })) as any
    await subtitleStore.loadSRTFile(srtFile)

    const loadDuration = Date.now() - loadStartTime
    logger.info('打开最近文件', {
      path: recentFile.path,
      entries: srtFile.entries?.length,
      loadTime: `${loadDuration}ms`,
    })
    
    // 更新最近文件列表（将此文件移到最前）
    configStore.addRecentFile(recentFile.path)
    
    // 更新菜单
    await updateRecentFilesMenu()
    
    // 如果当前不在编辑器页面，导航到编辑器
    if (router.currentRoute.value.path !== '/editor') {
      router.push('/editor')
    }
  } catch (error) {
    logger.error('打开最近文件失败', { error: String(error) })
  }
}

// 更新最近文件菜单
const updateRecentFilesMenu = async () => {
  try {
    const { useConfigStore } = await import('./stores/config')
    const configStore = useConfigStore()
    
    const files = configStore.recentFiles.map(f => ({
      path: f.path,
      name: f.name,
    }))
    
    await invoke('update_recent_files_menu', { files })
  } catch (error) {
    // 更新菜单失败，静默处理
  }
}

// 将全局函数暴露到 window 对象
;(window as any).__globalOpenFile = globalOpenFile
;(window as any).__globalSaveFile = globalSaveFile
;(window as any).__globalBatchAddCJKSpaces = globalBatchAddCJKSpaces
;(window as any).__globalBatchRemoveHTML = globalBatchRemoveHTML
;(window as any).__globalBatchRemovePunctuation = globalBatchRemovePunctuation
;(window as any).__globalClearRecentFiles = globalClearRecentFiles
;(window as any).__globalOpenRecentFile = globalOpenRecentFile
;(window as any).__globalCloseCurrentTab = globalCloseCurrentTab
;(window as any).__updateRecentFilesMenu = updateRecentFilesMenu

// 全局菜单事件监听器（在应用启动时注册）
listen<void>('menu:open-file', async () => {
  // 触发全局回调函数（由各页面注册）
  if ((window as any).__handleMenuOpenFile && typeof (window as any).__handleMenuOpenFile === 'function') {
    await (window as any).__handleMenuOpenFile()
  }
}).catch(() => { })

listen<void>('menu:save', async () => {
  // 触发全局回调函数（由各页面注册）
  if ((window as any).__handleMenuSave && typeof (window as any).__handleMenuSave === 'function') {
    await (window as any).__handleMenuSave()
  }
}).catch(() => { })

// 全局键盘快捷键监听（仅处理打开文件，保存由 EditorPage 组件处理）
document.addEventListener('keydown', (e: KeyboardEvent) => {
  const isMac = /Mac|iPhone|iPad|iPod/.test(navigator.platform)

  // 检查 Cmd+O (macOS) 或 Ctrl+O (Windows/Linux)
  // 仅在编辑器页面未加载时使用这个全局快捷键
  if ((isMac && e.metaKey && e.key.toLowerCase() === 'o') ||
    (!isMac && e.ctrlKey && e.key.toLowerCase() === 'o')) {
    // 如果编辑器页面已注册自己的快捷键处理，就不必理会这个
    // 这是备用方案
  }
}, true)

app.mount('#app')

// 应用启动后初始化最近文件菜单
setTimeout(async () => {
  await updateRecentFilesMenu()
}, 100)

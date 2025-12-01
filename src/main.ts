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

if (process.env.NODE_ENV === 'development') {
  devtools.connect('http://localhost', 8098)
}

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
      const { useSubtitleStore } = await import('./stores/subtitle')
      const store = useSubtitleStore()
      const srtFile = await invoke('read_srt', { filePath: selected })
      await store.loadSRTFile(srtFile)

      // 如果当前不在编辑器页面，导航到编辑器
      if (router.currentRoute.value.path !== '/editor') {
        router.push('/editor')
      }
    }
  } catch (error) {
    // 加载失败，静默处理
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
  } catch (error) {
    // 保存失败，静默处理
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
    if (store.currentFilePath) {
      await store.saveToFile()
    }
  } catch (error) {
    // 处理失败，静默处理
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
    if (store.currentFilePath) {
      await store.saveToFile()
    }
  } catch (error) {
    // 处理失败，静默处理
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
    if (store.currentFilePath) {
      await store.saveToFile()
    }
  } catch (error) {
    // 处理失败，静默处理
  }
}

// 将全局函数暴露到 window 对象
;(window as any).__globalOpenFile = globalOpenFile
;(window as any).__globalSaveFile = globalSaveFile
;(window as any).__globalBatchAddCJKSpaces = globalBatchAddCJKSpaces
;(window as any).__globalBatchRemoveHTML = globalBatchRemoveHTML
;(window as any).__globalBatchRemovePunctuation = globalBatchRemovePunctuation

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

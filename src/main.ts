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
import { ElMessage } from 'element-plus'

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
      ElMessage.success({ message: 'SRT 文件加载成功', duration: 300 })

      // 如果当前不在编辑器页面，导航到编辑器
      if (router.currentRoute.value.path !== '/editor') {
        router.push('/editor')
      }
    }
  } catch (error) {
    ElMessage.error(`加载失败: ${error}`)
  }
}

// 全局保存文件函数
const globalSaveFile = async () => {
  try {
    const { useSubtitleStore } = await import('./stores/subtitle')
    const store = useSubtitleStore()
    if (!store.currentFilePath) {
      ElMessage.warning('没有可保存的文件')
      return
    }
    await store.saveToFile()
    ElMessage.success('保存成功')
  } catch (error) {
    ElMessage.error(`保存失败: ${error}`)
  }
}

  // 将全局函数暴露到 window 对象
  ; (window as any).__globalOpenFile = globalOpenFile
  ; (window as any).__globalSaveFile = globalSaveFile

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

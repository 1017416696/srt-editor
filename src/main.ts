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

// 全局批量转换为大写函数
const globalBatchToUpperCase = async () => {
  try {
    const { useSubtitleStore } = await import('./stores/subtitle')
    const store = useSubtitleStore()
    if (store.entries.length === 0) {
      return
    }
    store.convertToUpperCase()
    logger.info('批量转换为大写', { entries: store.entries.length })
    if (store.currentFilePath) {
      await store.saveToFile()
    }
  } catch (error) {
    logger.error('批量转换为大写失败', { error: String(error) })
  }
}

// 全局批量转换为小写函数
const globalBatchToLowerCase = async () => {
  try {
    const { useSubtitleStore } = await import('./stores/subtitle')
    const store = useSubtitleStore()
    if (store.entries.length === 0) {
      return
    }
    store.convertToLowerCase()
    logger.info('批量转换为小写', { entries: store.entries.length })
    if (store.currentFilePath) {
      await store.saveToFile()
    }
  } catch (error) {
    logger.error('批量转换为小写失败', { error: String(error) })
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

// 全局导出字幕函数
const globalExportSubtitles = async (format: string) => {
  try {
    const { useSubtitleStore } = await import('./stores/subtitle')
    const { useConfigStore } = await import('./stores/config')
    const { save } = await import('@tauri-apps/plugin-dialog')
    const { ElMessage, ElMessageBox } = await import('element-plus')
    const store = useSubtitleStore()
    const configStore = useConfigStore()
    
    if (store.entries.length === 0) {
      ElMessage.warning('没有字幕内容可导出')
      return
    }
    
    // 准备导出数据（转换为后端需要的格式，使用 camelCase 匹配 Rust serde rename）
    const entries = store.entries.map(e => ({
      id: e.id,
      startTime: {
        hours: e.startTime.hours,
        minutes: e.startTime.minutes,
        seconds: e.startTime.seconds,
        milliseconds: e.startTime.milliseconds,
      },
      endTime: {
        hours: e.endTime.hours,
        minutes: e.endTime.minutes,
        seconds: e.endTime.seconds,
        milliseconds: e.endTime.milliseconds,
      },
      text: e.text,
    }))
    
    // 根据格式设置文件扩展名和过滤器
    const formatConfig: Record<string, { ext: string; name: string }> = {
      txt: { ext: 'txt', name: '纯文本文件' },
      vtt: { ext: 'vtt', name: 'WebVTT 字幕文件' },
      srt: { ext: 'srt', name: 'SRT 字幕文件' },
      markdown: { ext: 'md', name: 'Markdown 文件' },
      fcpxml: { ext: 'fcpxml', name: 'Final Cut Pro XML' },
    }
    
    const config = formatConfig[format]
    if (!config) {
      ElMessage.error('不支持的导出格式')
      return
    }
    
    // FCPXML 需要选择帧率和字幕位置
    const defaultFps = configStore.defaultFcpxmlFps
    let fps = defaultFps
    let positionX = 0
    let positionY = -415
    if (format === 'fcpxml') {
      // 创建一个 Promise 来处理用户选择
      const result = await new Promise<{ fps: number; posX: number; posY: number } | null>((resolve) => {
        // 创建对话框容器
        const container = document.createElement('div')
        container.className = 'export-dialog-overlay'
        const fpsOptions = [
          { value: 24, label: '24 fps (电影)' },
          { value: 25, label: '25 fps (PAL)' },
          { value: 30, label: '30 fps (NTSC)' },
          { value: 50, label: '50 fps' },
          { value: 60, label: '60 fps (高帧率)' },
        ]
        container.innerHTML = `
          <div class="export-dialog-backdrop"></div>
          <div class="export-dialog-content">
            <div class="export-dialog-header">
              <span class="export-dialog-title">导出 FCPXML</span>
              <button class="export-dialog-close" type="button">×</button>
            </div>
            <div class="export-dialog-body">
              <div class="export-form-row">
                <label class="export-form-label">帧率</label>
                <select id="fcpxml-fps" class="export-select">
                  ${fpsOptions.map(opt => `<option value="${opt.value}"${opt.value === defaultFps ? ' selected' : ''}>${opt.label}</option>`).join('')}
                </select>
              </div>
              <div class="export-form-row">
                <label class="export-form-label">字幕位置</label>
                <div class="export-position-inputs">
                  <div class="export-input-group">
                    <span class="export-input-prefix">X</span>
                    <input type="number" id="fcpxml-pos-x" value="0" class="export-input" />
                    <span class="export-input-suffix">px</span>
                  </div>
                  <div class="export-input-group">
                    <span class="export-input-prefix">Y</span>
                    <input type="number" id="fcpxml-pos-y" value="-415" class="export-input" />
                    <span class="export-input-suffix">px</span>
                  </div>
                </div>
              </div>
            </div>
            <div class="export-dialog-footer">
              <button class="export-btn export-btn-cancel" type="button">取消</button>
              <button class="export-btn export-btn-confirm" type="button">导出</button>
            </div>
          </div>
        `
        document.body.appendChild(container)
        
        const getValues = () => {
          const fpsSelect = container.querySelector('#fcpxml-fps') as HTMLSelectElement
          const posXInput = container.querySelector('#fcpxml-pos-x') as HTMLInputElement
          const posYInput = container.querySelector('#fcpxml-pos-y') as HTMLInputElement
          return {
            fps: parseInt(fpsSelect?.value || '25'),
            posX: parseInt(posXInput?.value || '0'),
            posY: parseInt(posYInput?.value || '-415'),
          }
        }
        
        // 绑定按钮事件
        container.querySelector('.export-btn-cancel')?.addEventListener('click', () => {
          document.body.removeChild(container)
          resolve(null)
        })
        
        container.querySelector('.export-dialog-close')?.addEventListener('click', () => {
          document.body.removeChild(container)
          resolve(null)
        })
        
        container.querySelector('.export-btn-confirm')?.addEventListener('click', () => {
          const values = getValues()
          document.body.removeChild(container)
          resolve(values)
        })
        
        container.querySelector('.export-dialog-backdrop')?.addEventListener('click', () => {
          document.body.removeChild(container)
          resolve(null)
        })
        
        // ESC 键关闭弹窗
        const handleKeydown = (e: KeyboardEvent) => {
          if (e.key === 'Escape') {
            document.removeEventListener('keydown', handleKeydown)
            document.body.removeChild(container)
            resolve(null)
          }
        }
        document.addEventListener('keydown', handleKeydown)
      })
      
      if (result === null) {
        return // 用户取消
      }
      fps = result.fps
      positionX = result.posX
      positionY = result.posY
    }
    
    // 获取默认文件名
    const currentFileName = store.currentFilePath?.split('/').pop()?.replace('.srt', '') || 'subtitles'
    
    // 打开保存对话框
    const filePath = await save({
      filters: [{ name: config.name, extensions: [config.ext] }],
      defaultPath: `${currentFileName}.${config.ext}`,
    })
    
    if (!filePath) return
    
    // 调用对应的导出命令
    if (format === 'fcpxml') {
      await invoke('export_fcpxml', { filePath, entries, fps, positionX, positionY })
    } else if (format === 'txt') {
      await invoke('export_txt', { filePath, entries })
    } else if (format === 'vtt') {
      await invoke('export_vtt', { filePath, entries })
    } else if (format === 'srt') {
      await invoke('write_srt', { filePath, entries })
    } else if (format === 'markdown') {
      await invoke('export_markdown', { filePath, entries })
    }
    
    ElMessage.success(`已导出为 ${config.ext.toUpperCase()} 格式`)
    logger.info('导出字幕', { format, path: filePath, entries: entries.length })
  } catch (error) {
    const { ElMessage } = await import('element-plus')
    ElMessage.error(`导出失败：${error instanceof Error ? error.message : String(error)}`)
    logger.error('导出字幕失败', { format, error: String(error) })
  }
}

// 全局显示导出格式选择对话框（Cmd+E）
const globalShowExportDialog = async () => {
  try {
    const { useSubtitleStore } = await import('./stores/subtitle')
    const { useConfigStore } = await import('./stores/config')
    const { ElMessage } = await import('element-plus')
    const store = useSubtitleStore()
    const configStore = useConfigStore()
    
    if (store.entries.length === 0) {
      ElMessage.warning('没有字幕内容可导出')
      return
    }
    
    const exportFormats = [
      { value: 'txt', label: 'TXT', desc: '纯文本格式' },
      { value: 'vtt', label: 'VTT', desc: 'WebVTT 字幕' },
      { value: 'srt', label: 'SRT', desc: 'SRT 字幕' },
      { value: 'markdown', label: 'Markdown', desc: '带时间戳的文档' },
      { value: 'fcpxml', label: 'FCPXML', desc: 'Final Cut Pro' },
    ]
    
    const defaultFormat = configStore.defaultExportFormat
    
    // 创建格式选择对话框
    const selectedFormat = await new Promise<string | null>((resolve) => {
      const container = document.createElement('div')
      container.className = 'export-dialog-overlay'
      container.innerHTML = `
        <div class="export-dialog-backdrop"></div>
        <div class="export-dialog-content">
          <div class="export-dialog-header">
            <span class="export-dialog-title">导出字幕</span>
            <button class="export-dialog-close" type="button">×</button>
          </div>
          <div class="export-dialog-body">
            <div class="export-form-row">
              <label class="export-form-label">导出格式</label>
              <select id="export-format" class="export-select">
                ${exportFormats.map(fmt => `<option value="${fmt.value}"${fmt.value === defaultFormat ? ' selected' : ''}>${fmt.label} - ${fmt.desc}</option>`).join('')}
              </select>
            </div>
          </div>
          <div class="export-dialog-footer">
            <button class="export-btn export-btn-cancel" type="button">取消</button>
            <button class="export-btn export-btn-confirm" type="button">导出</button>
          </div>
        </div>
      `
      document.body.appendChild(container)
      
      // 绑定按钮事件
      container.querySelector('.export-btn-cancel')?.addEventListener('click', () => {
        document.body.removeChild(container)
        resolve(null)
      })
      
      container.querySelector('.export-dialog-close')?.addEventListener('click', () => {
        document.body.removeChild(container)
        resolve(null)
      })
      
      container.querySelector('.export-btn-confirm')?.addEventListener('click', () => {
        const select = container.querySelector('#export-format') as HTMLSelectElement
        document.body.removeChild(container)
        resolve(select?.value || 'txt')
      })
      
      container.querySelector('.export-dialog-backdrop')?.addEventListener('click', () => {
        document.body.removeChild(container)
        resolve(null)
      })
      
      // ESC 键关闭弹窗
      const handleKeydown = (e: KeyboardEvent) => {
        if (e.key === 'Escape') {
          document.removeEventListener('keydown', handleKeydown)
          document.body.removeChild(container)
          resolve(null)
        }
      }
      document.addEventListener('keydown', handleKeydown)
    })
    
    if (selectedFormat) {
      // 调用对应格式的导出函数
      await globalExportSubtitles(selectedFormat)
    }
  } catch (error) {
    logger.error('显示导出对话框失败', { error: String(error) })
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
;(window as any).__globalBatchToUpperCase = globalBatchToUpperCase
;(window as any).__globalBatchToLowerCase = globalBatchToLowerCase
;(window as any).__globalClearRecentFiles = globalClearRecentFiles
;(window as any).__globalOpenRecentFile = globalOpenRecentFile
;(window as any).__globalCloseCurrentTab = globalCloseCurrentTab
;(window as any).__globalExportSubtitles = globalExportSubtitles
;(window as any).__globalShowExportDialog = globalShowExportDialog
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

// 全局禁用所有按钮的 Tab 键导航
const disableButtonTabNavigation = () => {
  // 获取所有按钮元素
  const buttons = document.querySelectorAll('button:not([tabindex]), .el-button:not([tabindex])')
  buttons.forEach(button => {
    button.setAttribute('tabindex', '-1')
  })
}

// 初始执行一次
setTimeout(disableButtonTabNavigation, 100)

// 使用 MutationObserver 监听 DOM 变化，自动为新增的按钮设置 tabindex
const observer = new MutationObserver(() => {
  disableButtonTabNavigation()
})

observer.observe(document.body, {
  childList: true,
  subtree: true,
})

// 应用启动后初始化最近文件菜单
setTimeout(async () => {
  await updateRecentFilesMenu()
}, 100)

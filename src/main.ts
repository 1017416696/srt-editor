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
      const { ElMessageBox } = await import('element-plus')
      const store = useSubtitleStore()
      const configStore = useConfigStore()
      
      // 检查文件写入权限
      const permissionCheck = (await invoke('check_file_write_permission', {
        filePath: selected,
      })) as {
        readable: boolean
        writable: boolean
        error_message: string | null
        is_locked: boolean
      }

      if (!permissionCheck.writable) {
        if (permissionCheck.is_locked) {
          // 文件被锁定，提供解锁选项
          try {
            await ElMessageBox.confirm(
              '文件已被锁定，无法写入。\n\n点击「解锁」按钮可以解除锁定并继续编辑。',
              '文件已锁定',
              { confirmButtonText: '解锁', cancelButtonText: '取消', type: 'warning' }
            )
            // 用户点击解锁
            await invoke('unlock_file_cmd', { filePath: selected })
          } catch {
            // 用户点击取消
            logger.warn('用户取消解锁文件', { path: selected })
            return
          }
        } else {
          // 其他权限问题
          const warningMessage = permissionCheck.error_message || '文件无法写入。'
          await ElMessageBox.alert(warningMessage, '无法打开文件', {
            confirmButtonText: '我知道了',
            type: 'warning',
            dangerouslyUseHTMLString: true,
          })
          logger.warn('文件权限受限，取消打开', { path: selected })
          return
        }
      }

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
    
    // 检查是否是权限问题，给出详细提示
    const errorStr = String(error)
    if (errorStr.includes('Permission denied') || errorStr.includes('os error 13')) {
      const { ElMessageBox } = await import('element-plus')
      const { useSubtitleStore } = await import('./stores/subtitle')
      const store = useSubtitleStore()
      const filePath = store.currentFilePath || ''
      
      await ElMessageBox.alert(
        `<p><strong>文件保存失败：权限被拒绝</strong></p>
        <p style="margin-top: 10px;">这可能是因为文件从网络下载或从其他人处接收，macOS 为其添加了安全隔离属性。</p>
        <p style="margin-top: 10px;"><strong>解决方法：</strong></p>
        <p>在终端运行以下命令移除隔离属性：</p>
        <pre style="background: #f5f5f5; padding: 10px; margin-top: 5px; border-radius: 4px; overflow-x: auto;">xattr -d com.apple.quarantine "${filePath}"</pre>
        <p style="margin-top: 10px;">或者使用「另存为」功能将文件保存到其他位置。</p>`,
        '保存失败',
        {
          confirmButtonText: '我知道了',
          type: 'error',
          dangerouslyUseHTMLString: true,
        }
      )
    }
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

// 全局批量转换为首字母大写函数
const globalBatchToCapitalize = async () => {
  try {
    const { useSubtitleStore } = await import('./stores/subtitle')
    const store = useSubtitleStore()
    if (store.entries.length === 0) {
      return
    }
    store.convertToCapitalize()
    logger.info('批量转换为首字母大写', { entries: store.entries.length })
    if (store.currentFilePath) {
      await store.saveToFile()
    }
  } catch (error) {
    logger.error('批量转换为首字母大写失败', { error: String(error) })
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
;(window as any).__globalBatchToCapitalize = globalBatchToCapitalize
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

// 通过文件关联打开文件的处理函数
const openFileFromAssociation = async (filePath: string) => {
  logger.info('通过文件关联打开文件', { path: filePath })
  
  try {
    const loadStartTime = Date.now()
    const { useSubtitleStore } = await import('./stores/subtitle')
    const { useConfigStore } = await import('./stores/config')
    const { ElMessageBox } = await import('element-plus')
    const store = useSubtitleStore()
    const configStore = useConfigStore()
    
    // 检查文件写入权限
    const permissionCheck = (await invoke('check_file_write_permission', {
      filePath: filePath,
    })) as {
      readable: boolean
      writable: boolean
      error_message: string | null
      is_locked: boolean
    }

    if (!permissionCheck.writable) {
      if (permissionCheck.is_locked) {
        // 文件被锁定，提供解锁选项
        try {
          await ElMessageBox.confirm(
            '文件已被锁定，无法写入。\n\n点击「解锁」按钮可以解除锁定并继续编辑。',
            '文件已锁定',
            { confirmButtonText: '解锁', cancelButtonText: '取消', type: 'warning' }
          )
          // 用户点击解锁
          await invoke('unlock_file_cmd', { filePath: filePath })
        } catch {
          // 用户点击取消
          logger.warn('用户取消解锁文件', { path: filePath })
          return
        }
      } else {
        // 其他权限问题
        const warningMessage = permissionCheck.error_message || '文件无法写入。'
        await ElMessageBox.alert(warningMessage, '无法打开文件', {
          confirmButtonText: '我知道了',
          type: 'warning',
          dangerouslyUseHTMLString: true,
        })
        logger.warn('文件权限受限，取消打开', { path: filePath })
        return
      }
    }

    const srtFile = (await invoke('read_srt', { filePath: filePath })) as any
    await store.loadSRTFile(srtFile)

    const loadDuration = Date.now() - loadStartTime
    logger.info('打开 SRT 文件', {
      path: filePath,
      entries: srtFile.entries?.length,
      loadTime: `${loadDuration}ms`,
    })

    // 添加到最近文件列表
    configStore.addRecentFile(filePath)
    
    // 更新菜单
    await updateRecentFilesMenu()

    // 如果当前不在编辑器页面，导航到编辑器
    if (router.currentRoute.value.path !== '/editor') {
      router.push('/editor')
    }
  } catch (error) {
    logger.error('通过文件关联打开文件失败', { error: String(error), path: filePath })
  }
}

// 应用启动后初始化
setTimeout(async () => {
  // 初始化最近文件菜单
  await updateRecentFilesMenu()
  
  // 检查是否有通过文件关联待打开的文件（冷启动场景）
  try {
    const pendingFile = await invoke('get_pending_file_open') as string | null
    if (pendingFile) {
      logger.info('检测到待打开的文件关联文件', { path: pendingFile })
      await openFileFromAssociation(pendingFile)
    }
  } catch (error) {
    logger.error('检查待打开文件失败', { error: String(error) })
  }
}, 100)

// 监听文件关联打开事件（应用已运行时双击 .srt 文件）
listen<string>('file-association-open', async (event) => {
  await openFileFromAssociation(event.payload)
}).catch(() => { })

// 检查更新
const checkForAppUpdates = async () => {
  try {
    const { checkForUpdates } = await import('./utils/updater')
    const { useConfigStore } = await import('./stores/config')
    const configStore = useConfigStore()

    const result = await checkForUpdates()

    if (result.error) {
      logger.warn('自动检查更新失败', { error: result.error })
      return
    }

    if (result.hasUpdate && result.releaseInfo) {
      // 检查是否跳过了此版本
      if (configStore.skippedVersion === result.latestVersion) {
        logger.debug('自动检查更新：用户已跳过此版本', { version: result.latestVersion })
        return
      }

      logger.info('自动检查更新：发现新版本', {
        current: result.currentVersion,
        latest: result.latestVersion,
      })

      // 触发更新提示（通过全局事件）
      window.dispatchEvent(new CustomEvent('app-update-available', {
        detail: {
          currentVersion: result.currentVersion,
          releaseInfo: result.releaseInfo,
        },
      }))
    }
  } catch (error) {
    logger.error('自动检查更新异常', { error: String(error) })
  }
}

// 应用启动 3 秒后检查更新（避免影响启动速度）
setTimeout(checkForAppUpdates, 3000)

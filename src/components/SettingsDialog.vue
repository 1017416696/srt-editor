<script setup lang="ts">
import { ref, computed, watch, onBeforeUnmount, nextTick } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { useConfigStore, DEFAULT_PUNCTUATION } from '@/stores/config'
import { useSmartDictionaryStore } from '@/stores/smartDictionary'
import { Setting, Key, InfoFilled, ChatDotRound, Message, Document, Microphone, FolderOpened, Collection } from '@element-plus/icons-vue'
import { open } from '@tauri-apps/plugin-shell'
import { invoke } from '@tauri-apps/api/core'
import {
  CHINESE_PUNCTUATION,
  ENGLISH_PUNCTUATION,
  SPECIAL_PUNCTUATION,
} from '@/utils/punctuation'

const props = defineProps<{
  visible: boolean
}>()

const emit = defineEmits<{
  (e: 'update:visible', value: boolean): void
}>()

const configStore = useConfigStore()
const smartDictionary = useSmartDictionaryStore()

// 当前选中的菜单项
const activeMenu = ref<'general' | 'whisper' | 'dictionary' | 'shortcuts' | 'logs' | 'contact' | 'about'>('general')

// 菜单项配置
const menuItems = [
  { key: 'general', label: '常规设置', icon: Setting },
  { key: 'whisper', label: '语音模型', icon: Microphone },
  { key: 'dictionary', label: '本地词典', icon: Collection },
  { key: 'shortcuts', label: '快捷键列表', icon: Key },
  { key: 'logs', label: '日志', icon: Document },
  { key: 'contact', label: '联系开发者', icon: ChatDotRound },
  { key: 'about', label: '关于', icon: InfoFilled },
] as const

// 词典相关
const dictionaryFilter = ref<'all' | 'manual' | 'auto'>('all')
const newWordCorrect = ref('')
const newWordVariant = ref('')
const newWordVariants = ref<string[]>([]) // 新增：存储多个变体标签
const editingVariantId = ref<string | null>(null) // 当前正在编辑变体的词条ID
const newVariantInput = ref('') // 新变体输入
const dictSearchQuery = ref('') // 词典搜索关键词
const showAddWordDialog = ref(false) // 添加词条弹窗
const addWordInputRef = ref<HTMLInputElement | null>(null) // 添加词条输入框ref

// 添加词条弹窗的ESC键处理
const handleAddWordKeydown = (e: KeyboardEvent) => {
  if (e.key === 'Escape' && showAddWordDialog.value) {
    e.preventDefault()
    e.stopPropagation()
    showAddWordDialog.value = false
  }
}

// 打开添加词条弹窗
const openAddWordDialog = () => {
  showAddWordDialog.value = true
  document.addEventListener('keydown', handleAddWordKeydown, true)
  nextTick(() => {
    addWordInputRef.value?.focus()
  })
}

// 关闭添加词条弹窗
const closeAddWordDialog = () => {
  showAddWordDialog.value = false
  document.removeEventListener('keydown', handleAddWordKeydown, true)
}

const filteredDictionaryEntries = computed(() => {
  let entries = smartDictionary.entries
  if (dictionaryFilter.value === 'manual') {
    entries = smartDictionary.manualEntries
  } else if (dictionaryFilter.value === 'auto') {
    entries = smartDictionary.autoEntries
  }
  // 搜索过滤
  const query = dictSearchQuery.value.trim().toLowerCase()
  if (query) {
    entries = entries.filter(e => 
      e.correct.toLowerCase().includes(query) ||
      e.variants.some(v => v.toLowerCase().includes(query))
    )
  }
  return entries
})

// 添加变体标签
const addVariantTag = () => {
  const text = newWordVariant.value.trim()
  if (!text) return
  // 支持逗号分隔批量添加
  const parts = text.split(/[,，]/).map(v => v.trim()).filter(v => v)
  for (const part of parts) {
    if (!newWordVariants.value.includes(part)) {
      newWordVariants.value.push(part)
    }
  }
  newWordVariant.value = ''
}

// 移除变体标签
const removeVariantTag = (index: number) => {
  newWordVariants.value.splice(index, 1)
}

const addNewWord = () => {
  if (!newWordCorrect.value.trim()) {
    ElMessage.warning('请输入正确写法')
    return
  }
  // 支持从变体输入框直接添加（逗号分隔）
  const variantText = newWordVariant.value.trim()
  const variants = variantText 
    ? variantText.split(/[,，]/).map(v => v.trim()).filter(v => v)
    : []
  smartDictionary.addManual(newWordCorrect.value.trim(), variants)
  newWordCorrect.value = ''
  newWordVariant.value = ''
  newWordVariants.value = []
  closeAddWordDialog()
  ElMessage.success('添加成功')
}

// 开始编辑词条的变体
const startEditVariant = (entryId: string) => {
  editingVariantId.value = entryId
  newVariantInput.value = ''
}

// 自动聚焦指令
const vAutoFocus = {
  mounted: (el: HTMLElement) => {
    setTimeout(() => el.focus(), 0)
  }
}

// 取消编辑变体
const cancelEditVariant = () => {
  editingVariantId.value = null
  newVariantInput.value = ''
}

// 添加变体到词条
const addVariantToEntry = (entryId: string) => {
  const text = newVariantInput.value.trim()
  if (!text) return
  // 支持逗号分隔批量添加
  const parts = text.split(/[,，]/).map(v => v.trim()).filter(v => v)
  for (const part of parts) {
    smartDictionary.addVariant(entryId, part)
  }
  newVariantInput.value = ''
}

// 删除词条的变体
const removeVariantFromEntry = (entryId: string, variant: string) => {
  smartDictionary.removeVariant(entryId, variant)
}

const removeWord = async (id: string) => {
  try {
    await ElMessageBox.confirm('确定要删除这个词条吗？', '删除确认', {
      confirmButtonText: '删除',
      cancelButtonText: '取消',
      type: 'warning'
    })
    smartDictionary.removeEntry(id)
    ElMessage.success('已删除')
  } catch {
    // 取消
  }
}

const clearAllWords = async () => {
  try {
    await ElMessageBox.confirm('确定要清空所有词条吗？此操作不可恢复。', '清空确认', {
      confirmButtonText: '清空',
      cancelButtonText: '取消',
      type: 'warning'
    })
    smartDictionary.clearAll()
    ElMessage.success('已清空')
  } catch {
    // 取消
  }
}

const exportDictionary = () => {
  const json = smartDictionary.exportDictionary()
  const blob = new Blob([json], { type: 'application/json' })
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = 'smart-dictionary.json'
  a.click()
  URL.revokeObjectURL(url)
  ElMessage.success('导出成功')
}

const importDictionary = async () => {
  const input = document.createElement('input')
  input.type = 'file'
  input.accept = '.json'
  input.onchange = async (e) => {
    const file = (e.target as HTMLInputElement).files?.[0]
    if (!file) return
    try {
      const text = await file.text()
      if (smartDictionary.importDictionary(text)) {
        ElMessage.success('导入成功')
      } else {
        ElMessage.error('导入失败，格式不正确')
      }
    } catch {
      ElMessage.error('导入失败')
    }
  }
  input.click()
}

// Whisper 模型相关
interface WhisperModelInfo {
  name: string
  size: string
  downloaded: boolean
  path?: string
}

// SenseVoice 环境状态
interface SenseVoiceEnvStatus {
  uv_installed: boolean
  env_exists: boolean
  ready: boolean
}

// FireRedASR 环境状态
interface FireRedEnvStatus {
  uv_installed: boolean
  env_exists: boolean
  ready: boolean
}

const whisperModels = ref<WhisperModelInfo[]>([])
const downloadingModel = ref<string | null>(null)
const downloadProgress = ref(0)
const downloadMessage = ref('')

// SenseVoice 相关
const sensevoiceStatus = ref<SenseVoiceEnvStatus>({ uv_installed: false, env_exists: false, ready: false })
const isInstallingSensevoice = ref(false)
const sensevoiceProgress = ref(0)
const sensevoiceMessage = ref('')

// FireRedASR 相关
const fireredStatus = ref<FireRedEnvStatus>({ uv_installed: false, env_exists: false, ready: false })
const isInstallingFirered = ref(false)
const fireredProgress = ref(0)
const fireredMessage = ref('')

const fetchWhisperModels = async () => {
  try {
    whisperModels.value = await invoke<WhisperModelInfo[]>('get_whisper_models')
  } catch (e) {
    console.error('Failed to fetch whisper models:', e)
  }
}

const fetchSensevoiceStatus = async () => {
  try {
    sensevoiceStatus.value = await invoke<SenseVoiceEnvStatus>('check_sensevoice_env_status')
  } catch (e) {
    console.error('Failed to fetch sensevoice status:', e)
  }
}

const fetchFireredStatus = async () => {
  try {
    fireredStatus.value = await invoke<FireRedEnvStatus>('check_firered_env_status')
  } catch (e) {
    console.error('Failed to fetch firered status:', e)
  }
}

const installSensevoice = async () => {
  if (!sensevoiceStatus.value.uv_installed) {
    ElMessage.warning('请先安装 uv 包管理器')
    return
  }
  
  isInstallingSensevoice.value = true
  sensevoiceProgress.value = 0
  sensevoiceMessage.value = '准备安装...'
  
  // 监听安装进度
  const unlisten = await listen<{ progress: number; current_text: string }>('sensevoice-progress', (event) => {
    sensevoiceProgress.value = event.payload.progress
    sensevoiceMessage.value = event.payload.current_text
  })
  
  try {
    await invoke('install_sensevoice')
    await fetchSensevoiceStatus()
    ElMessage.success('SenseVoice 环境安装成功')
  } catch (error) {
    ElMessage.error(`安装失败：${error instanceof Error ? error.message : '未知错误'}`)
  } finally {
    isInstallingSensevoice.value = false
    unlisten()
  }
}

const uninstallSensevoice = async () => {
  try {
    await ElMessageBox.confirm('确定要卸载 SenseVoice 环境吗？这将删除所有相关文件。', '卸载确认', {
      confirmButtonText: '卸载',
      cancelButtonText: '取消',
      type: 'warning'
    })
    await invoke('uninstall_sensevoice')
    await fetchSensevoiceStatus()
    ElMessage.success('SenseVoice 环境已卸载')
  } catch (e) {
    if (e !== 'cancel') {
      ElMessage.error(`卸载失败：${e instanceof Error ? e.message : '未知错误'}`)
    }
  }
}

// FireRedASR 安装
const installFirered = async () => {
  if (!fireredStatus.value.uv_installed) {
    ElMessage.warning('请先安装 uv 包管理器')
    return
  }
  
  isInstallingFirered.value = true
  fireredProgress.value = 0
  fireredMessage.value = '准备安装...'
  
  const unlisten = await listen<{ progress: number; current_text: string }>('firered-progress', (event) => {
    fireredProgress.value = event.payload.progress
    fireredMessage.value = event.payload.current_text
  })
  
  try {
    await invoke('install_firered')
    await fetchFireredStatus()
    ElMessage.success('FireRedASR 环境安装成功')
  } catch (error) {
    ElMessage.error(`安装失败：${error instanceof Error ? error.message : '未知错误'}`)
  } finally {
    isInstallingFirered.value = false
    unlisten()
  }
}

const uninstallFirered = async () => {
  try {
    await ElMessageBox.confirm('确定要卸载 FireRedASR 环境吗？这将删除所有相关文件。', '卸载确认', {
      confirmButtonText: '卸载',
      cancelButtonText: '取消',
      type: 'warning'
    })
    await invoke('uninstall_firered')
    await fetchFireredStatus()
    ElMessage.success('FireRedASR 环境已卸载')
  } catch (e) {
    if (e !== 'cancel') {
      ElMessage.error(`卸载失败：${e instanceof Error ? e.message : '未知错误'}`)
    }
  }
}

const downloadWhisperModel = async (modelName: string) => {
  downloadingModel.value = modelName
  downloadProgress.value = 0
  downloadMessage.value = '准备下载...'
  
  try {
    await invoke('download_whisper_model', { modelSize: modelName })
    await fetchWhisperModels()
    ElMessage.success(`模型 ${modelName} 下载完成`)
  } catch (error) {
    ElMessage.error(`下载失败：${error instanceof Error ? error.message : '未知错误'}`)
  } finally {
    downloadingModel.value = null
  }
}

const deleteWhisperModel = async (modelName: string) => {
  try {
    await ElMessageBox.confirm(`确定要卸载模型 ${modelName} 吗？`, '卸载确认', { confirmButtonText: '卸载', cancelButtonText: '取消', type: 'warning' })
    await invoke('delete_whisper_model', { modelSize: modelName })
    await fetchWhisperModels()
    ElMessage.success(`模型 ${modelName} 已卸载`)
  } catch (e) {
    if (e !== 'cancel') {
      ElMessage.error(`卸载失败：${e instanceof Error ? e.message : '未知错误'}`)
    }
  }
}

const openModelDir = async () => {
  try {
    await invoke('open_whisper_model_dir')
  } catch (e) {
    ElMessage.error('无法打开模型目录')
  }
}

const setDefaultModel = (modelName: string) => {
  configStore.transcriptionEngine = 'whisper'
  configStore.whisperModel = modelName
  configStore.saveWhisperSettings()
  ElMessage.success(`已将 Whisper ${modelName} 设为默认模型`)
}

// 监听下载进度
import { listen } from '@tauri-apps/api/event'
let unlistenProgress: (() => void) | null = null

// 日志文件路径
const logPath = ref('')

// 获取日志文件路径
const fetchLogPath = async () => {
  try {
    logPath.value = await invoke<string>('get_log_path')
  } catch {
    logPath.value = '无法获取日志路径'
  }
}

// 在 Finder 中显示日志文件
const showLogInFolder = async () => {
  try {
    await invoke('show_log_in_folder')
  } catch {
    ElMessage.error('无法打开日志文件位置')
  }
}

// 复制日志路径
const copyLogPath = async () => {
  try {
    await navigator.clipboard.writeText(logPath.value)
    ElMessage.success('已复制日志路径')
  } catch {
    ElMessage.error('复制失败')
  }
}

// 初始化时获取日志路径
fetchLogPath()

// 初始化时获取 Whisper 模型列表并监听下载进度
const setupWhisperListener = async () => {
  await fetchWhisperModels()
  await fetchSensevoiceStatus()
  await fetchFireredStatus()
  unlistenProgress = await listen<{ progress: number; current_text: string }>('transcription-progress', (event) => {
    downloadProgress.value = event.payload.progress
    downloadMessage.value = event.payload.current_text
  })
}
setupWhisperListener()

// 联系方式
const contactInfo = {
  email: ' 1017416696@qq.com',
  github: 'https://github.com/1017416696/srt-editor.git',
}

// 打开外部链接
const openLink = async (url: string) => {
  try {
    await open(url)
  } catch {
    ElMessage.error('无法打开链接')
  }
}

// 复制到剪贴板
const copyToClipboard = async (text: string) => {
  try {
    await navigator.clipboard.writeText(text)
    ElMessage.success('已复制到剪贴板')
  } catch {
    ElMessage.error('复制失败')
  }
}

// 关闭弹窗
const handleClose = () => {
  emit('update:visible', false)
}

// ESC 键关闭弹窗
const handleKeydown = (e: KeyboardEvent) => {
  // 如果添加词条弹窗打开，不处理ESC（让添加词条弹窗处理）
  if (e.key === 'Escape' && props.visible && !showAddWordDialog.value) {
    e.preventDefault()
    e.stopPropagation()
    handleClose()
  }
}

// 监听弹窗显示状态，添加/移除键盘监听
watch(() => props.visible, (visible) => {
  if (visible) {
    document.addEventListener('keydown', handleKeydown, true)
    // 每次打开对话框时刷新状态
    fetchSensevoiceStatus()
    fetchFireredStatus()
  } else {
    document.removeEventListener('keydown', handleKeydown, true)
  }
})

// 组件卸载时清理
onBeforeUnmount(() => {
  document.removeEventListener('keydown', handleKeydown, true)
  if (unlistenProgress) unlistenProgress()
})

// 检测平台
const isMac = computed(() => {
  return typeof navigator !== 'undefined' && /Mac|iPhone|iPad|iPod/.test(navigator.platform)
})

// 将快捷键拆分为单独的按键数组
const splitShortcut = (key: string): string[] => {
  // 先替换修饰键符号
  let formatted = key
  if (isMac.value) {
    formatted = formatted
      .replace('Ctrl', '⌘')
      .replace('Shift', '⇧')
      .replace('Alt', '⌥')
  }
  // 按 + 分割成数组
  return formatted.split('+').map(k => k.trim()).filter(k => k)
}

// 应用版本
const appVersion = '0.0.4'

// 切换标点符号
const togglePunctuation = (char: string) => {
  if (configStore.punctuationToRemove.includes(char)) {
    configStore.punctuationToRemove = configStore.punctuationToRemove
      .split('')
      .filter((c) => c !== char)
      .join('')
  } else {
    configStore.punctuationToRemove += char
  }
  configStore.savePunctuation()
}

// 快捷键分类
const shortcutCategories = computed(() => {
  const categories = [
    {
      name: '播放控制',
      actions: ['toggle-play', 'speed-up', 'speed-reset']
    },
    {
      name: '字幕导航',
      actions: ['prev-subtitle', 'next-subtitle']
    },
    {
      name: '字幕编辑',
      actions: ['new-subtitle', 'delete-subtitle', 'split-subtitle', 'merge-subtitles']
    },
    {
      name: '时间调整',
      actions: ['move-subtitle-left', 'move-subtitle-right', 'align-to-waveform', 'toggle-snap']
    },
    {
      name: '波形缩放',
      actions: ['zoom-in', 'zoom-out', 'zoom-reset']
    },
    {
      name: '文件操作',
      actions: ['save-file', 'open-file', 'export-dialog', 'close-tab', 'close-window']
    },
    {
      name: '编辑操作',
      actions: ['undo', 'redo', 'find', 'replace']
    },
    {
      name: '词典',
      actions: ['add-to-dictionary']
    },
    {
      name: '其他',
      actions: ['settings']
    }
  ]

  return categories.map(cat => {
    const bindings = configStore.keyBindings.filter(b => cat.actions.includes(b.action))
    // 按 actions 数组顺序排序
    bindings.sort((a, b) => cat.actions.indexOf(a.action) - cat.actions.indexOf(b.action))
    return { ...cat, bindings }
  }).filter(cat => cat.bindings.length > 0)
})
</script>

<template>
  <Teleport to="body">
    <Transition name="fade">
      <div v-if="visible" class="settings-overlay" @click.self="handleClose">
        <div class="settings-dialog">
          <!-- 侧边栏 -->
          <div class="settings-sidebar">
            <div
              v-for="item in menuItems"
              :key="item.key"
              class="sidebar-item"
              :class="{ active: activeMenu === item.key }"
              @click="activeMenu = item.key"
            >
              <el-icon class="item-icon"><component :is="item.icon" /></el-icon>
              <span class="item-label">{{ item.label }}</span>
            </div>
          </div>

          <!-- 内容区 -->
          <div class="settings-content">
            <!-- 关闭按钮 -->
            <button class="close-btn" @click="handleClose">×</button>

            <!-- 常规设置 -->
            <div v-if="activeMenu === 'general'" class="content-section">
              <h2 class="section-title">常规设置</h2>
              
              <div class="setting-group">
                <div class="setting-item">
                  <div class="setting-info">
                    <span class="setting-label">新增字幕时长</span>
                    <span class="setting-desc">新增字幕的默认持续时间</span>
                  </div>
                  <div class="duration-control">
                    <el-slider
                      v-model="configStore.config.newSubtitleDuration"
                      :min="1"
                      :max="10"
                      :step="0.5"
                      :show-tooltip="false"
                      @change="configStore.saveConfig()"
                    />
                    <span class="duration-value">{{ configStore.config.newSubtitleDuration }}s</span>
                  </div>
                </div>

                <div class="setting-item setting-item-vertical">
                  <div class="setting-header">
                    <div class="setting-info">
                      <span class="setting-label">删除标点符号列表</span>
                      <span class="setting-desc">点击符号切换选中状态，选中的符号会被删除</span>
                    </div>
                    <el-button 
                      size="small" 
                      @click="configStore.resetPunctuation()"
                      :disabled="configStore.punctuationToRemove === DEFAULT_PUNCTUATION"
                    >
                      恢复默认
                    </el-button>
                  </div>
                  
                  <div class="punctuation-categories">
                    <div class="punctuation-category">
                      <span class="category-label">中文标点</span>
                      <div class="punctuation-chars">
                        <span 
                          v-for="char in CHINESE_PUNCTUATION" 
                          :key="char"
                          class="punct-char"
                          :class="{ active: configStore.punctuationToRemove.includes(char) }"
                          @click="togglePunctuation(char)"
                        >{{ char }}</span>
                      </div>
                    </div>
                    
                    <div class="punctuation-category">
                      <span class="category-label">英文标点</span>
                      <div class="punctuation-chars">
                        <span 
                          v-for="char in ENGLISH_PUNCTUATION" 
                          :key="char"
                          class="punct-char"
                          :class="{ active: configStore.punctuationToRemove.includes(char) }"
                          @click="togglePunctuation(char)"
                        >{{ char }}</span>
                      </div>
                    </div>
                    
                    <div class="punctuation-category">
                      <span class="category-label">特殊符号</span>
                      <div class="punctuation-chars">
                        <span 
                          v-for="char in SPECIAL_PUNCTUATION" 
                          :key="char"
                          class="punct-char"
                          :class="{ active: configStore.punctuationToRemove.includes(char) }"
                          @click="togglePunctuation(char)"
                        >{{ char }}</span>
                      </div>
                    </div>
                  </div>
                </div>
              </div>
            </div>

            <!-- 语音模型 -->
            <div v-if="activeMenu === 'whisper'" class="content-section">
              <div class="section-header">
                <h2 class="section-title">语音模型</h2>
                <el-button size="small" @click="openModelDir">
                  <el-icon><FolderOpened /></el-icon>
                  <span>打开模型目录</span>
                </el-button>
              </div>
              
              <!-- Whisper 部分 -->
              <div class="engine-card">
                <div class="engine-header">
                  <div class="engine-icon openai-icon">
                    <svg viewBox="0 0 24 24" fill="currentColor">
                      <path d="M22.282 9.821a5.985 5.985 0 0 0-.516-4.91 6.046 6.046 0 0 0-6.51-2.9A6.065 6.065 0 0 0 4.981 4.18a5.985 5.985 0 0 0-3.998 2.9 6.046 6.046 0 0 0 .743 7.097 5.98 5.98 0 0 0 .51 4.911 6.051 6.051 0 0 0 6.515 2.9A5.985 5.985 0 0 0 13.26 24a6.056 6.056 0 0 0 5.772-4.206 5.99 5.99 0 0 0 3.997-2.9 6.056 6.056 0 0 0-.747-7.073zM13.26 22.43a4.476 4.476 0 0 1-2.876-1.04l.141-.081 4.779-2.758a.795.795 0 0 0 .392-.681v-6.737l2.02 1.168a.071.071 0 0 1 .038.052v5.583a4.504 4.504 0 0 1-4.494 4.494zM3.6 18.304a4.47 4.47 0 0 1-.535-3.014l.142.085 4.783 2.759a.771.771 0 0 0 .78 0l5.843-3.369v2.332a.08.08 0 0 1-.033.062L9.74 19.95a4.5 4.5 0 0 1-6.14-1.646zM2.34 7.896a4.485 4.485 0 0 1 2.366-1.973V11.6a.766.766 0 0 0 .388.676l5.815 3.355-2.02 1.168a.076.076 0 0 1-.071 0l-4.83-2.786A4.504 4.504 0 0 1 2.34 7.872zm16.597 3.855l-5.833-3.387L15.119 7.2a.076.076 0 0 1 .071 0l4.83 2.791a4.494 4.494 0 0 1-.676 8.105v-5.678a.79.79 0 0 0-.407-.667zm2.01-3.023l-.141-.085-4.774-2.782a.776.776 0 0 0-.785 0L9.409 9.23V6.897a.066.066 0 0 1 .028-.061l4.83-2.787a4.5 4.5 0 0 1 6.68 4.66zm-12.64 4.135l-2.02-1.164a.08.08 0 0 1-.038-.057V6.075a4.5 4.5 0 0 1 7.375-3.453l-.142.08L8.704 5.46a.795.795 0 0 0-.393.681zm1.097-2.365l2.602-1.5 2.607 1.5v2.999l-2.597 1.5-2.607-1.5z"/>
                    </svg>
                  </div>
                  <div class="engine-info">
                    <h3 class="engine-title">Whisper</h3>
                    <span class="engine-badge">OpenAI</span>
                  </div>
                </div>
                <p class="engine-desc">OpenAI 开发的语音识别模型，支持多语言，模型越大精度越高。</p>
                
                <div class="models-grid">
                  <div
                    v-for="model in whisperModels"
                    :key="model.name"
                    class="model-card"
                    :class="{ 
                      'is-selected': model.downloaded && configStore.transcriptionEngine === 'whisper' && configStore.whisperModel === model.name, 
                      'is-downloaded': model.downloaded,
                      'is-downloading': downloadingModel === model.name
                    }"
                    @click="model.downloaded && setDefaultModel(model.name)"
                  >
                    <div class="model-card-header">
                      <div class="model-select-indicator">
                        <span class="select-dot" :class="{ 
                          active: model.downloaded && configStore.transcriptionEngine === 'whisper' && configStore.whisperModel === model.name,
                          disabled: !model.downloaded 
                        }"></span>
                      </div>
                      <span class="model-name">{{ model.name }}</span>
                    </div>
                    <span class="model-size">{{ model.size }}</span>
                    <div class="model-card-actions" @click.stop>
                      <template v-if="downloadingModel === model.name">
                        <div class="download-progress-inline">
                          <el-progress :percentage="Math.round(downloadProgress)" :stroke-width="4" :show-text="false" />
                          <span class="progress-text">{{ Math.round(downloadProgress) }}%</span>
                        </div>
                      </template>
                      <template v-else>
                        <el-button v-if="!model.downloaded" size="small" type="primary" :disabled="!!downloadingModel" @click="downloadWhisperModel(model.name)">下载</el-button>
                        <el-button v-else size="small" type="danger" plain :disabled="!!downloadingModel || (configStore.transcriptionEngine === 'whisper' && configStore.whisperModel === model.name)" @click="deleteWhisperModel(model.name)">卸载</el-button>
                      </template>
                    </div>
                  </div>
                </div>
              </div>
              
              <!-- SenseVoice 部分 -->
              <div class="engine-card">
                <div class="engine-header">
                  <div class="engine-icon tongyi-icon">
                    <img src="/qwen.png" alt="通义千问" />
                  </div>
                  <div class="engine-info">
                    <h3 class="engine-title">SenseVoice</h3>
                    <span class="engine-badge alibaba">阿里达摩院</span>
                  </div>
                  <div class="engine-status">
                    <span class="status-badge" :class="{ ready: sensevoiceStatus.ready, pending: !sensevoiceStatus.ready }">
                      {{ sensevoiceStatus.ready ? '已就绪' : (sensevoiceStatus.env_exists ? '依赖不完整' : '未安装') }}
                    </span>
                  </div>
                </div>
                <p class="engine-desc">阿里达摩院开发的语音识别模型，中文识别效果优秀，支持情感识别。</p>
                
                <div v-if="!sensevoiceStatus.uv_installed" class="env-warning">
                  <span class="warning-icon">⚠️</span>
                  <span>需要先安装 <a href="https://docs.astral.sh/uv/getting-started/installation/" target="_blank">uv 包管理器</a></span>
                </div>
                
                <div v-if="isInstallingSensevoice" class="install-progress-card">
                  <el-progress :percentage="Math.round(sensevoiceProgress)" :stroke-width="6" />
                  <span class="install-message">{{ sensevoiceMessage }}</span>
                </div>
                
                <div class="engine-content">
                  <template v-if="!sensevoiceStatus.ready">
                    <el-button 
                      type="primary" 
                      :disabled="isInstallingSensevoice || !sensevoiceStatus.uv_installed"
                      @click="installSensevoice"
                    >
                      {{ isInstallingSensevoice ? '安装中...' : '安装环境' }}
                    </el-button>
                  </template>
                  <template v-else>
                    <div 
                      class="model-card single-model"
                      :class="{ 'is-selected': configStore.transcriptionEngine === 'sensevoice' }"
                      @click="configStore.transcriptionEngine = 'sensevoice'; configStore.saveWhisperSettings()"
                    >
                      <div class="model-card-header">
                        <div class="model-select-indicator">
                          <span class="select-dot" :class="{ active: configStore.transcriptionEngine === 'sensevoice' }"></span>
                        </div>
                        <span class="model-name">SenseVoiceSmall</span>
                      </div>
                      <span class="model-size">~500 MB</span>
                      <div class="model-card-actions" @click.stop>
                        <el-button size="small" type="danger" plain :disabled="configStore.transcriptionEngine === 'sensevoice'" @click="uninstallSensevoice">卸载</el-button>
                      </div>
                    </div>
                  </template>
                </div>
              </div>
              
              <!-- FireRedASR 部分 -->
              <div class="engine-card">
                <div class="engine-header">
                  <div class="engine-icon xiaohongshu-icon">
                    <svg viewBox="0 0 100 100" fill="none">
                      <rect width="100" height="100" rx="20" fill="#fe2c55"/>
                      <text x="50" y="62" text-anchor="middle" fill="#fff" font-size="28" font-weight="700" font-family="PingFang SC, Microsoft YaHei, sans-serif">小红书</text>
                    </svg>
                  </div>
                  <div class="engine-info">
                    <h3 class="engine-title">FireRedASR</h3>
                    <span class="engine-badge xiaohongshu">小红书</span>
                  </div>
                  <div class="engine-status">
                    <span class="status-badge" :class="{ ready: fireredStatus.ready, pending: !fireredStatus.ready }">
                      {{ fireredStatus.ready ? '已就绪' : (fireredStatus.env_exists ? '依赖不完整' : '未安装') }}
                    </span>
                  </div>
                </div>
                <p class="engine-desc">小红书开源的语音识别模型，用于字幕二次校正，可提升识别准确率。</p>
                
                <div v-if="!fireredStatus.uv_installed" class="env-warning">
                  <span class="warning-icon">⚠️</span>
                  <span>需要先安装 <a href="https://docs.astral.sh/uv/getting-started/installation/" target="_blank">uv 包管理器</a></span>
                </div>
                
                <div v-if="isInstallingFirered" class="install-progress-card">
                  <el-progress :percentage="Math.round(fireredProgress)" :stroke-width="6" />
                  <span class="install-message">{{ fireredMessage }}</span>
                </div>
                
                <div class="engine-content">
                  <template v-if="!fireredStatus.ready">
                    <el-button 
                      type="primary" 
                      :disabled="isInstallingFirered || !fireredStatus.uv_installed"
                      @click="installFirered"
                    >
                      {{ isInstallingFirered ? '安装中...' : '安装环境' }}
                    </el-button>
                  </template>
                  <template v-else>
                    <div class="model-card single-model is-downloaded">
                      <div class="model-card-header">
                        <span class="model-name">FireRedASR-AED</span>
                      </div>
                      <span class="model-size">~600 MB</span>
                      <div class="model-card-actions" @click.stop>
                        <el-button size="small" type="danger" plain @click="uninstallFirered">卸载</el-button>
                      </div>
                    </div>
                    
                    <!-- FireRedASR 校正选项 -->
                    <div class="engine-options">
                      <div class="option-row">
                        <div class="option-info">
                          <span class="option-label">保留原始英文大小写</span>
                          <span class="option-desc">校正时保留原字幕中英文字母的大小写格式</span>
                        </div>
                        <el-switch 
                          v-model="configStore.fireredPreserveCase" 
                          @change="configStore.saveWhisperSettings()"
                        />
                      </div>
                    </div>
                  </template>
                </div>
              </div>
              
              <div class="tips-card">
                <div class="tips-header">
                  <span class="tips-icon">💡</span>
                  <span class="tips-title">模型说明</span>
                </div>
                <div class="tips-content">
                  <div class="tip-item"><span class="tip-label">Whisper tiny/base</span><span class="tip-desc">快速预览，适合短音频</span></div>
                  <div class="tip-item"><span class="tip-label">Whisper small/medium</span><span class="tip-desc">平衡选择，日常使用</span></div>
                  <div class="tip-item"><span class="tip-label">Whisper large/turbo</span><span class="tip-desc">高精度，专业场景</span></div>
                  <div class="tip-item"><span class="tip-label">SenseVoice</span><span class="tip-desc">中文识别优秀，首次使用需下载模型</span></div>
                  <div class="tip-item"><span class="tip-label">FireRedASR</span><span class="tip-desc">字幕校正专用，可对已有字幕进行二次校正</span></div>
                </div>
              </div>
            </div>

            <!-- 本地词典 -->
            <div v-if="activeMenu === 'dictionary'" class="content-section dict-section">
              <!-- 顶部标题栏 -->
              <div class="dict-header">
                <div class="dict-header-left">
                  <h2 class="dict-title">本地词典</h2>
                  <span class="dict-count">{{ smartDictionary.totalCount }} 个词条</span>
                </div>
                <div class="dict-header-actions">
                  <button class="dict-action-btn" @click="importDictionary">
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                      <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/>
                      <polyline points="7 10 12 15 17 10"/>
                      <line x1="12" y1="15" x2="12" y2="3"/>
                    </svg>
                    导入
                  </button>
                  <button class="dict-action-btn" @click="exportDictionary">
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                      <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/>
                      <polyline points="17 8 12 3 7 8"/>
                      <line x1="12" y1="3" x2="12" y2="15"/>
                    </svg>
                    导出
                  </button>
                </div>
              </div>

              <!-- 词条列表 -->
              <div class="dict-list">
                <div class="dict-list-header">
                  <div class="dict-search-box">
                    <svg class="search-icon" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                      <circle cx="11" cy="11" r="8"/>
                      <path d="m21 21-4.35-4.35"/>
                    </svg>
                    <input 
                      v-model="dictSearchQuery"
                      type="text"
                      placeholder="搜索词条..."
                      class="dict-search-input"
                    />
                    <button 
                      v-if="dictSearchQuery"
                      class="search-clear-btn"
                      @click="dictSearchQuery = ''"
                    >×</button>
                  </div>
                  <div class="dict-list-actions">
                    <button class="dict-add-entry-btn" @click="openAddWordDialog">
                      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                        <line x1="12" y1="5" x2="12" y2="19"/>
                        <line x1="5" y1="12" x2="19" y2="12"/>
                      </svg>
                      添加
                    </button>
                    <button 
                      v-if="smartDictionary.totalCount > 0"
                      class="clear-all-btn"
                      @click="clearAllWords"
                    >
                      清空
                    </button>
                  </div>
                </div>
                
                <div class="dict-entries">
                  <div 
                    v-for="entry in filteredDictionaryEntries" 
                    :key="entry.id" 
                    class="dict-entry"
                  >
                    <div class="entry-top">
                      <span class="entry-correct">{{ entry.correct }}</span>
                      <div class="entry-actions">
                        <span v-if="entry.useCount > 0" class="entry-count">已用 {{ entry.useCount }} 次</span>
                        <button class="entry-delete" @click="removeWord(entry.id)">×</button>
                      </div>
                    </div>
                    
                    <div class="entry-variants">
                      <span 
                        v-for="(variant, idx) in entry.variants" 
                        :key="idx" 
                        class="variant-tag"
                      >
                        {{ variant }}
                        <button @click="removeVariantFromEntry(entry.id, variant)">×</button>
                      </span>
                      
                      <div v-if="editingVariantId === entry.id" class="variant-input">
                        <input 
                          v-auto-focus
                          v-model="newVariantInput"
                          type="text"
                          placeholder="输入变体后回车"
                          @keyup.enter="addVariantToEntry(entry.id)"
                          @keyup.escape="cancelEditVariant"
                          @blur="newVariantInput.trim() ? addVariantToEntry(entry.id) : cancelEditVariant()"
                        />
                      </div>
                      <button 
                        v-else
                        class="add-variant-btn"
                        @click="startEditVariant(entry.id)"
                      >
                        +
                      </button>
                    </div>
                  </div>
                  
                  <!-- 搜索无结果 -->
                  <div v-if="dictSearchQuery && filteredDictionaryEntries.length === 0" class="dict-empty">
                    <svg width="36" height="36" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
                      <circle cx="11" cy="11" r="8"/>
                      <path d="m21 21-4.35-4.35"/>
                    </svg>
                    <p>未找到匹配的词条</p>
                    <span>尝试其他关键词</span>
                  </div>
                  
                  <!-- 词典为空 -->
                  <div v-else-if="smartDictionary.totalCount === 0" class="dict-empty">
                    <svg width="36" height="36" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
                      <path d="M4 19.5A2.5 2.5 0 0 1 6.5 17H20"/>
                      <path d="M6.5 2H20v20H6.5A2.5 2.5 0 0 1 4 19.5v-15A2.5 2.5 0 0 1 6.5 2z"/>
                    </svg>
                    <p>词典为空</p>
                    <span>添加常用术语，提高识别准确率</span>
                  </div>
                </div>
              </div>
              
              <!-- 添加词条弹窗 -->
              <Teleport to="body">
                <Transition name="fade">
                  <div v-if="showAddWordDialog" class="add-word-overlay" @click.self="closeAddWordDialog">
                    <div class="add-word-dialog">
                      <div class="add-word-header">
                        <h3>添加词条</h3>
                        <button class="add-word-close" @click="closeAddWordDialog">×</button>
                      </div>
                      <div class="add-word-body">
                        <div class="add-word-field">
                          <label>正确写法</label>
                          <input 
                            ref="addWordInputRef"
                            v-model="newWordCorrect"
                            type="text"
                            placeholder="输入正确的词语，如 Kubernetes"
                            @keyup.enter="addNewWord"
                          />
                        </div>
                        <div class="add-word-field">
                          <label>错误变体 <span class="field-hint">可选</span></label>
                          <input 
                            v-model="newWordVariant"
                            type="text"
                            placeholder="语音识别可能出现的错误写法，多个用逗号分隔"
                            @keyup.enter="addNewWord"
                          />
                          <p class="field-desc">例如：酷伯内特斯, K8S, 库伯内特斯</p>
                        </div>
                      </div>
                      <div class="add-word-footer">
                        <button class="add-word-cancel" @click="closeAddWordDialog">取消</button>
                        <button 
                          class="add-word-confirm" 
                          :disabled="!newWordCorrect.trim()"
                          @click="addNewWord"
                        >
                          添加
                        </button>
                      </div>
                    </div>
                  </div>
                </Transition>
              </Teleport>
            </div>

            <!-- 快捷键列表 -->
            <div v-if="activeMenu === 'shortcuts'" class="content-section">
              <h2 class="section-title">快捷键列表</h2>
              
              <div class="shortcuts-categories">
                <div
                  v-for="category in shortcutCategories"
                  :key="category.name"
                  class="shortcut-category"
                >
                  <div class="category-header">
                    <span class="category-name">{{ category.name }}</span>
                  </div>
                  <div class="shortcuts-list">
                    <div
                      v-for="binding in category.bindings"
                      :key="binding.action"
                      class="shortcut-item"
                    >
                      <span class="shortcut-desc">{{ binding.description }}</span>
                      <div class="shortcut-keys">
                        <kbd
                          v-for="(k, index) in splitShortcut(binding.key)"
                          :key="index"
                          class="key-cap"
                        >{{ k }}</kbd>
                      </div>
                    </div>
                  </div>
                </div>
              </div>
            </div>

            <!-- 日志 -->
            <div v-if="activeMenu === 'logs'" class="content-section">
              <h2 class="section-title">日志</h2>
              
              <div class="logs-content">
                <div class="log-card">
                  <div class="log-icon">
                    <el-icon :size="32"><Document /></el-icon>
                  </div>
                  <div class="log-info">
                    <h4>应用日志</h4>
                    <p>记录关键操作和错误信息，可用于问题排查</p>
                  </div>
                </div>
                
                <div class="log-actions">
                  <el-button size="large" @click="showLogInFolder">
                    打开日志目录
                  </el-button>
                  <el-button size="large" @click="copyLogPath">
                    复制目录路径
                  </el-button>
                </div>
              </div>
            </div>

            <!-- 联系开发者 -->
            <div v-if="activeMenu === 'contact'" class="content-section">
              <h2 class="section-title">联系开发者</h2>
              
              <div class="contact-content">
                <p class="contact-intro">如果您有任何问题、建议或反馈，欢迎扫码加入用户交流群：</p>
                
                <!-- 群二维码 -->
                <div class="qrcode-section">
                  <div class="qrcode-wrapper">
                    <img src="/qrcode-placeholder.JPG" alt="用户交流群二维码" class="qrcode-img" />
                  </div>
                  <p class="qrcode-hint">QQ扫码加入用户交流群</p>
                </div>
                
                <div class="contact-list">
                  <div class="contact-item">
                    <div class="contact-info">
                      <span class="contact-label"><el-icon><Message /></el-icon> 邮箱</span>
                      <span class="contact-value">{{ contactInfo.email }}</span>
                    </div>
                    <el-button size="small" @click="copyToClipboard(contactInfo.email)">复制</el-button>
                  </div>
                  
                  <div class="contact-item">
                    <div class="contact-info">
                      <span class="contact-label">
                        <svg class="github-icon" viewBox="0 0 16 16" fill="currentColor">
                          <path d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.013 8.013 0 0016 8c0-4.42-3.58-8-8-8z"/>
                        </svg>
                        GitHub
                      </span>
                      <span class="contact-value">{{ contactInfo.github }}</span>
                    </div>
                    <el-button size="small" @click="openLink(contactInfo.github)">访问</el-button>
                  </div>
                </div>
              </div>
            </div>

            <!-- 关于 -->
            <div v-if="activeMenu === 'about'" class="content-section">
              <h2 class="section-title">关于</h2>
              
              <div class="about-content">
                <div class="app-logo">
                  <img src="/favicon.ico" alt="SRT Editor" class="logo-img" />
                </div>
                <h3 class="app-name">SRT 字幕编辑器</h3>
                <p class="app-version">版本 {{ appVersion }}</p>
                <p class="app-desc">
                  一款简洁高效的 SRT 字幕编辑工具，支持音频波形显示、字幕时间轴编辑等功能。
                </p>
                <div class="app-links">
                  <span class="copyright">© 2025 Penrose</span>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
.settings-overlay {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: rgba(0, 0, 0, 0.5);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 2000;
}

.settings-dialog {
  width: 800px;
  max-width: 92vw;
  min-width: 600px;
  height: 640px;
  max-height: 88vh;
  min-height: 500px;
  background: #fff;
  border-radius: 12px;
  box-shadow: 0 20px 60px rgba(0, 0, 0, 0.2);
  display: flex;
  overflow: hidden;
}

/* 侧边栏 */
.settings-sidebar {
  width: 180px;
  background: #f7f7f7;
  border-right: 1px solid #e5e5e5;
  padding: 16px 8px;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.sidebar-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px 12px;
  border-radius: 8px;
  cursor: pointer;
  transition: all 0.2s ease;
  color: #666;
}

.sidebar-item:hover {
  background: #ebebeb;
  color: #333;
}

.sidebar-item.active {
  background: #e8e8e8;
  color: #333;
  font-weight: 500;
}

.item-icon {
  font-size: 18px;
}

.item-label {
  font-size: 14px;
}

/* 内容区 */
.settings-content {
  flex: 1;
  padding: 24px 32px;
  overflow: hidden;
  position: relative;
  display: flex;
  flex-direction: column;
}

.content-section {
  display: flex;
  flex-direction: column;
  flex: 1;
  overflow-y: auto;
  padding-right: 8px;
}

.close-btn {
  position: absolute;
  top: 16px;
  right: 16px;
  width: 28px;
  height: 28px;
  border: none;
  background: transparent;
  font-size: 20px;
  color: #999;
  cursor: pointer;
  border-radius: 6px;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all 0.2s;
}

.close-btn:hover {
  background: #f0f0f0;
  color: #333;
}

.section-title {
  font-size: 20px;
  font-weight: 600;
  color: #333;
  margin: 0;
}

.section-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 20px;
}

.section-header .el-button {
  display: flex;
  align-items: center;
  gap: 6px;
}

/* 设置项 */
.setting-group {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.setting-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px 0;
  border-bottom: 1px solid #f0f0f0;
}

.setting-item:last-child {
  border-bottom: none;
}

.setting-item-vertical {
  flex-direction: column;
  align-items: stretch;
  gap: 12px;
}

.setting-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.punctuation-categories {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.punctuation-category {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.category-label {
  font-size: 12px;
  color: #888;
  font-weight: 500;
}

.punctuation-chars {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.punct-char {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 32px;
  height: 32px;
  padding: 0 6px;
  background: #f5f5f5;
  border: 1px solid #e0e0e0;
  border-radius: 6px;
  font-size: 15px;
  font-family: monospace;
  color: #666;
  cursor: pointer;
  transition: all 0.15s ease;
  user-select: none;
}

.punct-char:hover {
  background: #eee;
  border-color: #ccc;
}

.punct-char.active {
  background: #e0f2fe;
  border-color: #38bdf8;
  color: #0284c7;
}

.setting-info {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.setting-label {
  font-size: 14px;
  font-weight: 500;
  color: #333;
}

.setting-desc {
  font-size: 12px;
  color: #999;
}

/* 时长控制 */
.duration-control {
  display: flex;
  align-items: center;
  gap: 12px;
  width: 140px;
}

.duration-control .el-slider {
  flex: 1;
}

.duration-value {
  font-size: 13px;
  color: #666;
  min-width: 32px;
  text-align: right;
}

/* 快捷键列表 */
.shortcuts-categories {
  display: flex;
  flex-direction: column;
  gap: 16px;
  flex: 1;
  overflow-y: auto;
  padding-right: 8px;
}

.shortcut-category {
  background: #fafafa;
  border-radius: 10px;
  padding: 12px 16px;
}

.category-header {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 10px;
  padding-bottom: 8px;
  border-bottom: 1px solid #eee;
}

.category-icon {
  font-size: 14px;
}

.category-name {
  font-size: 13px;
  font-weight: 600;
  color: #555;
}

.shortcuts-list {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.shortcut-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 6px 0;
}

.shortcut-desc {
  font-size: 13px;
  color: #444;
}

.shortcut-keys {
  display: flex;
  align-items: center;
  gap: 4px;
}

.key-cap {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 28px;
  height: 26px;
  padding: 0 8px;
  background: #fff;
  border: 1px solid #ddd;
  border-radius: 6px;
  font-size: 12px;
  font-family: -apple-system, BlinkMacSystemFont, 'SF Pro Text', sans-serif;
  color: #555;
  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.04);
}

/* 关于页面 */
.about-content {
  display: flex;
  flex-direction: column;
  align-items: center;
  text-align: center;
  padding-top: 20px;
}

.app-logo {
  width: 72px;
  height: 72px;
  margin-bottom: 16px;
}

.logo-img {
  width: 100%;
  height: 100%;
  object-fit: contain;
}

.app-name {
  font-size: 20px;
  font-weight: 600;
  color: #333;
  margin-bottom: 8px;
}

.app-version {
  font-size: 14px;
  color: #999;
  margin-bottom: 16px;
}

.app-desc {
  font-size: 14px;
  color: #666;
  line-height: 1.6;
  max-width: 360px;
  margin-bottom: 24px;
}

.copyright {
  font-size: 12px;
  color: #bbb;
}

/* 日志页面 */
.logs-content {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding-top: 40px;
  gap: 32px;
}

.log-card {
  display: flex;
  flex-direction: column;
  align-items: center;
  text-align: center;
  gap: 16px;
}

.log-icon {
  width: 64px;
  height: 64px;
  background: #f5f5f5;
  border-radius: 16px;
  display: flex;
  align-items: center;
  justify-content: center;
  color: #666;
}

.log-info h4 {
  font-size: 16px;
  font-weight: 600;
  color: #333;
  margin-bottom: 6px;
}

.log-info p {
  font-size: 13px;
  color: #999;
}

.log-actions {
  display: flex;
  gap: 12px;
}

/* 联系开发者页面 */
.contact-content {
  display: flex;
  flex-direction: column;
  gap: 20px;
}

.contact-intro {
  font-size: 14px;
  color: #666;
  line-height: 1.6;
}

.contact-list {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.contact-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 16px;
  background: #f9f9f9;
  border-radius: 8px;
}

.contact-info {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.contact-label {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 14px;
  font-weight: 500;
  color: #333;
}

.github-icon {
  width: 1em;
  height: 1em;
}

.contact-value {
  font-size: 13px;
  color: #666;
}

/* 群二维码 */
.qrcode-section {
  display: flex;
  flex-direction: column;
  align-items: center;
  padding: 20px;
  background: #f9f9f9;
  border-radius: 12px;
}

.qrcode-wrapper {
  width: 160px;
  height: 160px;
  background: #fff;
  border-radius: 8px;
  padding: 8px;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.06);
}

.qrcode-img {
  width: 100%;
  height: 100%;
  object-fit: contain;
}

.qrcode-hint {
  margin-top: 12px;
  font-size: 13px;
  color: #666;
}

/* 语音模型页面 - 新设计 */
.engine-card {
  background: #fff;
  border: 1px solid #e8e8e8;
  border-radius: 12px;
  padding: 20px;
  margin-bottom: 16px;
  transition: box-shadow 0.2s;
}

.engine-card:hover {
  box-shadow: 0 2px 12px rgba(0, 0, 0, 0.06);
}

.engine-header {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 12px;
}

.engine-icon {
  width: 40px;
  height: 40px;
  border-radius: 10px;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.engine-icon svg {
  width: 22px;
  height: 22px;
}

.openai-icon {
  background: #000;
  color: #fff;
}

.tongyi-icon {
  background: transparent;
  padding: 0;
}

.tongyi-icon img {
  width: 40px;
  height: 40px;
  object-fit: contain;
}

.xiaohongshu-icon {
  background: transparent;
  padding: 0;
  border-radius: 8px;
  overflow: hidden;
}

.xiaohongshu-icon svg {
  width: 40px;
  height: 40px;
}

.engine-info {
  display: flex;
  align-items: center;
  gap: 10px;
  flex: 1;
}

.engine-title {
  font-size: 16px;
  font-weight: 600;
  color: #333;
  margin: 0;
}

.engine-badge {
  font-size: 11px;
  padding: 2px 8px;
  border-radius: 10px;
  background: #f0f0f0;
  color: #666;
}

.engine-badge.alibaba {
  background: linear-gradient(135deg, #6366f1 0%, #8b5cf6 100%);
  color: #fff;
}

.engine-badge.xiaohongshu {
  background: linear-gradient(135deg, #fe2c55 0%, #ff6b6b 100%);
  color: #fff;
}

.engine-status {
  display: flex;
  align-items: center;
  gap: 8px;
}

.status-badge {
  font-size: 12px;
  font-weight: 500;
  padding: 4px 10px;
  border-radius: 12px;
}

.status-badge.ready {
  color: #52c41a;
  background: rgba(82, 196, 26, 0.1);
}

.status-badge.pending {
  color: #faad14;
  background: rgba(250, 173, 20, 0.1);
}

.engine-desc {
  font-size: 13px;
  color: #666;
  line-height: 1.5;
  margin: 0 0 16px;
}

.engine-content {
  margin-top: 12px;
}

/* 环境警告 */
.env-warning {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 14px;
  background: #fffbe6;
  border: 1px solid #ffe58f;
  border-radius: 8px;
  margin-bottom: 12px;
  font-size: 13px;
  color: #ad6800;
}

.env-warning a {
  color: #1890ff;
  text-decoration: none;
}

.env-warning a:hover {
  text-decoration: underline;
}

/* 安装进度 */
.install-progress-card {
  padding: 12px 16px;
  background: #f6f8fa;
  border-radius: 8px;
  margin-bottom: 12px;
}

.install-message {
  display: block;
  font-size: 12px;
  color: #909399;
  margin-top: 8px;
}

/* 模型网格 */
.models-grid {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 10px;
}

.model-card {
  display: flex;
  flex-direction: column;
  padding: 14px;
  background: #f9fafb;
  border: 1px solid #e5e7eb;
  border-radius: 10px;
  transition: all 0.2s;
  cursor: default;
}

.model-card.is-downloaded {
  cursor: pointer;
}

.model-card.is-downloaded:hover {
  background: #f3f4f6;
  border-color: #d1d5db;
}

.model-card.is-selected {
  background: #eff6ff;
  border-color: #93c5fd;
}

.model-card.is-downloading {
  background: #fefce8;
  border-color: #fde047;
}

.model-card.single-model {
  flex-direction: row;
  align-items: center;
  gap: 12px;
}

.model-card.single-model .model-card-header {
  flex: 1;
  margin-bottom: 0;
}

.model-card.single-model .model-size {
  margin-bottom: 0;
}

.model-card-header {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 6px;
}

.model-select-indicator {
  width: 16px;
  height: 16px;
  display: flex;
  align-items: center;
  justify-content: center;
}

.select-dot {
  width: 14px;
  height: 14px;
  border-radius: 50%;
  border: 2px solid #d1d5db;
  transition: all 0.2s;
}

.select-dot.active {
  border-color: #3b82f6;
  background: #3b82f6;
  box-shadow: inset 0 0 0 2px #fff;
}

.select-dot.disabled {
  border-color: #e5e7eb;
  background: #f3f4f6;
}

.model-name {
  font-size: 14px;
  font-weight: 600;
  color: #374151;
}

.model-size {
  font-size: 12px;
  color: #9ca3af;
  margin-bottom: 10px;
}

.model-card-actions {
  margin-top: auto;
}

.download-progress-inline {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.download-progress-inline .el-progress {
  width: 100%;
}

.progress-text {
  font-size: 11px;
  color: #6b7280;
  text-align: center;
}

/* 引擎选项 */
.engine-options {
  margin-top: 16px;
  padding: 14px 16px;
  background: #f9fafb;
  border-radius: 10px;
}

.option-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.option-info {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.option-label {
  font-size: 14px;
  font-weight: 500;
  color: #374151;
}

.option-desc {
  font-size: 12px;
  color: #9ca3af;
}

/* 词典相关样式 */
.add-word-form {
  display: flex;
  gap: 12px;
  margin-bottom: 16px;
  padding: 16px;
  background: #f8fafc;
  border-radius: 8px;
}

.filter-tabs {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 16px;
}

.dictionary-list {
  max-height: 400px;
  overflow-y: auto;
  border: 1px solid #e5e7eb;
  border-radius: 8px;
}

.dict-item {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 12px 16px;
  border-bottom: 1px solid #f3f4f6;
  transition: background 0.2s;
}

/* 本地词典专用样式 */
.dict-section {
  display: flex;
  flex-direction: column;
  gap: 16px;
  flex: 1;
  min-height: 0;
  overflow: hidden;
}

.dict-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.dict-header-left {
  display: flex;
  align-items: baseline;
  gap: 12px;
}

.dict-title {
  font-size: 18px;
  font-weight: 600;
  color: #1f2937;
  margin: 0;
}

.dict-count {
  font-size: 13px;
  color: #9ca3af;
}

.dict-header-actions {
  display: flex;
  gap: 8px;
}

.dict-action-btn {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 6px 12px;
  font-size: 13px;
  color: #6b7280;
  background: #f3f4f6;
  border: none;
  border-radius: 6px;
  cursor: pointer;
  transition: all 0.15s;
}

.dict-action-btn:hover {
  background: #e5e7eb;
  color: #374151;
}

/* 列表头部操作按钮 */
.dict-list-actions {
  display: flex;
  align-items: center;
  gap: 8px;
}

.dict-add-entry-btn {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 6px 12px;
  font-size: 13px;
  font-weight: 500;
  color: white;
  background: #10b981;
  border: none;
  border-radius: 6px;
  cursor: pointer;
  transition: all 0.15s;
}

.dict-add-entry-btn:hover {
  background: #059669;
}

/* 添加词条弹窗 */
.add-word-overlay {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: rgba(0, 0, 0, 0.4);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 3000;
}

.add-word-dialog {
  width: 420px;
  background: white;
  border-radius: 12px;
  box-shadow: 0 20px 40px rgba(0, 0, 0, 0.15);
  overflow: hidden;
}

.add-word-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 16px 20px;
  border-bottom: 1px solid #e5e7eb;
}

.add-word-header h3 {
  margin: 0;
  font-size: 16px;
  font-weight: 600;
  color: #1f2937;
}

.add-word-close {
  width: 28px;
  height: 28px;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 20px;
  color: #9ca3af;
  background: none;
  border: none;
  border-radius: 6px;
  cursor: pointer;
  transition: all 0.15s;
}

.add-word-close:hover {
  background: #f3f4f6;
  color: #6b7280;
}

.add-word-body {
  padding: 20px;
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.add-word-field {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.add-word-field label {
  font-size: 13px;
  font-weight: 500;
  color: #374151;
}

.field-hint {
  font-weight: 400;
  color: #9ca3af;
}

.add-word-field input {
  height: 40px;
  padding: 0 12px;
  font-size: 14px;
  color: #374151;
  background: #fff;
  border: 1px solid #d1d5db;
  border-radius: 8px;
  outline: none;
  transition: all 0.15s;
}

.add-word-field input:focus {
  border-color: #10b981;
  box-shadow: 0 0 0 3px rgba(16, 185, 129, 0.1);
}

.add-word-field input::placeholder {
  color: #9ca3af;
}

.field-desc {
  margin: 0;
  font-size: 12px;
  color: #9ca3af;
}

.add-word-footer {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
  padding: 16px 20px;
  background: #f9fafb;
  border-top: 1px solid #e5e7eb;
}

.add-word-cancel {
  padding: 8px 16px;
  font-size: 14px;
  color: #6b7280;
  background: white;
  border: 1px solid #d1d5db;
  border-radius: 6px;
  cursor: pointer;
  transition: all 0.15s;
}

.add-word-cancel:hover {
  background: #f3f4f6;
}

.add-word-confirm {
  padding: 8px 20px;
  font-size: 14px;
  font-weight: 500;
  color: white;
  background: #10b981;
  border: none;
  border-radius: 6px;
  cursor: pointer;
  transition: all 0.15s;
}

.add-word-confirm:hover:not(:disabled) {
  background: #059669;
}

.add-word-confirm:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

/* 词条列表 */
.dict-list {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-height: 0;
  background: #fff;
  border: 1px solid #e2e8f0;
  border-radius: 10px;
  overflow: hidden;
}

.dict-list-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 12px;
  padding: 10px 14px;
  background: #f8fafc;
  border-bottom: 1px solid #e2e8f0;
}

.dict-search-box {
  display: flex;
  align-items: center;
  flex: 1;
  max-width: 240px;
  height: 30px;
  padding: 0 10px;
  background: #fff;
  border: 1px solid #e2e8f0;
  border-radius: 6px;
  transition: all 0.15s;
}

.dict-search-box:focus-within {
  border-color: #10b981;
  box-shadow: 0 0 0 2px rgba(16, 185, 129, 0.1);
}

.search-icon {
  color: #9ca3af;
  flex-shrink: 0;
}

.dict-search-input {
  flex: 1;
  height: 100%;
  padding: 0 8px;
  font-size: 13px;
  color: #374151;
  background: transparent;
  border: none;
  outline: none;
}

.dict-search-input::placeholder {
  color: #9ca3af;
}

.search-clear-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 16px;
  height: 16px;
  min-width: 16px;
  min-height: 16px;
  padding: 0;
  font-size: 12px;
  line-height: 1;
  color: #9ca3af;
  background: #e5e7eb;
  border: none;
  border-radius: 50%;
  cursor: pointer;
  transition: all 0.15s;
  flex-shrink: 0;
}

.search-clear-btn:hover {
  background: #d1d5db;
  color: #6b7280;
}

.clear-all-btn {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 6px 12px;
  font-size: 13px;
  font-weight: 500;
  color: #ef4444;
  background: #fff;
  border: 1px solid #fecaca;
  border-radius: 6px;
  cursor: pointer;
  transition: all 0.15s;
}

.clear-all-btn:hover {
  background: #fef2f2;
  border-color: #fca5a5;
}

.dict-entries {
  flex: 1;
  overflow-y: auto;
  min-height: 0;
}

.dict-entry {
  padding: 12px 14px;
  border-bottom: 1px solid #f1f5f9;
}

.dict-entry:last-child {
  border-bottom: none;
}

.entry-top {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 6px;
}

.entry-correct {
  font-size: 15px;
  font-weight: 600;
  color: #10b981;
}

.entry-actions {
  display: flex;
  align-items: center;
  gap: 8px;
}

.entry-count {
  font-size: 11px;
  color: #9ca3af;
}

.entry-delete {
  width: 20px;
  height: 20px;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 14px;
  color: #9ca3af;
  background: none;
  border: none;
  border-radius: 4px;
  cursor: pointer;
  transition: all 0.15s;
}

.entry-delete:hover {
  color: #ef4444;
  background: #fef2f2;
}

.entry-variants {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 6px;
}

.variant-tag {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 2px 8px;
  font-size: 12px;
  background: #fef3c7;
  color: #92400e;
  border-radius: 4px;
}

.variant-tag button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 14px;
  height: 14px;
  font-size: 12px;
  color: #b45309;
  background: none;
  border: none;
  border-radius: 50%;
  cursor: pointer;
  opacity: 0.6;
  transition: all 0.15s;
}

.variant-tag button:hover {
  opacity: 1;
  background: rgba(180, 83, 9, 0.15);
}

.variant-input {
  display: inline-flex;
}

.variant-input input {
  width: 120px;
  padding: 4px 8px;
  font-size: 13px;
  color: #374151;
  border: 1px solid #10b981;
  border-radius: 4px;
  outline: none;
  background: white;
}

.variant-input input::placeholder {
  color: #9ca3af;
}

.variant-input input:focus {
  box-shadow: 0 0 0 2px rgba(16, 185, 129, 0.15);
}

.add-variant-btn {
  width: 24px;
  height: 24px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  font-size: 14px;
  color: #9ca3af;
  background: none;
  border: 1px dashed #d1d5db;
  border-radius: 4px;
  cursor: pointer;
  transition: all 0.15s;
}

.add-variant-btn:hover {
  color: #10b981;
  border-color: #10b981;
  background: #f0fdf4;
}

.dict-empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 48px 24px;
  color: #9ca3af;
}

.dict-empty svg {
  margin-bottom: 12px;
  opacity: 0.5;
}

.dict-empty p {
  margin: 0 0 4px;
  font-size: 14px;
  font-weight: 500;
  color: #6b7280;
}

.dict-empty span {
  font-size: 13px;
}

/* 提示卡片 */
.tips-card {
  background: linear-gradient(135deg, #f0f9ff 0%, #e0f2fe 100%);
  border: 1px solid #bae6fd;
  border-radius: 12px;
  padding: 16px 20px;
  margin-top: 8px;
}

.tips-header {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 12px;
}

.tips-icon {
  font-size: 16px;
}

.tips-title {
  font-size: 14px;
  font-weight: 600;
  color: #0369a1;
}

.tips-content {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.tip-item {
  display: flex;
  align-items: baseline;
  gap: 8px;
  font-size: 13px;
}

.tip-label {
  font-weight: 500;
  color: #0c4a6e;
  min-width: 140px;
}

.tip-desc {
  color: #0369a1;
}

/* 过渡动画 */
.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.2s ease;
}

.fade-enter-active .settings-dialog,
.fade-leave-active .settings-dialog {
  transition: transform 0.2s ease, opacity 0.2s ease;
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}

.fade-enter-from .settings-dialog,
.fade-leave-to .settings-dialog {
  transform: scale(0.95);
  opacity: 0;
}
</style>

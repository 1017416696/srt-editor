<script setup lang="ts">
import { ref, computed, watch, onBeforeUnmount } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { useConfigStore, DEFAULT_PUNCTUATION } from '@/stores/config'
import { Setting, Key, InfoFilled, ChatDotRound, Message, Document, Microphone, FolderOpened } from '@element-plus/icons-vue'
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

// 当前选中的菜单项
const activeMenu = ref<'general' | 'whisper' | 'shortcuts' | 'logs' | 'contact' | 'about'>('general')

// 菜单项配置
const menuItems = [
  { key: 'general', label: '常规设置', icon: Setting },
  { key: 'whisper', label: '语音模型', icon: Microphone },
  { key: 'shortcuts', label: '快捷键列表', icon: Key },
  { key: 'logs', label: '日志', icon: Document },
  { key: 'contact', label: '联系开发者', icon: ChatDotRound },
  { key: 'about', label: '关于', icon: InfoFilled },
] as const

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
    await ElMessageBox.confirm(`确定要删除模型 ${modelName} 吗？`, '删除模型', { confirmButtonText: '删除', cancelButtonText: '取消', type: 'warning' })
    await invoke('delete_whisper_model', { modelSize: modelName })
    await fetchWhisperModels()
    ElMessage.success(`模型 ${modelName} 已删除`)
  } catch (e) {
    if (e !== 'cancel') {
      ElMessage.error(`删除失败：${e instanceof Error ? e.message : '未知错误'}`)
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
  if (e.key === 'Escape' && props.visible) {
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
      actions: ['save-file', 'open-file', 'close-tab', 'close-window']
    },
    {
      name: '编辑操作',
      actions: ['undo', 'redo', 'find', 'replace']
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
                <el-button @click="openModelDir">打开模型目录</el-button>
              </div>
              
              <!-- Whisper 部分 -->
              <div class="engine-section">
                <h3 class="engine-title">Whisper (OpenAI)</h3>
                <p class="engine-desc">OpenAI 开发的语音识别模型，支持多语言，模型越大精度越高。</p>
                
                <div class="whisper-models-list">
                  <div
                    v-for="model in whisperModels"
                    :key="model.name"
                    class="whisper-model-item"
                    :class="{ 'is-default': model.downloaded && configStore.transcriptionEngine === 'whisper' && configStore.whisperModel === model.name, 'is-downloaded': model.downloaded }"
                    @click="model.downloaded && setDefaultModel(model.name)"
                  >
                    <div class="model-radio">
                      <span v-if="model.downloaded" class="radio-dot" :class="{ active: configStore.transcriptionEngine === 'whisper' && configStore.whisperModel === model.name }"></span>
                      <span v-else class="radio-placeholder"></span>
                    </div>
                    <div class="model-info">
                      <span class="model-name">{{ model.name }}</span>
                      <span class="model-size">{{ model.size }}</span>
                    </div>
                    <div class="model-actions" @click.stop>
                      <template v-if="downloadingModel === model.name">
                        <div class="download-progress">
                          <el-progress :percentage="Math.round(downloadProgress)" :stroke-width="6" :show-text="false" style="width: 100px" />
                          <span class="progress-text">{{ Math.round(downloadProgress) }}%</span>
                        </div>
                      </template>
                      <template v-else>
                        <el-button v-if="!model.downloaded" size="small" type="primary" :disabled="!!downloadingModel" @click="downloadWhisperModel(model.name)">下载</el-button>
                        <el-button v-else size="small" type="danger" plain :disabled="!!downloadingModel || (configStore.transcriptionEngine === 'whisper' && configStore.whisperModel === model.name)" @click="deleteWhisperModel(model.name)">删除</el-button>
                      </template>
                    </div>
                  </div>
                </div>
              </div>
              
              <!-- SenseVoice 部分 -->
              <div class="engine-section sensevoice-section">
                <h3 class="engine-title">SenseVoice (阿里)</h3>
                <p class="engine-desc">阿里达摩院开发的语音识别模型，中文识别效果优秀，支持情感识别。</p>
                
                <div class="sensevoice-status">
                  <div class="status-item">
                    <span class="status-label">环境状态</span>
                    <span class="status-value" :class="{ ready: sensevoiceStatus.ready, pending: !sensevoiceStatus.ready }">
                      {{ sensevoiceStatus.ready ? '已就绪' : (sensevoiceStatus.env_exists ? '依赖不完整' : '未安装') }}
                    </span>
                  </div>
                  <div v-if="!sensevoiceStatus.uv_installed" class="status-item">
                    <span class="status-label">uv 包管理器</span>
                    <span class="status-value pending">未安装</span>
                  </div>
                  <el-button size="small" text @click="fetchSensevoiceStatus">
                    <i class="i-mdi-refresh"></i> 刷新状态
                  </el-button>
                </div>
                
                <div v-if="isInstallingSensevoice" class="install-progress">
                  <el-progress :percentage="Math.round(sensevoiceProgress)" :stroke-width="8" />
                  <span class="install-message">{{ sensevoiceMessage }}</span>
                </div>
                
                <div class="sensevoice-actions">
                  <template v-if="!sensevoiceStatus.ready">
                    <el-button 
                      type="primary" 
                      :disabled="isInstallingSensevoice || !sensevoiceStatus.uv_installed"
                      @click="installSensevoice"
                    >
                      {{ isInstallingSensevoice ? '安装中...' : '安装 SenseVoice 环境' }}
                    </el-button>
                    <p v-if="!sensevoiceStatus.uv_installed" class="uv-hint">
                      需要先安装 <a href="https://docs.astral.sh/uv/getting-started/installation/" target="_blank">uv 包管理器</a>
                    </p>
                  </template>
                  <template v-else>
                    <div 
                      class="whisper-model-item is-downloaded"
                      :class="{ 'is-default': configStore.transcriptionEngine === 'sensevoice' }"
                      @click="configStore.transcriptionEngine = 'sensevoice'; configStore.saveWhisperSettings()"
                    >
                      <div class="model-radio">
                        <span class="radio-dot" :class="{ active: configStore.transcriptionEngine === 'sensevoice' }"></span>
                      </div>
                      <div class="model-info">
                        <span class="model-name">SenseVoiceSmall</span>
                        <span class="model-size">~500 MB</span>
                      </div>
                      <div class="model-actions" @click.stop>
                        <el-button size="small" type="danger" plain :disabled="configStore.transcriptionEngine === 'sensevoice'" @click="uninstallSensevoice">卸载</el-button>
                      </div>
                    </div>
                  </template>
                </div>
              </div>
              
              <!-- FireRedASR 部分 -->
              <div class="engine-section firered-section">
                <h3 class="engine-title">FireRedASR (小红书)</h3>
                <p class="engine-desc">小红书开源的语音识别模型，用于字幕二次校正，可提升识别准确率。</p>
                
                <div class="sensevoice-status">
                  <div class="status-item">
                    <span class="status-label">环境状态</span>
                    <span class="status-value" :class="{ ready: fireredStatus.ready, pending: !fireredStatus.ready }">
                      {{ fireredStatus.ready ? '已就绪' : (fireredStatus.env_exists ? '依赖不完整' : '未安装') }}
                    </span>
                  </div>
                  <el-button size="small" text @click="fetchFireredStatus">
                    <i class="i-mdi-refresh"></i> 刷新状态
                  </el-button>
                </div>
                
                <div v-if="isInstallingFirered" class="install-progress">
                  <el-progress :percentage="Math.round(fireredProgress)" :stroke-width="8" />
                  <span class="install-message">{{ fireredMessage }}</span>
                </div>
                
                <div class="sensevoice-actions">
                  <template v-if="!fireredStatus.ready">
                    <el-button 
                      type="primary" 
                      :disabled="isInstallingFirered || !fireredStatus.uv_installed"
                      @click="installFirered"
                    >
                      {{ isInstallingFirered ? '安装中...' : '安装 FireRedASR 环境' }}
                    </el-button>
                    <p v-if="!fireredStatus.uv_installed" class="uv-hint">
                      需要先安装 <a href="https://docs.astral.sh/uv/getting-started/installation/" target="_blank">uv 包管理器</a>
                    </p>
                  </template>
                  <template v-else>
                    <div class="whisper-model-item is-downloaded">
                      <div class="model-info">
                        <span class="model-name">FireRedASR-AED</span>
                        <span class="model-size">~600 MB</span>
                      </div>
                      <div class="model-actions" @click.stop>
                        <el-button size="small" type="danger" plain @click="uninstallFirered">卸载</el-button>
                      </div>
                    </div>
                    
                    <!-- FireRedASR 校正选项 -->
                    <div class="firered-options">
                      <div class="option-item">
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
              
              <div class="whisper-tips">
                <h4>模型说明</h4>
                <ul>
                  <li><strong>Whisper tiny/base</strong> - 快速预览，适合短音频</li>
                  <li><strong>Whisper small/medium</strong> - 平衡选择，日常使用</li>
                  <li><strong>Whisper large/turbo</strong> - 高精度，专业场景</li>
                  <li><strong>SenseVoice</strong> - 中文识别优秀，首次使用需下载模型</li>
                  <li><strong>FireRedASR</strong> - 字幕校正专用，可对已有字幕进行二次校正</li>
                </ul>
              </div>
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
  margin-bottom: 24px;
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

/* 语音模型页面 */
.engine-section {
  margin-bottom: 24px;
  padding-bottom: 20px;
  border-bottom: 1px solid #f0f0f0;
}

.engine-section:last-of-type {
  border-bottom: none;
}

.engine-title {
  font-size: 15px;
  font-weight: 600;
  color: #333;
  margin: 0 0 6px;
}

.engine-desc {
  font-size: 13px;
  color: #666;
  line-height: 1.5;
  margin: 0 0 12px;
}

/* SenseVoice 状态 */
.sensevoice-status {
  display: flex;
  gap: 20px;
  margin-bottom: 12px;
}

.status-item {
  display: flex;
  align-items: center;
  gap: 8px;
}

.status-label {
  font-size: 13px;
  color: #666;
}

.status-value {
  font-size: 13px;
  font-weight: 500;
  padding: 2px 8px;
  border-radius: 4px;
}

.status-value.ready {
  color: #67c23a;
  background: rgba(103, 194, 58, 0.1);
}

.status-value.pending {
  color: #e6a23c;
  background: rgba(230, 162, 60, 0.1);
}

.install-progress {
  margin-bottom: 12px;
}

.install-message {
  display: block;
  font-size: 12px;
  color: #909399;
  margin-top: 6px;
}

.sensevoice-actions {
  margin-top: 12px;
}

.uv-hint {
  font-size: 12px;
  color: #909399;
  margin-top: 8px;
}

.uv-hint a {
  color: #409eff;
  text-decoration: none;
}

.uv-hint a:hover {
  text-decoration: underline;
}

.whisper-intro {
  margin-bottom: 20px;
}

.whisper-intro p {
  font-size: 13px;
  color: #666;
  line-height: 1.6;
  margin: 0;
}

.whisper-models-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin-bottom: 24px;
}

.whisper-model-item {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 12px 16px;
  background: #f9f9f9;
  border-radius: 8px;
  border: 1px solid transparent;
  transition: all 0.15s;
}

.whisper-model-item.is-downloaded {
  cursor: pointer;
}

.whisper-model-item.is-downloaded:hover {
  background: #f0f0f0;
}

.model-radio {
  width: 20px;
  display: flex;
  align-items: center;
  justify-content: center;
}

.radio-dot {
  width: 16px;
  height: 16px;
  border-radius: 50%;
  border: 2px solid #ddd;
  transition: all 0.2s;
}

.radio-dot.active {
  border-color: #409eff;
  background: #409eff;
  box-shadow: inset 0 0 0 3px #fff;
}

.radio-placeholder {
  width: 16px;
  height: 16px;
}

.model-info {
  display: flex;
  align-items: center;
  gap: 12px;
  flex: 1;
}

.model-name {
  font-size: 14px;
  font-weight: 600;
  color: #333;
  min-width: 60px;
}

.model-size {
  font-size: 13px;
  color: #888;
}

.whisper-model-item.is-default {
  background: #f5f9ff;
  border-color: #d9ecff;
}

.model-actions {
  display: flex;
  align-items: center;
}

.download-progress {
  display: flex;
  align-items: center;
  gap: 8px;
}

.progress-text {
  font-size: 12px;
  color: #666;
  min-width: 36px;
}

/* FireRedASR 选项 */
.firered-options {
  margin-top: 16px;
  padding: 12px 16px;
  background: #f9f9f9;
  border-radius: 8px;
}

.firered-options .option-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.firered-options .option-info {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.firered-options .option-label {
  font-size: 14px;
  font-weight: 500;
  color: #333;
}

.firered-options .option-desc {
  font-size: 12px;
  color: #909399;
}

.whisper-tips {
  background: #fafafa;
  border-radius: 8px;
  padding: 16px;
}

.whisper-tips h4 {
  font-size: 14px;
  font-weight: 600;
  color: #333;
  margin: 0 0 12px;
}

.whisper-tips ul {
  margin: 0;
  padding-left: 20px;
}

.whisper-tips li {
  font-size: 13px;
  color: #666;
  line-height: 1.8;
}

.whisper-tips li strong {
  color: #333;
}

.whisper-tips .whisper-hint {
  margin-top: 12px;
  font-size: 12px;
  color: #999;
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

<script setup lang="ts">
import { ref, computed, watch, onBeforeUnmount } from 'vue'
import { ElMessage } from 'element-plus'
import { useConfigStore, DEFAULT_PUNCTUATION } from '@/stores/config'
import { Setting, Key, InfoFilled, ChatDotRound, Message, Document } from '@element-plus/icons-vue'
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
const activeMenu = ref<'general' | 'shortcuts' | 'logs' | 'contact' | 'about'>('general')

// 菜单项配置
const menuItems = [
  { key: 'general', label: '常规设置', icon: Setting },
  { key: 'shortcuts', label: '快捷键列表', icon: Key },
  { key: 'logs', label: '日志', icon: Document },
  { key: 'contact', label: '联系开发者', icon: ChatDotRound },
  { key: 'about', label: '关于', icon: InfoFilled },
] as const

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
  } else {
    document.removeEventListener('keydown', handleKeydown, true)
  }
})

// 组件卸载时清理
onBeforeUnmount(() => {
  document.removeEventListener('keydown', handleKeydown, true)
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
  overflow: hidden;
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

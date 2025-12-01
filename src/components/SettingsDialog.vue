<script setup lang="ts">
import { ref, computed, watch, onMounted, onBeforeUnmount } from 'vue'
import { useConfigStore } from '@/stores/config'
import { Setting, Key, InfoFilled } from '@element-plus/icons-vue'

const props = defineProps<{
  visible: boolean
}>()

const emit = defineEmits<{
  (e: 'update:visible', value: boolean): void
}>()

const configStore = useConfigStore()

// 当前选中的菜单项
const activeMenu = ref<'general' | 'shortcuts' | 'about'>('general')

// 菜单项配置
const menuItems = [
  { key: 'general', label: '常规设置', icon: Setting },
  { key: 'shortcuts', label: '快捷键列表', icon: Key },
  { key: 'about', label: '关于', icon: InfoFilled },
] as const

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
                    <span class="setting-label">自动保存</span>
                    <span class="setting-desc">编辑后自动保存字幕文件</span>
                  </div>
                  <el-switch
                    v-model="configStore.config.autoSave"
                    @change="configStore.saveConfig()"
                  />
                </div>

                <div class="setting-item">
                  <div class="setting-info">
                    <span class="setting-label">自动滚动</span>
                    <span class="setting-desc">播放时自动滚动到当前字幕</span>
                  </div>
                  <el-switch
                    v-model="configStore.config.autoscroll"
                    @change="configStore.saveConfig()"
                  />
                </div>

                <div class="setting-item">
                  <div class="setting-info">
                    <span class="setting-label">显示波形</span>
                    <span class="setting-desc">在时间轴上显示音频波形</span>
                  </div>
                  <el-switch
                    v-model="configStore.config.showWaveform"
                    @change="configStore.saveConfig()"
                  />
                </div>

                <div class="setting-item">
                  <div class="setting-info">
                    <span class="setting-label">显示快捷键提示</span>
                    <span class="setting-desc">在界面上显示快捷键提示</span>
                  </div>
                  <el-switch
                    v-model="configStore.config.showKeyboardHints"
                    @change="configStore.saveConfig()"
                  />
                </div>

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
              </div>
            </div>

            <!-- 快捷键列表 -->
            <div v-if="activeMenu === 'shortcuts'" class="content-section">
              <h2 class="section-title">快捷键列表</h2>
              
              <div class="shortcuts-list">
                <div
                  v-for="binding in configStore.keyBindings"
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
  width: 680px;
  max-width: 90vw;
  height: 480px;
  max-height: 80vh;
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
  overflow-y: auto;
  position: relative;
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
.shortcuts-list {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.shortcut-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px 0;
  border-bottom: 1px solid #f5f5f5;
}

.shortcut-item:last-child {
  border-bottom: none;
}

.shortcut-desc {
  font-size: 14px;
  color: #333;
}

.shortcut-keys {
  display: flex;
  align-items: center;
  gap: 6px;
}

.key-cap {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 32px;
  height: 32px;
  padding: 0 10px;
  background: #f5f5f5;
  border: 1px solid #e0e0e0;
  border-radius: 8px;
  font-size: 14px;
  font-family: -apple-system, BlinkMacSystemFont, 'SF Pro Text', sans-serif;
  color: #666;
  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.05);
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

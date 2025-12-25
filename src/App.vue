<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import UpdateDialog from '@/components/UpdateDialog.vue'
import ChristmasSnow from '@/components/ChristmasSnow.vue'
import ChristmasGreeting from '@/components/ChristmasGreeting.vue'
import type { ReleaseInfo } from '@/utils/updater'
import { useConfigStore } from '@/stores/config'

const configStore = useConfigStore()

// 更新对话框状态
const showUpdateDialog = ref(false)
const currentVersion = ref('')
const releaseInfo = ref<ReleaseInfo | null>(null)

// 🎄 圣诞彩蛋状态
const isChristmasSeason = () => {
  const now = new Date()
  const month = now.getMonth() + 1
  const day = now.getDate()
  // 12月20日 - 12月31日 显示圣诞彩蛋
  return month === 12 && day >= 20 && day <= 31
}

// 飘雪效果：圣诞季节 + 用户开关
const showChristmasSnow = computed(() => isChristmasSeason() && configStore.showChristmasSnow)
const showChristmasGreeting = ref(false)

// 检查是否需要显示圣诞祝福（每天只显示一次）
const checkChristmasGreeting = () => {
  if (!isChristmasSeason()) return

  const today = new Date().toDateString()
  const lastShown = localStorage.getItem('vosub-christmas-greeting-shown')

  if (lastShown !== today) {
    showChristmasGreeting.value = true
    localStorage.setItem('vosub-christmas-greeting-shown', today)
  }
}

const handleGreetingClose = () => {
  showChristmasGreeting.value = false
}

// 监听更新事件
const handleUpdateAvailable = (event: CustomEvent<{ currentVersion: string; releaseInfo: ReleaseInfo }>) => {
  currentVersion.value = event.detail.currentVersion
  releaseInfo.value = event.detail.releaseInfo
  showUpdateDialog.value = true
}

// 跳过版本
const handleSkipVersion = (version: string) => {
  configStore.skipVersion(version)
}

onMounted(() => {
  window.addEventListener('app-update-available', handleUpdateAvailable as EventListener)
  // 🎄 检查圣诞祝福
  checkChristmasGreeting()
})

onUnmounted(() => {
  window.removeEventListener('app-update-available', handleUpdateAvailable as EventListener)
})
</script>

<template>
  <div id="app" class="w-screen h-screen overflow-hidden">
    <router-view />
    
    <!-- 🎄 圣诞飘雪效果 -->
    <ChristmasSnow :enabled="showChristmasSnow" />
    
    <!-- 🎄 圣诞祝福弹窗 -->
    <ChristmasGreeting v-if="showChristmasGreeting" @close="handleGreetingClose" />
    
    <!-- 更新提示对话框 -->
    <UpdateDialog
      v-model:visible="showUpdateDialog"
      :current-version="currentVersion"
      :release-info="releaseInfo"
      @skip-version="handleSkipVersion"
    />
  </div>
</template>

<style>
* {
  margin: 0;
  padding: 0;
  box-sizing: border-box;
}

#app {
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, 'Helvetica Neue', Arial,
    sans-serif;
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
}
</style>

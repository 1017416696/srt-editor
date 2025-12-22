<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue'
import UpdateDialog from '@/components/UpdateDialog.vue'
import type { ReleaseInfo } from '@/utils/updater'
import { useConfigStore } from '@/stores/config'

const configStore = useConfigStore()

// 更新对话框状态
const showUpdateDialog = ref(false)
const currentVersion = ref('')
const releaseInfo = ref<ReleaseInfo | null>(null)

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
})

onUnmounted(() => {
  window.removeEventListener('app-update-available', handleUpdateAvailable as EventListener)
})
</script>

<template>
  <div id="app" class="w-screen h-screen overflow-hidden">
    <router-view />
    
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

<script setup lang="ts">
import { ref, computed } from 'vue'
import type { ReleaseInfo, ReleaseAsset } from '@/utils/updater'
import { getPlatformAsset, openDownloadPage, formatFileSize } from '@/utils/updater'

const props = defineProps<{
  visible: boolean
  currentVersion: string
  releaseInfo: ReleaseInfo | null
}>()

const emit = defineEmits<{
  (e: 'update:visible', value: boolean): void
  (e: 'skip-version', version: string): void
}>()

const isDownloading = ref(false)

const dialogVisible = computed({
  get: () => props.visible,
  set: (val) => emit('update:visible', val),
})

const platformAsset = computed<ReleaseAsset | null>(() => {
  if (!props.releaseInfo) return null
  return getPlatformAsset(props.releaseInfo.assets)
})

const releaseNotes = computed(() => {
  if (!props.releaseInfo?.body) return ''
  // 简单处理 markdown，移除一些复杂格式
  return props.releaseInfo.body
    .replace(/^#+\s*/gm, '') // 移除标题标记
    .replace(/\*\*/g, '') // 移除加粗
    .replace(/`([^`]+)`/g, '$1') // 移除代码标记
    .trim()
})

const handleDownload = async () => {
  if (!platformAsset.value) {
    // 没有找到对应平台的资源，打开 Release 页面
    if (props.releaseInfo?.htmlUrl) {
      await openDownloadPage(props.releaseInfo.htmlUrl)
    }
    return
  }

  isDownloading.value = true
  try {
    await openDownloadPage(platformAsset.value.browserDownloadUrl)
  } finally {
    isDownloading.value = false
  }
}

const handleSkip = () => {
  if (props.releaseInfo) {
    emit('skip-version', props.releaseInfo.version)
  }
  dialogVisible.value = false
}

const handleLater = () => {
  dialogVisible.value = false
}
</script>

<template>
  <el-dialog
    v-model="dialogVisible"
    title="发现新版本"
    width="460px"
    :close-on-click-modal="false"
    class="update-dialog"
  >
    <div class="update-content">
      <!-- 版本信息 -->
      <div class="version-info">
        <div class="version-badge new">
          <span class="label">新版本</span>
          <span class="version">v{{ releaseInfo?.version }}</span>
        </div>
        <div class="version-arrow">
          <i class="i-mdi-arrow-left"></i>
        </div>
        <div class="version-badge current">
          <span class="label">当前版本</span>
          <span class="version">v{{ currentVersion }}</span>
        </div>
      </div>

      <!-- 更新说明 -->
      <div v-if="releaseNotes" class="release-notes">
        <p class="notes-title">更新内容：</p>
        <div class="notes-content">
          {{ releaseNotes }}
        </div>
      </div>

      <!-- 下载信息 -->
      <div v-if="platformAsset" class="download-info">
        <i class="i-mdi-download"></i>
        <span class="file-name">{{ platformAsset.name }}</span>
        <span class="file-size">({{ formatFileSize(platformAsset.size) }})</span>
      </div>

      <!-- macOS 提示 -->
      <div v-if="navigator.platform.toLowerCase().includes('mac')" class="macos-hint">
        <i class="i-mdi-information-outline"></i>
        <span>下载后如遇「无法打开」提示，请在终端执行：<code>xattr -cr '/Applications/VoSub.app'</code></span>
      </div>
    </div>

    <template #footer>
      <div class="dialog-footer">
        <el-button @click="handleSkip">跳过此版本</el-button>
        <el-button @click="handleLater">稍后提醒</el-button>
        <el-button type="primary" :loading="isDownloading" @click="handleDownload">
          立即下载
        </el-button>
      </div>
    </template>
  </el-dialog>
</template>

<style scoped>
.update-content {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.version-info {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 12px;
  padding: 16px;
  background: #f5f7fa;
  border-radius: 8px;
}

.version-badge {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 4px;
}

.version-badge .label {
  font-size: 12px;
  color: #909399;
}

.version-badge .version {
  font-size: 18px;
  font-weight: 600;
}

.version-badge.new .version {
  color: #67c23a;
}

.version-badge.current .version {
  color: #909399;
}

.version-arrow {
  color: #c0c4cc;
  font-size: 20px;
}

.release-notes {
  background: #fafafa;
  border-radius: 6px;
  padding: 12px;
}

.notes-title {
  font-size: 13px;
  font-weight: 500;
  color: #606266;
  margin-bottom: 8px;
}

.notes-content {
  font-size: 13px;
  color: #909399;
  line-height: 1.6;
  max-height: 120px;
  overflow-y: auto;
  white-space: pre-wrap;
}

.download-info {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 13px;
  color: #606266;
}

.download-info i {
  color: #409eff;
}

.file-size {
  color: #909399;
}

.macos-hint {
  display: flex;
  align-items: flex-start;
  gap: 6px;
  padding: 10px 12px;
  background: #fdf6ec;
  border-radius: 6px;
  font-size: 12px;
  color: #e6a23c;
  line-height: 1.5;
}

.macos-hint i {
  flex-shrink: 0;
  margin-top: 2px;
}

.macos-hint code {
  background: rgba(0, 0, 0, 0.06);
  padding: 2px 4px;
  border-radius: 3px;
  font-family: monospace;
  font-size: 11px;
}

.dialog-footer {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}
</style>

<script setup lang="ts">
import { ref } from 'vue'

const emit = defineEmits<{
  (e: 'close'): void
}>()

const currentStep = ref(0)

const steps = [
  {
    icon: 'i-mdi-file-document-outline',
    title: '打开字幕文件',
    desc: '点击「打开字幕文件」按钮，或直接拖放 SRT 文件到窗口中',
  },
  {
    icon: 'i-mdi-microphone',
    title: 'AI 语音转录',
    desc: '没有字幕？选择音频文件，AI 自动识别语音生成字幕',
  },
]

const nextStep = () => {
  if (currentStep.value < steps.length - 1) {
    currentStep.value++
  } else {
    emit('close')
  }
}

const skip = () => {
  emit('close')
}
</script>

<template>
  <div class="welcome-guide-overlay" @click.self="skip">
    <div class="welcome-guide-card">
      <button class="close-btn" @click="skip">
        <i class="i-mdi-close"></i>
      </button>

      <div class="guide-header">
        <span class="guide-badge">新手引导</span>
        <h3>欢迎使用 VoSub</h3>
      </div>

      <div class="guide-content">
        <div class="step-icon">
          <i :class="steps[currentStep]?.icon"></i>
        </div>
        <h4>{{ steps[currentStep]?.title }}</h4>
        <p>{{ steps[currentStep]?.desc }}</p>
      </div>

      <div class="guide-footer">
        <div class="step-dots">
          <span
            v-for="(_, index) in steps"
            :key="index"
            class="dot"
            :class="{ active: index === currentStep }"
            @click="currentStep = index"
          ></span>
        </div>
        <div class="guide-actions">
          <button class="skip-btn" @click="skip">跳过</button>
          <button class="next-btn" @click="nextStep">
            {{ currentStep === steps.length - 1 ? '开始使用' : '下一步' }}
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.welcome-guide-overlay {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.5);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 9999;
  backdrop-filter: blur(4px);
}

.welcome-guide-card {
  position: relative;
  background: var(--el-bg-color);
  border-radius: 16px;
  padding: 32px;
  width: 380px;
  box-shadow: 0 20px 60px rgba(0, 0, 0, 0.3);
  animation: slideUp 0.3s ease-out;
  -webkit-user-select: none;
  -moz-user-select: none;
  -ms-user-select: none;
  user-select: none;
}

.welcome-guide-card * {
  -webkit-user-select: none;
  -moz-user-select: none;
  -ms-user-select: none;
  user-select: none;
}

@keyframes slideUp {
  from {
    opacity: 0;
    transform: translateY(20px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

.close-btn {
  position: absolute;
  top: 16px;
  right: 16px;
  width: 28px;
  height: 28px;
  border: none;
  background: transparent;
  color: var(--el-text-color-secondary);
  cursor: pointer;
  border-radius: 6px;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all 0.2s;
}

.close-btn:hover {
  background: var(--el-fill-color);
  color: var(--el-text-color-primary);
}

.guide-header {
  text-align: center;
  margin-bottom: 24px;
}

.guide-badge {
  display: inline-block;
  padding: 4px 12px;
  background: var(--el-color-primary-light-9);
  color: var(--el-color-primary);
  border-radius: 12px;
  font-size: 12px;
  font-weight: 500;
  margin-bottom: 12px;
}

.guide-header h3 {
  margin: 0;
  font-size: 20px;
  font-weight: 600;
  color: var(--el-text-color-primary);
}

.guide-content {
  text-align: center;
  padding: 20px 0;
  min-height: 160px;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
}

.step-icon {
  width: 64px;
  height: 64px;
  background: linear-gradient(135deg, var(--el-color-primary-light-7), var(--el-color-primary-light-5));
  border-radius: 16px;
  display: flex;
  align-items: center;
  justify-content: center;
  margin-bottom: 16px;
}

.step-icon i {
  font-size: 32px;
  color: var(--el-color-primary);
}

.guide-content h4 {
  margin: 0 0 8px;
  font-size: 16px;
  font-weight: 600;
  color: var(--el-text-color-primary);
}

.guide-content p {
  margin: 0;
  font-size: 14px;
  color: var(--el-text-color-secondary);
  line-height: 1.6;
}

.guide-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-top: 24px;
  padding-top: 20px;
  border-top: 1px solid var(--el-border-color-lighter);
}

.step-dots {
  display: flex;
  gap: 8px;
}

.dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--el-fill-color-darker);
  cursor: pointer;
  transition: all 0.2s;
}

.dot.active {
  background: var(--el-color-primary);
  width: 20px;
  border-radius: 4px;
}

.guide-actions {
  display: flex;
  gap: 12px;
}

.skip-btn {
  padding: 8px 16px;
  border: none;
  background: transparent;
  color: var(--el-text-color-secondary);
  cursor: pointer;
  border-radius: 8px;
  font-size: 14px;
  transition: all 0.2s;
}

.skip-btn:hover {
  background: var(--el-fill-color);
  color: var(--el-text-color-primary);
}

.next-btn {
  padding: 8px 20px;
  border: none;
  background: var(--el-color-primary);
  color: white;
  cursor: pointer;
  border-radius: 8px;
  font-size: 14px;
  font-weight: 500;
  transition: all 0.2s;
}

.next-btn:hover {
  background: var(--el-color-primary-dark-2);
}
</style>

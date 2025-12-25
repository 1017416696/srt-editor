<script setup lang="ts">
import { ref, onMounted } from 'vue'

const emit = defineEmits<{
  close: []
}>()

const visible = ref(false)
const showContent = ref(false)

onMounted(() => {
  setTimeout(() => {
    visible.value = true
    setTimeout(() => {
      showContent.value = true
    }, 300)
  }, 500)
})

const handleClose = () => {
  showContent.value = false
  setTimeout(() => {
    visible.value = false
    setTimeout(() => emit('close'), 300)
  }, 200)
}
</script>

<template>
  <Teleport to="body">
    <Transition name="fade">
      <div v-if="visible" class="greeting-overlay" @click.self="handleClose">
        <Transition name="scale">
          <div v-if="showContent" class="greeting-card">
            <!-- Logo -->
            <div class="logo-area">
              <div class="logo-wrapper">
                <img src="/icon-concept-6-small-v.svg" alt="VoSub" class="logo" />
              </div>
            </div>

            <!-- 祝福文字 -->
            <h1 class="title">圣诞快乐</h1>
            <p class="subtitle">Merry Christmas</p>
            
            <p class="message">
              感谢你使用 VoSub，愿你的创作之路精彩纷呈 ✨
            </p>

            <!-- 圣诞装饰 -->
            <div class="christmas-deco">
              <span>🎄</span>
              <span>🎁</span>
              <span>🎄</span>
            </div>

            <!-- 按钮 -->
            <button class="close-btn" @click="handleClose">
              开始使用
            </button>
          </div>
        </Transition>
      </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
.greeting-overlay {
  position: fixed;
  top: 0;
  left: 0;
  width: 100vw;
  height: 100vh;
  background: rgba(0, 0, 0, 0.4);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 9999;
  backdrop-filter: blur(8px);
}

.greeting-card {
  background: #fff;
  border-radius: 20px;
  padding: 48px 64px;
  text-align: center;
  box-shadow: 0 20px 60px rgba(0, 0, 0, 0.15);
  width: 440px;
  position: relative;
  overflow: hidden;
}

/* 顶部红色装饰条 */
.greeting-card::before {
  content: '';
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  height: 4px;
  background: linear-gradient(90deg, #22c55e, #dc2626, #22c55e);
}

.logo-area {
  margin-bottom: 24px;
  display: flex;
  justify-content: center;
}

.logo-wrapper {
  position: relative;
  display: inline-block;
}

.logo {
  width: 80px;
  height: 80px;
  border-radius: 18px;
  box-shadow: 0 8px 24px rgba(59, 130, 246, 0.2);
}

.title {
  font-size: 32px;
  font-weight: 600;
  color: #dc2626;
  margin: 0 0 6px;
}

.subtitle {
  font-size: 14px;
  color: #94a3b8;
  margin: 0 0 20px;
  font-style: italic;
}

.message {
  font-size: 15px;
  line-height: 1.6;
  color: #64748b;
  margin: 0 0 24px;
}

.christmas-deco {
  display: flex;
  justify-content: center;
  gap: 16px;
  margin-bottom: 28px;
  font-size: 28px;
}

.close-btn {
  background: linear-gradient(135deg, #dc2626, #b91c1c);
  color: #fff;
  border: none;
  padding: 12px 48px;
  border-radius: 10px;
  font-size: 15px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s;
  box-shadow: 0 4px 12px rgba(220, 38, 38, 0.25);
}

.close-btn:hover {
  transform: translateY(-1px);
  box-shadow: 0 6px 16px rgba(220, 38, 38, 0.3);
}

/* 过渡动画 */
.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.3s ease;
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}

.scale-enter-active,
.scale-leave-active {
  transition: all 0.3s ease;
}

.scale-enter-from,
.scale-leave-to {
  opacity: 0;
  transform: scale(0.95);
}
</style>

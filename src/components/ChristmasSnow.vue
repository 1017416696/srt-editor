<script setup lang="ts">
import { ref, watch, onUnmounted, nextTick } from 'vue'

const props = defineProps<{
  enabled: boolean
}>()

const canvasRef = ref<HTMLCanvasElement | null>(null)
let animationId: number | null = null
let particles: Particle[] = []

interface Particle {
  x: number
  y: number
  size: number
  speed: number
  wind: number
  opacity: number
  type: 'snow' | 'star' | 'sparkle'
  rotation: number
  rotationSpeed: number
  color: string
}

const colors = {
  snow: ['rgba(255,255,255,', 'rgba(200,220,255,', 'rgba(220,240,255,'],
  star: ['rgba(255,215,0,', 'rgba(255,223,100,'],
  sparkle: ['rgba(255,215,0,', 'rgba(255,223,100,'],
}

const createParticle = (canvas: HTMLCanvasElement): Particle => {
  const rand = Math.random()
  let type: 'snow' | 'star' | 'sparkle'
  
  if (rand < 0.7) type = 'snow'
  else if (rand < 0.85) type = 'star'
  else type = 'sparkle'

  const colorArr = colors[type]
  const baseColor = colorArr[Math.floor(Math.random() * colorArr.length)]!

  return {
    x: Math.random() * canvas.width,
    y: Math.random() * -150,
    size: type === 'snow' ? Math.random() * 4 + 2 : Math.random() * 6 + 3,
    speed: type === 'snow' ? Math.random() * 1.5 + 0.8 : Math.random() * 0.8 + 0.3,
    wind: Math.random() * 0.8 - 0.4,
    opacity: Math.random() * 0.5 + 0.5,
    type,
    rotation: Math.random() * Math.PI * 2,
    rotationSpeed: (Math.random() - 0.5) * 0.02,
    color: baseColor,
  }
}

const drawSnowflake = (ctx: CanvasRenderingContext2D, p: Particle) => {
  ctx.save()
  ctx.translate(p.x, p.y)
  ctx.rotate(p.rotation)
  
  // 绘制六角雪花
  ctx.beginPath()
  for (let i = 0; i < 6; i++) {
    const angle = (i * Math.PI) / 3
    ctx.moveTo(0, 0)
    ctx.lineTo(Math.cos(angle) * p.size, Math.sin(angle) * p.size)
  }
  ctx.strokeStyle = `${p.color}${p.opacity})`
  ctx.lineWidth = 1.5
  ctx.stroke()
  
  // 中心圆点
  ctx.beginPath()
  ctx.arc(0, 0, p.size * 0.2, 0, Math.PI * 2)
  ctx.fillStyle = `${p.color}${p.opacity})`
  ctx.fill()
  
  ctx.restore()
}

const drawStar = (ctx: CanvasRenderingContext2D, p: Particle) => {
  ctx.save()
  ctx.translate(p.x, p.y)
  ctx.rotate(p.rotation)
  
  // 绘制四角星
  ctx.beginPath()
  for (let i = 0; i < 4; i++) {
    const angle = (i * Math.PI) / 2
    ctx.moveTo(0, 0)
    ctx.lineTo(Math.cos(angle) * p.size, Math.sin(angle) * p.size)
  }
  ctx.strokeStyle = `${p.color}${p.opacity})`
  ctx.lineWidth = 2
  ctx.stroke()
  
  // 发光效果
  ctx.beginPath()
  ctx.arc(0, 0, p.size * 0.3, 0, Math.PI * 2)
  ctx.fillStyle = `${p.color}${p.opacity * 0.8})`
  ctx.fill()
  
  ctx.restore()
}

const drawSparkle = (ctx: CanvasRenderingContext2D, p: Particle) => {
  ctx.save()
  ctx.translate(p.x, p.y)
  
  // 闪烁的小圆点
  const glowSize = p.size * (0.8 + Math.sin(Date.now() * 0.005 + p.x) * 0.2)
  
  // 外发光
  const gradient = ctx.createRadialGradient(0, 0, 0, 0, 0, glowSize)
  gradient.addColorStop(0, `${p.color}${p.opacity})`)
  gradient.addColorStop(1, `${p.color}0)`)
  
  ctx.beginPath()
  ctx.arc(0, 0, glowSize, 0, Math.PI * 2)
  ctx.fillStyle = gradient
  ctx.fill()
  
  ctx.restore()
}

const animate = () => {
  const canvas = canvasRef.value
  if (!canvas || !props.enabled) return

  const ctx = canvas.getContext('2d')
  if (!ctx) return

  ctx.clearRect(0, 0, canvas.width, canvas.height)

  particles.forEach((p, index) => {
    p.y += p.speed
    p.x += p.wind + Math.sin(p.y * 0.01) * 0.3
    p.rotation += p.rotationSpeed

    // 重置超出边界的粒子
    if (p.y > canvas.height + 20) {
      particles[index] = createParticle(canvas)
      particles[index]!.y = -20
    }
    if (p.x > canvas.width + 20) p.x = -20
    if (p.x < -20) p.x = canvas.width + 20

    // 根据类型绘制
    if (p.type === 'snow') drawSnowflake(ctx, p)
    else if (p.type === 'star') drawStar(ctx, p)
    else drawSparkle(ctx, p)
  })

  animationId = requestAnimationFrame(animate)
}

const stopAnimation = () => {
  if (animationId) {
    cancelAnimationFrame(animationId)
    animationId = null
  }
  particles = []
}

const initSnow = () => {
  const canvas = canvasRef.value
  if (!canvas) return

  canvas.width = window.innerWidth
  canvas.height = window.innerHeight

  // 创建粒子（增加数量）
  const count = Math.floor((canvas.width * canvas.height) / 8000)
  particles = Array.from({ length: count }, () => createParticle(canvas))
  // 让部分粒子从屏幕中间开始
  particles.forEach((p, i) => {
    if (i % 3 === 0) p.y = Math.random() * canvas.height
  })

  animate()
}

const handleResize = () => {
  const canvas = canvasRef.value
  if (!canvas) return
  canvas.width = window.innerWidth
  canvas.height = window.innerHeight
}

// 监听 enabled 变化
watch(() => props.enabled, async (newVal) => {
  if (newVal) {
    await nextTick()
    initSnow()
    window.addEventListener('resize', handleResize)
  } else {
    stopAnimation()
    window.removeEventListener('resize', handleResize)
  }
}, { immediate: true })

onUnmounted(() => {
  stopAnimation()
  window.removeEventListener('resize', handleResize)
})
</script>

<template>
  <canvas
    v-if="enabled"
    ref="canvasRef"
    class="christmas-snow"
  />
</template>

<style scoped>
.christmas-snow {
  position: fixed;
  top: 0;
  left: 0;
  width: 100vw;
  height: 100vh;
  pointer-events: none;
  z-index: 9998;
}
</style>

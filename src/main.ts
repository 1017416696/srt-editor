import { devtools } from '@vue/devtools'
import { createPinia } from 'pinia'
import { createApp } from 'vue'
import ElementPlus from 'element-plus'
import 'element-plus/dist/index.css'
import App from './App.vue'
import router from './router'
import './assets/main.css'
import { listen } from '@tauri-apps/api/event'

if (process.env.NODE_ENV === 'development') {
  devtools.connect('http://localhost', 8098)
}

const app = createApp(App)
const pinia = createPinia()

app.use(pinia)
app.use(router)
app.use(ElementPlus)

// 全局菜单事件监听器（在应用启动时注册）
listen<void>('menu:open-file', async () => {
  console.log('✓ Global menu:open-file event received - triggering callback')
  // 触发全局回调函数（由各页面注册）
  if ((window as any).__handleMenuOpenFile && typeof (window as any).__handleMenuOpenFile === 'function') {
    console.log('Calling registered menu open file handler...')
    await (window as any).__handleMenuOpenFile()
  } else {
    console.warn('No menu open file handler registered')
  }
}).catch(err => console.error('Failed to listen menu:open-file:', err))

listen<void>('menu:save', async () => {
  console.log('✓ Global menu:save event received - triggering callback')
  // 触发全局回调函数（由各页面注册）
  if ((window as any).__handleMenuSave && typeof (window as any).__handleMenuSave === 'function') {
    console.log('Calling registered menu save handler...')
    await (window as any).__handleMenuSave()
  } else {
    console.warn('No menu save handler registered')
  }
}).catch(err => console.error('Failed to listen menu:save:', err))

app.mount('#app')

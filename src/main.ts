import { createApp } from 'vue'
import { createPinia } from 'pinia'
import ElementPlus, { ElMessage } from 'element-plus'
import zhCn from 'element-plus/es/locale/lang/zh-cn'
import 'element-plus/dist/index.css'
import * as ElementPlusIconsVue from '@element-plus/icons-vue'

import App from './App.vue'
import router from './router'
import './styles/index.css'

const app = createApp(App)

app.use(createPinia())
app.use(router)
app.use(ElementPlus, { locale: zhCn })

// 无 panic 设计（前端侧兜底）：渲染层未捕获错误不白屏，仅提示并记录
app.config.errorHandler = (err, _instance, info) => {
  console.error('[fgpui] 未处理错误:', err, info)
  const text = err instanceof Error ? err.message : String(err)
  ElMessage.error(`界面错误：${text}`)
}

// 未处理的 Promise 拒绝仅记录（invoke 调用点均已捕获）
window.addEventListener('unhandledrejection', (event) => {
  console.error('[fgpui] 未处理的 Promise 拒绝:', event.reason)
})

for (const [key, component] of Object.entries(ElementPlusIconsVue)) {
  app.component(key, component)
}

app.mount('#app')

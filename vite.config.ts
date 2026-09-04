import { fileURLToPath, URL } from 'node:url'

import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'

// Tauri 前端构建配置：
// - 固定端口 1420，devUrl 与 src-tauri/tauri.conf.json 保持一致
// - clearScreen false 保留 Rust 编译日志输出
export default defineConfig({
  plugins: [vue()],
  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./src', import.meta.url)),
    },
  },
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      ignored: ['**/src-tauri/**'],
    },
  },
  envPrefix: ['VITE_', 'TAURI_ENV_'],
  build: {
    target: 'chrome105',
    minify: 'esbuild',
    sourcemap: false,
    outDir: 'dist',
  },
})

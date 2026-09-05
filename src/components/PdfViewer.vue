<script setup lang="ts">
/**
 * PdfViewer —— PDF.js 内嵌预览（无需外部浏览器）。
 * 通过后端命令读取 PDF 字节 → Blob 数据 → 逐页渲染到 canvas。
 */
import { onBeforeUnmount, onMounted, ref, watch } from 'vue'
import * as pdfjsLib from 'pdfjs-dist'
import workerUrl from 'pdfjs-dist/build/pdf.worker.min.mjs?url'
import { readPdfBytes } from '@/api/client'

pdfjsLib.GlobalWorkerOptions.workerSrc = workerUrl

const props = defineProps<{ path: string | null }>()

const container = ref<HTMLDivElement | null>(null)
const pageCount = ref(0)
const loading = ref(false)
const error = ref<string | null>(null)
let renderSeq = 0

onMounted(() => {
  if (props.path) void load(props.path)
})

watch(
  () => props.path,
  (p) => {
    if (p) void load(p)
  },
)

onBeforeUnmount(() => {
  renderSeq++ // 使进行中的渲染失效
})

/** 暴露已渲染页面 canvas（视觉回归截图用） */
function getCanvases(): HTMLCanvasElement[] {
  return container.value ? Array.from(container.value.querySelectorAll('canvas')) : []
}

defineExpose({ getCanvases })

async function load(path: string): Promise<void> {
  const seq = ++renderSeq
  loading.value = true
  error.value = null
  try {
    const bytes = await readPdfBytes(path)
    if (seq !== renderSeq) return
    const data = new Uint8Array(bytes)
    const doc = await pdfjsLib.getDocument({ data }).promise
    if (seq !== renderSeq) return
    pageCount.value = doc.numPages

    const host = container.value
    if (!host) return
    host.innerHTML = ''
    for (let i = 1; i <= doc.numPages; i++) {
      if (seq !== renderSeq) return
      const page = await doc.getPage(i)
      const viewport = page.getViewport({ scale: 1.4 })
      const canvas = document.createElement('canvas')
      canvas.width = viewport.width
      canvas.height = viewport.height
      canvas.className = 'pdf-canvas'
      host.appendChild(canvas)
      // pdf.js v6：render 参数直接传 canvas（内部自取 2D 上下文）
      await page.render({ canvas, viewport }).promise
    }
  } catch (e) {
    if (seq === renderSeq) {
      error.value = e instanceof Error ? e.message : String(e)
    }
  } finally {
    if (seq === renderSeq) loading.value = false
  }
}
</script>

<template>
  <div class="pdf-viewer">
    <div v-if="loading" class="status">PDF 加载中…</div>
    <el-alert v-else-if="error" type="error" :title="`PDF 加载失败：${error}`" :closable="false" />
    <div v-else-if="!path" class="status">尚未生成 PDF——点击上方「生成 PDF」开始</div>
    <template v-else>
      <div class="meta">共 {{ pageCount }} 页 · 预览比例 140%</div>
      <div ref="container" class="pages"></div>
    </template>
  </div>
</template>

<style scoped>
.pdf-viewer {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.status {
  color: var(--el-text-color-secondary);
  font-size: 13px;
  padding: 18px 0;
  text-align: center;
}

.meta {
  font-size: 12px;
  color: var(--el-text-color-secondary);
}

.pages {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 12px;
  max-height: 560px;
  overflow: auto;
  padding: 10px;
  background: var(--el-fill-color-lighter);
  border-radius: 6px;
}

:deep(.pdf-canvas) {
  background: #fff;
  box-shadow: 0 1px 4px rgba(0, 0, 0, 0.18);
}
</style>

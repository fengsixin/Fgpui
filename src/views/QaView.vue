<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { ElMessage } from 'element-plus'
import { useProjectsStore } from '@/stores/projectsStore'
import { formatAppError } from '@/stores/appStore'
import * as api from '@/api/client'
import PdfViewer from '@/components/PdfViewer.vue'
import type { SampleCompileResult } from '@/api/types'

const store = useProjectsStore()

const selectedId = ref('')
const compiling = ref(false)
const sample = ref<SampleCompileResult | null>(null)
const pdfViewer = ref<InstanceType<typeof PdfViewer> | null>(null)
const baselineInfo = ref<{ checksum: string; savedAt: string; pages: number } | null>(null)
const comparing = ref(false)
const compareResult = ref<{ page: number; diffPercent: number }[] | null>(null)
const saving = ref(false)

onMounted(() => void store.refreshTemplates())

const templates = computed(() => store.templates.filter((t) => t.manifest && !t.error))

async function compileSample(): Promise<void> {
  if (!selectedId.value) {
    ElMessage.warning('请选择模板')
    return
  }
  compiling.value = true
  compareResult.value = null
  try {
    sample.value = await api.compileTemplateSample(selectedId.value)
    const baseline = await api.getQaBaseline(selectedId.value, sample.value.templateVersion)
    baselineInfo.value = baseline
      ? { checksum: baseline.checksum, savedAt: baseline.savedAt, pages: baseline.pages.length }
      : null
  } catch (err) {
    ElMessage.error(formatAppError(err))
  } finally {
    compiling.value = false
  }
}

function capturePages(): string[] {
  const canvases = pdfViewer.value?.getCanvases() ?? []
  if (!canvases.length) throw new Error('PDF 尚未渲染完成，请稍后重试')
  return canvases.map((c) => (c as HTMLCanvasElement).toDataURL('image/png'))
}

async function saveBaseline(): Promise<void> {
  if (!sample.value) return
  saving.value = true
  try {
    const pages = capturePages()
    const checksum = await api.saveQaBaseline(
      selectedId.value,
      sample.value.templateVersion,
      pages,
      sample.value.sourceHash,
    )
    baselineInfo.value = {
      checksum,
      savedAt: new Date().toLocaleString('zh-CN', { hour12: false }),
      pages: pages.length,
    }
    ElMessage.success('视觉基线已保存（此后可用「视觉比对」检测排版变化）')
  } catch (err) {
    ElMessage.error(formatAppError(err))
  } finally {
    saving.value = false
  }
}

async function compareBaseline(): Promise<void> {
  if (!sample.value) return
  comparing.value = true
  try {
    const baseline = await api.getQaBaseline(selectedId.value, sample.value.templateVersion)
    if (!baseline || !baseline.pages.length) {
      ElMessage.warning('尚未建立基线，请先「保存基线」')
      return
    }
    const pages = capturePages()
    const results: { page: number; diffPercent: number }[] = []
    const count = Math.max(pages.length, baseline.pages.length)
    for (let i = 0; i < count; i++) {
      results.push({ page: i + 1, diffPercent: await diffPercent(pages[i], baseline.pages[i]) })
    }
    compareResult.value = results
    const changed = results.filter((r) => r.diffPercent > 0.5)
    if (changed.length === 0) ElMessage.success('视觉比对：与基线一致，无排版变化')
    else ElMessage.warning(`视觉比对：${changed.length} 页存在排版差异`)
  } catch (err) {
    ElMessage.error(err instanceof Error ? err.message : String(err))
  } finally {
    comparing.value = false
  }
}

/** 像素级差异百分比（尺寸不一致按 100%） */
async function diffPercent(a?: string, b?: string): Promise<number> {
  const load = (url: string): Promise<HTMLImageElement> =>
    new Promise((resolve, reject) => {
      const img = new Image()
      img.onload = () => resolve(img)
      img.onerror = () => reject(new Error('基线图片加载失败'))
      img.src = url
    })
  if (!a || !b) return 100
  const [imgA, imgB] = await Promise.all([load(a), load(b)])
  if (imgA.width !== imgB.width || imgA.height !== imgB.height) return 100
  const c1 = document.createElement('canvas')
  c1.width = imgA.width
  c1.height = imgA.height
  const c2 = document.createElement('canvas')
  c2.width = imgB.width
  c2.height = imgB.height
  const ctx1 = c1.getContext('2d')
  const ctx2 = c2.getContext('2d')
  if (!ctx1 || !ctx2) return 100
  ctx1.drawImage(imgA, 0, 0)
  ctx2.drawImage(imgB, 0, 0)
  const d1 = ctx1.getImageData(0, 0, imgA.width, imgA.height).data
  const d2 = ctx2.getImageData(0, 0, imgB.width, imgB.height).data
  let diff = 0
  for (let i = 0; i < d1.length; i += 4) {
    if (
      Math.abs(d1[i]! - d2[i]!) > 8 ||
      Math.abs(d1[i + 1]! - d2[i + 1]!) > 8 ||
      Math.abs(d1[i + 2]! - d2[i + 2]!) > 8
    ) {
      diff++
    }
  }
  return Number(((diff / (d1.length / 4)) * 100).toFixed(2))
}

async function publish(id: string): Promise<void> {
  try {
    const [version, checksum] = await api.publishTemplate(id)
    ElMessage.success(`已发布登记：${id} ${version}（校验和 ${checksum.slice(0, 12)}…）`)
    void store.refreshTemplates()
  } catch (err) {
    ElMessage.error(formatAppError(err))
  }
}
</script>

<template>
  <div class="qa-page">
    <el-alert
      type="info"
      show-icon
      :closable="false"
      class="phase-banner"
      title="阶段 5 · 模板质量保障"
      description="用模板示例数据编译样例 PDF，建立视觉基线；模板修改后重新渲染即可逐页像素比对，排版变化一目了然。模板可发布登记（校验和锁定，防原地篡改）。"
    />

    <el-card shadow="never">
      <template #header><span>模板发布登记</span></template>
      <el-table :data="store.templates" size="small" v-loading="store.templatesLoading">
        <el-table-column prop="dirName" label="模板" width="180" />
        <el-table-column label="版本" width="100">
          <template #default="{ row }">{{ row.manifest?.version ?? '—' }}</template>
        </el-table-column>
        <el-table-column label="登记状态" width="140">
          <template #default="{ row }">
            <el-tag v-if="row.registry_status === 'ok'" type="success" size="small">已发布 · 一致</el-tag>
            <el-tag v-else-if="row.registry_status === 'drifted'" type="danger" size="small">已被修改</el-tag>
            <el-tag v-else-if="row.registry_status === 'unregistered'" type="info" size="small">未登记</el-tag>
            <el-tag v-else type="warning" size="small">损坏/无效</el-tag>
          </template>
        </el-table-column>
        <el-table-column label="说明" min-width="220">
          <template #default="{ row }">
            <span v-if="row.error" class="mono err">{{ row.error }}</span>
            <span v-else-if="row.manifest?.description">{{ row.manifest.description }}</span>
          </template>
        </el-table-column>
        <el-table-column label="操作" width="130">
          <template #default="{ row }">
            <el-button
              v-if="row.manifest"
              size="small"
              :type="row.registry_status === 'drifted' ? 'warning' : 'default'"
              @click="publish(row.dirName)"
            >
              {{ row.registry_status === 'drifted' ? '重新发布' : '发布登记' }}
            </el-button>
          </template>
        </el-table-column>
      </el-table>
    </el-card>

    <el-card shadow="never">
      <template #header>
        <div class="header-row">
          <span>视觉回归（示例数据 → 编译 → 逐页比对）</span>
          <div>
            <el-select v-model="selectedId" placeholder="选择模板" style="width: 240px">
              <el-option
                v-for="t in templates"
                :key="t.manifest!.id"
                :label="`${t.manifest!.name}（${t.manifest!.version}）`"
                :value="t.manifest!.id"
              />
            </el-select>
            <el-button type="primary" :loading="compiling" @click="compileSample">编译样例 PDF</el-button>
            <el-button v-if="sample" :loading="saving" @click="saveBaseline">保存基线</el-button>
            <el-button v-if="sample" type="warning" :loading="comparing" @click="compareBaseline">
              视觉比对
            </el-button>
          </div>
        </div>
      </template>

      <div v-if="baselineInfo" class="baseline-info">
        <el-tag type="success" effect="plain">
          基线：{{ baselineInfo.pages }} 页 · 校验和 {{ baselineInfo.checksum.slice(0, 12) }}… · 保存于
          {{ baselineInfo.savedAt }}
        </el-tag>
      </div>

      <el-table
        v-if="compareResult"
        :data="compareResult"
        size="small"
        class="compare-table"
        :row-class-name="(row: any) => (row.diffPercent > 0.5 ? 'diff-row' : '')"
      >
        <el-table-column prop="page" label="页" width="80" />
        <el-table-column label="像素差异">
          <template #default="{ row }">
            <el-tag :type="row.diffPercent > 0.5 ? 'danger' : 'success'" size="small">
              {{ row.diffPercent }}%
            </el-tag>
          </template>
        </el-table-column>
      </el-table>

      <template v-if="sample">
        <div class="sample-meta mono">{{ sample.pdfPath }}</div>
        <PdfViewer ref="pdfViewer" :path="sample.pdfPath" />
      </template>
      <el-empty v-else description="选择模板并编译样例 PDF 后开始视觉回归" :image-size="70" />
    </el-card>
  </div>
</template>

<style scoped>
.qa-page {
  max-width: 1280px;
  margin: 0 auto;
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.phase-banner {
  margin-bottom: 2px;
}

.header-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  flex-wrap: wrap;
}

.baseline-info {
  margin-bottom: 12px;
}

.compare-table {
  margin-bottom: 14px;
}

.compare-table :deep(.diff-row) {
  background: var(--el-color-danger-light-9);
}

.mono {
  font-family: Consolas, 'Courier New', monospace;
  font-size: 12px;
  word-break: break-all;
}

.err {
  color: var(--el-color-danger);
}

.sample-meta {
  color: var(--el-text-color-secondary);
  margin-bottom: 8px;
}
</style>

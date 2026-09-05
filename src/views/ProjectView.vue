<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { ArrowLeft, MagicStick } from '@element-plus/icons-vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { open as openFileDialog } from '@tauri-apps/plugin-dialog'
import { useProjectsStore } from '@/stores/projectsStore'
import { docTypeText, errorKindText } from '@/api/types'
import type { AppError, CompileState, ImportPreview, SchemaIssue } from '@/api/types'
import { formatAppError } from '@/stores/appStore'
import * as api from '@/api/client'
import SchemaForm from '@/components/SchemaForm.vue'
import PdfViewer from '@/components/PdfViewer.vue'

const route = useRoute()
const router = useRouter()
const store = useProjectsStore()

const projectId = computed(() => String(route.params.id ?? ''))

// 模板 Schema（表单结构来源）
const schema = ref<Record<string, unknown> | null>(null)
const schemaError = ref<string | null>(null)

// 表单数据（由 SchemaForm 驱动）
const formData = ref<Record<string, unknown>>({})
const issues = ref<SchemaIssue[]>([])

// 原始 JSON（高级）
const rawVisible = ref<string[]>([])
const rawText = ref('')
const rebuilding = ref(false)

// 编译与预览
const compileState = ref<CompileState>('idle')
const compileError = ref<AppError | null>(null)
const pdfPath = ref<string | null>(null)
const compileDuration = ref<number | null>(null)
const compileStarting = ref(false)
let pollTimer: number | undefined
let saveTimer: number | undefined

onMounted(async () => {
  const ok = await store.openProject(projectId.value)
  if (!ok || !store.current) return
  formData.value = clone(store.current.data)
  rawText.value = JSON.stringify(formData.value, null, 2)
  try {
    schema.value = await api.getTemplateSchema(store.current.project.templateId)
    schemaError.value = null
  } catch (err) {
    schemaError.value = err instanceof Error ? err.message : String(err)
  }
  void validate()
  pdfPath.value = await api.latestOutput(projectId.value).catch(() => null)
  const status = await api.getCompileStatus(projectId.value).catch(() => null)
  if (status && (status.state === 'succeeded' || status.state === 'failed')) {
    compileState.value = status.state
    if (status.state === 'failed') compileError.value = status.error
  }
})

onBeforeUnmount(() => {
  window.clearTimeout(saveTimer)
  window.clearInterval(pollTimer)
})

function clone(v: Record<string, unknown>): Record<string, unknown> {
  return JSON.parse(JSON.stringify(v))
}

// 数据变化：防抖自动保存 + 校验
watch(formData, () => {
  rawText.value = JSON.stringify(formData.value, null, 2)
  window.clearTimeout(saveTimer)
  saveTimer = window.setTimeout(() => {
    void persist()
  }, 800)
})

async function persist(): Promise<void> {
  if (!store.current) return
  await store.saveData(clone(formData.value))
  await validate()
}

async function validate(): Promise<void> {
  if (!store.current) return
  try {
    issues.value = await api.validateDocumentData(
      store.current.project.templateId,
      clone(formData.value),
    )
  } catch {
    issues.value = []
  }
}

async function validateNow(): Promise<void> {
  await validate()
  if (issues.value.length === 0) {
    ElMessage.success('Schema 校验通过')
  }
}

async function loadSample(): Promise<void> {
  if (!store.current) return
  try {
    const sample = await api.getTemplateSample(store.current.project.templateId)
    formData.value = clone(sample)
    ElMessage.success('已加载示例数据（将自动保存）')
  } catch (err) {
    ElMessage.error(err instanceof Error ? err.message : String(err))
  }
}

function applyRaw(): void {
  try {
    formData.value = JSON.parse(rawText.value)
    ElMessage.success('已应用 JSON 到表单（将自动保存）')
  } catch {
    ElMessage.error('JSON 解析失败，请检查格式')
  }
}

function exportJson(): void {
  const text = JSON.stringify(formData.value, null, 2)
  void navigator.clipboard
    ?.writeText(text)
    .then(() => ElMessage.success('JSON 已复制到剪贴板'))
    .catch(() => undefined)
  void ElMessageBox.alert(
    `<pre style="max-height:50vh;overflow:auto;white-space:pre-wrap;margin:0;">${text.replace(/</g, '&lt;')}</pre>`,
    '导出 JSON（已复制到剪贴板）',
    { dangerouslyUseHTMLString: true, confirmButtonText: '关闭' },
  )
}

// 导入预览（阶段 4）
const importDialogVisible = ref(false)
const importPreview = ref<ImportPreview | null>(null)
const importSourceText = ref('')
const importApplying = ref(false)

/** 与当前数据的顶层字段对比（让用户亲眼确认修改被识别） */
const importDiff = computed(() => {
  const preview = importPreview.value
  if (!preview) return []
  const old = (store.current?.data ?? {}) as Record<string, unknown>
  const keys = new Set([...Object.keys(old), ...Object.keys(preview.data)])
  const rows: { key: string; kind: string; oldV: string; newV: string }[] = []
  for (const key of keys) {
    const oldText = JSON.stringify(old[key] ?? null)
    const newText = JSON.stringify(preview.data[key] ?? null)
    if (oldText !== newText) {
      rows.push({
        key,
        kind: key in old ? '修改' : '新增',
        oldV: oldText.length > 80 ? `${oldText.slice(0, 80)}…` : oldText,
        newV: newText.length > 80 ? `${newText.slice(0, 80)}…` : newText,
      })
    }
  }
  return rows
})

async function pickAndImport(kind: 'excel' | 'json'): Promise<void> {
  if (!store.current) return
  const file = await openFileDialog({
    multiple: false,
    title: kind === 'excel' ? '选择 Excel 文件' : '选择 JSON 文件',
    filters:
      kind === 'excel'
        ? [{ name: 'Excel', extensions: ['xlsx', 'xls', 'xlsm'] }]
        : [{ name: 'JSON', extensions: ['json'] }],
  })
  if (!file || typeof file !== 'string') return
  try {
    const preview =
      kind === 'excel'
        ? await api.importExcelData(projectId.value, file)
        : await api.importJsonData(projectId.value, file)
    importPreview.value = preview
    importSourceText.value = kind === 'excel' ? 'Excel' : 'JSON'
    importDialogVisible.value = true
  } catch (err) {
    ElMessage.error(formatAppError(err))
  }
}

async function confirmImport(): Promise<void> {
  if (!importPreview.value) return
  importApplying.value = true
  try {
    await store.saveData(clone(importPreview.value.data))
    importDialogVisible.value = false
    ElMessage.success('导入已写入项目数据（将自动保存）')
  } catch (err) {
    ElMessage.error(formatAppError(err))
  } finally {
    importApplying.value = false
  }
}

/** 导入图片到 assets 并把引用追加到正文 */
async function importImage(): Promise<void> {
  if (!store.current) return
  const file = await openFileDialog({
    multiple: false,
    title: '选择图片',
    filters: [{ name: '图片', extensions: ['png', 'jpg', 'jpeg', 'webp', 'gif', 'svg', 'bmp'] }],
  })
  if (!file || typeof file !== 'string') return
  try {
    const rel = await api.importProjectAsset(projectId.value, file)
    const name = file.split(/[\\/]/).pop()?.replace(/\.[^.]+$/, '') ?? '图片'
    const body = (formData.value['body'] as string | undefined) ?? ''
    const snippet = `\n#figure(\n  image("${rel}", width: 10cm),\n  caption: [${name}],\n)\n`
    formData.value = { ...formData.value, body: body + snippet }
    ElMessage.success(`图片已导入：${rel}（引用已追加到正文）`)
  } catch (err) {
    ElMessage.error(formatAppError(err))
  }
}

// ---------- 编译与预览 ----------

async function generate(): Promise<void> {
  if (!store.current) return
  compileStarting.value = true
  compileError.value = null
  try {
    compileState.value = await api.compileDocument(projectId.value)
    startPolling()
  } catch (err) {
    compileState.value = 'failed'
    compileError.value = err as AppError
  } finally {
    compileStarting.value = false
  }
}

function startPolling(): void {
  window.clearInterval(pollTimer)
  pollTimer = window.setInterval(async () => {
    try {
      const s = await api.getCompileStatus(projectId.value)
      compileState.value = s.state
      if (s.state === 'succeeded') {
        window.clearInterval(pollTimer)
        compileDuration.value = s.durationMs
        compileError.value = null
        pdfPath.value = s.outputPath
        ElMessage.success(`PDF 已生成（${s.durationMs ?? 0}ms）`)
      } else if (s.state === 'failed') {
        window.clearInterval(pollTimer)
        compileError.value = s.error
      } else if (s.state === 'cancelled') {
        window.clearInterval(pollTimer)
        ElMessage.info('编译已取消')
      }
    } catch {
      // 瞬时查询失败继续轮询
    }
  }, 400)
}

async function cancelCompile(): Promise<void> {
  try {
    await api.cancelCompile(projectId.value)
  } catch (err) {
    ElMessage.error(err instanceof Error ? err.message : String(err))
  }
}

function openPdf(): void {
  if (!pdfPath.value) return
  void api.openOutputFile(pdfPath.value).catch((err) => ElMessage.error(err instanceof Error ? err.message : String(err)))
}

function revealPdf(): void {
  if (!pdfPath.value) return
  void api.revealInExplorer(pdfPath.value).catch((err) => ElMessage.error(err instanceof Error ? err.message : String(err)))
}

async function rebuildAndRetry(): Promise<void> {
  rebuilding.value = true
  try {
    const n = await store.rebuildIndex()
    ElMessage.success(`索引已重建（${n} 个项目）`)
    const ok = await store.openProject(projectId.value)
    if (ok && store.current) {
      formData.value = clone(store.current.data)
      rawText.value = JSON.stringify(formData.value, null, 2)
    }
  } catch (err) {
    ElMessage.error(err instanceof Error ? err.message : String(err))
  } finally {
    rebuilding.value = false
  }
}

const saveStateText = computed(() => {
  if (store.saving) return '保存中…'
  if (store.saveError) return '保存失败'
  if (store.lastSavedAt) return `已自动保存 ${store.lastSavedAt}`
  return '自动保存已开启'
})

const saveStateType = computed(() => {
  if (store.saving) return 'info'
  if (store.saveError) return 'danger'
  return 'success'
})

const stateTextMap: Record<CompileState, string> = {
  idle: '待生成',
  validating: '校验中…',
  compiling: '编译中…',
  succeeded: '编译成功',
  failed: '编译失败',
  cancelled: '已取消',
}
const stateTagMap: Record<CompileState, string> = {
  idle: 'info',
  validating: 'info',
  compiling: 'warning',
  succeeded: 'success',
  failed: 'danger',
  cancelled: 'info',
}
</script>

<template>
  <div class="project-page" v-loading="store.currentLoading">
    <div class="page-topbar">
      <el-button :icon="ArrowLeft" @click="router.push('/')">返回列表</el-button>
      <template v-if="store.current">
        <h2 class="project-name">{{ store.current.project.name }}</h2>
        <el-tag type="info" effect="plain">{{ docTypeText(store.current.project.documentType) }}</el-tag>
        <el-tag type="info" effect="plain">模板 {{ store.current.project.templateVersion }}</el-tag>
      </template>
      <div class="topbar-spacer" />
      <el-tag v-if="store.current" :type="(saveStateType as any)" effect="dark">{{ saveStateText }}</el-tag>
    </div>

    <el-alert
      v-if="store.currentError"
      type="error"
      show-icon
      :closable="false"
      class="load-error"
      :title="`${errorKindText(store.currentErrorKind)}：${store.currentError}`"
    >
      <template v-if="store.currentErrorKind === 'project_files_missing' || store.currentErrorKind === 'db_corrupted'">
        <el-button size="small" type="primary" :icon="MagicStick" :loading="rebuilding" @click="rebuildAndRetry">
          重建索引并重试
        </el-button>
      </template>
    </el-alert>

    <template v-if="store.current">
      <!-- 表单卡片 -->
      <el-card shadow="never">
        <template #header>
          <div class="editor-header">
            <span>文档表单（由模板 Schema 驱动）</span>
            <div class="editor-actions">
              <el-button size="small" @click="loadSample">加载示例数据</el-button>
              <el-button size="small" @click="pickAndImport('excel')">导入 Excel</el-button>
              <el-button size="small" @click="pickAndImport('json')">导入 JSON</el-button>
              <el-button size="small" @click="importImage">导入图片</el-button>
              <el-button size="small" @click="validateNow">校验</el-button>
              <el-button size="small" @click="exportJson">导出 JSON</el-button>
            </div>
          </div>
        </template>

        <el-alert
          v-if="schemaError"
          type="error"
          show-icon
          :closable="false"
          :title="`模板 Schema 加载失败：${schemaError}`"
          class="block-alert"
        />

        <el-alert
          v-else-if="issues.length"
          type="warning"
          show-icon
          :closable="false"
          :title="`Schema 校验：${issues.length} 个问题`"
          class="block-alert"
        >
          <ul class="issue-list">
            <li v-for="(it, i) in issues.slice(0, 10)" :key="i">
              <code>{{ it.instancePath || '(根)' }}</code> {{ it.message }}
            </li>
          </ul>
          <span v-if="issues.length > 10">… 共 {{ issues.length }} 条</span>
        </el-alert>

        <SchemaForm v-if="schema" :schema="schema" v-model="formData" />
        <el-empty v-else-if="!schemaError" description="正在加载模板 Schema…" :image-size="60" />

        <el-collapse v-model="rawVisible" class="raw-collapse">
          <el-collapse-item title="Typst 语法速查" name="cheatsheet">
            <pre class="cheatsheet mono">== 二级标题        === 三级标题
- 无序列表        1. 有序列表
*加粗*            _斜体_           `等宽代码`

#table(
  columns: 2,
  table.header([*列一*], [*列二*]),
  [值], [值],
)

#figure(
  image("assets/图片.png", width: 10cm),
  caption: [图注文字],
)

#pagebreak()      // 分页</pre>
          </el-collapse-item>
          <el-collapse-item title="原始 JSON（高级）" name="raw">
            <el-input
              v-model="rawText"
              type="textarea"
              :rows="14"
              spellcheck="false"
              class="raw-editor"
            />
            <div class="raw-actions">
              <el-button size="small" type="primary" @click="applyRaw">应用到表单</el-button>
            </div>
          </el-collapse-item>
        </el-collapse>
      </el-card>

      <!-- 编译与预览卡片 -->
      <el-card shadow="never">
        <template #header>
          <div class="editor-header">
            <span>PDF 生成与预览（内置 Typst sidecar）</span>
            <div class="editor-actions">
              <el-button
                size="small"
                type="primary"
                :loading="compileStarting || compileState === 'compiling'"
                @click="generate"
              >
                生成 PDF
              </el-button>
              <el-button v-if="compileState === 'compiling'" size="small" type="warning" @click="cancelCompile">
                取消编译
              </el-button>
              <el-button v-if="pdfPath" size="small" @click="openPdf">打开 PDF 文件</el-button>
              <el-button v-if="pdfPath" size="small" @click="revealPdf">所在文件夹</el-button>
            </div>
          </div>
        </template>

        <div class="compile-status">
          <el-tag :type="(stateTagMap[compileState] as any)" effect="dark">
            {{ stateTextMap[compileState] }}
          </el-tag>
          <span v-if="compileDuration !== null" class="hint">上次编译耗时 {{ compileDuration }}ms</span>
          <span v-if="pdfPath" class="mono hint">{{ pdfPath }}</span>
        </div>

        <el-alert
          v-if="compileError"
          type="error"
          show-icon
          :closable="false"
          class="block-alert"
          :title="`${errorKindText(compileError.kind)}：${compileError.message}`"
        >
          <el-table
            v-if="compileError.diagnostics?.length"
            :data="compileError.diagnostics"
            size="small"
            class="diag-table"
          >
            <el-table-column label="级别" width="90">
              <template #default="{ row }">
                <el-tag size="small" :type="row.severity === 'error' ? 'danger' : 'warning'">{{ row.severity }}</el-tag>
              </template>
            </el-table-column>
            <el-table-column prop="message" label="错误内容" min-width="220" />
            <el-table-column label="文件 / 行号" width="240">
              <template #default="{ row }">
                <span class="mono">{{ row.file ?? '—' }}{{ row.line ? `:${row.line}:${row.column ?? 0}` : '' }}</span>
              </template>
            </el-table-column>
          </el-table>
          <details v-if="compileError.stderr" class="stderr-box">
            <summary>完整编译器输出</summary>
            <pre class="mono">{{ compileError.stderr }}</pre>
          </details>
        </el-alert>

        <PdfViewer :path="pdfPath" />
      </el-card>
    </template>

    <!-- 导入预览对话框 -->
    <el-dialog v-model="importDialogVisible" :title="`导入预览（${importSourceText}）`" width="840px">
      <template v-if="importPreview">
        <el-alert
          v-if="importPreview.issues.some((i) => i.severity === 'error')"
          type="warning"
          show-icon
          :closable="false"
          class="block-alert"
          :title="`发现 ${importPreview.issues.filter((i) => i.severity === 'error').length} 个校验问题，确认写入前请检查`"
        />
        <el-descriptions :column="2" border size="small" class="imp-desc">
          <el-descriptions-item v-if="importPreview.sheetName" label="工作表">
            {{ importPreview.sheetName }}
          </el-descriptions-item>
          <el-descriptions-item label="数据行">{{ importPreview.rows.length }}</el-descriptions-item>
        </el-descriptions>

        <div class="imp-section">与当前数据的变更对比</div>
        <el-alert
          v-if="!importDiff.length"
          type="info"
          :closable="false"
          title="导入内容与当前数据无差异"
        />
        <el-table v-else :data="importDiff" size="small" max-height="200">
          <el-table-column prop="key" label="字段" width="140" />
          <el-table-column label="类型" width="80">
            <template #default="{ row }">
              <el-tag size="small" :type="row.kind === '新增' ? 'success' : 'warning'">{{ row.kind }}</el-tag>
            </template>
          </el-table-column>
          <el-table-column prop="oldV" label="当前值">
            <template #default="{ row }"><span class="mono">{{ row.oldV }}</span></template>
          </el-table-column>
          <el-table-column prop="newV" label="导入值">
            <template #default="{ row }"><span class="mono">{{ row.newV }}</span></template>
          </el-table-column>
        </el-table>

        <template v-if="importPreview.columnPaths.length">
          <div class="imp-section">列映射</div>
          <el-table :data="importPreview.columnPaths" size="small" max-height="180">
            <el-table-column prop="column" label="列名" width="180" />
            <el-table-column prop="path" label="写入字段" />
          </el-table>
        </template>

        <template v-if="importPreview.rows.length">
          <div class="imp-section">行预览</div>
          <el-table :data="importPreview.rows" size="small" max-height="220">
            <el-table-column prop="row" label="行号" width="70" />
            <el-table-column label="写入内容">
              <template #default="{ row }">
                <span v-for="(v, k) in row.values" :key="k" class="mono imp-kv">{{ k }}={{ v }}</span>
              </template>
            </el-table-column>
          </el-table>
        </template>

        <template v-if="importPreview.issues.length">
          <div class="imp-section">问题列表</div>
          <el-table :data="importPreview.issues" size="small" max-height="200">
            <el-table-column label="级别" width="80">
              <template #default="{ row }">
                <el-tag size="small" :type="row.severity === 'error' ? 'danger' : 'warning'">
                  {{ row.severity === 'error' ? '错误' : '警告' }}
                </el-tag>
              </template>
            </el-table-column>
            <el-table-column label="位置" width="220">
              <template #default="{ row }">
                <span class="mono">
                  {{ row.sheet ?? '' }}{{ row.row ? ` 第${row.row}行` : '' }}{{ row.column ? ` 「${row.column}」` : '' }}
                </span>
              </template>
            </el-table-column>
            <el-table-column prop="message" label="说明" />
          </el-table>
        </template>
      </template>
      <template #footer>
        <el-button @click="importDialogVisible = false">取消</el-button>
        <el-button type="primary" :loading="importApplying" @click="confirmImport">
          确认写入项目数据
        </el-button>
      </template>
    </el-dialog>

    <el-empty
      v-if="!store.current && !store.currentLoading && !store.currentError"
      description="项目未找到"
    />
  </div>
</template>

<style scoped>
.project-page {
  max-width: 1280px;
  margin: 0 auto;
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.page-topbar {
  display: flex;
  align-items: center;
  gap: 12px;
  flex-wrap: wrap;
}

.project-name {
  margin: 0;
  font-size: 18px;
}

.topbar-spacer {
  flex: 1;
}

.load-error {
  margin-bottom: 2px;
}

.editor-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  flex-wrap: wrap;
}

.editor-actions {
  display: flex;
  gap: 0;
  flex-wrap: wrap;
}

.block-alert {
  margin-bottom: 14px;
}

.issue-list {
  margin: 6px 0 0;
  padding-left: 18px;
  font-size: 12px;
}

.issue-list code {
  background: var(--el-fill-color);
  padding: 0 4px;
  border-radius: 3px;
  margin-right: 4px;
}

.raw-collapse {
  margin-top: 18px;
  border-top: 1px dashed var(--el-border-color-lighter);
}

.cheatsheet {
  margin: 0;
  font-size: 12px;
  line-height: 1.8;
  background: var(--el-fill-color-lighter);
  padding: 10px 12px;
  border-radius: 6px;
  white-space: pre-wrap;
}

.raw-editor :deep(textarea) {
  font-family: Consolas, 'Courier New', monospace;
  font-size: 13px;
  line-height: 1.6;
}

.raw-actions {
  margin-top: 8px;
  display: flex;
  justify-content: flex-end;
}

.compile-status {
  display: flex;
  align-items: center;
  gap: 12px;
  flex-wrap: wrap;
  margin-bottom: 12px;
}

.compile-status .hint {
  font-size: 12px;
  color: var(--el-text-color-secondary);
  word-break: break-all;
}

.mono {
  font-family: Consolas, 'Courier New', monospace;
}

.diag-table {
  margin-top: 10px;
}

.stderr-box {
  margin-top: 10px;
  font-size: 12px;
}

.stderr-box summary {
  cursor: pointer;
  color: var(--el-color-primary);
}

.stderr-box pre {
  max-height: 220px;
  overflow: auto;
  background: var(--el-fill-color);
  padding: 8px;
  border-radius: 4px;
  white-space: pre-wrap;
}

.imp-desc {
  margin-bottom: 12px;
}

.imp-section {
  font-weight: 600;
  font-size: 13px;
  margin: 12px 0 6px;
}

.imp-kv {
  display: inline-block;
  margin-right: 10px;
  font-size: 12px;
}
</style>

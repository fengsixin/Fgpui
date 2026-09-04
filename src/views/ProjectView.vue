<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { ArrowLeft, MagicStick } from '@element-plus/icons-vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { useProjectsStore } from '@/stores/projectsStore'
import { docTypeText, errorKindText } from '@/api/types'
import type { SchemaIssue } from '@/api/types'
import * as api from '@/api/client'
import SchemaForm from '@/components/SchemaForm.vue'

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
const rawVisible = ref(false)
const rawText = ref('')
const rebuilding = ref(false)

let timer: number | undefined

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
})

onBeforeUnmount(() => {
  window.clearTimeout(timer)
})

function clone(v: Record<string, unknown>): Record<string, unknown> {
  return JSON.parse(JSON.stringify(v))
}

// 数据变化：防抖自动保存 + 校验
watch(formData, () => {
  rawText.value = JSON.stringify(formData.value, null, 2)
  window.clearTimeout(timer)
  timer = window.setTimeout(() => {
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

async function validateNow(): Promise<void> {
  await validate()
  if (issues.value.length === 0) {
    ElMessage.success('Schema 校验通过')
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
      <el-card shadow="never">
        <template #header>
          <div class="editor-header">
            <span>文档表单（由模板 Schema 驱动）</span>
            <div class="editor-actions">
              <el-button size="small" @click="loadSample">加载示例数据</el-button>
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
    </template>

    <el-empty v-else-if="!store.currentLoading && !store.currentError" description="项目未找到" />
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
</style>

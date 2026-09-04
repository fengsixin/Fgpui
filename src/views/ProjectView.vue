<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { ArrowLeft, MagicStick } from '@element-plus/icons-vue'
import { ElMessage } from 'element-plus'
import { useProjectsStore } from '@/stores/projectsStore'
import { docTypeText, errorKindText } from '@/api/types'

const route = useRoute()
const router = useRouter()
const store = useProjectsStore()

const projectId = computed(() => String(route.params.id ?? ''))
const dataText = ref('')
const jsonValid = ref(true)
const rebuilding = ref(false)

let timer: number | undefined

onMounted(async () => {
  const ok = await store.openProject(projectId.value)
  if (ok && store.current) {
    dataText.value = JSON.stringify(store.current.data, null, 2)
  }
})

onBeforeUnmount(() => {
  window.clearTimeout(timer)
})

watch(dataText, (value) => {
  try {
    JSON.parse(value)
    jsonValid.value = true
  } catch {
    jsonValid.value = false
    return // 非法 JSON 不自动保存，避免把坏数据写盘
  }
  window.clearTimeout(timer)
  timer = window.setTimeout(() => {
    void autosave()
  }, 800)
})

async function autosave(): Promise<void> {
  if (!store.current || !jsonValid.value) return
  const canonical = JSON.stringify(store.current.data, null, 2)
  if (dataText.value === canonical) return // 无变化不写盘
  await store.saveData(JSON.parse(dataText.value))
}

async function saveNow(): Promise<void> {
  if (!jsonValid.value) {
    ElMessage.warning('JSON 格式有误，无法保存')
    return
  }
  await store.saveData(JSON.parse(dataText.value))
}

async function rebuildAndRetry(): Promise<void> {
  rebuilding.value = true
  try {
    const n = await store.rebuildIndex()
    ElMessage.success(`索引已重建（${n} 个项目）`)
    const ok = await store.openProject(projectId.value)
    if (ok && store.current) {
      dataText.value = JSON.stringify(store.current.data, null, 2)
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
      <el-tag v-if="store.current && !jsonValid" type="warning" effect="dark">JSON 格式错误，已暂停保存</el-tag>
      <el-tag v-if="store.current" :type="(saveStateType as any)" effect="dark">{{ saveStateText }}</el-tag>
    </div>

    <el-alert
      v-if="store.currentError"
      type="error"
      show-icon
      :closable="false"
      class="load-error"
      :title="`${errorKindText(store.currentErrorKind)}：${store.currentError}`"
      :description="store.currentErrorKind === 'project_files_missing' || store.currentErrorKind === 'db_corrupted'
        ? '项目文件仍保存在磁盘上，可尝试重建索引恢复，或返回列表删除残留。'
        : undefined"
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
            <span>文档数据（JSON）</span>
            <span class="hint">阶段 2 起该区域将由模板 Schema 驱动为业务表单；当前可直接编辑 JSON，修改后 0.8 秒自动保存</span>
          </div>
        </template>

        <el-alert
          v-if="!jsonValid"
          type="warning"
          show-icon
          :closable="false"
          title="JSON 解析失败"
          description="已暂停自动保存，修正格式后恢复。"
          class="json-alert"
        />

        <el-alert
          v-if="store.saveError"
          type="error"
          show-icon
          :closable="false"
          :title="`保存失败：${store.saveError}`"
          class="json-alert"
        />

        <el-input
          v-model="dataText"
          type="textarea"
          :rows="20"
          spellcheck="false"
          class="data-editor"
          placeholder='{"title": "文档标题"}'
        />

        <div class="editor-footer">
          <span class="hint">{{ store.workspaceInfo?.projects ?? '' }}</span>
          <el-button size="small" :disabled="!jsonValid" :loading="store.saving" @click="saveNow">
            立即保存
          </el-button>
        </div>
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
  flex-direction: column;
  gap: 4px;
}

.editor-header .hint {
  font-size: 12px;
  color: var(--el-text-color-secondary);
  font-weight: normal;
}

.json-alert {
  margin-bottom: 12px;
}

.data-editor :deep(textarea) {
  font-family: Consolas, 'Courier New', monospace;
  font-size: 13px;
  line-height: 1.6;
}

.editor-footer {
  margin-top: 12px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.editor-footer .hint {
  font-size: 12px;
  color: var(--el-text-color-secondary);
  word-break: break-all;
}
</style>

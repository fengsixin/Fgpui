<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { Delete, Folder, MagicStick, Plus, Refresh, Upload } from '@element-plus/icons-vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { open as openFileDialog } from '@tauri-apps/plugin-dialog'
import { useProjectsStore } from '@/stores/projectsStore'
import { formatAppError } from '@/stores/appStore'
import { docTypeText } from '@/api/types'

const router = useRouter()
const store = useProjectsStore()

const createVisible = ref(false)
const creating = ref(false)
const rebuilding = ref(false)
const importing = ref(false)
const form = ref({ name: '', templateId: 'technical-design' })

/** 可用模板（有效包） */
const availableTemplates = computed(() =>
  store.templates.filter((t) => t.manifest && !t.error),
)

onMounted(() => {
  void store.refreshList()
  void store.loadWorkspaceInfo()
  void store.refreshTemplates()
})

// 每次打开新建对话框时重新扫描模板（新导入的模板即刻可用）
watch(createVisible, (visible) => {
  if (visible) void store.refreshTemplates()
})

function fmtDate(iso: string | null | undefined): string {
  if (!iso) return '—'
  const d = new Date(iso)
  if (Number.isNaN(d.getTime())) return iso
  return d.toLocaleString('zh-CN', { hour12: false })
}

function openProject(id: string): void {
  void router.push(`/project/${id}`)
}

async function submitCreate(): Promise<void> {
  if (!form.value.name.trim()) {
    ElMessage.warning('请输入项目名称')
    return
  }
  if (!form.value.templateId) {
    ElMessage.warning('请选择模板')
    return
  }
  creating.value = true
  try {
    const created = await store.createProject(form.value.name, form.value.templateId)
    createVisible.value = false
    form.value = { name: '', templateId: form.value.templateId }
    ElMessage.success('项目已创建')
    void router.push(`/project/${created.id}`)
  } catch (err) {
    ElMessage.error(formatAppError(err))
  } finally {
    creating.value = false
  }
}

async function rebuild(): Promise<void> {
  rebuilding.value = true
  try {
    const n = await store.rebuildIndex()
    ElMessage.success(`索引已重建，从文件恢复 ${n} 个项目`)
  } catch (err) {
    ElMessage.error(formatAppError(err))
  } finally {
    rebuilding.value = false
  }
}

async function importTemplate(): Promise<void> {
  const dir = await openFileDialog({
    multiple: false,
    directory: true,
    title: '选择模板包目录（内含 manifest.json）',
  })
  if (!dir || typeof dir !== 'string') return
  importing.value = true
  try {
    await store.importTemplate(dir)
    ElMessage.success('模板已导入')
  } catch (err) {
    ElMessage.error(formatAppError(err))
  } finally {
    importing.value = false
  }
}

async function confirmDelete(id: string): Promise<void> {
  try {
    await store.removeProject(id)
    ElMessage.success('项目已删除')
  } catch (err) {
    ElMessage.error(formatAppError(err))
  }
}

function showWorkspacePath(): void {
  const root = store.workspaceInfo?.root
  if (root) {
    void ElMessageBox.alert(root, '数据目录（可直接复制该目录进行备份）', { confirmButtonText: '知道了' })
  }
}
</script>

<template>
  <div class="projects-page">
    <el-card shadow="never" class="toolbar">
      <div class="toolbar-row">
        <div class="toolbar-actions">
          <el-button type="primary" :icon="Plus" @click="createVisible = true">新建项目</el-button>
          <el-button :icon="Refresh" :loading="store.listLoading" @click="store.refreshList()">刷新</el-button>
          <el-tooltip content="项目文件是事实来源：数据库损坏或索引缺失时，从 projects 目录的 project.json 恢复索引" placement="bottom">
            <el-button :icon="MagicStick" :loading="rebuilding" @click="rebuild">重建索引</el-button>
          </el-tooltip>
          <el-tooltip content="从本地目录导入模板包（目录内需含 manifest.json）" placement="bottom">
            <el-button :icon="Upload" :loading="importing" @click="importTemplate">导入模板</el-button>
          </el-tooltip>
        </div>
        <el-tag
          v-if="store.workspaceInfo"
          type="info"
          effect="plain"
          class="ws-tag"
          @click="showWorkspacePath"
        >
          <el-icon><Folder /></el-icon>
          数据目录：{{ store.workspaceInfo.root }}
        </el-tag>
      </div>
    </el-card>

    <el-alert
      v-if="store.listErrorKind === 'db_corrupted'"
      type="error"
      show-icon
      :closable="false"
      class="db-alert"
      :title="store.listError ?? '项目索引数据库损坏'"
      description="项目文件本身未受影响（保存在 projects 目录）。点击「重建索引」即可从 project.json 恢复项目列表。"
    />

    <el-card shadow="never">
      <el-table
        :data="store.list"
        v-loading="store.listLoading"
        empty-text="暂无项目，点击右上角「新建项目」开始"
        style="width: 100%"
      >
        <el-table-column prop="name" label="项目名称" min-width="220">
          <template #default="{ row }">
            <el-link type="primary" :underline="false" @click="openProject(row.id)">{{ row.name }}</el-link>
          </template>
        </el-table-column>
        <el-table-column label="文档类型" width="140">
          <template #default="{ row }">{{ docTypeText(row.documentType) }}</template>
        </el-table-column>
        <el-table-column label="模板版本" width="110">
          <template #default="{ row }">
            <el-tag size="small" effect="plain">{{ row.templateVersion }}</el-tag>
          </template>
        </el-table-column>
        <el-table-column label="创建时间" width="180">
          <template #default="{ row }">{{ fmtDate(row.createdAt) }}</template>
        </el-table-column>
        <el-table-column label="最近更新" width="180">
          <template #default="{ row }">{{ fmtDate(row.updatedAt) }}</template>
        </el-table-column>
        <el-table-column label="状态" width="110">
          <template #default="{ row }">
            <el-tag v-if="row.integrity === 'ok'" type="success" size="small">正常</el-tag>
            <el-tag v-else type="danger" size="small">文件缺失</el-tag>
          </template>
        </el-table-column>
        <el-table-column label="操作" width="170" fixed="right">
          <template #default="{ row }">
            <el-button
              size="small"
              type="primary"
              link
              :disabled="row.integrity !== 'ok'"
              @click="openProject(row.id)"
            >
              打开
            </el-button>
            <el-popconfirm
              title="删除项目目录及其数据？模板与字体不受影响。"
              confirm-button-text="删除"
              cancel-button-text="取消"
              confirm-button-type="danger"
              @confirm="confirmDelete(row.id)"
            >
              <template #reference>
                <el-button size="small" type="danger" link :icon="Delete">删除</el-button>
              </template>
            </el-popconfirm>
          </template>
        </el-table-column>
      </el-table>
    </el-card>

    <el-dialog v-model="createVisible" title="新建项目" width="480px">
      <el-form label-width="90px" @submit.prevent>
        <el-form-item label="项目名称" required>
          <el-input
            v-model="form.name"
            placeholder="例如：XX 系统技术方案"
            maxlength="100"
            show-word-limit
            @keyup.enter="submitCreate"
          />
        </el-form-item>
        <el-form-item label="文档模板" required>
          <el-select
            v-model="form.templateId"
            :loading="store.templatesLoading"
            style="width: 100%"
            placeholder="选择模板"
          >
            <el-option
              v-for="t in availableTemplates"
              :key="t.manifest!.id"
              :label="`${t.manifest!.name}（${t.manifest!.version}）`"
              :value="t.manifest!.id"
            />
          </el-select>
          <div v-if="!availableTemplates.length" class="tpl-empty">
            未发现可用模板，请检查工作区 templates 目录或使用「导入模板」。
          </div>
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="createVisible = false">取消</el-button>
        <el-button type="primary" :loading="creating" @click="submitCreate">创建并打开</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<style scoped>
.projects-page {
  max-width: 1280px;
  margin: 0 auto;
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.toolbar-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  flex-wrap: wrap;
}

.toolbar-actions {
  display: flex;
  align-items: center;
  gap: 0;
  flex-wrap: wrap;
}

.ws-tag {
  cursor: pointer;
  max-width: 560px;
  overflow: hidden;
  text-overflow: ellipsis;
}

.db-alert {
  margin-bottom: 2px;
}

.tpl-empty {
  font-size: 12px;
  color: var(--el-color-danger);
  margin-top: 4px;
}
</style>

<script setup lang="ts">
import { onMounted, computed } from 'vue'
import { Refresh, VideoPlay, FolderOpened, CircleCheckFilled, CircleCloseFilled } from '@element-plus/icons-vue'
import { useAppStore } from '@/stores/appStore'
import { errorKindText } from '@/api/types'

const store = useAppStore()

onMounted(() => {
  void store.refreshTypstStatus()
})

const envOk = computed(() => store.typstStatus?.ok === true)
const versionText = computed(() => store.typstStatus?.version ?? '—')
const pathText = computed(() => store.typstStatus?.path ?? '—')

const severityMap: Record<string, string> = {
  error: 'danger',
  warning: 'warning',
  note: 'info',
}
</script>

<template>
  <div class="home">
    <el-alert
      type="info"
      show-icon
      :closable="false"
      class="phase-banner"
      title="阶段 0 · 工程初始化与 Typst 可行性验证"
      description="验证「内置 Typst sidecar → 编译测试模板 → 生成 PDF」链路。本页所有检查均由 Rust 后端调用随应用打包的 typst.exe 完成，不依赖系统安装的 Typst。"
    />

    <el-row :gutter="16">
      <el-col :span="12">
        <el-card shadow="never" class="panel">
          <template #header>
            <div class="panel-header">
              <span>Typst 环境自检</span>
              <el-button
                size="small"
                :icon="Refresh"
                :loading="store.typstChecking"
                @click="store.refreshTypstStatus()"
              >
                重新检查
              </el-button>
            </div>
          </template>

          <el-skeleton v-if="store.typstChecking && !store.typstStatus" :rows="3" animated />
          <template v-else>
            <div class="env-line">
              <el-icon :class="envOk ? 'ok' : 'bad'">
                <CircleCheckFilled v-if="envOk" />
                <CircleCloseFilled v-else />
              </el-icon>
              <span>{{ envOk ? '内置 Typst 可用' : '内置 Typst 不可用' }}</span>
            </div>
            <el-descriptions :column="1" border size="small" class="env-desc">
              <el-descriptions-item label="版本">{{ versionText }}</el-descriptions-item>
              <el-descriptions-item label="程序路径">{{ pathText }}</el-descriptions-item>
              <el-descriptions-item label="说明">{{ store.typstStatus?.detail ?? '—' }}</el-descriptions-item>
            </el-descriptions>
            <el-alert
              v-if="!envOk && store.typstStatus"
              type="error"
              :title="errorKindText(store.typstStatus.kind)"
              :description="store.typstStatus.detail"
              show-icon
              :closable="false"
              class="env-error"
            />
          </template>
        </el-card>
      </el-col>

      <el-col :span="12">
        <el-card shadow="never" class="panel">
          <template #header>
            <div class="panel-header">
              <span>测试模板编译（hello.typ → PDF）</span>
              <el-button
                size="small"
                type="primary"
                :icon="VideoPlay"
                :loading="store.smokeRunning"
                :disabled="store.smokeRunning"
                @click="store.runSmokeTest()"
              >
                编译测试 PDF
              </el-button>
            </div>
          </template>

          <el-alert
            v-if="store.smokeRunning"
            type="info"
            :closable="false"
            show-icon
            title="正在后台编译…"
            description="编译在 Rust 侧异步执行，窗口保持可操作；此按钮转圈期间仍可拖动、缩放窗口。"
          />

          <template v-if="store.smokeResult">
            <el-result
              icon="success"
              title="编译成功"
              :sub-title="`耗时 ${store.smokeResult.durationMs} ms · ${store.smokeResult.typstVersion}`"
            >
              <template #extra>
                <el-button :icon="FolderOpened" @click="store.reveal(store.smokeResult!.outputPath)">
                  在资源管理器中显示 PDF
                </el-button>
              </template>
            </el-result>
            <el-descriptions :column="1" border size="small">
              <el-descriptions-item label="PDF 输出路径">
                <span class="mono">{{ store.smokeResult.outputPath }}</span>
              </el-descriptions-item>
              <el-descriptions-item label="警告数">
                {{ store.smokeResult.warnings.length }}
              </el-descriptions-item>
            </el-descriptions>
            <el-table
              v-if="store.smokeResult.warnings.length"
              :data="store.smokeResult.warnings"
              size="small"
              class="diag-table"
            >
              <el-table-column prop="severity" label="级别" width="90" />
              <el-table-column prop="message" label="内容" min-width="240" />
              <el-table-column label="位置" width="180">
                <template #default="{ row }">
                  <span class="mono">{{ row.file ?? '' }}{{ row.line ? `:${row.line}:${row.column ?? 0}` : '' }}</span>
                </template>
              </el-table-column>
            </el-table>
          </template>

          <el-alert
            v-else-if="store.smokeError"
            type="error"
            :title="store.smokeError"
            show-icon
            :closable="false"
          >
            <template v-if="store.smokeErrorRaw?.diagnostics?.length">
              <el-table :data="store.smokeErrorRaw.diagnostics" size="small" class="diag-table">
                <el-table-column label="级别" width="90">
                  <template #default="{ row }">
                    <el-tag :type="(severityMap[row.severity] ?? 'info') as any" size="small">
                      {{ row.severity }}
                    </el-tag>
                  </template>
                </el-table-column>
                <el-table-column prop="message" label="错误内容" min-width="220" />
                <el-table-column label="文件 / 行号" width="200">
                  <template #default="{ row }">
                    <span class="mono">
                      {{ row.file ?? '—' }}{{ row.line ? `:${row.line}:${row.column ?? 0}` : '' }}
                    </span>
                  </template>
                </el-table-column>
              </el-table>
              <details v-if="store.smokeErrorRaw?.stderr" class="stderr-box">
                <summary>完整编译器输出</summary>
                <pre class="mono">{{ store.smokeErrorRaw.stderr }}</pre>
              </details>
            </template>
            <template v-else-if="store.smokeErrorRaw?.stderr">
              <details class="stderr-box">
                <summary>完整编译器输出</summary>
                <pre class="mono">{{ store.smokeErrorRaw.stderr }}</pre>
              </details>
            </template>
          </el-alert>

          <el-empty
            v-else-if="!store.smokeRunning"
            description="点击「编译测试 PDF」验证编译链路"
            :image-size="80"
          />
        </el-card>
      </el-col>
    </el-row>
  </div>
</template>

<style scoped>
.home {
  max-width: 1280px;
  margin: 0 auto;
}

.phase-banner {
  margin-bottom: 16px;
}

.panel {
  height: 100%;
}

.panel-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.env-line {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 12px;
  font-size: 15px;
  font-weight: 600;
}

.env-line .ok {
  color: var(--el-color-success);
}

.env-line .bad {
  color: var(--el-color-danger);
}

.env-desc {
  margin-bottom: 12px;
}

.env-error {
  margin-top: 4px;
}

.diag-table {
  margin-top: 12px;
}

.mono {
  font-family: Consolas, 'Courier New', monospace;
  font-size: 12px;
  word-break: break-all;
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
</style>

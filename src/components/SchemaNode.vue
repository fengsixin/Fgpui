<script setup lang="ts">
/**
 * SchemaNode —— 递归渲染 JSON Schema 节点。
 * 支持：object（嵌套分组）/ string（枚举下拉、textarea）/ number / integer /
 * boolean / array（对象卡片列表、字符串列表）。字段全部由 Schema 驱动，
 * 不把任何业务字段硬编码在页面中。
 *
 * 更新语义：所有修改沿组件链「不可变替换」逐层上抛，由根组件整体替换数据对象。
 */
import { computed } from 'vue'

defineOptions({ name: 'SchemaNode' })

const props = defineProps<{
  schema: Record<string, unknown>
  modelValue: unknown
  required?: boolean
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', value: unknown): void
}>()

const label = computed(() => (props.schema?.title as string) ?? '')
const hint = computed(() => (props.schema?.description as string) ?? '')
const options = computed(() =>
  Array.isArray(props.schema?.enum) ? (props.schema.enum as unknown[]) : null,
)
const isTextarea = computed(
  () => props.schema?.format === 'textarea' || ((props.schema?.maxLength as number) ?? 0) > 100,
)
/** 代码编辑（Typst 源码等）：等宽、多行 */
const isCode = computed(() => props.schema?.format === 'code' || props.schema?.['x-ui'] === 'code')
const type = computed(() => inferType(props.schema))
const isEmpty = computed(
  () => props.modelValue === undefined || props.modelValue === null || props.modelValue === '',
)
/** 数组视图（modelValue 非数组时视为空） */
const items = computed(() => (Array.isArray(props.modelValue) ? (props.modelValue as unknown[]) : []))

function inferType(s: Record<string, unknown>): string {
  const t = s?.type
  if (typeof t === 'string') return t
  if (Array.isArray(s?.enum)) {
    return (s.enum as unknown[]).every((v) => typeof v === 'number') ? 'integer' : 'string'
  }
  if (s?.properties) return 'object'
  if (s?.items) return 'array'
  return 'string'
}

function update(value: unknown): void {
  emit('update:modelValue', value)
}

function setField(key: string | number, value: unknown): void {
  update({ ...((props.modelValue as Record<string, unknown>) ?? {}), [key]: value })
}

function defaultFor(s: Record<string, unknown>): unknown {
  if (s?.default !== undefined) return s.default
  switch (inferType(s)) {
    case 'object':
      return {}
    case 'array':
      return []
    case 'integer':
    case 'number':
      return (s?.minimum as number) ?? 0
    case 'boolean':
      return false
    default:
      return ''
  }
}

function addItem(): void {
  const arr = Array.isArray(props.modelValue) ? [...props.modelValue] : []
  arr.push(defaultFor((props.schema?.items as Record<string, unknown>) ?? {}))
  update(arr)
}

function removeItem(index: number): void {
  const arr = (props.modelValue as unknown[] ?? []).filter((_, i) => i !== index)
  update(arr)
}

function setItem(index: number, value: unknown): void {
  const arr = [...((props.modelValue as unknown[]) ?? [])]
  arr[index] = value
  update(arr)
}
</script>

<template>
  <!-- 数组 -->
  <div v-if="type === 'array'" class="node">
    <div class="node-head">
      <span class="label">{{ label }}<span v-if="required" class="req">*</span></span>
      <el-button size="small" type="primary" link @click="addItem">＋ 添加</el-button>
    </div>
    <div v-if="hint" class="hint">{{ hint }}</div>
    <div v-if="!items.length" class="empty">暂无条目</div>
    <div v-for="(item, i) in items" :key="i" class="array-item">
      <div class="array-item-tools">
        <span class="idx">#{{ i + 1 }}</span>
        <el-button size="small" type="danger" link @click="removeItem(i)">删除</el-button>
      </div>
      <SchemaNode
        :schema="(schema.items as Record<string, unknown>)"
        :model-value="item"
        @update:model-value="setItem(i, $event)"
      />
    </div>
  </div>

  <!-- 对象（嵌套分组） -->
  <div v-else-if="type === 'object'" class="node">
    <div v-if="label" class="node-head">
      <span class="label">{{ label }}<span v-if="required" class="req">*</span></span>
    </div>
    <div v-if="hint" class="hint">{{ hint }}</div>
    <div class="obj-props">
      <div v-for="(childSchema, key) in ((schema?.properties ?? {}) as Record<string, Record<string, unknown>>)" :key="key" class="prop">
        <SchemaNode
          :schema="childSchema"
          :model-value="((modelValue as Record<string, unknown>) ?? {})[key]"
          :required="((schema?.required as string[]) ?? []).includes(String(key))"
          @update:model-value="setField(key, $event)"
        />
      </div>
    </div>
  </div>

  <!-- 枚举 -->
  <div v-else-if="options" class="node inline">
    <span class="label">{{ label }}<span v-if="required" class="req">*</span></span>
    <el-select
      :model-value="modelValue"
      clearable
      style="min-width: 180px"
      @update:model-value="update"
    >
      <el-option v-for="o in options" :key="String(o)" :label="String(o)" :value="o" />
    </el-select>
    <span v-if="isEmpty && required" class="req-hint">必填</span>
  </div>

  <!-- 字符串 -->
  <div v-else-if="type === 'string'" class="node">
    <div v-if="label" class="node-head">
      <span class="label">{{ label }}<span v-if="required" class="req">*</span></span>
    </div>
    <el-input
      v-if="isCode"
      type="textarea"
      :rows="14"
      spellcheck="false"
      class="code-editor"
      :model-value="(modelValue as string) ?? ''"
      :placeholder="hint || undefined"
      @update:model-value="update($event || undefined)"
    />
    <el-input
      v-else-if="isTextarea"
      type="textarea"
      :rows="3"
      :model-value="(modelValue as string) ?? ''"
      @update:model-value="update($event || undefined)"
    />
    <el-input
      v-else
      :model-value="(modelValue as string) ?? ''"
      @update:model-value="update($event || undefined)"
    />
    <div v-if="hint" class="hint">{{ hint }}</div>
    <span v-if="isEmpty && required" class="req-hint">必填</span>
  </div>

  <!-- 数字 -->
  <div v-else-if="type === 'number' || type === 'integer'" class="node inline">
    <span class="label">{{ label }}<span v-if="required" class="req">*</span></span>
    <el-input-number
      :model-value="modelValue as number"
      :step="type === 'integer' ? 1 : 0.1"
      :value-on-clear="undefined"
      @update:model-value="update"
    />
    <span v-if="isEmpty && required" class="req-hint">必填</span>
    <div v-if="hint" class="hint">{{ hint }}</div>
  </div>

  <!-- 布尔 -->
  <div v-else-if="type === 'boolean'" class="node inline">
    <span class="label">{{ label }}<span v-if="required" class="req">*</span></span>
    <el-switch :model-value="!!modelValue" @update:model-value="update" />
  </div>

  <!-- 兜底 -->
  <div v-else class="node">
    <span class="label">{{ label }}</span>
    <el-input
      :model-value="modelValue == null ? '' : JSON.stringify(modelValue)"
      @update:model-value="update"
    />
  </div>
</template>

<style scoped>
.node {
  margin: 4px 0;
}

.node.inline {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-wrap: wrap;
}

.node-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  margin-bottom: 4px;
}

.label {
  font-weight: 600;
  font-size: 13px;
}

.req {
  color: var(--el-color-danger);
  margin-left: 2px;
}

.req-hint {
  font-size: 12px;
  color: var(--el-color-danger);
}

.hint {
  font-size: 12px;
  color: var(--el-text-color-secondary);
  margin: 2px 0 4px;
}

.code-editor :deep(textarea) {
  font-family: Consolas, 'Courier New', monospace;
  font-size: 13px;
  line-height: 1.6;
}

.empty {
  font-size: 12px;
  color: var(--el-text-color-placeholder);
  padding: 6px 0;
}

.obj-props {
  padding: 10px 12px;
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 6px;
  background: var(--el-fill-color-extra-light);
}

.prop + .prop {
  margin-top: 12px;
}

.array-item {
  padding: 10px 12px;
  margin: 8px 0;
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 6px;
  background: var(--el-fill-color-extra-light);
}

.array-item-tools {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 6px;
}

.idx {
  font-size: 12px;
  color: var(--el-text-color-secondary);
}
</style>

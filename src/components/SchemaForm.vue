<script setup lang="ts">
/**
 * SchemaForm —— Schema 驱动表单入口（根对象渲染）。
 * 表单结构完全来自模板的 schema.json，新增模板无需改动本组件。
 */
import SchemaNode from './SchemaNode.vue'

defineProps<{
  schema: Record<string, unknown>
  modelValue: Record<string, unknown>
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', value: Record<string, unknown>): void
}>()
</script>

<template>
  <div class="schema-form">
    <SchemaNode
      :schema="schema"
      :model-value="modelValue"
      @update:model-value="emit('update:modelValue', $event as Record<string, unknown>)"
    />
  </div>
</template>

<style scoped>
.schema-form {
  padding: 4px 2px;
}
</style>

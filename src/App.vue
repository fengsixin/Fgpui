<script setup lang="ts">
import { computed, onMounted } from 'vue'
import { useRoute } from 'vue-router'
import { useAppStore } from '@/stores/appStore'

const route = useRoute()
const appStore = useAppStore()

const phaseTitle = computed(() => route.meta.phaseTitle as string | undefined)
const envOk = computed(() => appStore.typstStatus?.ok === true)

// 应用启动即做一次 Typst 环境自检（顶栏状态不再依赖环境自检页）
onMounted(() => {
  void appStore.refreshTypstStatus()
})
</script>

<template>
  <el-container class="app-shell">
    <el-header class="app-header" height="56px">
      <div class="app-brand">
        <div class="app-logo">F</div>
        <div class="app-title">
          <span class="app-name">Fgpui</span>
          <span class="app-subname">单机标准文档生成工具</span>
        </div>
      </div>
      <el-menu mode="horizontal" router :default-active="route.path" class="app-nav" :ellipsis="false">
        <el-menu-item index="/">项目管理</el-menu-item>
        <el-menu-item index="/env">环境自检</el-menu-item>
      </el-menu>
      <div class="app-header-right">
        <el-tag v-if="phaseTitle" type="info" effect="plain" size="large">{{ phaseTitle }}</el-tag>
        <el-tooltip
          :content="envOk ? `Typst 就绪：${appStore.typstStatus?.version ?? ''}` : 'Typst 环境未就绪'"
          placement="bottom"
        >
          <el-tag
            :type="appStore.typstChecking ? 'info' : envOk ? 'success' : 'danger'"
            effect="dark"
            size="large"
          >
            {{ appStore.typstChecking ? '检测中…' : envOk ? 'Typst 就绪' : 'Typst 未就绪' }}
          </el-tag>
        </el-tooltip>
      </div>
    </el-header>
    <el-main class="app-main">
      <router-view />
    </el-main>
  </el-container>
</template>

<style scoped>
.app-shell {
  height: 100vh;
}

.app-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  border-bottom: 1px solid var(--el-border-color-light);
  background: var(--el-bg-color);
}

.app-brand {
  display: flex;
  align-items: center;
  gap: 12px;
}

.app-logo {
  width: 36px;
  height: 36px;
  border-radius: 9px;
  background: var(--el-color-primary);
  color: #fff;
  font-size: 20px;
  font-weight: 700;
  display: flex;
  align-items: center;
  justify-content: center;
}

.app-title {
  display: flex;
  flex-direction: column;
  line-height: 1.25;
}

.app-name {
  font-weight: 700;
  font-size: 16px;
}

.app-subname {
  font-size: 12px;
  color: var(--el-text-color-secondary);
}

.app-header-right {
  display: flex;
  align-items: center;
  gap: 10px;
}

.app-nav {
  flex: 1;
  margin-left: 16px;
  border-bottom: none !important;
  background: transparent;
}

.app-nav .el-menu-item {
  height: 55px;
  line-height: 55px;
}

.app-main {
  background: var(--el-fill-color-lighter);
  overflow: auto;
}
</style>

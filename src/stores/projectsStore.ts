import { defineStore } from 'pinia'
import { ref } from 'vue'
import * as api from '@/api/client'
import type { Project, ProjectDetail, ProjectSummary, WorkspaceInfo } from '@/api/types'
import { formatAppError } from '@/stores/appStore'

export const useProjectsStore = defineStore('projects', () => {
  // 列表
  const list = ref<ProjectSummary[]>([])
  const listLoading = ref(false)
  const listError = ref<string | null>(null)
  const listErrorKind = ref<string | null>(null)
  const workspaceInfo = ref<WorkspaceInfo | null>(null)

  // 当前打开的项目
  const current = ref<ProjectDetail | null>(null)
  const currentLoading = ref(false)
  const currentError = ref<string | null>(null)
  const currentErrorKind = ref<string | null>(null)

  // 自动保存状态
  const saving = ref(false)
  const lastSavedAt = ref<string | null>(null)
  const saveError = ref<string | null>(null)

  async function refreshList(): Promise<void> {
    listLoading.value = true
    listError.value = null
    listErrorKind.value = null
    try {
      list.value = await api.listProjects()
    } catch (err) {
      list.value = []
      const e = err as { kind?: string }
      listErrorKind.value = typeof e?.kind === 'string' ? e.kind : 'internal'
      listError.value = formatAppError(err)
    } finally {
      listLoading.value = false
    }
  }

  async function loadWorkspaceInfo(): Promise<void> {
    try {
      workspaceInfo.value = await api.getWorkspaceInfo()
    } catch {
      workspaceInfo.value = null
    }
  }

  async function createProject(name: string, documentType: string): Promise<Project> {
    const created = await api.createProject(name, documentType)
    await refreshList()
    return created
  }

  async function openProject(id: string): Promise<boolean> {
    currentLoading.value = true
    currentError.value = null
    currentErrorKind.value = null
    try {
      current.value = await api.openProject(id)
      lastSavedAt.value = null
      saveError.value = null
      return true
    } catch (err) {
      current.value = null
      const e = err as { kind?: string }
      currentErrorKind.value = typeof e?.kind === 'string' ? e.kind : 'internal'
      currentError.value = formatAppError(err)
      return false
    } finally {
      currentLoading.value = false
    }
  }

  async function saveData(data: Record<string, unknown>): Promise<void> {
    if (!current.value) return
    saving.value = true
    try {
      const updated = await api.saveProject(current.value.project.id, data)
      // 同步更新 project 与 data，避免 canonical 比对漂移导致重复保存
      current.value = { project: updated, data }
      lastSavedAt.value = new Date().toLocaleTimeString('zh-CN', { hour12: false })
      saveError.value = null
    } catch (err) {
      saveError.value = formatAppError(err)
    } finally {
      saving.value = false
    }
  }

  async function removeProject(id: string): Promise<void> {
    await api.deleteProject(id)
    if (current.value?.project.id === id) {
      current.value = null
    }
    await refreshList()
  }

  async function rebuildIndex(): Promise<number> {
    const count = await api.rebuildProjectIndex()
    await refreshList()
    return count
  }

  return {
    list,
    listLoading,
    listError,
    listErrorKind,
    workspaceInfo,
    current,
    currentLoading,
    currentError,
    currentErrorKind,
    saving,
    lastSavedAt,
    saveError,
    refreshList,
    loadWorkspaceInfo,
    createProject,
    openProject,
    saveData,
    removeProject,
    rebuildIndex,
  }
})

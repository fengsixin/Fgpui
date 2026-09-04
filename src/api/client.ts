import { invoke } from '@tauri-apps/api/core'
import type {
  Project,
  ProjectDetail,
  ProjectSummary,
  SmokeTestResult,
  TypstStatus,
  WorkspaceInfo,
} from './types'

// ---------- 阶段 0：环境自检 ----------

/** 检查内置 Typst 是否存在并返回版本信息 */
export function checkTypst(): Promise<TypstStatus> {
  return invoke('check_typst')
}

/** 运行 Typst 冒烟测试：编译内置 hello 模板并生成 PDF */
export function runTypstSmokeTest(): Promise<SmokeTestResult> {
  return invoke('run_typst_smoke_test')
}

/** 在资源管理器中定位文件 */
export function revealInExplorer(path: string): Promise<void> {
  return invoke('reveal_in_explorer', { path })
}

// ---------- 阶段 1：项目管理 ----------

/** 创建项目（名称 + 文档类型） */
export function createProject(name: string, documentType: string): Promise<Project> {
  return invoke('create_project', { name, documentType })
}

/** 最近项目列表（按更新时间倒序，附完整性标记） */
export function listProjects(): Promise<ProjectSummary[]> {
  return invoke('list_projects')
}

/** 打开项目（元数据 + 数据） */
export function openProject(id: string): Promise<ProjectDetail> {
  return invoke('open_project', { id })
}

/** 保存项目数据（原子写 + 刷新 updated_at） */
export function saveProject(id: string, data: Record<string, unknown>): Promise<Project> {
  return invoke('save_project', { id, data })
}

/** 删除项目（只删 projects/{id}，不触碰模板与字体） */
export function deleteProject(id: string): Promise<void> {
  return invoke('delete_project', { id })
}

/** 从 project.json 重建索引（数据库损坏时恢复） */
export function rebuildProjectIndex(): Promise<number> {
  return invoke('rebuild_project_index')
}

/** 工作区目录信息 */
export function getWorkspaceInfo(): Promise<WorkspaceInfo> {
  return invoke('get_workspace_info')
}

/**
 * 与 Rust 侧 AppError 序列化结构对齐的错误类型。
 * Rust 侧使用 serde tag = "kind" 的内部标记枚举。
 */
export interface AppError {
  kind: string
  message: string
  detail?: string
  diagnostics?: Diagnostic[]
  stderr?: string
  timeoutSecs?: number
}

export interface Diagnostic {
  severity: string
  file: string | null
  line: number | null
  column: number | null
  message: string
}

export interface TypstStatus {
  ok: boolean
  path: string | null
  version: string | null
  kind: string | null
  detail: string
}

export interface SmokeTestResult {
  outputPath: string
  durationMs: number
  warnings: Diagnostic[]
  typstVersion: string
}

/** 项目元数据（与 Rust ProjectMeta / project.json 对齐） */
export interface Project {
  id: string
  name: string
  documentType: string
  templateId: string
  templateVersion: string
  dataPath: string
  createdAt: string
  updatedAt: string
}

/** 项目摘要（列表视图，附完整性标记，不落盘） */
export interface ProjectSummary extends Project {
  integrity: 'ok' | 'missing-files'
}

/** 项目详情（元数据 + 表单数据） */
export interface ProjectDetail {
  project: Project
  data: Record<string, unknown>
}

/** 工作区目录信息 */
export interface WorkspaceInfo {
  root: string
  projects: string
  templates: string
  fonts: string
  backups: string
  logs: string
}

export const DOC_TYPE_TEXT: Record<string, string> = {
  'technical-design': '技术方案',
  'test-report': '测试报告',
}

export function docTypeText(type: string | null | undefined): string {
  if (!type) return '—'
  return DOC_TYPE_TEXT[type] ?? type
}

export const ERROR_KIND_TEXT: Record<string, string> = {
  typst_not_found: '未找到内置 Typst',
  typst_version_check_failed: 'Typst 版本检查失败',
  compile_failed: 'Typst 编译失败',
  compile_timeout: '编译超时',
  db_corrupted: '项目索引数据库损坏',
  project_not_found: '项目不存在',
  project_files_missing: '项目文件缺失',
  validation: '输入无效',
  io: '文件读写错误',
  internal: '内部错误',
}

export function errorKindText(kind: string | null | undefined): string {
  if (!kind) return '未知错误'
  return ERROR_KIND_TEXT[kind] ?? kind
}

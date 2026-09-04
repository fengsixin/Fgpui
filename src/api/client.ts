import { invoke } from '@tauri-apps/api/core'
import type {
  CompileState,
  ImportPreview,
  Project,
  ProjectDetail,
  ProjectSummary,
  SchemaIssue,
  SessionSnapshot,
  SmokeTestResult,
  TemplateInfo,
  TemplateManifest,
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

/** 创建项目（名称 + 模板；模板版本自动记录自 manifest） */
export function createProject(name: string, templateId: string): Promise<Project> {
  return invoke('create_project', { name, templateId })
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

// ---------- 阶段 2：模板与 Schema ----------

/** 扫描模板目录（含损坏包与错误说明） */
export function listTemplates(): Promise<TemplateInfo[]> {
  return invoke('list_templates')
}

/** 读取模板 JSON Schema */
export function getTemplateSchema(templateId: string): Promise<Record<string, unknown>> {
  return invoke('get_template_schema', { templateId })
}

/** 读取模板示例数据 */
export function getTemplateSample(templateId: string): Promise<Record<string, unknown>> {
  return invoke('get_template_sample', { templateId })
}

/** 导入模板包（本地目录路径） */
export function importTemplate(sourcePath: string): Promise<TemplateManifest> {
  return invoke('import_template', { sourcePath })
}

/** 用模板 Schema 校验文档数据 */
export function validateDocumentData(
  templateId: string,
  data: Record<string, unknown>,
): Promise<SchemaIssue[]> {
  return invoke('validate_document_data', { templateId, data })
}

// ---------- 阶段 3：编译与预览 ----------

/** 启动编译（异步立即返回；结果用 getCompileStatus 轮询） */
export function compileDocument(projectId: string): Promise<CompileState> {
  return invoke('compile_document', { projectId })
}

/** 取消进行中的编译 */
export function cancelCompile(projectId: string): Promise<boolean> {
  return invoke('cancel_compile', { projectId })
}

/** 查询编译状态 */
export function getCompileStatus(projectId: string): Promise<SessionSnapshot> {
  return invoke('get_compile_status', { projectId })
}

/** 项目最近一次成功输出 */
export function latestOutput(projectId: string): Promise<string | null> {
  return invoke('latest_output', { projectId })
}

/** 用系统默认程序打开 PDF */
export function openOutputFile(path: string): Promise<void> {
  return invoke('open_output_file', { path })
}

/** 读取 PDF 字节（PDF.js 内嵌预览） */
export function readPdfBytes(path: string): Promise<number[]> {
  return invoke('read_pdf_bytes', { path })
}

// ---------- 阶段 4：导入 ----------

/** Excel 导入预览（固定列名映射；确认后用 save_project 写入） */
export function importExcelData(projectId: string, filePath: string): Promise<ImportPreview> {
  return invoke('import_excel_data', { projectId, filePath })
}

/** JSON 导入预览（整体替换；确认后用 save_project 写入） */
export function importJsonData(projectId: string, filePath: string): Promise<ImportPreview> {
  return invoke('import_json_data', { projectId, filePath })
}

/** 导入图片到项目 assets，返回正文引用的相对路径 */
export function importProjectAsset(projectId: string, filePath: string): Promise<string> {
  return invoke('import_project_asset', { projectId, filePath })
}

/** 快速校验文件存在 */
export function assertFileExists(path: string): Promise<void> {
  return invoke('assert_file_exists', { path })
}

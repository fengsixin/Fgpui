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

export const ERROR_KIND_TEXT: Record<string, string> = {
  typst_not_found: '未找到内置 Typst',
  typst_version_check_failed: 'Typst 版本检查失败',
  compile_failed: 'Typst 编译失败',
  compile_timeout: '编译超时',
  io: '文件读写错误',
  internal: '内部错误',
}

export function errorKindText(kind: string | null | undefined): string {
  if (!kind) return '未知错误'
  return ERROR_KIND_TEXT[kind] ?? kind
}

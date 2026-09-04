import { invoke } from '@tauri-apps/api/core'
import type { SmokeTestResult, TypstStatus } from './types'

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

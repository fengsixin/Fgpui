import { defineStore } from 'pinia'
import { ref } from 'vue'
import * as api from '@/api/client'
import type { AppError, SmokeTestResult, TypstStatus } from '@/api/types'
import { errorKindText } from '@/api/types'

/** 将 invoke 抛出的序列化错误转成可读文本 */
export function formatAppError(err: unknown): string {
  const e = err as AppError
  if (e && typeof e === 'object' && typeof e.kind === 'string') {
    const parts = [errorKindText(e.kind)]
    if (e.message) parts.push(e.message)
    return parts.join('：')
  }
  if (err instanceof Error) return err.message
  return String(err)
}

export const useAppStore = defineStore('app', () => {
  const typstStatus = ref<TypstStatus | null>(null)
  const typstChecking = ref(false)

  const smokeRunning = ref(false)
  const smokeResult = ref<SmokeTestResult | null>(null)
  const smokeError = ref<string | null>(null)
  const smokeErrorRaw = ref<AppError | null>(null)

  async function refreshTypstStatus(): Promise<void> {
    typstChecking.value = true
    try {
      typstStatus.value = await api.checkTypst()
    } catch (err) {
      typstStatus.value = {
        ok: false,
        path: null,
        version: null,
        kind: 'internal',
        detail: formatAppError(err),
      }
    } finally {
      typstChecking.value = false
    }
  }

  async function runSmokeTest(): Promise<void> {
    smokeRunning.value = true
    smokeError.value = null
    smokeErrorRaw.value = null
    try {
      smokeResult.value = await api.runTypstSmokeTest()
    } catch (err) {
      smokeResult.value = null
      smokeError.value = formatAppError(err)
      smokeErrorRaw.value = err as AppError
    } finally {
      smokeRunning.value = false
    }
  }

  async function reveal(path: string): Promise<void> {
    try {
      await api.revealInExplorer(path)
    } catch (err) {
      smokeError.value = formatAppError(err)
    }
  }

  return {
    typstStatus,
    typstChecking,
    smokeRunning,
    smokeResult,
    smokeError,
    smokeErrorRaw,
    refreshTypstStatus,
    runSmokeTest,
    reveal,
  }
})

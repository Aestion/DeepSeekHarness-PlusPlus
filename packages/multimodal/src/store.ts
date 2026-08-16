/** 追加式 Observation 落库（JSONL，按会话分文件）。 */

import { appendFile, mkdir, readFile } from 'node:fs/promises'
import { join } from 'node:path'
import type { Observation } from './types.ts'

/** 一条已持久化的观察记录。 */
export interface StoredObservation {
  readonly sessionId?: string
  readonly messageId?: string
  readonly observedAt: string
  readonly observation: Observation
}

/** Observation 落库：`<root>/<sessionId>.jsonl`，每行一条 JSON 记录。 */
export class ObservationStore {
  constructor(private readonly root: string) {}

  private fileFor(sessionId?: string): string {
    const safe = (sessionId ?? '_no-session').replace(/[^\w.-]/g, '_')
    return join(this.root, `${safe}.jsonl`)
  }

  /** 追加一条记录（失败向上抛，由调用方决定是否降级）。 */
  async append(entry: {
    readonly sessionId?: string
    readonly messageId?: string
    readonly observation: Observation
  }): Promise<void> {
    const line = `${JSON.stringify({ ...entry, observedAt: new Date().toISOString() })}\n`
    await mkdir(this.root, { recursive: true })
    await appendFile(this.fileFor(entry.sessionId), line, 'utf8')
  }

  /** 按会话（或全部未归属会话）读取记录，缺失时返回空数组。 */
  async list(sessionId?: string): Promise<StoredObservation[]> {
    try {
      const text = await readFile(this.fileFor(sessionId), 'utf8')
      return text
        .split('\n')
        .filter(Boolean)
        .map(line => JSON.parse(line) as StoredObservation)
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code === 'ENOENT') return []
      throw error
    }
  }
}

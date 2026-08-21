/**
 * Shared-tab Chrome bridge ("chromeUse" capability).
 *
 * Topology (zero-dependency, HTTP long-poll):
 *   gateway (MCP server, 18766)
 *     ├── /ext/poll     native host long-polls for queued requests (25s hold)
 *     └── /ext/response native host posts extension replies
 *               └── native-host.mjs (spawned by Chrome, stdin/stdout frames)
 *                     └── Native Messaging ── extension background
 *                           └── chrome.tabs.sendMessage ── content script
 *
 * When no native host is connected, shared-tab tools return a clear
 * diagnostic error instead of failing silently.
 */

import { randomUUID } from 'node:crypto'
import type { IncomingMessage, ServerResponse } from 'node:http'

export interface SharedTabRequest {
  id: string
  action: string
  payload?: Record<string, unknown>
}

export interface SharedTabResponse {
  id: string
  ok: boolean
  result?: unknown
  error?: string
}

const POLL_HOLD_MS = 25_000
const REQUEST_TIMEOUT_MS = 45_000

/** Overridable bridge timing (tests shorten the waits). */
export interface SharedTabBridgeOptions {
  readonly requestTimeoutMs?: number
  readonly pollHoldMs?: number
}

export class SharedTabBridge {
  /** Requests waiting for a native host poll to pick them up. */
  private readonly queue: SharedTabRequest[] = []
  /** Long-poll responses currently held open. */
  private readonly pollers: Array<{ resolve: (value: { request?: SharedTabRequest }) => void; timer: NodeJS.Timeout }> = []
  /** Requests awaiting the extension reply. */
  private readonly pending = new Map<string, { resolve: (value: SharedTabResponse) => void; timer: NodeJS.Timeout }>()
  private readonly requestTimeoutMs: number
  private readonly pollHoldMs: number
  /** Last native host contact time, used for health reporting. */
  lastContactAt: number | null = null

  constructor(options: SharedTabBridgeOptions = {}) {
    this.requestTimeoutMs = options.requestTimeoutMs ?? REQUEST_TIMEOUT_MS
    this.pollHoldMs = options.pollHoldMs ?? POLL_HOLD_MS
  }

  get connected(): boolean {
    return this.lastContactAt !== null && Date.now() - this.lastContactAt < 60_000
  }

  /** Number of requests still awaiting a native host poll (diagnostics). */
  get queued(): number {
    return this.queue.length
  }

  /** Handle GET /ext/poll: answer immediately if a request is queued, else hold. */
  handlePoll(response: ServerResponse): void {
    // A poll arriving is itself proof the native host is alive.
    this.lastContactAt = Date.now()
    const request = this.queue.shift()
    if (request !== undefined) {
      response.writeHead(200, { 'Content-Type': 'application/json' })
      response.end(JSON.stringify({ request }))
      return
    }
    const timer = setTimeout(() => {
      const index = this.pollers.findIndex((poller) => poller.timer === timer)
      if (index !== -1) this.pollers.splice(index, 1)
      response.writeHead(200, { 'Content-Type': 'application/json' })
      response.end(JSON.stringify({ request: null }))
    }, this.pollHoldMs)
    this.pollers.push({
      resolve: (value) => {
        clearTimeout(timer)
        response.writeHead(200, { 'Content-Type': 'application/json' })
        response.end(JSON.stringify(value))
      },
      timer,
    })
  }

  /** Handle POST /ext/response from the native host. */
  handleResponse(request: IncomingMessage, response: ServerResponse): void {
    let body = ''
    let overLimit = false
    const MAX_BODY = 1_000_000
    request.setEncoding('utf8')
    // 只在未超限时累加，避免恶意超长 body 撑爆内存；一旦超限就记录标志。
    request.on('data', (chunk) => {
      if (overLimit) return
      body += chunk
      if (body.length > MAX_BODY) overLimit = true
    })
    request.on('end', () => {
      this.lastContactAt = Date.now()
      // 超限时回 413 而不是 destroy() 后不写响应——否则 native host 的
      // fetch('/ext/response') 永远等待，轮询循环会卡死。
      if (overLimit) {
        response.writeHead(413, { 'Content-Type': 'application/json' })
        response.end(JSON.stringify({ error: 'payload too large' }))
        return
      }
      try {
        const message = JSON.parse(body) as SharedTabResponse
        const entry = this.pending.get(message.id)
        if (entry !== undefined) {
          clearTimeout(entry.timer)
          this.pending.delete(message.id)
          entry.resolve(message)
        }
        response.writeHead(200, { 'Content-Type': 'application/json' })
        response.end('{}')
      } catch (error) {
        response.writeHead(400, { 'Content-Type': 'application/json' })
        response.end(JSON.stringify({ error: String(error) }))
      }
    })
  }

  /** Send one request through the native host and await the extension reply. */
  request(action: string, payload: Record<string, unknown> = {}): Promise<SharedTabResponse> {
    const id = randomUUID()
    const request: SharedTabRequest = { id, action, payload }
    return new Promise((resolve) => {
      const timer = setTimeout(() => {
        this.pending.delete(id)
        // 超时后从待轮询队列中移除：native host 长期离线时，超时请求不再残留堆积。
        const index = this.queue.findIndex((queued) => queued.id === id)
        if (index !== -1) this.queue.splice(index, 1)
        resolve({ id, ok: false, error: `Chrome 共享标签请求超时（${action}）` })
      }, this.requestTimeoutMs)
      this.pending.set(id, { resolve, timer })
      this.queue.push(request)
      const poller = this.pollers.shift()
      if (poller !== undefined) {
        const queued = this.queue.shift()
        if (queued !== undefined) poller.resolve({ request: queued })
      }
    })
  }

  async status(): Promise<Record<string, unknown>> {
    return {
      connected: this.connected,
      hint: this.connected
        ? 'Chrome 扩展桥已连接'
        : '未连接：请在控制中心安装并启用 Chrome 扩展（chromeUse），并保持 Chrome 运行',
    }
  }
}

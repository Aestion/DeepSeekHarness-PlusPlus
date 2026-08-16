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

export class SharedTabBridge {
  /** Requests waiting for a native host poll to pick them up. */
  private readonly queue: SharedTabRequest[] = []
  /** Long-poll responses currently held open. */
  private readonly pollers: Array<{ resolve: (value: { request?: SharedTabRequest }) => void; timer: NodeJS.Timeout }> = []
  /** Requests awaiting the extension reply. */
  private readonly pending = new Map<string, { resolve: (value: SharedTabResponse) => void; timer: NodeJS.Timeout }>()
  /** Last native host contact time, used for health reporting. */
  lastContactAt: number | null = null

  get connected(): boolean {
    return this.lastContactAt !== null && Date.now() - this.lastContactAt < 60_000
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
    }, POLL_HOLD_MS)
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
    request.setEncoding('utf8')
    request.on('data', (chunk) => {
      body += chunk
      if (body.length > 1_000_000) request.destroy()
    })
    request.on('end', () => {
      this.lastContactAt = Date.now()
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

  /** Queue a request for the next native host poll. */
  dispatch(action: string, payload: Record<string, unknown> = {}): void {
    this.queue.push({ id: randomUUID(), action, payload })
    const poller = this.pollers.shift()
    if (poller !== undefined) {
      const request = this.queue.shift()
      if (request !== undefined) poller.resolve({ request })
    }
  }

  /** Send one request through the native host and await the extension reply. */
  request(action: string, payload: Record<string, unknown> = {}): Promise<SharedTabResponse> {
    const id = randomUUID()
    const request: SharedTabRequest = { id, action, payload }
    return new Promise((resolve) => {
      const timer = setTimeout(() => {
        this.pending.delete(id)
        resolve({ id, ok: false, error: `Chrome 共享标签请求超时（${action}）` })
      }, REQUEST_TIMEOUT_MS)
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

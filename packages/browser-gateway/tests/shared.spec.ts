import { EventEmitter } from 'node:events'
import type { IncomingMessage, ServerResponse } from 'node:http'
import { describe, expect, it, vi } from 'vitest'
import { SharedTabBridge } from '../src/shared.js'

describe('SharedTabBridge queue pruning', () => {
  it('removes a request from the queue when it times out with no native host connected', async () => {
    const bridge = new SharedTabBridge({ requestTimeoutMs: 40 })
    const result = await bridge.request('stale', { url: 'https://example.com' })
    expect(result.ok).toBe(false)
    expect(result.error).toContain('超时')
    // 超时后该请求不应继续留在待轮询队列，否则 native host 长期离线时会无限堆积。
    expect(bridge.queued).toBe(0)
  })
})

describe('SharedTabBridge /ext/response size limit', () => {
  it('responds 413 to an oversized extension reply instead of hanging the native host', async () => {
    const bridge = new SharedTabBridge()
    const big = JSON.stringify({ id: 'x', ok: true, result: 'a'.repeat(1_100_000) })
    const req = new EventEmitter() as unknown as IncomingMessage
    // IncomingMessage.setEncoding/destroy 的返回类型是 `this`，fake 必须回传自己以免 TS 报错。
    req.setEncoding = () => req
    req.destroy = () => req
    const writeHead = vi.fn()
    const end = vi.fn()
    const res = { writeHead, end } as unknown as ServerResponse

    bridge.handleResponse(req as unknown as IncomingMessage, res)
    req.emit('data', big)
    req.emit('end')

    // 超限时必须回一个 413，而不是 destroy() 之后什么都不写——否则 native host 的
    // fetch('/ext/response') 永远等待，轮询循环会卡死。
    expect(writeHead).toHaveBeenCalledWith(413, expect.anything())
    expect(end).toHaveBeenCalled()
  })
})

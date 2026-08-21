import { readFile } from 'node:fs/promises'
import { createServer, type AddressInfo } from 'node:net'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { afterAll, beforeAll, describe, expect, it } from 'vitest'
import { ManagedChrome } from '../src/chrome.js'
import { BrowserMcpServer } from '../src/mcp.js'
import { SharedTabBridge } from '../src/shared.js'

/** 借一个临时空闲端口（listen 0 -> 释放 -> 复用）。 */
async function freePort(): Promise<number> {
  const server = createServer()
  await new Promise<void>((resolve, reject) => {
    server.once('error', reject)
    server.listen(0, '127.0.0.1', () => resolve())
  })
  const { port } = server.address() as AddressInfo
  await new Promise<void>((resolve) => server.close(() => resolve()))
  return port
}

let server: BrowserMcpServer
let port: number

beforeAll(async () => {
  port = await freePort()
  // ManagedChrome 懒启动：构造不拉起浏览器，测试进程退出前停掉服务即可。
  server = new BrowserMcpServer(tmpdir(), new ManagedChrome(tmpdir()), new SharedTabBridge())
  await server.start('127.0.0.1', port)
})

afterAll(async () => {
  await server.stop()
})

describe('browser gateway /api/health', () => {
  it('reports the shared bridge connection state', async () => {
    const response = await fetch(`http://127.0.0.1:${port}/api/health`)
    expect(response.status).toBe(200)
    const body = (await response.json()) as {
      status: string
      shared?: { connected: boolean }
    }
    expect(body.status).toBe('ok')
    // 没有扩展/native host 在线时必须显式给出 connected: false，
    // 桌面端据此渲染“已安装未连接”。
    expect(body.shared).toEqual({ connected: false })
  })
})

describe('browser gateway /ext/* token auth', () => {
  it('generates a shared token file for the native host', async () => {
    const token = (
      await readFile(join(tmpdir(), 'browser-extension', 'gateway.token'), 'utf8')
    ).trim()
    expect(token).toMatch(/^[0-9a-f]{64}$/)
  })

  it('rejects /ext/poll without a valid token', async () => {
    const response = await fetch(`http://127.0.0.1:${port}/ext/poll`)
    expect(response.status).toBe(401)
  })

  it('rejects /ext/poll with a wrong token', async () => {
    const response = await fetch(`http://127.0.0.1:${port}/ext/poll`, {
      headers: { 'x-dshplusplus-token': 'wrong-token' },
    })
    expect(response.status).toBe(401)
  })

  it('rejects /ext/response without a valid token', async () => {
    const response = await fetch(`http://127.0.0.1:${port}/ext/response`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ id: 'x', ok: true }),
    })
    expect(response.status).toBe(401)
  })
})

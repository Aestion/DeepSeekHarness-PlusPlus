// DSH++ Browser Control - Native Messaging host.
// Spawned by Chrome when the extension opens the native messaging port.
// Bridges the extension (stdin/stdout length-prefixed JSON frames) and the
// DSH++ browser gateway (HTTP long-poll on /ext/poll and /ext/response).
//
// Frame format (Native Messaging): 4-byte little-endian length + UTF-8 JSON.

import { resolve, dirname } from 'node:path'
import { fileURLToPath } from 'node:url'
import { appendFileSync, readFileSync } from 'node:fs'
import { spawnSync } from 'node:child_process'

// The gateway base URL is injected by the launcher (native hosts are spawned
// by Chrome and do not inherit the desktop app's environment). Defaults keep
// manual runs working.
const GATEWAY = process.env.DSHPLUSPLUS_GATEWAY ?? 'http://127.0.0.1:18766'

// 与网关共享的 token：读取与 native-host.mjs 同目录的 gateway.token（由网关在
// 启动时生成，与扩展同目录）。首装时网关可能尚未落盘，故每轮轮询前重读，得到 401
// 时再刷一次重试，避免"网关晚写 token → host 已死"的竞态。
const HOST_DIR = dirname(fileURLToPath(import.meta.url))
let TOKEN = ''
function refreshToken() {
  try {
    const next = readFileSync(resolve(HOST_DIR, 'gateway.token'), 'utf8').trim()
    if (next && next !== TOKEN) {
      TOKEN = next
      trace('token-refreshed')
    }
  } catch {
    // token 文件尚未生成：保留当前值，下一轮再试。
  }
}
refreshToken()

// Debug tracing: DSHPLUSPLUS_HOST_DEBUG_FILE=<path> writes a JSONL trace of
// every frame and poll round-trip, otherwise invisible because Chrome owns
// the host's stdout pipe.
const DEBUG_FILE = process.env.DSHPLUSPLUS_HOST_DEBUG_FILE
const NATIVE_HOST_REG_KEY = process.env.DSHPLUSPLUS_NATIVE_HOST_REG_KEY
  ?? 'HKCU\\Software\\Google\\Chrome\\NativeMessagingHosts\\com.dshplusplus.browser'
const MAX_GATEWAY_FAILURES = 3
function trace(event, detail) {
  if (!DEBUG_FILE) return
  try {
    appendFileSync(DEBUG_FILE, JSON.stringify({ t: Date.now(), event, ...detail }) + '\n')
  } catch {
    // ignore
  }
}

let stdinBuffer = Buffer.alloc(0)
const pendingFromExtension = new Map() // frameId -> resolve

function readFrame() {
  while (stdinBuffer.length >= 4) {
    const length = stdinBuffer.readUInt32LE(0)
    if (stdinBuffer.length < 4 + length) return
    const payload = stdinBuffer.subarray(4, 4 + length).toString('utf8')
    stdinBuffer = stdinBuffer.subarray(4 + length)
    try {
      const message = JSON.parse(payload)
      emit(message)
    } catch (error) {
      console.error('[native-host] bad frame', error)
    }
  }
}

function emit(message) {
  // Extension replies (id matches a request we forwarded from the gateway)
  if (message && typeof message.id === 'string' && !message.action) {
    trace('extension-reply', { id: message.id, ok: message.ok })
    const resolve = pendingFromExtension.get(message.id)
    if (resolve) {
      pendingFromExtension.delete(message.id)
      resolve(message)
    }
    return
  }
  // Extension-initiated messages are not expected; log and ignore.
  console.error('[native-host] unexpected extension message', JSON.stringify(message).slice(0, 200))
}

process.stdin.on('data', (chunk) => {
  stdinBuffer = Buffer.concat([stdinBuffer, chunk])
  readFrame()
})
process.stdin.on('end', () => process.exit(0))

function writeFrame(message) {
  trace('write-frame', { id: message.id, action: message.action })
  const payload = Buffer.from(JSON.stringify(message), 'utf8')
  const header = Buffer.alloc(4)
  header.writeUInt32LE(payload.length, 0)
  process.stdout.write(Buffer.concat([header, payload]))
}

function unregisterUnavailableHost() {
  if (process.platform !== 'win32') return
  // DSH++ registers the host again whenever chromeUse starts. Removing the
  // registration here prevents Chrome's MV3 worker from immediately spawning
  // a new host after the desktop app/browser gateway has exited.
  try {
    spawnSync('reg.exe', ['delete', NATIVE_HOST_REG_KEY, '/f'], {
      windowsHide: true,
      stdio: 'ignore',
    })
  } catch {
    // The host must still exit even if the registry key is already absent.
  }
}

async function pollLoop() {
  let consecutiveGatewayFailures = 0
  for (;;) {
    let timeout
    try {
      // 每轮先刷新 token：首装时网关可能刚落盘，迟到的 host 仍能接上。
      refreshToken()
      const controller = new AbortController()
      timeout = setTimeout(() => controller.abort(), 28_000)
      const headers = TOKEN ? { 'x-dshplusplus-token': TOKEN } : undefined
      const response = await fetch(`${GATEWAY}/ext/poll`, { signal: controller.signal, headers })
      clearTimeout(timeout)
      timeout = undefined
      // 401 = 网关已起但未认我们的 token（多半是 token 刚写入）。刷新后立即重试，
      // 不计入"网关离线"的退出计数。
      if (response.status === 401) {
        trace('poll-unauthorized', { tokenLen: TOKEN.length })
        refreshToken()
        continue
      }
      if (!response.ok) throw new Error(`poll HTTP ${response.status}`)
      consecutiveGatewayFailures = 0
      const body = await response.json()
      const request = body && body.request
      if (!request) {
        trace('poll-empty')
        continue
      }
      trace('poll-request', { id: request.id, action: request.action })
      // Forward the gateway request to the extension and await its reply.
      const reply = await new Promise((resolve) => {
        pendingFromExtension.set(request.id, resolve)
        writeFrame(request)
        setTimeout(() => {
          if (pendingFromExtension.delete(request.id)) {
            trace('extension-timeout', { id: request.id })
            resolve({ id: request.id, ok: false, error: 'extension 无响应（页面未刷新或扩展未启用）' })
          }
        }, 20_000)
      })
      // POST 回包带超时：网关即使 413/异常，也要让轮询循环继续，不能卡死。
      const replyController = new AbortController()
      const replyTimeout = setTimeout(() => replyController.abort(), 10_000)
      let replyResponse
      try {
        replyResponse = await fetch(`${GATEWAY}/ext/response`, {
          method: 'POST',
          signal: replyController.signal,
          headers: { 'Content-Type': 'application/json', ...(TOKEN ? { 'x-dshplusplus-token': TOKEN } : {}) },
          body: JSON.stringify(reply),
        })
      } finally {
        clearTimeout(replyTimeout)
      }
      trace('response-posted', { id: request.id, status: replyResponse.status })
      if (!replyResponse.ok) console.error('[native-host] response post failed', replyResponse.status)
    } catch (error) {
      if (timeout) clearTimeout(timeout)
      consecutiveGatewayFailures += 1
      trace('gateway-failure', { count: consecutiveGatewayFailures, error: String(error) })
      if (consecutiveGatewayFailures >= MAX_GATEWAY_FAILURES) {
        trace('gateway-offline-exit', { gateway: GATEWAY })
        unregisterUnavailableHost()
        process.exit(0)
      }
      // Expected while DSH++ is shutting down. Do not write to stderr: on
      // Windows Chrome owns the native-host stdio through cmd.exe, and stderr
      // output can leave a visible terminal behind.
      await new Promise((resolve) => setTimeout(resolve, 500))
    }
  }
}

// Keep the process alive; Chrome holds stdin open while the port is connected.
pollLoop()

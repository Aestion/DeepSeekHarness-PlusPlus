/**
 * Minimal Chrome DevTools Protocol client built on Node's built-in WebSocket.
 * Zero dependencies: Node >= 22 provides a global WebSocket implementation.
 */

export interface CdpTarget {
  id: string
  type: string
  title: string
  url: string
  webSocketDebuggerUrl: string
}

export interface CdpError {
  code: number
  message: string
}

export class CdpSession {
  private readonly socket: WebSocket
  private nextId = 1
  private readonly pending = new Map<number, { resolve: (value: unknown) => void; reject: (error: Error) => void }>()
  private readonly listeners = new Map<string, Set<(params: unknown) => void>>()
  private closed = false

  private constructor(socket: WebSocket) {
    this.socket = socket
    socket.addEventListener('message', (event) => {
      let payload: unknown
      try {
        payload = JSON.parse(String(event.data))
      } catch {
        return
      }
      if (typeof payload !== 'object' || payload === null) return
      const message = payload as { id?: unknown; method?: unknown; params?: unknown }
      if (typeof message.id === 'number') {
        const entry = this.pending.get(message.id)
        if (entry === undefined) return
        this.pending.delete(message.id)
        const response = payload as { result?: unknown; error?: CdpError }
        if (response.error !== undefined) {
          entry.reject(new Error(`CDP ${response.error.code}: ${response.error.message}`))
        } else {
          entry.resolve(response.result)
        }
        return
      }
      // Notifications (no id) are dispatched to on() subscribers, e.g. Page.loadEventFired.
      if (typeof message.method === 'string') {
        const set = this.listeners.get(message.method)
        if (set !== undefined) {
          for (const listener of set) listener(message.params)
        }
      }
    })
    socket.addEventListener('close', () => {
      this.closed = true
      for (const entry of this.pending.values()) {
        entry.reject(new Error('CDP connection closed'))
      }
      this.pending.clear()
    })
  }

  static connect(webSocketDebuggerUrl: string, timeoutMs = 8000): Promise<CdpSession> {
    return new Promise((resolve, reject) => {
      const socket = new WebSocket(webSocketDebuggerUrl)
      const timer = setTimeout(() => {
        socket.close()
        reject(new Error(`CDP connect timeout: ${webSocketDebuggerUrl}`))
      }, timeoutMs)
      socket.addEventListener('open', () => {
        clearTimeout(timer)
        resolve(new CdpSession(socket))
      })
      socket.addEventListener('error', () => {
        clearTimeout(timer)
        reject(new Error(`CDP connect failed: ${webSocketDebuggerUrl}`))
      })
    })
  }

  get isClosed(): boolean {
    return this.closed
  }

  close(): void {
    this.closed = true
    try {
      this.socket.close()
    } catch {
      // ignore
    }
  }

  /** Subscribe to an event notification (e.g. Page.loadEventFired). Returns an unsubscribe fn. */
  on(method: string, listener: (params: unknown) => void): () => void {
    let set = this.listeners.get(method)
    if (set === undefined) {
      set = new Set()
      this.listeners.set(method, set)
    }
    set.add(listener)
    return () => {
      set.delete(listener)
    }
  }

  /**
   * Send a CDP command. A per-call timeout guards against a page that never
   * answers (hung async IIFE, blocking dialog) — otherwise the MCP request
   * waits forever and leaks the session.
   */
  send(method: string, params: Record<string, unknown> = {}, timeoutMs = 30_000): Promise<unknown> {
    if (this.closed) return Promise.reject(new Error('CDP session closed'))
    const id = this.nextId++
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        this.pending.delete(id)
        reject(new Error(`CDP ${method} timed out after ${timeoutMs}ms`))
      }, timeoutMs)
      this.pending.set(id, {
        resolve: (value) => {
          clearTimeout(timer)
          resolve(value)
        },
        reject: (error) => {
          clearTimeout(timer)
          reject(error)
        },
      })
      this.socket.send(JSON.stringify({ id, method, params }))
    })
  }
}

/** Fetch the browser-level version endpoint. */
export async function fetchJson<T>(url: string, init?: RequestInit): Promise<T> {
  const response = await fetch(url, init)
  if (!response.ok) {
    throw new Error(`HTTP ${response.status} from ${url}`)
  }
  return (await response.json()) as T
}

/** List page targets from the CDP HTTP endpoint. */
export async function listTargets(port: number): Promise<CdpTarget[]> {
  return fetchJson<CdpTarget[]>(`http://127.0.0.1:${port}/json/list`)
}

/** Create a new page target (about:blank) and return its descriptor. */
export async function createTarget(port: number): Promise<CdpTarget> {
  const target = await fetchJson<CdpTarget>(`http://127.0.0.1:${port}/json/new?about%3Ablank`, {
    method: 'PUT',
  })
  return target
}

/** Navigate an attached page session to a URL, awaiting the load event. */
export async function navigate(session: CdpSession, url: string): Promise<void> {
  await session.send('Page.enable')
  const loaded = new Promise<void>((resolve) => {
    const off = session.on('Page.loadEventFired', () => {
      off()
      resolve()
    })
  })
  const result = (await session.send('Page.navigate', { url })) as { errorText?: string }
  if (result.errorText !== undefined) {
    throw new Error(`navigate failed: ${result.errorText}`)
  }
  // 超时兜底：有些页面（如被拦截的导航）永远不触发 loadEventFired。
  const timeout = new Promise<never>((_, reject) => {
    setTimeout(() => reject(new Error('页面加载超时')), 30_000)
  })
  await Promise.race([loaded, timeout])
}

/** Navigate back/forward in the page session history. delta=-1 back, +1 forward. */
export async function historyNavigate(
  session: CdpSession,
  delta: number,
): Promise<{ url: string; title: string }> {
  const history = (await session.send('Page.getNavigationHistory')) as {
    currentIndex: number
    entries: Array<{ id: number; url: string; title: string }>
  }
  const index = history.currentIndex + delta
  if (index < 0 || index >= history.entries.length) {
    throw new Error(delta < 0 ? '已在第一页，无法后退' : '已在最后一页，无法前进')
  }
  const entry = history.entries[index]!
  await session.send('Page.navigateToHistoryEntry', { entryId: entry.id })
  return { url: entry.url, title: entry.title }
}

/** Evaluate an expression in the page; returns JSON-serializable result. */
export async function evaluate(session: CdpSession, expression: string): Promise<unknown> {
  const result = (await session.send('Runtime.evaluate', {
    expression,
    returnByValue: true,
    awaitPromise: true,
  })) as { result?: { value?: unknown; description?: string }; exceptionDetails?: unknown }
  if (result.exceptionDetails !== undefined) {
    throw new Error(`page evaluate failed: ${JSON.stringify(result.exceptionDetails).slice(0, 300)}`)
  }
  return result.result?.value ?? result.result?.description ?? null
}

/** Capture a PNG screenshot of the attached page. Returns base64 data. */
export async function captureScreenshot(session: CdpSession): Promise<string> {
  const result = (await session.send('Page.captureScreenshot', {
    format: 'png',
  })) as { data: string }
  return result.data
}

/** Dispatch a raw keyboard key (e.g. Enter, Escape, Tab, ArrowDown). */
export async function pressKey(session: CdpSession, key: string): Promise<void> {
  const mapping: Record<string, { code: string; windowsVirtualKeyCode?: number }> = {
    Enter: { code: 'Enter', windowsVirtualKeyCode: 13 },
    Escape: { code: 'Escape', windowsVirtualKeyCode: 27 },
    Tab: { code: 'Tab', windowsVirtualKeyCode: 9 },
    Backspace: { code: 'Backspace', windowsVirtualKeyCode: 8 },
    ArrowUp: { code: 'ArrowUp', windowsVirtualKeyCode: 38 },
    ArrowDown: { code: 'ArrowDown', windowsVirtualKeyCode: 40 },
    ArrowLeft: { code: 'ArrowLeft', windowsVirtualKeyCode: 37 },
    ArrowRight: { code: 'ArrowRight', windowsVirtualKeyCode: 39 },
    Home: { code: 'Home', windowsVirtualKeyCode: 36 },
    End: { code: 'End', windowsVirtualKeyCode: 35 },
    PageUp: { code: 'PageUp', windowsVirtualKeyCode: 33 },
    PageDown: { code: 'PageDown', windowsVirtualKeyCode: 34 },
  }
  const keyInfo = mapping[key] ?? { code: key }
  // Enter 需要 text:'\r' 才会触发表单提交/按钮激活；单字符键补 text 触发默认行为。
  const text = key === 'Enter' ? '\r' : (key.length === 1 ? key : undefined)
  const event = { ...keyInfo, ...(text !== undefined ? { text, unmodifiedText: text } : {}) }
  await session.send('Input.dispatchKeyEvent', { type: 'keyDown', ...event })
  await session.send('Input.dispatchKeyEvent', { type: 'keyUp', ...event })
}

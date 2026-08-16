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

  send(method: string, params: Record<string, unknown> = {}): Promise<unknown> {
    if (this.closed) return Promise.reject(new Error('CDP session closed'))
    const id = this.nextId++
    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject })
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

/** Navigate an attached page session to a URL. */
export async function navigate(session: CdpSession, url: string): Promise<void> {
  await session.send('Page.enable')
  await session.send('Page.navigate', { url })
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
  await session.send('Input.dispatchKeyEvent', { type: 'keyDown', ...keyInfo })
  await session.send('Input.dispatchKeyEvent', { type: 'keyUp', ...keyInfo })
}

/**
 * Streamable-HTTP MCP server for DSH++ Browser.
 * Serves the tool surface consumed by DSH's `dsh-mcp-client`:
 *   browser_open / browser_observe / browser_click / browser_type /
 *   browser_press / browser_list_tabs / browser_close / browser_status
 *
 * Responses use SSE framing (`data: <json>\n\n`) to match the MCA sidecar
 * that DSH's MCP client is already verified against.
 */

import { randomBytes } from 'node:crypto'
import { mkdir, readFile, writeFile } from 'node:fs/promises'
import { createServer, type IncomingMessage, type ServerResponse } from 'node:http'
import { join } from 'node:path'
import { ManagedChrome } from './chrome.js'
import { SharedTabBridge } from './shared.js'
import { CdpSession, evaluate, historyNavigate, pressKey } from './cdp.js'
import { PAGE_SNAPSHOT_SCRIPT } from './snapshot.js'

export const BROWSER_PORT = 18766

/** 网关与 Chrome native-host 之间的共享 token 文件（与扩展一起放在数据目录）。 */
const TOKEN_FILE_NAME = 'gateway.token'

/** 读取或生成 `/ext/*` 的共享 token；文件缺失或不可读时生成新值并写入。 */
async function loadOrCreateToken(dataRoot: string): Promise<string> {
  const dir = join(dataRoot, 'browser-extension')
  const file = join(dir, TOKEN_FILE_NAME)
  try {
    const existing = (await readFile(file, 'utf8')).trim()
    if (existing.length > 0) return existing
  } catch {
    // 缺失或不可读 —— 生成新 token。
  }
  const token = randomBytes(32).toString('hex')
  await mkdir(dir, { recursive: true })
  await writeFile(file, token, 'utf8')
  return token
}

interface ToolDefinition {
  name: string
  description: string
  inputSchema: Record<string, unknown>
}

const TOOLS: ToolDefinition[] = [
  {
    name: 'browser_open',
    description:
      '在浏览器中打开一个网址。mode=managed（默认）使用 DSH++ 受管 Chrome（独立实例）；mode=shared 在你已打开的 Chrome 中新建标签页（需安装 chromeUse 扩展）。',
    inputSchema: {
      type: 'object',
      properties: {
        url: { type: 'string', description: '要打开的网址（http/https）' },
        mode: { type: 'string', enum: ['managed', 'shared'], description: 'managed=受管实例，shared=你的 Chrome' },
      },
      required: ['url'],
    },
  },
  {
    name: 'browser_observe',
    description:
      '观察当前浏览器页面：返回标题、URL、可见文本（截断）、可交互元素列表（带 index 供点击/输入）以及截图文件路径。mode 同 browser_open。',
    inputSchema: {
      type: 'object',
      properties: {
        mode: { type: 'string', enum: ['managed', 'shared'] },
        tabId: { type: 'number', description: 'shared 模式指定标签页，缺省为当前活动标签' },
      },
    },
  },
  {
    name: 'browser_click',
    description: '点击页面元素。可用 browser_observe 返回的 index，或用 CSS selector。',
    inputSchema: {
      type: 'object',
      properties: {
        ref: { type: 'number', description: 'browser_observe 元素列表中的 index' },
        selector: { type: 'string', description: 'CSS 选择器（与 ref 二选一）' },
        mode: { type: 'string', enum: ['managed', 'shared'] },
        tabId: { type: 'number' },
      },
    },
  },
  {
    name: 'browser_type',
    description: '向输入元素输入文本。目标用 ref 或 selector 指定。',
    inputSchema: {
      type: 'object',
      properties: {
        text: { type: 'string' },
        ref: { type: 'number' },
        selector: { type: 'string' },
        mode: { type: 'string', enum: ['managed', 'shared'] },
        tabId: { type: 'number' },
      },
      required: ['text'],
    },
  },
  {
    name: 'browser_press',
    description: '按下按键：Enter、Escape、Tab、Backspace、ArrowUp/Down/Left/Right、Home、End、PageUp、PageDown。',
    inputSchema: {
      type: 'object',
      properties: {
        key: { type: 'string' },
        mode: { type: 'string', enum: ['managed', 'shared'] },
        tabId: { type: 'number' },
      },
      required: ['key'],
    },
  },
  {
    name: 'browser_list_tabs',
    description: '列出你已打开 Chrome 的标签页（shared 模式）。',
    inputSchema: { type: 'object', properties: {} },
  },
  {
    name: 'browser_close',
    description: '关闭当前页面/标签。managed 模式关闭受管页面；shared 模式关闭指定标签页。',
    inputSchema: {
      type: 'object',
      properties: {
        mode: { type: 'string', enum: ['managed', 'shared'] },
        tabId: { type: 'number' },
      },
    },
  },
  {
    name: 'browser_status',
    description: '查询浏览器能力状态：受管 Chrome 是否就绪、Chrome 扩展桥是否连接。',
    inputSchema: { type: 'object', properties: {} },
  },
  {
    name: 'browser_back',
    description: '后退到历史记录中的上一页（受管 Chrome）。',
    inputSchema: { type: 'object', properties: {} },
  },
  {
    name: 'browser_forward',
    description: '前进到历史记录中的下一页（受管 Chrome）。',
    inputSchema: { type: 'object', properties: {} },
  },
  {
    name: 'browser_evaluate',
    description:
      '在页面中执行任意 JavaScript 并返回 JSON 结果（Tabbit/Playwright 式进阶能力）。' +
      'managed 模式在受管 Chrome 的当前页面执行，跨调用保持页面状态与登录态；' +
      'shared 模式在你已打开的 Chrome 标签页执行（需安装 chromeUse 扩展）。' +
      '表达式可为 async IIFE（自动 await），返回 JSON 可序列化值；页面对象（window/document）只在页面作用域内。',
    inputSchema: {
      type: 'object',
      properties: {
        expression: { type: 'string', description: '要执行的 JavaScript 表达式/IIFE（async 可用，自动 await）' },
        mode: { type: 'string', enum: ['managed', 'shared'] },
        tabId: { type: 'number', description: 'shared 模式指定标签页，缺省为当前活动标签' },
      },
      required: ['expression'],
    },
  },
]

const CLICK_SCRIPT = (target: { ref?: number | undefined; selector?: string | undefined }): string => `(() => {
  const all = [...document.querySelectorAll('a,button,input,textarea,select,[role="button"],[role="link"],[onclick]')];
  const el = ${target.ref !== undefined ? 'all[' + target.ref + ']' : `document.querySelector(${JSON.stringify(target.selector)})`};
  if (!el) return { ok: false, error: 'element not found' };
  el.scrollIntoView({ block: 'center' });
  el.click();
  return { ok: true };
})()`

const TYPE_SCRIPT = (text: string, target: { ref?: number | undefined; selector?: string | undefined }): string => `(() => {
  // Same element set as browser_observe so ref indexes are consistent.
  const all = [...document.querySelectorAll('a,button,input,textarea,select,[role="button"],[role="link"],[onclick]')];
  const el = ${target.ref !== undefined ? 'all[' + target.ref + ']' : `document.querySelector(${JSON.stringify(target.selector)})`};
  if (!el) return { ok: false, error: 'element not found' };
  if (!(el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement || el.isContentEditable)) {
    return { ok: false, error: 'target is not an input element (tag=' + el.tagName + ')' };
  }
  el.focus();
  const setter = el instanceof HTMLTextAreaElement
    ? Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value')?.set
    : Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')?.set;
  if (setter && !el.isContentEditable) setter.call(el, ${JSON.stringify(text)});
  else el.textContent = ${JSON.stringify(text)};
  el.dispatchEvent(new Event('input', { bubbles: true }));
  el.dispatchEvent(new Event('change', { bubbles: true }));
  return { ok: true };
})()`

export class BrowserMcpServer {
  private readonly chrome: ManagedChrome
  private readonly shared: SharedTabBridge
  private readonly dataRoot: string
  private token = ''
  private server: ReturnType<typeof createServer> | null = null

  constructor(dataRoot: string, chrome: ManagedChrome, shared: SharedTabBridge) {
    this.dataRoot = dataRoot
    this.chrome = chrome
    this.shared = shared
  }

  async start(host: string, port: number): Promise<void> {
    this.token = await loadOrCreateToken(this.dataRoot)
    const server = createServer((request, response) => this.handle(request, response))
    this.server = server
    await new Promise<void>((resolve, reject) => {
      server.once('error', reject)
      server.listen(port, host, () => resolve())
    })
  }

  async stop(): Promise<void> {
    if (this.server !== null) {
      this.server.close()
      this.server = null
    }
  }

  private async handle(request: IncomingMessage, response: ServerResponse): Promise<void> {
    const url = new URL(request.url ?? '/', 'http://127.0.0.1')
    const path = url.pathname
    try {
      if (request.method === 'GET' && path === '/api/health') {
        response.writeHead(200, { 'Content-Type': 'application/json' })
        response.end(
          JSON.stringify({
            status: 'ok',
            contract: 'dshplusplus.browser-gateway.v1',
            shared: { connected: this.shared.connected },
          }),
        )
        return
      }
      if (request.method === 'GET' && path === '/ext/poll') {
        if (!this.authorizeExt(request)) {
          response.writeHead(401, { 'Content-Type': 'application/json' })
          response.end(JSON.stringify({ error: 'unauthorized' }))
          return
        }
        this.shared.handlePoll(response)
        return
      }
      if (request.method === 'POST' && path === '/ext/response') {
        if (!this.authorizeExt(request)) {
          response.writeHead(401, { 'Content-Type': 'application/json' })
          response.end(JSON.stringify({ error: 'unauthorized' }))
          return
        }
        this.shared.handleResponse(request, response)
        return
      }
      if (request.method === 'POST' && path === '/mcp') {
        await this.handleMcp(request, response)
        return
      }
      response.writeHead(404, { 'Content-Type': 'application/json' })
      response.end(JSON.stringify({ error: 'not found' }))
    } catch (error) {
      response.writeHead(500, { 'Content-Type': 'application/json' })
      response.end(JSON.stringify({ error: String(error) }))
    }
  }

  private async handleMcp(request: IncomingMessage, response: ServerResponse): Promise<void> {
    let body = ''
    request.setEncoding('utf8')
    for await (const chunk of request) {
      body += chunk
      if (body.length > 2_000_000) {
        response.writeHead(413).end()
        return
      }
    }
    let message: { id?: unknown; method?: string; params?: Record<string, unknown> }
    try {
      message = JSON.parse(body) as typeof message
    } catch {
      response.writeHead(400, { 'Content-Type': 'application/json' })
      response.end(JSON.stringify({ jsonrpc: '2.0', error: { code: -32700, message: 'invalid json' } }))
      return
    }
    const send = (payload: Record<string, unknown>): void => {
      const accept = String(request.headers.accept ?? '')
      if (accept.includes('text/event-stream')) {
        response.writeHead(200, {
          'Content-Type': 'text/event-stream',
          'Cache-Control': 'no-cache',
          Connection: 'keep-alive',
        })
        response.write(`data: ${JSON.stringify(payload)}\n\n`)
        response.end()
      } else {
        response.writeHead(200, { 'Content-Type': 'application/json' })
        response.end(JSON.stringify(payload))
      }
    }

    if (message.method === undefined) {
      send({ jsonrpc: '2.0', error: { code: -32600, message: 'method required' } })
      return
    }
    if (message.method === 'initialize') {
      send({
        jsonrpc: '2.0',
        id: message.id,
        result: {
          protocolVersion: '2025-03-26',
          capabilities: { tools: {} },
          serverInfo: { name: 'DSH++ Browser Gateway', version: '0.1.0-dev.2' },
        },
      })
      return
    }
    if (message.method === 'notifications/initialized' || message.method === 'notifications/cancelled') {
      send({ jsonrpc: '2.0' })
      return
    }
    if (message.method === 'ping') {
      send({ jsonrpc: '2.0', id: message.id, result: {} })
      return
    }
    if (message.method === 'tools/list') {
      send({ jsonrpc: '2.0', id: message.id, result: { tools: TOOLS } })
      return
    }
    if (message.method === 'tools/call') {
      const params = message.params ?? {}
      const name = String(params.name ?? '')
      const args = (params.arguments ?? {}) as Record<string, unknown>
      const tool = TOOLS.find((entry) => entry.name === name)
      if (tool === undefined) {
        send({ jsonrpc: '2.0', id: message.id, error: { code: -32602, message: `unknown tool: ${name}` } })
        return
      }
      try {
        const text = await this.dispatchTool(name, args)
        send({ jsonrpc: '2.0', id: message.id, result: { content: [{ type: 'text', text }] } })
      } catch (error) {
        send({
          jsonrpc: '2.0',
          id: message.id,
          result: {
            content: [{ type: 'text', text: `browser tool ${name} 失败：${String(error)}` }],
            isError: true,
          },
        })
      }
      return
    }
    send({ jsonrpc: '2.0', id: message.id, error: { code: -32601, message: `unknown method: ${message.method}` } })
  }

  private modeOf(args: Record<string, unknown>): 'managed' | 'shared' {
    return args.mode === 'shared' ? 'shared' : 'managed'
  }

  /** `/ext/*` 鉴权：native host 必须携带网关生成的共享 token。 */
  private authorizeExt(request: IncomingMessage): boolean {
    const header = request.headers['x-dshplusplus-token']
    return this.token.length > 0 && typeof header === 'string' && header === this.token
  }

  private async dispatchTool(name: string, args: Record<string, unknown>): Promise<string> {
    const mode = this.modeOf(args)
    switch (name) {
      case 'browser_back': {
        if (mode === 'shared') throw new Error('browser_back 仅支持受管模式（managed）')
        await this.ensureChrome()
        return await this.runManaged(async (session) => {
          const info = await historyNavigate(session, -1)
          return { ok: true, url: info.url, title: info.title }
        })
      }
      case 'browser_forward': {
        if (mode === 'shared') throw new Error('browser_forward 仅支持受管模式（managed）')
        await this.ensureChrome()
        return await this.runManaged(async (session) => {
          const info = await historyNavigate(session, 1)
          return { ok: true, url: info.url, title: info.title }
        })
      }
      case 'browser_status': {
        const [managed, shared] = await Promise.all([this.chrome.status(), this.shared.status()])
        return JSON.stringify({ managed, shared }, null, 2)
      }
      case 'browser_open': {
        const url = String(args.url ?? '')
        if (!/^https?:\/\//.test(url)) throw new Error('url 必须是 http/https 地址')
        if (mode === 'shared') {
          const result = await this.shared.request('open', { url })
          if (!result.ok) throw new Error(result.error ?? 'open failed')
          return JSON.stringify(result.result ?? { ok: true })
        }
        await this.ensureChrome()
        const session = await this.chrome.openPage(url)
        session.close()
        return JSON.stringify({ ok: true, mode: 'managed', url })
      }
      case 'browser_observe': {
        if (mode === 'shared') {
          const result = await this.shared.request('observe', {
            tabId: args.tabId,
            snapshotScript: PAGE_SNAPSHOT_SCRIPT,
          })
          if (!result.ok) throw new Error(result.error ?? 'observe failed')
          return JSON.stringify(result.result)
        }
        await this.ensureChrome()
        return JSON.stringify(await this.chrome.observe())
      }
      case 'browser_click': {
        const target = { ref: args.ref as number | undefined, selector: args.selector as string | undefined }
        if (target.ref === undefined && target.selector === undefined) throw new Error('需要 ref 或 selector')
        if (mode === 'shared') {
          const result = await this.shared.request('click', { ...target, tabId: args.tabId })
          if (!result.ok) throw new Error(result.error ?? 'click failed')
          return JSON.stringify(result.result)
        }
        await this.ensureChrome()
        return await this.runManaged((session) => evaluate(session, CLICK_SCRIPT(target)) as Promise<unknown>)
      }
      case 'browser_type': {
        const text = String(args.text ?? '')
        const target = { ref: args.ref as number | undefined, selector: args.selector as string | undefined }
        if (target.ref === undefined && target.selector === undefined) throw new Error('需要 ref 或 selector')
        if (mode === 'shared') {
          const result = await this.shared.request('type', { text, ...target, tabId: args.tabId })
          if (!result.ok) throw new Error(result.error ?? 'type failed')
          return JSON.stringify(result.result)
        }
        await this.ensureChrome()
        return await this.runManaged((session) => evaluate(session, TYPE_SCRIPT(text, target)) as Promise<unknown>)
      }
      case 'browser_press': {
        const key = String(args.key ?? '')
        if (mode === 'shared') {
          const result = await this.shared.request('press', { key, tabId: args.tabId })
          if (!result.ok) throw new Error(result.error ?? 'press failed')
          return JSON.stringify(result.result)
        }
        await this.ensureChrome()
        return await this.runManaged(async (session) => {
          await pressKey(session, key)
          return { ok: true }
        })
      }
      case 'browser_list_tabs': {
        const result = await this.shared.request('list_tabs', {})
        if (!result.ok) throw new Error(result.error ?? 'list_tabs failed')
        return JSON.stringify(result.result)
      }
      case 'browser_close': {
        if (mode === 'shared') {
          const result = await this.shared.request('close_tab', { tabId: args.tabId })
          if (!result.ok) throw new Error(result.error ?? 'close failed')
          return JSON.stringify(result.result)
        }
        return JSON.stringify({ ok: true, hint: '受管 Chrome 由 DSH++ 管理，无需单独关闭标签' })
      }
      case 'browser_evaluate': {
        const expression = String(args.expression ?? '')
        if (expression.trim().length === 0) throw new Error('expression 不能为空')
        // Runtime.evaluate 本就对 Promise 结果 await（awaitPromise:true），无需再做 IIFE。
        // 直接传原表达式，避免 `return (let x=1; x)` 这类多语句/非表达式输入被裹成语法错误。
        if (mode === 'shared') {
          const result = await this.shared.request('evaluate', { expression, tabId: args.tabId })
          if (!result.ok) throw new Error(result.error ?? 'evaluate failed')
          return JSON.stringify(result.result)
        }
        await this.ensureChrome()
        return await this.runManaged(async (session) => {
          return (await evaluate(session, expression)) as unknown
        })
      }
      default:
        throw new Error(`unknown tool: ${name}`)
    }
  }

  private async ensureChrome(): Promise<void> {
    if (!this.chrome.running) {
      await this.chrome.start()
    }
  }

  private async runManaged(action: (session: CdpSession) => Promise<unknown>): Promise<string> {
    // Stateless per call: reuse the current page if present, else open blank.
    const session = await this.chrome.ensurePage()
    try {
      return JSON.stringify(await action(session))
    } finally {
      session.close()
    }
  }
}

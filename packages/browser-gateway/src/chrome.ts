/**
 * Managed Chrome instance for the DSH++ "browser" capability (CDP route).
 * The instance uses its own user-data-dir so the user's daily browser is
 * never touched. The gateway owns the Chrome child process and kills it on
 * exit; the desktop app additionally places the gateway in its Job Object.
 */

import { spawn, type ChildProcess } from 'node:child_process'
import { existsSync } from 'node:fs'
import { mkdir } from 'node:fs/promises'
import { join } from 'node:path'
import { CdpSession, createTarget, evaluate, fetchJson, listTargets, navigate } from './cdp.js'

export const CDP_PORT = 18767

const CANDIDATE_PATHS = [
  process.env.DSHPLUSPLUS_CHROME,
  `${process.env.LOCALAPPDATA ?? ''}\\Google\\Chrome\\Application\\chrome.exe`,
  `${process.env.PROGRAMFILES ?? ''}\\Google\\Chrome\\Application\\chrome.exe`,
  `${process.env['PROGRAMFILES(X86)'] ?? ''}\\Google\\Chrome\\Application\\chrome.exe`,
  `${process.env.LOCALAPPDATA ?? ''}\\Microsoft\\Edge\\Application\\msedge.exe`,
  `${process.env.PROGRAMFILES ?? ''}\\Microsoft\\Edge\\Application\\msedge.exe`,
].filter((path): path is string => path !== undefined && path.length > 0)

/** Locate a Chromium-family browser executable. */
export function findChrome(): string | null {
  for (const candidate of CANDIDATE_PATHS) {
    if (candidate.trim().length > 0 && existsSync(candidate)) return candidate
  }
  return null
}

export class ManagedChrome {
  private child: ChildProcess | null = null
  private readonly dataDir: string
  private readonly screenshotsDir: string

  constructor(dataRoot: string) {
    this.dataDir = join(dataRoot, 'browser-data')
    this.screenshotsDir = join(dataRoot, 'browser-screenshots')
  }

  get running(): boolean {
    return this.child !== null && this.child.exitCode === null && !this.child.killed
  }

  get pid(): number | null {
    return this.child?.pid ?? null
  }

  async start(): Promise<string> {
    if (this.running) return 'already-running'
    const executable = findChrome()
    if (executable === null) {
      throw new Error('未找到 Chrome/Edge；请安装 Chrome 或设置 DSHPLUSPLUS_CHROME')
    }
    await mkdir(this.dataDir, { recursive: true })
    await mkdir(this.screenshotsDir, { recursive: true })
    const child = spawn(
      executable,
      [
        `--remote-debugging-port=${CDP_PORT}`,
        `--user-data-dir=${this.dataDir}`,
        '--no-first-run',
        '--no-default-browser-check',
        '--disable-background-networking',
        '--disable-component-update',
        '--disable-sync',
        'about:blank',
      ],
      { stdio: 'ignore', windowsHide: true },
    )
    this.child = child
    child.on('exit', () => {
      this.child = null
    })
    // Wait for the CDP endpoint.
    const deadline = Date.now() + 20_000
    while (Date.now() < deadline) {
      if (!this.running) throw new Error('受管 Chrome 进程退出')
      try {
        await fetchJson(`http://127.0.0.1:${CDP_PORT}/json/version`)
        return `ready pid=${child.pid}`
      } catch {
        await new Promise((resolve) => setTimeout(resolve, 300))
      }
    }
    throw new Error('受管 Chrome CDP 端点 20 秒内未就绪')
  }

  async stop(): Promise<void> {
    const child = this.child
    this.child = null
    if (child === null || child.exitCode !== null) return
    child.kill()
    await new Promise<void>((resolve) => {
      const timer = setTimeout(() => {
        try {
          child.kill('SIGKILL')
        } catch {
          // ignore
        }
        resolve()
      }, 3000)
      child.once('exit', () => {
        clearTimeout(timer)
        resolve()
      })
    })
  }

  /** Open (or reuse the first page target) and return its CDP session. */
  async openPage(url: string): Promise<CdpSession> {
    const page = await this.ensurePage()
    await navigate(page, url)
    return page
  }

  /** Connect to the first page target, creating one if necessary. No navigation. */
  async ensurePage(): Promise<CdpSession> {
    let targets = await listTargets(CDP_PORT)
    let page = targets.find((target) => target.type === 'page')
    if (page === undefined) {
      const created = await createTarget(CDP_PORT)
      targets = await listTargets(CDP_PORT)
      page = targets.find((target) => target.id === created.id) ?? created
    }
    return CdpSession.connect(page.webSocketDebuggerUrl)
  }

  /** Run a browser action against a freshly opened session (stateless per call). */
  async withPage<T>(url: string, action: (session: CdpSession) => Promise<T>): Promise<T> {
    const session = await this.openPage(url)
    try {
      return await action(session)
    } finally {
      session.close()
    }
  }

  async observe(): Promise<Record<string, unknown>> {
    const session = await this.ensurePage()
    try {
      const snapshot = (await evaluate(
        session,
        `(() => {
          const elements = [...document.querySelectorAll('a,button,input,textarea,select,[role="button"],[role="link"],[onclick]')]
            .slice(0, 200)
            .map((el, index) => ({
              index,
              tag: el.tagName.toLowerCase(),
              text: (el.innerText || el.value || el.placeholder || el.getAttribute('aria-label') || '').trim().slice(0, 80),
              href: el.tagName === 'A' && el.href ? el.href.slice(0, 200) : null,
              type: el.getAttribute('type'),
            }));
          return {
            title: document.title,
            url: location.href,
            text: (document.body ? document.body.innerText : '').slice(0, 20000),
            elements,
          };
        })()`,
      )) as Record<string, unknown>
      const screenshot = await this.captureScreenshot(session)
      return { ...snapshot, screenshot }
    } finally {
      session.close()
    }
  }

  private async captureScreenshot(session: CdpSession): Promise<string | null> {
    try {
      const { captureScreenshot } = await import('./cdp.js')
      const data = await captureScreenshot(session)
      const name = `shot-${Date.now()}.png`
      const { writeFile } = await import('node:fs/promises')
      await writeFile(join(this.screenshotsDir, name), Buffer.from(data, 'base64'))
      return join(this.screenshotsDir, name)
    } catch {
      return null
    }
  }

  async status(): Promise<Record<string, unknown>> {
    const executable = findChrome()
    return {
      running: this.running,
      pid: this.pid,
      browser: executable ?? null,
      cdpPort: CDP_PORT,
    }
  }
}

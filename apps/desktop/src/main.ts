import { invoke } from '@tauri-apps/api/core'
import './style.css'

type ServiceState = 'stopped' | 'starting' | 'running' | 'error'

interface AppConfig {
  dshHost: string
  dshPort: number
  workspace: string
  updateUrl: string
  autoStartDsh: boolean
  autoOpenDshWindow: boolean
  enableMca: boolean
  enableBrowser: boolean
  enableChromeUse: boolean
  mcaImage: boolean
  mcaVideo: boolean
  mcaAudio: boolean
  mcaDocument: boolean
  mcaWeb: boolean
  mcaComputerObserve: boolean
  mcaComputerAct: boolean
  deepseekBaseUrl: string
  deepseekModel: string
  visionProvider: string
  visionBaseUrl: string
  visionModel: string
  visionApi: string
  hasVisionKey: boolean
  enableMultimodal: boolean
}

interface RuntimeInfo {
  portable: boolean
  dataRoot: string
  dshHome: string | null
  dshCli: string | null
  nodeBinary: string | null
  mcaBinary: string | null
  browserGateway: string | null
}

interface McaRouteView {
  agentId: string
  routeAvailable: boolean
  capabilities: string[]
  availableCapabilities: string[]
  computerProviderEnabled: boolean
  health: string
  healthDetail: string
}

interface McaProviderView {
  providerId: string
  enabled: boolean
  available: boolean
  detail: string
}

interface AppSnapshot {
  version: string
  config: AppConfig
  runtime: RuntimeInfo
  dshState: ServiceState
  dshUrl: string
  dshMessage: string
  mcaState: ServiceState
  mcaUrl: string | null
  mcaMessage: string
  mcaRoute: McaRouteView | null
  mcaProviders: McaProviderView[]
  browserState: ServiceState
  browserMessage: string
}

const app = document.querySelector<HTMLDivElement>('#app')!

app.innerHTML = `
  <div class="app-shell">
    <aside class="sidebar">
      <div class="brand">
        <img src="/deepseek.svg" alt="DeepSeek" />
        <div class="brand-copy">
          <div><strong>deepseek</strong><span>HARNESS</span></div>
          <small>PlusPlus 控制中心</small>
        </div>
      </div>

      <nav class="side-nav" aria-label="DSH++ 设置">
        <button class="nav-item active" data-tab="models">
          <span class="nav-icon">✦</span><span>扩展能力</span>
        </button>
        <button class="nav-item" data-tab="runtime">
          <span class="nav-icon">⌘</span><span>运行环境</span>
        </button>
        <button class="nav-item" data-tab="logs">
          <span class="nav-icon">≡</span><span>诊断日志</span>
        </button>
      </nav>

      <div class="sidebar-note">
        <span class="note-icon">++</span>
        <div><strong>DSH++</strong><small>多模态 · 网页 · 浏览器</small></div>
      </div>
      <div class="sidebar-footer"><span id="portable-mode">正在检测运行时…</span><span id="version">—</span></div>
    </aside>

    <main class="content">
      <header class="page-header">
        <div><h1>DSH++ 控制中心</h1><p>为 DeepSeek Harness 补充多模态与内容工具能力</p></div>
        <button class="icon-button" id="refresh" title="刷新状态" aria-label="刷新状态">↻</button>
      </header>

      <section class="launcher">
        <div class="launcher-copy">
          <span class="preview-pill">PLUS++</span>
          <h2>DeepSeek Harness</h2>
          <p>一键启动内置 DSH（数据与独立安装的 dsh 共享），并加载多模态、网页搜索与浏览器能力。</p>
        </div>
        <div class="service-stack">
          <div class="service-row">
            <span class="dot" id="dsh-dot"></span>
            <div><strong>DSH 服务</strong><small id="dsh-message">正在读取状态…</small></div>
            <span class="state" id="dsh-state">—</span>
          </div>
          <div class="service-row">
            <span class="dot" id="mca-dot"></span>
            <div><strong>MCA 能力层</strong><small id="mca-message">正在读取状态…</small></div>
            <span class="state" id="mca-state">—</span>
          </div>
          <div class="service-row">
            <span class="dot" id="browser-dot"></span>
            <div><strong>浏览器网关</strong><small id="browser-message">正在读取状态…</small></div>
            <span class="state" id="browser-state">—</span>
          </div>
        </div>
        <div class="dsh-missing-banner" id="dsh-missing-banner" hidden>
          <span>未找到本地 DeepSeek Harness。安装 DSH 后即可从这里一键启动（控制中心其他功能不受影响）。</span>
          <button class="button outline" id="get-dsh">获取 DSH</button>
        </div>
        <div class="launcher-actions">
          <button class="button primary" id="start">启动 DSH</button>
          <button class="button outline" id="open">打开 DSH</button>
          <button class="button ghost" id="open-browser">浏览器打开</button>
          <button class="button ghost danger" id="stop">停止</button>
        </div>
      </section>

      <section class="page active" id="page-models">
        <div class="section-heading"><div><h2>扩展能力</h2><p>主模型由 DSH 自己管理；这里只配置 DSH++ 增加的能力。</p></div></div>

        <article class="settings-card dsh-owned">
          <div class="card-icon"><img src="/deepseek.svg" alt="" /></div>
          <div class="card-main">
            <div class="card-title"><h3>主模型 API</h3><span class="managed-pill">由 DSH 管理</span></div>
            <p>Provider、模型与 API Key 已从 DSH++ 隐藏。启动 DSH 后，在「设置 → 模型」中统一配置。</p>
          </div>
          <button class="button outline" id="configure-dsh">前往 DSH 配置</button>
        </article>

        <article class="settings-card capability-card" id="mca-capability-card">
          <div class="card-header capability-header">
            <div>
              <div class="card-title"><h3>MCA 能力层</h3><span class="managed-pill" id="mca-capability-count">—</span></div>
              <p>按需把内容、网页和电脑操作工具交给 DSH；每项能力都可单独关闭。</p>
            </div>
            <label class="switch" aria-label="启用 MCA 能力层"><input type="checkbox" id="enable-mca"><span></span></label>
          </div>
          <div class="capability-grid">
            <label class="capability-item">
              <span class="capability-icon">图</span><span class="capability-copy"><strong>图片</strong><small>读取图片并提取视觉信息</small><code>image</code><small class="provider-health" data-provider-health="image"></small></span>
              <span class="switch compact"><input type="checkbox" id="mca-image" data-mca-capability><span></span></span>
            </label>
            <label class="capability-item">
              <span class="capability-icon">视</span><span class="capability-copy"><strong>视频</strong><small>理解视频画面、字幕与时间线</small><code>video</code><small class="provider-health" data-provider-health="video"></small></span>
              <span class="switch compact"><input type="checkbox" id="mca-video" data-mca-capability><span></span></span>
            </label>
            <label class="capability-item">
              <span class="capability-icon">音</span><span class="capability-copy"><strong>音频</strong><small>转写并分析音频内容</small><code>audio</code><small class="provider-health" data-provider-health="audio"></small></span>
              <span class="switch compact"><input type="checkbox" id="mca-audio" data-mca-capability><span></span></span>
            </label>
            <label class="capability-item">
              <span class="capability-icon">文</span><span class="capability-copy"><strong>文档</strong><small>读取文档、表格与结构化内容</small><code>document</code><small class="provider-health" data-provider-health="document"></small></span>
              <span class="switch compact"><input type="checkbox" id="mca-document" data-mca-capability><span></span></span>
            </label>
            <label class="capability-item">
              <span class="capability-icon">网</span><span class="capability-copy"><strong>网页</strong><small>访问并采集网页内容</small><code>web</code><small class="provider-health" data-provider-health="web"></small></span>
              <span class="switch compact"><input type="checkbox" id="mca-web" data-mca-capability><span></span></span>
            </label>
            <label class="capability-item">
              <span class="capability-icon">览</span><span class="capability-copy"><strong>观察电脑</strong><small>查看屏幕与当前界面状态</small><code>computer.observe</code><small class="provider-health" data-provider-health="computer.observe"></small></span>
              <span class="switch compact"><input type="checkbox" id="mca-computer-observe" data-mca-capability><span></span></span>
            </label>
            <label class="capability-item">
              <span class="capability-icon">控</span><span class="capability-copy"><strong>操作电脑</strong><small>点击、输入并操作电脑；需同时启用观察</small><code>computer.act</code><small class="provider-health" data-provider-health="computer.act"></small></span>
              <span class="switch compact"><input type="checkbox" id="mca-computer-act" data-mca-capability><span></span></span>
            </label>
          </div>
          <p class="capability-note">电脑能力默认关闭；启用后仍受 MCA 的风险等级、确认策略与本机自动化运行环境约束。</p>
        </article>

        <article class="settings-card capability-card" id="browser-capability-card">
          <div class="card-header capability-header">
            <div>
              <div class="card-title"><h3>浏览器操作</h3><span class="managed-pill">DSH++ Browser</span></div>
              <p>在对话中打开网页、点击、输入、搜索；两种形态可同时启用。</p>
            </div>
          </div>
          <div class="capability-grid">
            <label class="capability-item">
              <span class="capability-icon">览</span><span class="capability-copy"><strong>浏览器</strong><small>独立受管 Chrome，不碰日常浏览器</small><code>browser</code></span>
              <span class="switch compact"><input type="checkbox" id="enable-browser"><span></span></span>
            </label>
            <label class="capability-item">
              <span class="capability-icon">控</span><span class="capability-copy"><strong>Chrome 共享标签</strong><small>在你已打开的 Chrome 中操作，复用登录态</small><code>chromeUse</code></span>
              <span class="switch compact"><input type="checkbox" id="enable-chrome-use"><span></span></span>
            </label>
          </div>
          <div class="card-actions">
            <button class="button outline" id="install-extension">安装 Chrome 扩展</button>
            <span class="capability-note" id="browser-hint">启用后需重启 DSH 使工具进入对话；网关本身实时启停。</span>
          </div>
        </article>

        <article class="settings-card form-card">
          <div class="card-header">
            <div><h3>多模态专家模型</h3><p>图片先由视觉模型生成可追溯观察，再交给 DeepSeek 继续处理。</p></div>
            <label class="switch" aria-label="启用多模态"><input type="checkbox" id="enable-multimodal"><span></span></label>
          </div>
          <div class="fields">
            <label>Provider ID<input id="vision-provider" placeholder="vision-gateway" /></label>
            <label>API 协议<select id="vision-api"><option value="openai-completions">OpenAI Chat Completions</option><option value="anthropic-messages">Anthropic Messages</option></select></label>
            <label class="wide">API Base URL<input id="vision-base" placeholder="https://example.com/v1" /></label>
            <label>视觉模型<input id="vision-model" placeholder="qwen-vl-max" /></label>
            <label>API Key<div class="secret"><input id="vision-key" type="password" autocomplete="off" placeholder="输入新密钥，留空则不修改"/><button type="button" data-reveal="vision-key">显示</button></div><small id="vision-key-state"></small></label>
          </div>
        </article>

        <div class="savebar"><span>密钥仅保存在当前便携目录，并使用 Windows DPAPI 加密。</span><button class="button primary" id="save">保存扩展配置</button></div>
      </section>

      <section class="page" id="page-runtime">
        <div class="section-heading"><div><h2>运行环境</h2><p>可连接已有 DSH，也可启动内置的 DSH；数据默认使用 dsh 标准目录（~/.dsh），卸载 dsh++ 不影响 dsh 数据。</p></div></div>
        <div class="runtime-grid">
          <article class="settings-card form-card">
            <div class="card-header"><div><h3>DSH 服务</h3><p>若地址和端口已有 DSH，启动时会自动复用。</p></div><span class="managed-pill">Loopback</span></div>
            <div class="fields">
              <label>监听地址<input id="dsh-host" value="127.0.0.1" /></label>
              <label>端口<input id="dsh-port" type="number" min="1024" max="65535" /></label>
              <label class="wide">默认工作目录<input id="workspace" placeholder="DSH 的默认工作目录" /></label>
            </div>
            <label class="check"><input type="checkbox" id="auto-start"><span>打开 DSH++ 时自动启动 DSH</span></label>
            <label class="check"><input type="checkbox" id="auto-open-window"><span>DSH 就绪后自动打开桌面窗口（替代系统浏览器）</span></label>
          </article>
          <article class="settings-card form-card">
            <div class="card-header"><div><h3>更新</h3><p>检查本地暂存的新版本，或配置远程更新源（JSON：{"version":"x.y.z","url":"…/DSHPlusPlus.update.exe"}）。</p></div><span class="managed-pill">更新器</span></div>
            <div class="fields">
              <label class="wide">远程更新源 URL<input id="update-url" placeholder="留空则只检测本地暂存更新" /></label>
            </div>
            <div class="launcher-actions">
              <button class="button outline" id="check-update">检查更新</button>
              <span class="update-result" id="update-result"></span>
            </div>
          </article>
          <article class="settings-card form-card">
            <div class="card-header"><div><h3>MCA 内容适配层</h3><p>能力开关已移至“扩展能力”页面；这里展示便携运行时位置。</p></div><span class="managed-pill">能力层</span></div>
            <dl class="runtime-list">
              <div><dt>数据目录</dt><dd id="data-root">—</dd></div>
              <div><dt>DSH 数据目录</dt><dd id="dsh-home">—</dd></div>
              <div><dt>Node Runtime</dt><dd id="node-runtime">—</dd></div>
              <div><dt>DSH Runtime</dt><dd id="dsh-runtime">—</dd></div>
              <div><dt>MCA Runtime</dt><dd id="mca-runtime">—</dd></div>
              <div><dt>Browser Gateway</dt><dd id="browser-runtime">—</dd></div>
            </dl>
          </article>
        </div>
      </section>

      <section class="page" id="page-logs">
        <div class="section-heading"><div><h2>诊断日志</h2><p>用于定位 DSH 与 MCA 的启动问题。</p></div><button class="button outline" id="reload-logs">重新读取</button></div>
        <article class="settings-card log-card"><pre id="logs">点击“重新读取”查看最近日志。</pre></article>
      </section>
    </main>
    <div class="toast" id="toast"></div>
  </div>
`

const byId = <T extends HTMLElement>(id: string) => document.getElementById(id) as T
let snapshot: AppSnapshot | null = null
let busy = false

function value(id: string): string { return byId<HTMLInputElement>(id).value.trim() }
function checked(id: string): boolean { return byId<HTMLInputElement>(id).checked }
function setValue(id: string, next: string | number): void { byId<HTMLInputElement>(id).value = String(next) }
function setChecked(id: string, next: boolean): void { byId<HTMLInputElement>(id).checked = next }

/** UI 开关 id → MCA 能力 id（来自 GET /api/agent-routes 的 available_capabilities）。 */
const MCA_CAPABILITY_IDS: Record<string, string> = {
  'mca-image': 'image',
  'mca-video': 'video',
  'mca-audio': 'audio',
  'mca-document': 'document',
  'mca-web': 'web',
  'mca-computer-observe': 'computer.observe',
  'mca-computer-act': 'computer.act',
}

/** 能力 id → 支撑该能力的 MCA Provider 列表（工具级健康展示）。 */
const MCA_CAPABILITY_PROVIDERS: Record<string, string[]> = {
  image: ['wheel.image-metadata', 'specialist.easyocr', 'builtin.windows-ocr', 'pipeline.media-vision'],
  video: ['wheel.yt-dlp-online-media', 'pipeline.media-vision', 'pipeline.media-local'],
  audio: ['specialist.whisper', 'specialist.pyannote-diarization', 'pipeline.media-local'],
  document: ['wheel.office-documents'],
  web: ['wheel.web-collection', 'builtin.html', 'wheel.playwright-browser'],
  'computer.observe': ['wheel.pyautogui-desktop', 'wheel.playwright-browser'],
  'computer.act': ['wheel.pyautogui-desktop'],
}

/**
 * 能力卡片的工具级健康行：展示支撑该能力的 MCA Provider 可用性
 * （✓ 可用 / ✗ 不可用或未启用），悬停显示具体原因。
 */
function syncProviderHealth(providers: McaProviderView[]): void {
  const byId = new Map(providers.map(provider => [provider.providerId, provider]))
  for (const [capability, providerIds] of Object.entries(MCA_CAPABILITY_PROVIDERS)) {
    const row = document.querySelector<HTMLElement>(`[data-provider-health="${capability}"]`)
    if (row === null) continue
    if (providers.length === 0) {
      row.textContent = ''
      row.title = ''
      continue
    }
    const parts = providerIds.map(providerId => {
      const provider = byId.get(providerId)
      if (provider === undefined) return `${providerId}：未安装`
      const ok = provider.enabled && provider.available
      return `${ok ? '✓' : '✗'} ${providerId}${provider.detail ? `：${provider.detail}` : ''}`
    })
    row.textContent = `Provider：${parts.join('  ')}`
    row.title = parts.join('\n')
  }
}

/**
 * 按 MCA 路由实时能力动态化能力开关：
 * - 路由不提供的能力 → 禁用并提示；
 * - 电脑能力在 computer Provider 未启用时 → 禁用并提示；
 * - MCA 未运行/无路由信息 → 仅受总开关控制（保持原行为）。
 */
function syncMcaAvailability(route: McaRouteView | null): void {
  const enabled = checked('enable-mca')
  for (const [id, capability] of Object.entries(MCA_CAPABILITY_IDS)) {
    const control = byId<HTMLInputElement>(id)
    let disabled = !enabled
    let reason = ''
    if (enabled && route !== null) {
      if (!route.availableCapabilities.includes(capability)) {
        disabled = true
        reason = 'MCA 路由当前不提供该能力'
      } else if (capability.startsWith('computer.') && !route.computerProviderEnabled) {
        disabled = true
        reason = '电脑 Provider 未启用（需在 MCA 控制中心启用）'
      }
    }
    control.disabled = disabled
    control.title = reason
  }
}

function syncMcaControls(): void {
  const enabled = checked('enable-mca')
  const controls = [...document.querySelectorAll<HTMLInputElement>('[data-mca-capability]')]
  byId('mca-capability-card').classList.toggle('capabilities-disabled', !enabled)
  const count = controls.filter(control => control.checked && !control.disabled).length
  const total = controls.filter(control => !control.disabled).length
  byId('mca-capability-count').textContent = enabled
    ? `${count} / ${total} 可用能力已启用`
    : '总开关已关闭'
  syncMcaAvailability(snapshot?.mcaRoute ?? null)
}

function toast(message: string, error = false): void {
  const node = byId('toast')
  node.textContent = message
  node.className = `toast show${error ? ' error' : ''}`
  window.setTimeout(() => { node.className = 'toast' }, 3600)
}

function stateLabel(state: ServiceState): string {
  return ({ stopped: '已停止', starting: '启动中', running: '运行中', error: '异常' })[state]
}

function renderStatus(data: AppSnapshot): void {
  for (const service of ['dsh', 'mca', 'browser'] as const) {
    const state = data[`${service}State`]
    byId(`${service}-dot`).className = `dot ${state}`
    byId(`${service}-state`).textContent = stateLabel(state)
    byId(`${service}-message`).textContent = data[`${service}Message`]
  }
  // MCA 消息附带 deepseek-tui 路由健康（含阻塞原因）。
  if (data.mcaRoute) {
    const route = data.mcaRoute
    const detail = route.healthDetail ? ` · ${route.healthDetail}` : ''
    byId('mca-message').textContent = `${data.mcaMessage} · 路由健康 ${route.health}${detail}`
  }
  syncMcaAvailability(data.mcaRoute)
  syncProviderHealth(data.mcaProviders)
  // 未找到本地 DSH：显示引导横幅（控制中心其余功能照常）。
  const missing = data.runtime.dshCli === null
  byId('dsh-missing-banner').hidden = !missing
  byId('dsh-runtime').textContent = data.runtime.dshCli ?? (missing ? '未找到（请先安装 DSH）' : '未找到')
  const start = byId<HTMLButtonElement>('start')
  start.disabled = data.dshState === 'starting' || data.dshState === 'running'
  start.textContent = data.dshState === 'starting' ? '正在启动…' : data.dshState === 'running' ? 'DSH 已启动' : '启动 DSH'
  byId<HTMLButtonElement>('open').disabled = data.dshState !== 'running'
  byId<HTMLButtonElement>('configure-dsh').disabled = data.dshState !== 'running'
  byId('portable-mode').textContent = data.runtime.portable ? '捆绑运行时已就绪' : '开发运行时'
  byId('dsh-home').textContent = data.runtime.dshHome ?? '—'
}

function renderAll(data: AppSnapshot): void {
  snapshot = data
  byId('version').textContent = `v${data.version}`
  const c = data.config
  setValue('vision-provider', c.visionProvider)
  setValue('vision-base', c.visionBaseUrl)
  setValue('vision-model', c.visionModel)
  setValue('vision-api', c.visionApi)
  setValue('dsh-host', c.dshHost)
  setValue('dsh-port', c.dshPort)
  setValue('workspace', c.workspace)
  setValue('update-url', c.updateUrl ?? '')
  setChecked('enable-multimodal', c.enableMultimodal)
  setChecked('enable-mca', c.enableMca)
  setChecked('mca-image', c.mcaImage)
  setChecked('mca-video', c.mcaVideo)
  setChecked('mca-audio', c.mcaAudio)
  setChecked('mca-document', c.mcaDocument)
  setChecked('mca-web', c.mcaWeb)
  setChecked('mca-computer-observe', c.mcaComputerObserve)
  setChecked('mca-computer-act', c.mcaComputerAct)
  setChecked('auto-start', c.autoStartDsh)
  setChecked('auto-open-window', c.autoOpenDshWindow)
  setChecked('enable-browser', c.enableBrowser)
  setChecked('enable-chrome-use', c.enableChromeUse)
  syncMcaControls()
  byId('vision-key-state').textContent = c.hasVisionKey ? '已安全保存密钥' : '尚未保存密钥'
  byId('data-root').textContent = data.runtime.dataRoot
  byId('node-runtime').textContent = data.runtime.nodeBinary ?? '未找到'
  byId('mca-runtime').textContent = data.runtime.mcaBinary ?? '未找到（可关闭 MCA）'
  byId('browser-runtime').textContent = data.runtime.browserGateway ?? '未找到'
  renderStatus(data)
}

async function refresh(full = false): Promise<void> {
  try {
    const data = await invoke<AppSnapshot>(full ? 'get_snapshot' : 'refresh_status')
    if (full || snapshot === null) renderAll(data)
    else { snapshot = data; renderStatus(data) }
  } catch (error) { toast(String(error), true) }
}

async function perform(action: () => Promise<void>): Promise<void> {
  if (busy) return
  busy = true
  document.body.classList.add('busy')
  try { await action() } catch (error) { toast(String(error), true) }
  finally { busy = false; document.body.classList.remove('busy') }
}

byId('save').addEventListener('click', () => perform(async () => {
  if (!snapshot) return
  const c = snapshot.config
  const input = {
    dshHost: value('dsh-host'), dshPort: Number(value('dsh-port')), workspace: value('workspace'),
    updateUrl: value('update-url'),
    autoStartDsh: checked('auto-start'), autoOpenDshWindow: checked('auto-open-window'),
    enableMca: checked('enable-mca'), enableBrowser: checked('enable-browser'),
    enableChromeUse: checked('enable-chrome-use'),
    mcaImage: checked('mca-image'), mcaVideo: checked('mca-video'), mcaAudio: checked('mca-audio'),
    mcaDocument: checked('mca-document'), mcaWeb: checked('mca-web'),
    mcaComputerObserve: checked('mca-computer-observe'), mcaComputerAct: checked('mca-computer-act'),
    deepseekBaseUrl: c.deepseekBaseUrl, deepseekModel: c.deepseekModel, deepseekApiKey: null,
    visionProvider: value('vision-provider'), visionBaseUrl: value('vision-base'),
    visionModel: value('vision-model'), visionApi: value('vision-api'),
    visionApiKey: value('vision-key') || null, enableMultimodal: checked('enable-multimodal'),
  }
  const data = await invoke<AppSnapshot>('save_config', { input })
  setValue('vision-key', '')
  renderAll(data)

  // 生效方式提示：MCA 能力热生效；多模态插件配置需重启 DSH。
  const before = c
  const after = data.config
  const mcaFields = ['enableMca', 'mcaImage', 'mcaVideo', 'mcaAudio', 'mcaDocument', 'mcaWeb', 'mcaComputerObserve', 'mcaComputerAct'] as const
  const mmFields = ['enableMultimodal', 'visionProvider', 'visionBaseUrl', 'visionModel', 'visionApi', 'hasVisionKey'] as const
  const browserFields = ['enableBrowser', 'enableChromeUse'] as const
  const mcaChanged = mcaFields.some(field => before[field] !== after[field])
  const mmChanged = mmFields.some(field => before[field] !== after[field])
  const browserChanged = browserFields.some(field => before[field] !== after[field])
  const hints: string[] = []
  if (mcaChanged) {
    hints.push(data.mcaState === 'running'
      ? 'MCA 能力已实时生效，无需重启 DSH'
      : 'MCA 未运行，能力配置将在下次启动 DSH 时生效')
  }
  if (mmChanged) {
    hints.push('多模态专家配置已保存，需要重启 DSH 后生效')
  }
  if (browserChanged) {
    hints.push(data.browserState === 'running'
      ? '浏览器网关已实时启停；DSH 中的浏览器工具需重启 DSH 后刷新'
      : '浏览器网关未运行，将在下次启动 DSH 时生效')
  }
  if (after.enableMultimodal && (!after.visionModel || !after.visionBaseUrl)) {
    hints.push('尚未填写视觉模型与 Base URL，多模态暂不生效')
  }
  toast(hints.length > 0 ? hints.join('；') : '配置已保存；主模型仍由 DSH 管理。')
}))

byId('start').addEventListener('click', () => perform(async () => {
  const data = await invoke<AppSnapshot>('start_services')
  snapshot = data
  renderStatus(data)
  toast('启动任务已提交，界面会持续更新状态。')
}))
byId('stop').addEventListener('click', () => perform(async () => { const data = await invoke<AppSnapshot>('stop_services'); snapshot = data; renderStatus(data); toast('DSH++ 管理的本地服务已停止。') }))
byId('open').addEventListener('click', () => perform(async () => { await invoke('open_dsh_window_command'); toast('已在桌面窗口中打开 DSH。') }))
byId('open-browser').addEventListener('click', () => perform(async () => { await invoke('open_dsh'); toast('已在系统浏览器中打开 DSH。') }))
byId('configure-dsh').addEventListener('click', () => perform(async () => { await invoke('open_dsh_window_command'); toast('请在 DSH 的“设置 → 模型”中配置主模型。') }))
byId('refresh').addEventListener('click', () => refresh(false))
byId('install-extension').addEventListener('click', () => perform(async () => {
  const hint = await invoke<string>('install_chrome_extension')
  toast('Chrome 扩展安装完成，请按提示操作。')
  byId('browser-hint').textContent = hint
}))
byId('reload-logs').addEventListener('click', async () => { byId('logs').textContent = await invoke<string>('read_logs') })
byId('enable-mca').addEventListener('change', syncMcaControls)
document.querySelectorAll<HTMLInputElement>('[data-mca-capability]').forEach(control => control.addEventListener('change', () => {
  if (control.id === 'mca-computer-act' && control.checked) setChecked('mca-computer-observe', true)
  if (control.id === 'mca-computer-observe' && !control.checked) setChecked('mca-computer-act', false)
  syncMcaControls()
}))

document.querySelectorAll<HTMLButtonElement>('[data-reveal]').forEach(button => button.addEventListener('click', () => {
  const input = byId<HTMLInputElement>(button.dataset.reveal!)
  input.type = input.type === 'password' ? 'text' : 'password'
  button.textContent = input.type === 'password' ? '显示' : '隐藏'
}))

document.querySelectorAll<HTMLButtonElement>('[data-tab]').forEach(tab => tab.addEventListener('click', () => {
  document.querySelectorAll('.nav-item,.page').forEach(node => node.classList.remove('active'))
  tab.classList.add('active')
  byId(`page-${tab.dataset.tab}`).classList.add('active')
}))

byId('check-update').addEventListener('click', () => perform(async () => {
  const result = await invoke<{ available: boolean; version: string | null; message: string }>('check_for_update')
  const node = byId('update-result')
  node.textContent = result.message
  node.className = `update-result ${result.available ? 'available' : ''}`
}))

byId('get-dsh').addEventListener('click', () => perform(async () => {
  await invoke('open_dsh_guide')
}))

void refresh(true)
window.setInterval(() => { if (!busy) void refresh(false) }, 1500)

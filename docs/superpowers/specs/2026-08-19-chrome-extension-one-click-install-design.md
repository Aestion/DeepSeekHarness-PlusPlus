# Chrome/Edge 浏览器扩展「一键安装 + 已安装检测」设计

- 日期：2026-08-19
- 状态：已批准（用户确认设计）
- 关联能力：`chromeUse`（浏览器操作/Chrome 共享标签）

## 目标

1. **一键安装**：扩展安装的后续设置与命令步骤无需用户单独操作（文件准备、native host 注册、浏览器拉起全部自动化；受 Chromium 安全边界限制，首次在最新版 Chrome/Edge 上最多需要一次"加载已解压"手动确认，加载后永久持久化）。
2. **已安装检测**：安装前/实时正确提示"已安装"。当前机器扩展已加载进 Chrome profile，但界面无任何检测，无法提示。

## 范围

- 浏览器：**Chrome + Edge 双支持**（用户已确认）。
- 检测逻辑：**Rust 侧**（方案 A，用户已确认）——读浏览器 profile 判定安装态，不依赖网关运行。
- 连接状态：网关 `/api/health` 补充 `shared.connected`。

## 现状（问题确认）

- 无检测：UI 从不读浏览器 profile，"已安装"状态无任何查询逻辑。
- 非一键：`install_chrome_extension` 只准备文件 + 打开 `chrome://extensions` + 复制路径到剪贴板，其余手动。
- native host 仅注册 Chrome 的 `HKCU\Software\Google\Chrome\NativeMessagingHosts`，未注册 Edge。
- 网关 `/api/health` 不暴露共享桥 `connected`，桌面端拿不到实时连接状态。

### 本机实况（2026-08-19 探查）

- Chrome 151（系统级安装，`C:\Program Files\Google\Chrome`），未运行；扩展记录在 `User Data\Default\Secure Preferences` 的 `extensions.settings.kikoigbglcakhdeknllbinnaepdaoofh`（`from_webstore:false`、service worker 已启动）。
- **记录的扩展 `path` 指向已消失的 dev.1 browser-extension 目录** → 扩展是失效残影，需路径校验并支持一键修复。
- Edge 151 正在运行（日常浏览器），扩展未装入 Edge profile。
- 用户真实安装：`E:\Dsh++\DSHPlusPlus-0.1.0-dev.5-windows-x64\`（native host 注册表指向该处）。

## 设计

### 1. 检测（Rust 新增）

**数据源**：`%LOCALAPPDATA%\Google\Chrome\User Data\{Default,Profile N}\` 与 `%LOCALAPPDATA%\Microsoft\Edge\User Data\{Default,Profile N}\` 下的 `Preferences` 与 `Secure Preferences`（JSON），取 `extensions.settings."kikoigbglcakhdeknllbinnaepdaoofh"`。扫描所有 profile 目录，任一命中即认为该浏览器存在记录。

**每浏览器四态**：

| 状态 | 判定 |
|---|---|
| `not-installed` 未安装 | 所有 profile 无记录 |
| `installed` 已安装 | 有记录、启用（无 `state:0` / `disable_reasons`）、且记录 `path` 目录存在且 manifest 与当前数据根一致 |
| `stale` 已安装但失效 | 有记录但 `path` 目录缺失或 manifest 不符 |
| `disabled` 已安装但禁用 | 记录 `state:0` 或含 `disable_reasons` |

**路径校验**：记录 `path` 指向的目录必须存在且含 `manifest.json`，且 manifest 的 `version`/`key` 与当前数据根 `browser-extension/` 一致。不一致视为 `stale`。

**连接状态**（实时，来自网关 `/api/health`）：`connected` = 共享桥在线（浏览器运行 + 扩展加载 + native host 通）。

**命令**：`chrome_extension_status()` → `{ chrome: { status, profile, path, connected }, edge: { status, profile, path, connected } }`。并入 `get_snapshot`（UI 轮询自动更新）。

**改造签名**：`install_chrome_extension(browser: "chrome" | "edge")`——新增 `browser` 参数指定目标浏览器；UI 按钮按状态分派传入。

### 2. 一键安装动作（改造 `install_chrome_extension`）

- **幂等准备**：现有复制扩展文件 + native host launcher + 注册表，**新增 Edge 注册** `HKCU\Software\Microsoft\Edge\NativeMessagingHosts\com.dshplusplus.browser`（Edge 与 Chrome 的 host 指向同一份 host-manifest.json / launcher）。
- **分派**（按目标浏览器参数）：
  - `installed`（健康）→ 不重装，直接拉起浏览器让桥自动连接。
  - `not-installed` / `stale` / `disabled` → 尝试 `--load-extension=<当前数据根 browser-extension>` 拉起浏览器；被新版浏览器拦截则回退「打开扩展页（`chrome://extensions` 或 `edge://extensions`）+ 剪贴板路径」引导，加载后永久持久化。
- 引导后 UI 轮询自动翻转状态。

### 3. 网关

`GET /api/health` 响应增加 `shared: { connected: boolean }`（取自 `SharedTabBridge.connected`）。

### 4. UI（`apps/desktop/src/main.ts`）

浏览器能力卡片每个浏览器一个状态行 + 按钮（按状态分派文案）：

- 未安装 → **「一键安装」**
- 已安装未连接 → **「打开浏览器连接」**
- 已连接 → 绿色「已连接」徽标
- 已安装但失效 → **「一键修复」**（自动重加载）

### 5. 测试

- **Rust 单测**：以 fixture profile JSON（构造 `Preferences` / `Secure Preferences` 样本）验证四态判定 + 路径校验。
- **网关测试**：`/api/health` 含 `connected` 字段。
- **本机实测**：Chrome 检测出"已安装但失效"，一键修复后翻转健康；Edge 装好后桥连接。

## 非目标（YAGNI）

- 不做 Chrome Web Store 上架 + 企业策略（ExtensionInstallForcelist）静默安装（需商店审核，另行立项）。
- 不做多浏览器 profile 的 UI 级区分（扫描所有 profile，结果归并到浏览器级状态）。
- 不自动刷新目标页面（保持现有"按 F5"语义）。

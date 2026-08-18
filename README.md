# DeepSeek Harness PlusPlus（DSH++）

> [English](README.en.md) · 中文

DeepSeek Harness（DSH）的增强层：为 DSH 补充**多模态视觉、网页读取/搜索与浏览器控制**能力，不修改、不注入、不替换 DSH 核心。基于 DSH 的 Profile/Bundle/Plugin/MCP 接口与外部 Sidecar 组合，升级边界和故障边界清晰。

> 状态：`0.1.0-dev.2` · Windows x64 · 配套 DSH `0.1.0-rc.6`

> **仓库范围**：本仓库**只包含 DSH++ 层**。DeepSeek Harness 核心是上游依赖（DeepSeek 官方发布的 `@deepseek-ai/dsh`），由用户安装时从 npm registry 获取——DSH 代码、依赖树或捆绑副本不会出现在本仓库或任何发布包中。GitHub Release 只发布 DSH++ 自身的产物：Lite 插件包（约 26 KB）与自包含桌面版（约 149 MB，控制中心 + Node + MCA + DSH++ 插件，**不含 DSH 本体**——exe 发现本地 DSH，缺失时引导安装）。支持的 DSH 上游版本见 [compatibility.json](runtime/manifests/compatibility.json)。

## 功能特性

- **为纯文本主模型补上图片能力**——图片先由外部视觉模型投影为文本观察，再交给 DeepSeek API（图片字节不会直达 DeepSeek）。
- **图片外发 ask-once 审批**——会话首次把图片发送给视觉模型前，通过 DSH 的审批通道询问用户；授权从持久化审计日志记住，同会话不再重复询问。
- **MCA 能力层**——图片 / 视频 / 音频 / 文档 / 网页 / computer.observe / computer.act 七项工具，可逐项开关；开启电脑能力时自动启用桌面自动化 Provider。
- **浏览器控制（双路线）**：
  - `managed`——CDP 受管独立 Chrome（隔离 profile、惰性启动）。
  - `shared`——通过小型 MV3 扩展（chromeUse）+ Native Messaging 桥接，控制你自己已登录的 Chrome，复用登录态。
- **网页读取与搜索**——MCA 页面采集（Playwright）+ DSH 的 `web_search` 通道（搜索结果带来源）。
- **系统代理透传**——MCA Sidecar（httpx / yt-dlp）自动跟随 Windows 系统代理。
- **桌面控制中心（Tauri）**——DSH 与 Sidecar 的启动/停止、能力开关（带 Provider 实时健康）、内嵌 DSH 窗口、系统托盘生命周期、单实例保护、更新检查与回滚助手。
- **DSH 数据始终属于 DSH**——默认使用 dsh 标准 home（`~/.dsh`），与你已有的 DSH 共享会话/设置/工作区；卸载或更新 DSH++ 不影响 dsh 数据。

## 架构

```
┌──────────────────────────────────────────────────────────────┐
│ DSHPlusPlus.exe（Tauri 控制中心）                             │
│  · 管理 DSH / MCA / 浏览器网关 生命周期                        │
│  · 发现本地 DSH（环境变量 / PATH / npm 全局 / 手动指定 CLI）， │
│    不捆绑 DSH 本体                                             │
│  · 在 ~/.dsh 下物化 dshplusplus Profile                      │
└──────────────┬───────────────────────────────┬───────────────┘
               │ 拉起本地 dsh                 │ 拉起
┌──────────────▼───────────────┐   ┌──────────▼────────────────┐
│ DeepSeek Harness（本地）     │   │ MCA Sidecar（18767）       │
│  · @dshplusplus/multimodal   │   │  image/video/audio/doc/   │
│  · multimodal-router         │   │  web/computer 各 Provider │
│  · tool-media-inspect        │   │  （跟随系统代理）          │
│  · dsh-mcp-client → MCA      │   └──────────┬────────────────┘
└──────────────┬───────────────┘              │ MCP
               │ MCP                          │
┌──────────────▼───────────────┐   ┌──────────▼────────────────┐
│ 浏览器网关（18766）          │   │ 视觉模型 Provider          │
│  managed Chrome（CDP）/      │   │ （任意 OpenAI 兼容的       │
│  chromeUse 共享标签扩展      │   │  图片模型）                 │
└──────────────────────────────┘   └───────────────────────────┘
```

## 快速开始

**方式 A — Lite 插件包（推荐，几 MB）。** 已经装有 DeepSeek Harness？下载 **Lite** 版 Release 资产（`DSHPlusPlus-lite-*.zip`，约 30 KB）：只含五个插件包 + 一键安装器，装进你已有的 DSH Profile——**不携带 Node/DSH/MCA 运行时**。要求：Node.js 22+、pnpm、`dsh` 在 PATH 中。解压后运行「安装到已有DSH.cmd」（或 `node install.mjs`），用 `dsh --profile dshplusplus` 启动。完整参数见「使用说明.md」。

**方式 B — 自包含桌面版。** 下载 Release 资产 `DSHPlusPlus-0.1.0-dev.2-windows-x64.zip`（约 149 MB）解压即可。**不捆绑 DeepSeek Harness**——控制中心会从环境变量、PATH、npm/pnpm 全局目录及相邻的 `DeepSeekHarness` 源码仓库发现本地 DSH 并代为启动。自动发现失败时，点击「选择已有 DSH」，可选择仓库根目录或在「运行环境」中指定 `apps\cli`、`lib`、`bin.js`、`dsh.cmd`、`dsh.exe`。未安装 DSH 时控制中心照常可用，「获取 DSH」可打开官方安装指引。内置 Node、MCA 与 DSH++ 插件。DSH 数据存放于标准 dsh home（`~/.dsh`）；`.portable` 目录只放 DSH++ 自身的配置、日志与 Sidecar 数据。在控制中心按需配置视觉模型（多模态专家），点击**启动 DSH**，在 DSH 的 `设置 → 模型` 配置主模型，然后发送一张图片——它会被视觉模型描述并投影进对话。

## 作为插件安装到已有 DSH

DSH++ 同时以纯 Cordis 插件包形式分发。源码检出后：

```bash
pnpm install
pnpm pack:plugins            # 生成 tarball 到 .tmp/packs
pnpm exec tsx scripts/install-plugins.ts --dsh-cli <dsh-bin.js 路径>
```

或手动安装：

```bash
dsh plugin --profile dshplusplus add \
  dshplusplus-multimodal-*.tgz \
  dshplusplus-multimodal-llm-*.tgz \
  dshplusplus-multimodal-router-*.tgz \
  dshplusplus-tool-media-inspect-*.tgz \
  dshplusplus-bundle-plus-*.tgz
dsh --profile dshplusplus
```

## 能力矩阵

| 类别 | 工具 | 实现 | 数据是否出本机 |
|---|---|---|---|
| 网页读取 | `mca_read_content`（http/https） | MCA Playwright 渲染 + 提取 | 仅发往内容源 |
| 网页搜索 | `web_search` | DSH `ctx.web` 通道，结果带来源 | 搜索请求发往 Provider |
| 浏览器观察 | `browser_observe` / `browser_status` / `browser_list_tabs` | managed Chrome（CDP）或共享标签快照 | 否 |
| 浏览器操作 | `browser_open/click/type/press/close/evaluate/back/forward` | managed Chrome（CDP）或共享标签桥 | 否 |

## 安全与隐私

- 图片内容**仅在 ask-once 同意后**才发送给所配置的视觉模型（按会话，依据审批审计日志）。
- 电脑操作保持 MCA 的风险等级与确认策略；外部内容不允许扩展自身权限。
- API Key 使用 Windows DPAPI 加密；明文只出现在受托管子进程的环境变量中。
- 浏览器 `evaluate` 限定在页面作用域内；shared 模式下未经扩展桥不得跨标签页操作。

## 仓库结构

```
apps/desktop/           Tauri 控制中心（Rust + TS）
packages/
  multimodal/           Provider 注册表 + Observation 落库 + 文本投影
  multimodal-llm/       基于 ctx.llm 的视觉 Provider
  multimodal-router/    agent/pre-step 图片路由 + 审批
  tool-media-inspect/   显式 media_inspect 工具
  bundle-plus/          Profile Bundle（cordis patch）
  browser-gateway/      CDP 受管 Chrome + 共享标签桥（MCP）
  compatibility/        版本锁定与兼容性 doctor
scripts/                构建 / 打包 / 冒烟 / 签名 / 压缩 / 安装器
```

## 从源码构建

环境要求：Node.js 22+、pnpm、Rust（MSVC）、DSH 运行时包（见 `scripts/build-portable.ps1`）。

```bash
pnpm install
pnpm check                 # 类型检查 + 测试
pnpm desktop:build         # 构建 apps/desktop/src-tauri/target/release/DSHPlusPlus.exe
powershell -File scripts/build-portable.ps1   # 组装便携发布目录
```

## 许可证

MIT

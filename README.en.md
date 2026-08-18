# DeepSeek Harness PlusPlus (DSH++)

> [中文](README.md) · English

An enhancement layer for [DeepSeek Harness](https://github.com/deepseek-ai/DeepSeekHarness) that adds **multimodal vision, web reading/search, and browser control** without modifying or injecting into the DSH core. It builds on DSH's profile/bundle/plugin/MCP interfaces and external sidecars, so upgrade and failure boundaries stay clean.

> Status: `0.1.0-dev.2` · Windows x64 · works with DSH `0.1.0-rc.6`

> **Repository scope**: this repository contains **only the DSH++ layer**. The DeepSeek Harness core is an upstream dependency (`@deepseek-ai/dsh`, published by DeepSeek) fetched from the npm registry at install time — DSH code, its dependency trees, or bundled copies are never part of this repository or the release archives. GitHub Releases publish DSH++ artifacts only: the Lite plugin pack (~26 KB) and the self-contained desktop build (~149 MB, control center + Node + MCA + DSH++ plugins, **no DSH inside** — the exe discovers a local DSH and guides installation when missing). See [compatibility.json](runtime/manifests/compatibility.json) for the supported upstream version.

## Features

- **Image support for text-only primary models** — images are projected to text observations by an external vision model before they reach the DeepSeek API (no image bytes ever go to DeepSeek).
- **Outbound-image consent** (`ask-once`) — the first time a session sends an image to a vision provider, the user is asked through DSH's approval channel; grants are remembered from the durable audit log.
- **MCA capability layer** — image / video / audio / document / web / computer.observe / computer.act tools, each individually toggleable; desktop provider auto-enabled when computer capabilities are turned on.
- **Browser control (two routes)**:
  - `managed` — CDP-controlled standalone Chrome (isolated profile, lazy launch).
  - `shared` — your own logged-in Chrome via a small MV3 extension (chromeUse), with a native-messaging bridge.
- **Web reading & search** — MCA page collection (Playwright) plus DSH's `web_search` seam with source-linked results.
- **System proxy passthrough** — the MCA sidecar (httpx / yt-dlp) follows the Windows system proxy automatically.
- **Desktop control center (Tauri)** — start/stop DSH & sidecars, capability toggles with live provider health, embedded DSH window, system tray lifecycle, single-instance guard, update checker and rollback helper.
- **DSH data stays with DSH** — DSH++ uses the standard dsh home (`~/.dsh`) by default; sessions, settings and workspaces are shared with any DSH you already run, and survive uninstalling DSH++.

## Architecture

```
┌──────────────────────────────────────────────────────────────┐
│ DSHPlusPlus.exe (Tauri control center)                       │
│  · manages DSH / MCA / browser-gateway lifecycles            │
│  · discovers the local DSH (env / PATH / npm global /        │
│    user-configured CLI); no DSH bundled                      │
│  · materializes the `dshplusplus` profile under ~/.dsh       │
└──────────────┬───────────────────────────────┬───────────────┘
               │ spawn local dsh               │ spawn
┌──────────────▼───────────────┐   ┌──────────▼────────────────┐
│ DeepSeek Harness (local)     │   │ MCA sidecar (18767)       │
│  · @dshplusplus/multimodal   │   │  image/video/audio/doc/   │
│  · multimodal-router         │   │  web/computer providers   │
│  · tool-media-inspect        │   │  (system-proxy aware)     │
│  · dsh-mcp-client → MCA      │   └──────────┬────────────────┘
└──────────────┬───────────────┘              │ MCP
               │ MCP                          │
┌──────────────▼───────────────┐   ┌──────────▼────────────────┐
│ Browser gateway (18766)      │   │ vision-gateway provider   │
│  managed Chrome (CDP) /      │   │ (any OpenAI-compatible    │
│  chromeUse shared extension  │   │  image model)             │
└──────────────────────────────┘   └───────────────────────────┘
```

## Quick start

**Option A — Lite plugin pack (recommended, a few MB).** Already have DeepSeek Harness installed? Download the **Lite** release asset (`DSHPlusPlus-lite-*.zip`, ~30 KB): it contains only the five plugin packages plus a one-click installer that targets your existing DSH profile — no bundled Node/DSH/MCA runtime. Requirements: Node.js 22+, pnpm, and `dsh` in PATH. Unzip, then run `安装到已有DSH.cmd` (or `node install.mjs`) and start with `dsh --profile dshplusplus`. Full CLI options are in `使用说明.md`.

**Option B — self-contained desktop build.** Download the `DSHPlusPlus-0.1.0-dev.2-windows-x64.zip` release asset (~149 MB) and extract anywhere. **DeepSeek Harness itself is not bundled** — the control center discovers local installations through the environment, PATH, npm/pnpm global locations, and nearby `DeepSeekHarness` source checkouts. If discovery fails, **选择已有 DSH** accepts the repository root, `apps\cli`, `lib`, `bin.js`, `dsh.cmd`, or `dsh.exe`; **获取 DSH** still opens the official install guide when DSH is absent. Node, MCA and the DSH++ plugins are bundled. DSH data lives in `~/.dsh`; the `.portable` folder only holds DSH++'s own config, logs and sidecar data. Configure an optional vision model (multimodal expert) in the control center, click **启动 DSH**, configure the primary model in DSH (`设置 → 模型`), then send an image — it is described by the vision model and projected into the conversation.

## Install as a plugin into an existing DSH

DSH++ is also distributed as plain Cordis plugin packages. From a source checkout:

```bash
pnpm install
pnpm pack:plugins            # builds tarballs into .tmp/packs
pnpm exec tsx scripts/install-plugins.ts --dsh-cli <path-to-dsh-bin.js>
```

or manually:

```bash
dsh plugin --profile dshplusplus add \
  dshplusplus-multimodal-*.tgz \
  dshplusplus-multimodal-llm-*.tgz \
  dshplusplus-multimodal-router-*.tgz \
  dshplusplus-tool-media-inspect-*.tgz \
  dshplusplus-bundle-plus-*.tgz
dsh --profile dshplusplus
```

## Capability matrix

| Category | Tool | Implementation | Data leaves the machine? |
|---|---|---|---|
| Web reading | `mca_read_content` (http/https) | MCA Playwright rendering + extraction | only to the content source |
| Web search | `web_search` | DSH `ctx.web` seam, source-linked results | search request to provider |
| Browser observe | `browser_observe` / `browser_status` / `browser_list_tabs` | managed Chrome (CDP) or shared-tab snapshot | no |
| Browser operate | `browser_open/click/type/press/close/evaluate/back/forward` | managed Chrome (CDP) or shared-tab bridge | no |

## Security & privacy

- Image content is sent to the configured vision provider **only after** `ask-once` consent (per session, from the approval audit log).
- Computer actions keep MCA's risk-level and confirmation policies; external content cannot expand its own authority.
- API keys are encrypted with Windows DPAPI; plaintext exists only in managed child-process environments.
- Browser `evaluate` is scoped to the page and never crosses to other tabs in shared mode without the extension bridge.

## Repository layout

```
apps/desktop/           Tauri control center (Rust + TS)
packages/
  multimodal/           provider registry + observation store + projection
  multimodal-llm/       ctx.llm-based vision provider
  multimodal-router/    agent/pre-step image routing + approval
  tool-media-inspect/   explicit media_inspect tool
  bundle-plus/          profile bundle (cordis patch)
  browser-gateway/      CDP managed Chrome + shared-tab bridge (MCP)
  compatibility/        version pinning & doctor
scripts/                build / pack / smoke / sign / compress / installer
```

## Building from source

Requirements: Node.js 22+, pnpm, Rust (MSVC), and the DSH runtime packages (see `scripts/build-portable.ps1`).

```bash
pnpm install
pnpm check                 # typecheck + tests
pnpm desktop:build         # builds apps/desktop/src-tauri/target/release/DSHPlusPlus.exe
powershell -File scripts/build-portable.ps1   # assembles the portable release
```

## License

MIT

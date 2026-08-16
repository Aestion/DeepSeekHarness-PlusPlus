# DeepSeek Harness PlusPlus (DSH++)

> [中文](README.zh-CN.md) · English

An enhancement layer for [DeepSeek Harness](https://github.com/deepseek-ai/DeepSeekHarness) that adds **multimodal vision, web reading/search, and browser control** without modifying or injecting into the DSH core. It builds on DSH's profile/bundle/plugin/MCP interfaces and external sidecars, so upgrade and failure boundaries stay clean.

> Status: `0.1.0-dev.1` · Windows x64 · works with DSH `0.1.0-rc.6`

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
│  · materializes the `dshplusplus` profile under ~/.dsh       │
└──────────────┬───────────────────────────────┬───────────────┘
               │ spawn                        │
┌──────────────▼───────────────┐   ┌──────────▼────────────────┐
│ DeepSeek Harness (bundled)   │   │ MCA sidecar (18765)       │
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

## Quick start (portable build)

1. Download the latest release archive and extract it anywhere.
2. Double-click `DSHPlusPlus.exe`, configure an optional vision model (multimodal expert), then click **启动 DSH**.
3. Configure the primary DeepSeek model in DSH (`设置 → 模型`).
4. Send an image — it is described by the vision model and projected into the conversation.

> DSH data lives in the standard dsh home (`~/.dsh`); the `.portable` folder only holds DSH++'s own config, logs and sidecar data.

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

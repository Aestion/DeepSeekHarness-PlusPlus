# DSHPlusPlus Portable

1. Keep the extracted directory structure unchanged and double-click `DSHPlusPlus.exe`.
2. Optionally configure the multimodal API in DSH++, then save.
3. Click `启动 DSH`; when ready, click `打开 DSH`.
4. Configure the primary model and API key in DSH under `设置 → 模型`.

**DeepSeek Harness itself is NOT bundled.** The control center discovers a local DSH installation through `DSHPLUSPLUS_DSH_CLI`, PATH, npm/pnpm global locations, and nearby `DeepSeekHarness` source checkouts. If automatic discovery fails, click `选择已有 DSH` and select the repository or installation directory; the control center resolves common layouts down to the actual CLI. If DSH is not installed yet, `获取 DSH` opens the official install guide (`npm install -g @deepseek-ai/dsh`, or the official installer). The DSH++ plugins ship inside this package (`plugins/@dshplusplus`) and are materialized into the dsh home profile on first start.

Node.js (bundled), MCA (bundled), and Python are included — no separate install needed. **DSH data (sessions, settings, workspaces) uses dsh's standard home (`~/.dsh`) and is shared with any dsh you installed yourself — uninstalling or updating DSH++ never touches it.** On first start after upgrading, sessions that were previously kept in the sibling `.portable` directory are merged into the standard home automatically (idempotent). DSH++ does not inject or overwrite DSH's primary-model settings. Multimodal API keys managed by DSH++ are encrypted with Windows DPAPI for the current Windows user and must be entered again after moving the directory to another computer.

To keep DSH data inside this package instead (fully portable / bring-your-own-data), set the environment variable `DSHPLUSPLUS_PORTABLE=1` before launching, or point `DSHPLUSPLUS_DSH_HOME` at a directory of your choice.

MCA supplies image, document, audio/video, web, and controlled-browser MCP tools by default. Computer actions still require confirmation and external content is not permitted to expand its own authority.

**Computer control (观察电脑 / 操作电脑).** MCA discovers the DSH agent through a `deepseek` command on its PATH. DSH++ generates `deepseek.cmd` / `dsh.cmd` shims under `.portable\agent-shims\` on every start and prepends that directory to the MCA process PATH, so detection and the capability route (including `computer.observe` / `computer.act`) work out of the box. The shims inject the default profile `dshplusplus` for bare invocations and emulate the `mcp list/add` probe MCA uses to register itself; the real MCP registration is written by the control center into the profile patch layers. If you move the package or relocate the DSH installation, simply restart via `启动 DSH` and the shims are regenerated.

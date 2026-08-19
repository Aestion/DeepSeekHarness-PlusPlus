# @dshplusplus/browser-gateway

DSH++ 浏览器操作网关（`browser` / `chromeUse` 能力）：

- `browser`：CDP 受管独立 Chrome（专用 user-data-dir，不碰日常浏览器）。
- `chromeUse`：Chrome 扩展 + Native Messaging 控制用户已打开的 Chrome（共享标签，复用登录态），机制参考 Codex CLI。

## 组件

```text
src/
  index.ts    CLI 入口（--host --port --data）
  cdp.ts      最小 CDP 客户端（Node 内置 WebSocket，零依赖）
  chrome.ts   受管 Chrome 查找/启动/页面会话
  mcp.ts      Streamable-HTTP MCP Server（工具面）
  shared.ts   共享标签桥（HTTP long-poll，零依赖）
extension/    Chrome 扩展（MV3，固定 key/ID）
native-host/  Native Messaging host（Node，零依赖）
```

## 运行

```powershell
node lib/index.js --host 127.0.0.1 --port 18766 --data <数据根>
```

- MCP 端点：`POST http://127.0.0.1:18766/mcp`
- 健康检查：`GET http://127.0.0.1:18766/api/health`（含 `shared.connected`：共享桥是否在线）
- 扩展桥：`GET /ext/poll`、`POST /ext/response`

## 工具

`browser_open`、`browser_observe`、`browser_click`、`browser_type`、`browser_press`、`browser_back`、`browser_forward`、`browser_list_tabs`、`browser_close`、`browser_status`、`browser_evaluate`。所有工具支持 `mode: managed | shared`（back/forward 仅受管模式，CDP 历史导航）。

## Chrome 扩展上架（Chrome Web Store）

商店提交材料与步骤见 `STORE_SUBMISSION.md`（隐私文案、权限清单、上架后 ExtensionInstallForcelist 静默安装）。

## Chrome 扩展安装（chromeUse）

1. 把 `extension/` 目录复制为用户数据根下的 `browser-extension/`（桌面端自动完成）。
2. 生成 Native Messaging host 注册：
   - `host-manifest.json`（`allowed_origins` 为固定扩展 ID `kikoigbglcakhdeknllbinnaepdaoofh`）
   - 注册表 `HKCU\Software\Google\Chrome\NativeMessagingHosts\com.dshplusplus.browser`
3. Chrome 以 `--load-extension=<browser-extension 目录>` 启动（或开发者模式加载）。
4. 已打开的页面按 F5 刷新后即可操作。

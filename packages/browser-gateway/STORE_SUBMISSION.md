# chromeUse 扩展上架材料（Chrome Web Store）

本目录说明如何把 DSH++ 的 Chrome 扩展（MV3）提交到 Chrome Web Store，实现真正全静默安装（ExtensionInstallForcelist 需要上架版本）。

## 扩展位置

- 源码/打包物：`packages/browser-gateway/extension/`
- 固定扩展 ID：`kikoigbglcakhdeknllbinnaepdaoofh`（清单 `key` 决定；上架后 ID 以商店分配的为准，如需保持 ID 需在开发者后台声明）

## 提交前检查清单

1. **打包 ZIP**：`extension/` 目录直接压缩（包含 `manifest.json` 在 zip 根）。
2. **manifest 核对**：
   - `manifest_version: 3`
   - 权限最小化：`nativeMessaging`（与 Native Messaging Host 桥接）、`activeTab`（点击扩展图标时）、`scripting`（内容脚本）
   - `background.service_worker` 已配置 alarms 保活（30s alarm 唤醒重连，桥断自愈）
   - `content_scripts` 与 `chrome.debugger` 的 shared `browser_evaluate` 逻辑保持同步
3. **隐私政策 URL**：商店要求公开隐私政策。建议内容：
   - 扩展仅在本机浏览器与 DSH++ 本地网关（127.0.0.1:18766）之间转发标签页快照/操作指令；
   - 不收集、不上传任何个人数据；页面内容仅在用户发起 DSH++ 会话操作时被读取；
   - 通过 Native Messaging Host 与本地网关通信，通信内容不出本机；
   - 无第三方分析、无广告、无跟踪。
4. **商店描述（建议文案）**：
   - 标题：DSH++ Chrome Bridge
   - 摘要：DSH++ 浏览器能力桥：让 DeepSeek Harness 在你自己已登录的 Chrome 标签页中执行观察与操作。
   - 详细描述：列出 shared 模式工具（open/observe/click/type/press/list_tabs/close/evaluate）、与 DSH++ 本地网关的桥接机制、隐私承诺。
5. **截图**：扩展弹窗（如有）与功能演示截图各 1-2 张，1280×800 或 640×400。
6. **开发者账号**：注册 Chrome Web Store 开发者（一次性 $5 注册费），需 Google 账号。

## 上架后

- 发布到公开渠道后，可在受管环境用 `ExtensionInstallForcelist` 策略实现全静默安装（无需 `--load-extension` 的一次性手动加载）。
- 保持 `native-host-launcher.exe` 与扩展的版本兼容矩阵；变更桥协议时同步升级两者。

## 当前状态

- 开发者模式加载：`install_chrome_extension`（控制中心一键部署 launcher + 注册表）可用；Chrome 137+ 首次需手动加载一次。
- 上架动作需要用户的 Google 开发者账号，属于人工步骤。

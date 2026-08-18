# MCA 电脑能力默认开启与实例隔离

日期:2026-08-18
状态:已批准(端口 18767;电脑两项默认开且启动时强制开启)

## 背景

- 「观察电脑 / 操作电脑」开关存在死锁:前端因 `computer_provider_enabled=false` 禁用开关
  (main.ts syncMcaAvailability),而 `enable_computer_provider` 后端命令依赖配置里已勾选
  `computer.*` 才会启用 Provider(lib.rs configure_mca_route),配置永远无法写入。
- DSH++ 自带 MCA 与独立的 MCA Control Center 互相影响:
  1. 共用端口 18765,`start_mca` 的"端口被占即复用"会误收编对方实例并推送配置;
  2. MCA 路由健康默认探测 `127.0.0.1:8766`(MCA 默认 agent-base-url),本机 8766 恰是
     MCA Control Center,DSH++ 的健康状态因此依赖对方进程存活。

已验证(2026-08-18,本机):POST `/api/providers/wheel.pyautogui-desktop/state`
`{"enabled":true}` 后,路由 `computer_provider_enabled` 立即翻转为 true,前端开关解锁。

## 决策

1. **默认开启**:`StoredConfig::default()` 中 `mca_computer_observe` / `mca_computer_act`
   改为 `true`。
2. **启动强制开启**:`load_config` 加载后把两个电脑开关归一化为 `true`(覆盖历史保存的
   false;范围仅电脑两项,其余五项能力仍可自由开关)。产品决策:开箱即用优先,
   安全性由 MCA 侧风险等级(low)+ 逐次确认(require_confirmation)兜底。
3. **解死锁**:提取 `enable_desktop_provider(agent) -> Result<(), String>`;
   `configure_mca_route` 保留条件调用,`enable_computer_provider` 无条件先调用。
   即使 MCA 实例上 Provider 被手动禁用,按钮也能解锁。
4. **端口隔离**:`MCA_PORT` 18765 → 18767(DSH++ 专属,紧邻 BROWSER_PORT 18766);
   lib.rs 中两处硬编码 "18765" 字符串改用常量派生。复用逻辑保留(应对自身崩溃残留)。
5. **健康自探测**:`mca-runtime serve` 参数追加
   `--agent-base-url http://127.0.0.1:{MCA_PORT}`,路由健康不再指向 8766 的外部 MCA。

## 测试策略(Rust 单测,位于 lib.rs `mod tests`)

1. 默认配置含 `mca_computer_observe` / `mca_computer_act` = true。
2. `load_config` 对保存值为 false 的配置强制归一化为 true。
3. `mca_serve_args` 包含 `--port 18767` 与 `--agent-base-url http://127.0.0.1:18767`。
4. `enable_desktop_provider` 向 `/api/providers/wheel.pyautogui-desktop/state` POST
   `{"enabled":true}`(本地 TcpListener 假 MCA 验证请求行与请求体)。

## 影响面

- DSH 注入 DSH++ 的 MCP URL 每次 `start_dsh` 重新生成,端口变更对升级用户无感。
- 与独立 MCA Control Center(8766)完全并行运行,互不探测、互不收编。

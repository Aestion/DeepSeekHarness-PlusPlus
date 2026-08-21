# Review Round #3 — 逐项修复计划

> 前置决策（用户确认）：
> - **M1** `/mcp` 保持不 gate；改为加固 gateway **拒绝绑定非回环地址**（`--host 0.0.0.0` 等）。
> - **H4** 桌面应用实测 Windows-only；非 Windows 的 base64「加密」分支改为**显式报错**，不再伪装加密。

## 包：browser-gateway（TS，vitest 有 seam）
| id | 问题 | 修法 | 测试 |
|----|------|------|------|
| M1 | `/mcp` 无鉴权可驱动真实浏览器 | `index.ts` 拒绝非回环 `--host`（127.0.0.1/localhost/::1 之外报错） | 单测 parseArgs 拒绝 0.0.0.0 |
| M2 | native-host 启动读一次 token，首装时序挂桥 | 空 token 时每轮/401 时重读文件 | native-host 逻辑小测 |
| C1 | `CdpSession.send` 无超时，挂死页面永久挂住 | 加 per-call timeout | 单测超时 reject |
| C2 | body>1MB `request.destroy()` 不响应，挂死 host | 改回 413 响应；host 的 `/ext/response` 加超时 | 单测 |
| C4 | `navigate` 不等页面加载 | await load 事件 | 单测（可选） |
| C5 | `pressKey` 不发 `text`，Enter 不触发 | 补 `text` | 单测 |
| C8 | `(async()=>{return(expr);})()` 破坏多语句 | 改为不包裹传给 Runtime.evaluate | 单测 |
| D1 | `withPage` 死代码 | 删除 | — |
| R1 | 截图无限累积 | 只保留最近 N 张 | — |

## 包：desktop Rust（lib.rs，#[cfg(test)]）
| id | 问题 | 修法 |
|----|------|------|
| H1 | `deepseek_secret` 存了不用 | `start_dsh` 注入 `DEEPSEEK_API_KEY` |
| H3 | `write_atomic` 非原子（remove→rename 窗口丢配置） | Windows 用 `MoveFileExW(MOVEFILE_REPLACE_EXISTING)`；换唯一临时名 |
| H4 | 非 Windows base64 明文等价 | 非 Windows 分支改为 `Err`（Windows-only） |
| M6 | `check_for_update` 暂存失败中断整命令 | 暂存失败软报错，保留组件状态 |
| M7 | Job Object 赋值失败→进程树泄漏 | 失败时回退 `taskkill /T /PID` |
| L2 | `load_config` 强制拉起 computer 开关 | 尊重保存值，规避死锁 |

## 包：multimodal
| id | 问题 | 修法 |
|----|------|------|
| H2 | provider 注册绑服务 fiber，重载残留/重复注册 | 绑调用方插件 fiber，插件销毁自动注销 |
| M3 | 并发 `ask-once` 发多次审批 | in-flight promise 守卫 |
| L5 | 截断切裂代理对 | 按 code point 边界截断 |
| L6 | 开头 marker 未转义 | 转义两处 marker |

## 包：compatibility / tool-media-inspect
| id | 问题 | 修法 |
|----|------|------|
| M4 | doctor manifest 路径发布后 ENOENT | 从包内 data 解析，非相对上溯 |
| M8 | `runDoctor` 吞 git 提交失败 | 保留 commit 失败原因 |
| M5 | `presentCall` 流式崩溃 | 守卫 `args.attachment` |

## 验证
- 各包 `tsc --noEmit` / `cargo check`
- 全工作区 `vitest run`

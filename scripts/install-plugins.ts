/**
 * DSH++ 插件一键安装器：把 @dshplusplus/* tarball 安装到用户已有 DSH 的
 * profile（默认 `dshplusplus`）。`dsh plugin` 会自动初始化 profile 并把
 * 声明 `dsh.bundle` 的依赖（bundle-plus）加入 bundles 层栈，无需手工改
 * manifest。
 *
 * 用法（tsx scripts/install-plugins.ts [选项]）：
 *   --dsh-cli <path>   DSH 仓库目录、bin.js 或可执行文件；默认按环境变量 →
 *                      PATH 中的 dsh → 相邻源码仓库 → 便携包顺序查找
 *   --home <path>      DSH_HOME（默认 $DSH_HOME 或 ~/.dsh）
 *   --profile <name>   目标 profile（默认 dshplusplus）
 *   --packs-dir <dir>  tarball 目录（默认：发布包内 packs/ 或 .tmp/packs）
 */

import { existsSync, readdirSync, statSync } from 'node:fs'
import { homedir } from 'node:os'
import { dirname, join, resolve } from 'node:path'
import { spawnSync } from 'node:child_process'

const REQUIRED_PACKAGES = [
  '@dshplusplus/multimodal',
  '@dshplusplus/multimodal-llm',
  '@dshplusplus/multimodal-router',
  '@dshplusplus/tool-media-inspect',
  '@dshplusplus/bundle-plus',
]

interface Options {
  dshCli?: string
  home?: string
  profile: string
  packsDir?: string
}

function parseArgs(argv: readonly string[]): Options {
  const options: Options = { profile: 'dshplusplus' }
  for (let index = 0; index < argv.length; index++) {
    const arg = argv[index]!
    const value = (): string => {
      const next = argv[index + 1]
      if (next === undefined) throw new Error(`${arg} 需要参数`)
      index += 1
      return next
    }
    switch (arg) {
      case '--dsh-cli': options.dshCli = value(); break
      case '--home': options.home = value(); break
      case '--profile': options.profile = value(); break
      case '--packs-dir': options.packsDir = value(); break
      default: throw new Error(`未知参数: ${arg}`)
    }
  }
  return options
}

interface DshLauncher {
  command: string
  prefix: string[]
  cli: string
}

function cliFileFromInput(input: string): string | undefined {
  const fullPath = resolve(input)
  const candidates = [
    fullPath,
    join(fullPath, 'apps', 'cli', 'lib', 'bin.js'),
    join(fullPath, 'lib', 'bin.js'),
    join(fullPath, 'node_modules', '@deepseek-ai', 'dsh', 'lib', 'bin.js'),
  ]
  return candidates.find((candidate) => existsSync(candidate) && statSync(candidate).isFile())
}

function launcherFor(cli: string): DshLauncher {
  if (/\.(?:c|m)?js$/i.test(cli)) return { command: process.execPath, prefix: [cli], cli }
  if (process.platform === 'win32' && /\.(?:cmd|bat)$/i.test(cli)) {
    return { command: process.env.ComSpec ?? 'cmd.exe', prefix: ['/d', '/s', '/c', cli], cli }
  }
  return { command: cli, prefix: [], cli }
}

function nearbySourceCli(): string | undefined {
  const starts = [resolve(import.meta.dirname), resolve(process.cwd())]
  const visited = new Set<string>()
  for (const start of starts) {
    let current = start
    while (!visited.has(current)) {
      visited.add(current)
      const candidates = [
        join(current, 'apps', 'cli', 'lib', 'bin.js'),
        join(current, 'DeepseekHarness', 'apps', 'cli', 'lib', 'bin.js'),
        join(current, 'DeepSeekHarness', 'apps', 'cli', 'lib', 'bin.js'),
        join(current, 'deepseek-harness', 'apps', 'cli', 'lib', 'bin.js'),
      ]
      const found = candidates.find((candidate) => existsSync(candidate))
      if (found !== undefined) return found
      const parent = dirname(current)
      if (parent === current) break
      current = parent
    }
  }
  return undefined
}

/** 定位 dsh CLI：显式路径 → 环境变量 → PATH → 相邻源码仓库 → 便携包。 */
function resolveDshCli(explicit?: string): DshLauncher {
  if (explicit !== undefined) {
    const cli = cliFileFromInput(explicit)
    if (cli === undefined) throw new Error(`DSH CLI 不存在: ${resolve(explicit)}`)
    return launcherFor(cli)
  }
  const fromEnv = process.env.DSHPLUSPLUS_DSH_CLI
  if (fromEnv !== undefined) {
    const cli = cliFileFromInput(fromEnv)
    if (cli !== undefined) return launcherFor(cli)
  }
  const fromPath = spawnSync(process.platform === 'win32' ? 'where' : 'which', ['dsh'], { encoding: 'utf8' })
  const pathHit = fromPath.status === 0 ? fromPath.stdout.split(/\r?\n/).find(Boolean) : undefined
  if (pathHit !== undefined) return launcherFor(resolve(pathHit))
  const sourceCli = nearbySourceCli()
  if (sourceCli !== undefined) return launcherFor(sourceCli)
  // 便携包：完整包布局 <root>/runtime/dsh + <root>/runtime/node 必须同时存在，
  // 否则视为未命中（避免向上误撞 workspace 根目录的 runtime/dsh 假阳性）。
  for (const root of [resolve(import.meta.dirname), resolve(import.meta.dirname, '..')]) {
    const portable = join(root, 'runtime', 'dsh', 'node_modules', '@deepseek-ai', 'dsh', 'lib', 'bin.js')
    const portableNode = join(root, 'runtime', 'node', process.platform === 'win32' ? 'node.exe' : 'node')
    if (existsSync(portable) && existsSync(portableNode)) {
      return { command: portableNode, prefix: [portable], cli: portable }
    }
  }
  throw new Error(
    '未找到 DSH CLI。请任选其一：\n' +
      '  1) 确保 dsh 在 PATH 中；\n' +
      '  2) 设置 DSHPLUSPLUS_DSH_CLI 指向 DSH 仓库或 bin.js；\n' +
      '  3) 通过 --dsh-cli <DSH 仓库或 CLI 路径> 显式指定。\n' +
      '（Lite 包面向“已有 DeepSeek Harness”的用户；若还没有 DSH，请先安装 DSH，或改用自包含完整包。）',
  )
}

function dshHome(explicit?: string): string {
  if (explicit !== undefined) return explicit
  if (process.env.DSH_HOME !== undefined && process.env.DSH_HOME !== '') return process.env.DSH_HOME
  return join(homedir(), '.dsh')
}

function findTarballs(packsDir: string): string[] {
  if (!existsSync(packsDir)) throw new Error(`tarball 目录不存在: ${packsDir}`)
  const files = readdirSync(packsDir)
  const tarballs: string[] = []
  for (const packageName of REQUIRED_PACKAGES) {
    const match = packageName.replace('@dshplusplus/', 'dshplusplus-')
    const found = files.find((file) => file.includes(match) && file.endsWith('.tgz'))
    if (found === undefined) throw new Error(`缺少 tarball: ${packageName}（${match}*.tgz 未找到于 ${packsDir}）`)
    tarballs.push(join(packsDir, found))
  }
  return tarballs
}

function run(): void {
  const options = parseArgs(process.argv.slice(2))
  const launcher = resolveDshCli(options.dshCli)
  const home = dshHome(options.home)

  // tarball 目录：--packs-dir → 发布包内 packs/ → .tmp/packs（绝对路径，
  // pnpm 在 profile 目录内运行，相对路径会被错误解析）。
  const packageRoot = resolve(import.meta.dirname)
  const workspaceRoot = resolve(import.meta.dirname, '..')
  const packsDir = resolve(options.packsDir
    ?? (existsSync(join(packageRoot, 'packs')) ? join(packageRoot, 'packs') : join(workspaceRoot, '.tmp', 'packs')))
  const tarballs = findTarballs(packsDir)

  console.log(`[install] DSH CLI: ${launcher.cli}`)
  console.log(`[install] DSH_HOME: ${home}`)
  console.log(`[install] profile: ${options.profile}`)
  console.log(`[install] tarballs: ${tarballs.map((t) => t.split(/[\\/]/).pop()).join(', ')}`)

  const result = spawnSync(launcher.command, [...launcher.prefix, 'plugin', '--profile', options.profile, 'add', '--save-prod', ...tarballs], {
    encoding: 'utf8',
    stdio: 'inherit',
    env: { ...process.env, DSH_HOME: home },
  })
  if (result.error !== undefined) throw result.error
  if (result.status !== 0) process.exit(result.status ?? 1)

  // 验证：dump-config 应包含 bundle-plus 层。
  const check = spawnSync(launcher.command, [...launcher.prefix, '--profile', options.profile, '--dump-config'], {
    encoding: 'utf8',
    env: { ...process.env, DSH_HOME: home },
  })
  const dumped = check.stdout ?? ''
  if (!dumped.includes('bundle-plus')) {
    console.warn('[install] 警告：dump-config 中未检测到 bundle-plus（请人工确认 profile 配置）')
  } else {
    console.log('[install] bundle-plus 已进入 profile 配置层 ✓')
  }
  const command = /\.(?:c|m)?js$/i.test(launcher.cli)
    ? `"${launcher.command}" "${launcher.cli}"`
    : `"${launcher.cli}"`
  console.log(`[install] 完成。启动方式：${command} --profile ${options.profile}`)
}

try {
  run()
} catch (error) {
  console.error(`[install] 失败: ${error instanceof Error ? error.message : String(error)}`)
  process.exit(1)
}

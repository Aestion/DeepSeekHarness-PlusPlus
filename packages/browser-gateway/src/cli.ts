/**
 * CLI option parsing for the browser gateway entry point.
 *
 * Kept in its own module (no import-time side effects) so the parsing rules
 * are unit-testable without starting the gateway — parsing/index.ts runs
 * main() on import.
 */

import { BROWSER_PORT } from './mcp.js'

export interface CliOptions {
  host: string
  port: number
  dataRoot: string
}

/** host is accepted; the loopback-only assertion is applied at the end of parseArgs. */
const LOOPBACK_HOSTS = new Set(['127.0.0.1', 'localhost', '::1'])

/**
 * The gateway exposes unauthenticated /mcp (consumed by DSH's MCP client) and
 * /ext/* — binding it to anything but loopback would expose browser control to
 * the local network. Refuse non-loopback hosts outright.
 */
export function assertLoopbackHost(host: string): void {
  if (!LOOPBACK_HOSTS.has(host)) {
    throw new Error(
      `网关只能绑定回环地址（127.0.0.1/localhost/::1），当前为 "${host}"；` +
        '为防止 /mcp 与 /ext/* 暴露给局域网，禁止绑定非回环地址',
    )
  }
}

export function parseArgs(argv: readonly string[]): CliOptions {
  const options: CliOptions = { host: '127.0.0.1', port: BROWSER_PORT, dataRoot: process.cwd() }
  for (let index = 0; index < argv.length; index += 1) {
    const flag = argv[index]
    const value = argv[index + 1]
    // 不要把跟在后面的 flag 当值（--host --port 9000 → host 保持默认）。
    const next = value !== undefined && !value.startsWith('--') ? value : undefined
    if (flag === '--host') options.host = next ?? options.host
    if (flag === '--data') options.dataRoot = next ?? options.dataRoot
    if (flag === '--port') {
      const parsed = Number.parseInt(next ?? '', 10)
      if (Number.isInteger(parsed) && parsed > 0 && parsed <= 65535) options.port = parsed
    }
  }
  assertLoopbackHost(options.host)
  return options
}

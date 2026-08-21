#!/usr/bin/env node
/**
 * DSH++ Browser Gateway entry point.
 *
 * Usage:
 *   node lib/index.js --host 127.0.0.1 --port 18766 --data <dataRoot>
 *
 * The gateway exposes:
 *   - POST /mcp        Streamable-HTTP MCP endpoint consumed by DSH's MCP client
 *   - GET  /api/health readiness probe used by the desktop app
 *   - GET  /ext/poll   long-poll channel for the Chrome native-messaging host
 *   - POST /ext/response replies from the native host
 *
 * The managed Chrome instance is spawned lazily on the first managed-mode
 * tool call and is terminated when the gateway exits.
 */

import { parseArgs } from './cli.js'
import { ManagedChrome } from './chrome.js'
import { BrowserMcpServer } from './mcp.js'
import { SharedTabBridge } from './shared.js'

async function main(): Promise<void> {
  const options = parseArgs(process.argv.slice(2))
  const chrome = new ManagedChrome(options.dataRoot)
  const shared = new SharedTabBridge()
  const server = new BrowserMcpServer(options.dataRoot, chrome, shared)

  const shutdown = async (): Promise<void> => {
    await chrome.stop()
    await server.stop()
    process.exit(0)
  }
  process.on('SIGINT', () => void shutdown())
  process.on('SIGTERM', () => void shutdown())
  process.on('exit', () => {
    void chrome.stop()
  })

  await server.start(options.host, options.port)
  console.log(
    JSON.stringify({
      status: 'ok',
      contract: 'dshplusplus.browser-gateway.v1',
      mcp: `http://${options.host}:${options.port}/mcp`,
      health: `http://${options.host}:${options.port}/api/health`,
      dataRoot: options.dataRoot,
    }),
  )
}

main().catch((error: unknown) => {
  console.error(String(error))
  process.exit(1)
})

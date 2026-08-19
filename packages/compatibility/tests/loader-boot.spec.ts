import { describe, expect, it } from 'vitest'
import { dirname, resolve } from 'node:path'
import { createRequire } from 'node:module'
import { fileURLToPath, pathToFileURL } from 'node:url'
import { boot } from '@deepseek-ai/dsh-app-boot'
import type {} from '@dshplusplus/multimodal'

describe('real DSH Loader composition', () => {
  it('loads the built multimodal service through a cordis.yml package row', async () => {
    const here = dirname(fileURLToPath(import.meta.url))
    const packageRoot = resolve(here, '..')
    const config = resolve(here, 'fixtures/cordis.yml')
    const ctx = await boot(
      'dshplusplus-loader-test',
      config,
      undefined,
      undefined,
      pathToFileURL(`${packageRoot}/`).href,
    )
    try {
      expect(ctx.multimodal.listProviders()).toEqual([])
    } finally {
      await ctx.fiber.dispose()
    }
  })

  it('imports the MCP client against an attachment instance exporting isImageAdmissionError', async () => {
    // dsh-mcp-client@rc.7 imports { isImageAdmissionError } from @deepseek-ai/dsh-attachment
    // at module top level. A mixed rc.6/rc.7 pnpm tree resolved rc.6 (which does not export
    // the symbol), so the cordis include failed at import time and the DSH service would not
    // boot. Guard both the module import and the attachment instance the client actually sees.
    const here = dirname(fileURLToPath(import.meta.url))
    const packageRoot = resolve(here, '..')
    const mcp = await import('@deepseek-ai/dsh-mcp-client')
    expect(mcp).toBeDefined()
    const req = createRequire(packageRoot)
    const mcpPath = req.resolve('@deepseek-ai/dsh-mcp-client')
    const attachmentPath = createRequire(mcpPath).resolve('@deepseek-ai/dsh-attachment')
    const attachment = await import(pathToFileURL(attachmentPath).href)
    expect(typeof attachment.isImageAdmissionError).toBe('function')
  })
})

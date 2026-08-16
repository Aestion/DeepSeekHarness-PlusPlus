import { describe, expect, it } from 'vitest'
import { dirname, resolve } from 'node:path'
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
})

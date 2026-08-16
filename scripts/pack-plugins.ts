/** Build and pack the M0 plugin transaction into `.tmp/packs`. */

import { mkdirSync } from 'node:fs'
import { resolve } from 'node:path'
import { spawnSync } from 'node:child_process'

const root = resolve(import.meta.dirname, '..')
const destination = resolve(root, '.tmp', 'packs')
const pnpmEntry = process.env.npm_execpath
const packages = [
  '@dshplusplus/multimodal',
  '@dshplusplus/multimodal-llm',
  '@dshplusplus/multimodal-router',
  '@dshplusplus/tool-media-inspect',
  '@dshplusplus/bundle-plus',
]

function run(args: readonly string[]): void {
  if (pnpmEntry === undefined) throw new Error('pack-plugins must run through pnpm')
  const result = spawnSync(process.execPath, [pnpmEntry, ...args], {
    cwd: root,
    stdio: 'inherit',
    shell: false,
  })
  if (result.error !== undefined) throw result.error
  if (result.status !== 0) process.exit(result.status ?? 1)
}

mkdirSync(destination, { recursive: true })
run(['run', 'build'])
for (const pkg of packages) {
  run(['--filter', pkg, 'pack', '--pack-destination', destination])
}

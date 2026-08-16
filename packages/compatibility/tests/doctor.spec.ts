import { describe, expect, it } from 'vitest'
import { fileURLToPath } from 'node:url'
import { loadManifest, runDoctor, supportsNode } from '../src/index.ts'

describe('compatibility doctor', () => {
  it('implements the pinned DSH Node engine range', () => {
    expect(supportsNode('22.18.0')).toBe(false)
    expect(supportsNode('22.19.0')).toBe(true)
    expect(supportsNode('23.9.0')).toBe(false)
    expect(supportsNode('24.0.0')).toBe(true)
    expect(supportsNode('25.9.0')).toBe(true)
  })

  it('loads the repository manifest and reports the current Node runtime', () => {
    const path = fileURLToPath(new URL('../../../runtime/manifests/compatibility.json', import.meta.url))
    const report = runDoctor(loadManifest(path))

    expect(report.manifest.schemaVersion).toBe(1)
    expect(report.checks.map(check => check.id)).toEqual(['node'])
    expect(report.checks[0]?.actual).toBe(process.versions.node)
  })
})


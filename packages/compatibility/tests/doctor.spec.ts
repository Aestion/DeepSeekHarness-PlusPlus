import { describe, expect, it } from 'vitest'
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
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

  it('loads the bundled manifest and reports the current Node runtime', () => {
    // 从包内 data 解析，而非相对上溯到工作区根，保证发布后 doctor 仍能找到 manifest（M4）。
    const path = fileURLToPath(new URL('../data/compatibility.json', import.meta.url))
    const report = runDoctor(loadManifest(path))

    expect(report.manifest.schemaVersion).toBe(1)
    expect(report.checks.map(check => check.id)).toEqual(['node'])
    expect(report.checks[0]?.actual).toBe(process.versions.node)
  })

  it('surfaces the git commit failure instead of swallowing it into a generic check', () => {
    const path = fileURLToPath(new URL('../data/compatibility.json', import.meta.url))
    const manifest = loadManifest(path)
    // 建一个"有 version 但绝不是 git 仓库"的目录：版本检查应 PASS，commit 检查应 FAIL 并附细节。
    const root = mkdtempSync(join(tmpdir(), 'dshplusplus-doctor-'))
    writeFileSync(join(root, 'package.json'), JSON.stringify({ version: manifest.deepseekHarness.sourceVersion }))
    try {
      const report = runDoctor(manifest, root)
      const ids = report.checks.map(check => check.id)
      expect(ids).toContain('dsh-source-version')
      expect(ids).toContain('dsh-source-commit')
      const version = report.checks.find(check => check.id === 'dsh-source-version')
      const commit = report.checks.find(check => check.id === 'dsh-source-commit')
      expect(version?.ok).toBe(true)
      expect(commit?.ok).toBe(false)
      // 失败原因被保留在 actual 里，而不是被笼统的 'dsh-source' 检查藏起来。
      expect(commit?.actual).toContain('git')
      expect(report.ok).toBe(false)
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })
})


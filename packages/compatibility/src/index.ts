/** Runtime-manifest parsing and local DeepSeek Harness compatibility checks. */

import { execFileSync } from 'node:child_process'
import { readFileSync } from 'node:fs'
import { join } from 'node:path'

/** Pinned external versions and protocol revisions shipped by DSH++. */
export interface CompatibilityManifest {
  readonly schemaVersion: number
  readonly dshplusplusVersion: string
  readonly milestone: string
  readonly node: string
  readonly pnpm: string
  readonly deepseekHarness: {
    readonly repository: string
    readonly commit: string
    readonly sourceVersion: string
    readonly publishedPackageBaseline: string
  }
  readonly protocols: {
    readonly multimodalObservation: number
    readonly mcaSidecar: number
    readonly runtimeControl: number
  }
}

/** One doctor assertion with user-actionable details. */
export interface DoctorCheck {
  readonly id: string
  readonly ok: boolean
  readonly expected: string
  readonly actual: string
  readonly message: string
}

/** Complete doctor output. */
export interface DoctorReport {
  readonly ok: boolean
  readonly manifest: CompatibilityManifest
  readonly checks: readonly DoctorCheck[]
}

/**
 * Load and minimally validate one compatibility manifest.
 * @param path - Absolute or process-relative manifest path.
 * @returns Parsed manifest.
 */
export function loadManifest(path: string): CompatibilityManifest {
  const value = JSON.parse(readFileSync(path, 'utf8')) as Partial<CompatibilityManifest>
  if (value.schemaVersion !== 1
    || typeof value.dshplusplusVersion !== 'string'
    || typeof value.node !== 'string'
    || typeof value.deepseekHarness?.commit !== 'string'
    || typeof value.deepseekHarness.sourceVersion !== 'string') {
    throw new Error(`unsupported or malformed compatibility manifest: ${path}`)
  }
  return value as CompatibilityManifest
}

/**
 * Test the current Node version against the pinned DSH engine baseline.
 * @param version - Semver-like Node version without a leading `v`.
 * @returns Whether it satisfies `^22.19.0 || >=24.0.0`.
 */
export function supportsNode(version: string): boolean {
  const [major = Number.NaN, minor = Number.NaN] = version.split('.').map(Number)
  return (major === 22 && minor >= 19) || major >= 24
}

function readJson(path: string): Record<string, unknown> {
  return JSON.parse(readFileSync(path, 'utf8')) as Record<string, unknown>
}

function gitHead(root: string): string {
  return execFileSync('git', ['-C', root, 'rev-parse', 'HEAD'], {
    encoding: 'utf8',
    stdio: ['ignore', 'pipe', 'pipe'],
  }).trim()
}

/**
 * Inspect the current Node runtime and an optional DSH source checkout.
 * @param manifest - Pinned compatibility policy.
 * @param dshRoot - Optional DeepSeek Harness repository root.
 * @returns Ordered checks and aggregate readiness.
 */
export function runDoctor(manifest: CompatibilityManifest, dshRoot?: string): DoctorReport {
  const checks: DoctorCheck[] = []
  checks.push({
    id: 'node',
    ok: supportsNode(process.versions.node),
    expected: manifest.node,
    actual: process.versions.node,
    message: supportsNode(process.versions.node)
      ? 'Node runtime satisfies the pinned DSH engine range.'
      : 'Install Node 22.19+ or 24+ for development; the portable product will carry its own runtime.',
  })
  if (dshRoot !== undefined) {
    try {
      const packageJson = readJson(join(dshRoot, 'package.json'))
      const actualVersion = typeof packageJson.version === 'string' ? packageJson.version : 'missing'
      checks.push({
        id: 'dsh-source-version',
        ok: actualVersion === manifest.deepseekHarness.sourceVersion,
        expected: manifest.deepseekHarness.sourceVersion,
        actual: actualVersion,
        message: actualVersion === manifest.deepseekHarness.sourceVersion
          ? 'DSH source version matches the pinned baseline.'
          : 'DSH source version differs; update the compatibility manifest only after regression tests pass.',
      })
      const actualCommit = gitHead(dshRoot)
      checks.push({
        id: 'dsh-source-commit',
        ok: actualCommit === manifest.deepseekHarness.commit,
        expected: manifest.deepseekHarness.commit,
        actual: actualCommit,
        message: actualCommit === manifest.deepseekHarness.commit
          ? 'DSH source commit matches the pinned baseline.'
          : 'DSH source commit differs; run the compatibility suite before loading Plus++.',
      })
    } catch (error: unknown) {
      checks.push({
        id: 'dsh-source',
        ok: false,
        expected: 'readable DSH repository',
        actual: error instanceof Error ? error.message : String(error),
        message: 'The supplied DSH root could not be inspected.',
      })
    }
  }
  return {
    ok: checks.every(check => check.ok),
    manifest,
    checks,
  }
}


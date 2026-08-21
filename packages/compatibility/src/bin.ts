#!/usr/bin/env node

import { fileURLToPath } from 'node:url'
import { loadManifest, runDoctor } from './index.ts'

function valueAfter(args: readonly string[], option: string): string | undefined {
  const index = args.indexOf(option)
  return index === -1 ? undefined : args[index + 1]
}

const args = process.argv.slice(2)
const manifestPath = fileURLToPath(new URL('../data/compatibility.json', import.meta.url))
const report = runDoctor(loadManifest(manifestPath), valueAfter(args, '--dsh-root'))

if (args.includes('--json')) {
  process.stdout.write(`${JSON.stringify(report, null, 2)}\n`)
} else {
  process.stdout.write(`DSH++ doctor ${report.manifest.dshplusplusVersion} (${report.manifest.milestone})\n`)
  for (const check of report.checks) {
    process.stdout.write(`${check.ok ? 'PASS' : 'FAIL'} ${check.id}: ${check.message}\n`)
    if (!check.ok) process.stdout.write(`     expected ${check.expected}; actual ${check.actual}\n`)
  }
}

if (!report.ok) process.exitCode = 1


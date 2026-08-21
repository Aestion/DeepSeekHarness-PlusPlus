import { describe, expect, it } from 'vitest'
import { createSingleFlightRefresh, pollingInterval } from '../src/polling.js'

describe('desktop status polling', () => {
  it('does not overlap refresh requests', async () => {
    let release!: () => void
    const gate = new Promise<void>((resolve) => { release = resolve })
    let calls = 0
    const refresh = createSingleFlightRefresh(async () => {
      calls += 1
      await gate
    })

    const first = refresh()
    await Promise.resolve()
    await expect(refresh()).resolves.toBe(false)
    expect(calls).toBe(1)

    release()
    await expect(first).resolves.toBe(true)
    await expect(refresh()).resolves.toBe(true)
    expect(calls).toBe(2)
  })

  it('polls quickly while a service is starting and slowly when stable', () => {
    expect(pollingInterval(['running', 'starting', 'stopped'])).toBe(1_500)
    expect(pollingInterval(['running', 'running', 'stopped'])).toBe(5_000)
    expect(pollingInterval(['running', 'error', 'stopped'])).toBe(5_000)
  })
})

export type PollingServiceState = 'stopped' | 'starting' | 'running' | 'error'

export function createSingleFlightRefresh<Args extends unknown[]>(
  run: (...args: Args) => Promise<void>,
): (...args: Args) => Promise<boolean> {
  let inFlight = false
  return async (...args: Args) => {
    if (inFlight) return false
    inFlight = true
    try {
      await run(...args)
      return true
    } finally {
      inFlight = false
    }
  }
}

export function pollingInterval(states: readonly PollingServiceState[]): number {
  return states.includes('starting') ? 1_500 : 5_000
}

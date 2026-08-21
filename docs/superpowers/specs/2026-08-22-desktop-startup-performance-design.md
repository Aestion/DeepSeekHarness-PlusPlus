# Desktop Startup Performance Design

## Goal

Make the DSH page normally usable within 5–8 seconds while preserving automatic startup of MCA and the browser gateway. A slow or failed auxiliary service must not delay DSH or prevent its page from opening.

## Current behavior

The desktop process starts MCA, performs agent-route detection, starts the browser gateway, and only then starts DSH. After DSH becomes reachable it migrates every session before opening the DSH window. The frontend also requests a full backend snapshot every 1.5 seconds without preventing overlapping refreshes.

Measured on the current Windows release build:

- MCA ready: about 8.2 seconds.
- Browser gateway ready: about 24.6 seconds.
- DSH ready with the full startup path: about 28.5 seconds.
- DSH ready when auxiliary services are disabled: about 4.75 seconds.

## Design

### Startup coordination

Start DSH, MCA, and the browser gateway as independent background tasks. Each service keeps its existing start guard, status, log file, and timeout. No auxiliary task may gate DSH startup.

When DSH becomes reachable, open the DSH window immediately when auto-open is enabled. Run session migration afterward as a background maintenance task. Opening the window uses a bounded retry policy and logs the final error instead of silently discarding it.

MCA route detection and provider configuration run only after MCA is reachable and outside the critical path. The operation is single-flight within one desktop process so concurrent startup or refresh actions cannot trigger duplicate detection.

### Snapshot polling

The frontend permits only one snapshot request at a time. While a service is starting it refreshes every 1.5 seconds; once all enabled services reach a stable state it refreshes every 5 seconds.

The backend copies service state while holding mutexes and releases those mutexes before making HTTP requests or scanning browser profiles. MCA route and provider reads may execute without holding DSH or browser locks.

### Repeated startup work

Cache automatic DSH CLI discovery for the desktop process so repeated start actions do not rerun global npm discovery. Explicit configuration changes invalidate the cached result.

Materialize DSH configuration once during startup. The service start path reuses that result and only rematerializes when the relevant configuration changes.

## Error handling

- DSH failure is shown in DSH status but does not cancel MCA or browser startup.
- MCA or browser failure is shown independently and does not affect DSH opening.
- Auto-open retries transient readiness failures with bounded backoff and records the terminal failure.
- Background session migration and MCA detection failures are logged and remain retryable through the existing manual actions.

## Testing

Use test-driven development for each behavior change:

1. Unit tests for startup task independence, bounded open retry, and single-flight behavior.
2. Frontend tests for non-overlapping refreshes and stable/starting polling intervals.
3. Existing TypeScript and Rust checks.
4. Isolated release-style startup probe comparing DSH readiness with the previous 28.5-second baseline.

## Acceptance criteria

- DSH normally becomes reachable and its page opens in 5–8 seconds on the test machine.
- MCA and browser gateway still start automatically when enabled.
- A slow or failed MCA/browser startup cannot delay DSH readiness or auto-open.
- Snapshot refreshes never overlap.
- Startup errors are observable in status or logs rather than silently ignored.

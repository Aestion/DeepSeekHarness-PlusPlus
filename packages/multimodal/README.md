# `@dshplusplus/multimodal`

Service Definition for `ctx.multimodal`. It owns provider registration, deterministic selection, provider-neutral Observation fields, machine-routable errors, and the version-one text projection used by stock DeepSeek Harness session logs.

Providers register with `ctx.multimodal.registerProvider()`. With no configured id, execution succeeds only when exactly one provider is available. Duplicate, missing, unavailable, and ambiguous providers fail explicitly.

## Model Experience

The service itself adds no prompt or tools. Its consumer writes a bounded `[DSH++ Multimodal Observation v1]` text block into the existing `user/message`, so stock DSH can save, restore, fork, and compact the model-visible observation without a private session-event type.

## Observation Store

With a `storeRoot` configured (the control center materializes `$DSH_HOME/dshplusplus/observations`), every inspection is appended as one JSONL record per session (`ObservationStore`), including failures. Records carry `sessionId` / `messageId` / `observedAt` plus the full observation (evidence refs included). Query via `ctx.multimodal.observations(sessionId?)`. Store failures never fail the inspection.

## Known Limitations and Deferred Work

- M0 accepts DSH raster-image attachment references only. Generic documents, audio, and video require the upstream generic-blob attachment path or an explicit path/URL tool.
- The model-visible text projection remains the compatibility source of truth; the store is a durable side channel for evidence and debugging.


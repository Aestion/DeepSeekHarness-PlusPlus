# `@dshplusplus/multimodal`

Service Definition for `ctx.multimodal`. It owns provider registration, deterministic selection, provider-neutral Observation fields, machine-routable errors, and the version-one text projection used by stock DeepSeek Harness session logs.

Providers register with `ctx.multimodal.registerProvider()`. With no configured id, execution succeeds only when exactly one provider is available. Duplicate, missing, unavailable, and ambiguous providers fail explicitly.

## Model Experience

The service itself adds no prompt or tools. Its consumer writes a bounded `[DSH++ Multimodal Observation v1]` text block into the existing `user/message`, so stock DSH can save, restore, fork, and compact the model-visible observation without a private session-event type.

## Known Limitations and Deferred Work

- M0 accepts DSH raster-image attachment references only. Generic documents, audio, and video require the upstream generic-blob attachment path or an explicit path/URL tool.
- Structured Observation and Evidence persistence belongs to the future Observation Store package; the model-visible text projection remains the compatibility source of truth.


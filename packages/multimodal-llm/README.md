# `@dshplusplus/multimodal-llm`

Service Provider for `ctx.multimodal`. It sends one DSH image attachment to an existing image-capable `ctx.llm` route and returns a provider-neutral Observation. API keys, base URLs, adapters, and model catalogs remain owned by DeepSeek Harness.

```yaml
- id: dshplusplus-multimodal-llm
  name: @dshplusplus/multimodal-llm
  config:
    provider: custom-vision
    model: qwen-vl-max
    maxTokens: 1200
```

The provider rejects a route that explicitly omits image input. An adapter whose modality metadata is unknown may still receive the request; its normal DSH failure becomes a machine-routable multimodal error.

## Model Experience

Each inspection is an independent auxiliary LLM request containing a stable visual-analysis system instruction, the caller task, and one image. The response does not enter the primary model directly; `@dshplusplus/multimodal-router` bounds and records its text projection.

## Known Limitations and Deferred Work

- M0 delegates authorization to deployment configuration. Session-scoped `ask-once` outbound-data approval is required before M1.
- The provider processes one raster image per request. Region expansion and multiple-image joint reasoning are deferred.


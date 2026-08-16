# `@dshplusplus/multimodal-router`

Consumer for the DSH++ multimodal seam. It wraps `agent/pre-step`, waits for downstream listeners, and replaces image blocks only when the primary DSH route does not explicitly advertise image support. The rewritten message preserves its original id, source, user text, and block ordering.

The generated `[DSH++ Multimodal Observation v1]` block is a normal text block. It is therefore persisted as `user/message` by stock DSH before the primary model request, satisfying replay without an out-of-tree session-event type.

```yaml
- id: dshplusplus-multimodal-router
  name: @dshplusplus/multimodal-router
  config:
    enabled: true
    unknownModelPolicy: inspect
    maxProjectionChars: 6000
    externalInspectionApproval: ask-once
```

## Model Experience

For each unsupported image block, the model sees one bounded versioned observation at the image's original position. The block names the attachment, provider, model, status, and untrusted observation text. Native image routes see the original image unless `alwaysInspect` is enabled.

## Outbound Image Approval (`externalInspectionApproval`)

Sending user image content to an external vision model is a privacy-sensitive outbound action. The router supports an `ask-once` approval flow built on the DSH approval channel:

- `ask-once` (default): the first time a session sends an image out, a DSH approval is requested (`approval/asked` + `approval/decided` audit pair, tool identity `dshplusplus:vision-external`). A grant (`allowed-once`) is remembered from the durable session audit log, so later images in the same session are not re-asked — no extra persistence is needed. A denial degrades the image to a model-visible `[图片未外发…]` placeholder instead of sending it; the conversation continues.
- `off`: never asks (legacy behavior).
- A session whose approval policy is `never` bypasses the ask entirely; deployments without a mounted approval service pass through unchanged.

## Known Limitations and Deferred Work

- M0 records the stable text projection but has no structured Observation Store or Evidence UI.
- Unknown model capability defaults to inspection for safety; operators can select `pass` when an adapter accepts images but cannot report modality metadata.


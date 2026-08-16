# `@dshplusplus/bundle-plus`

DeepSeek Harness bundle patch for the DSH++ M0 multimodal seam. It mounts `ctx.multimodal` and declares disabled LLM Provider and Router rows that a profile patch can configure and enable.

The defaults intentionally perform no external image transfer. A profile owner must select an existing image-capable DSH route and enable both rows.

The profile installs the bundle and its three peer plugin packages together. The future installer owns that four-package transaction so users still perform one install action without duplicating implementation inside the bundle.

## Known Limitations and Deferred Work

- M0 contains multimodal packages only. Safe Fetch, isolated browser, Manager UI, and MCA Sidecar rows join the bundle in later stages.

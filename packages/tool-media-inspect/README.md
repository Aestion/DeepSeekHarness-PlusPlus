# `@dshplusplus/tool-media-inspect`

Explicit `media_inspect` tool for DSH++: the model asks for a multimodal observation of an image attachment by passing back the attachment reference it saw in the conversation plus a task. It runs the same `ctx.multimodal` seam as automatic routing, so observations are persisted to the configured store and presented as a structured card in the UI.

```yaml
- id: dshplusplus-media-inspect
  name: @dshplusplus/tool-media-inspect
```

The tool is a DSH tool (consumer) and is included in the `@dshplusplus/bundle-plus` profile bundle.

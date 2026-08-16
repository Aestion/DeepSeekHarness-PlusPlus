import type { ImageAttachmentRef } from '@deepseek-ai/dsh-attachment'
import type { ObservationId } from './brand.ts'

/** Lossless JSON accepted by the Sidecar and durable observation store. */
export type JsonValue = null | boolean | number | string | JsonValue[] | { [key: string]: JsonValue }

/** One content-addressed fact supporting an observation. */
export interface EvidenceRef {
  readonly kind: 'attachment' | 'screenshot' | 'region' | 'provider-response'
  readonly id: string
  readonly mediaType?: string
  readonly description?: string
}

/** Image input understood by the M0 multimodal seam. */
export interface ImageInspectSource {
  readonly kind: 'image'
  readonly attachment: ImageAttachmentRef
}

/** One request to turn external content into model-readable evidence. */
export interface InspectRequest {
  readonly source: ImageInspectSource
  readonly task: string
  readonly sessionId?: string
  readonly messageId?: string
}

/** Provider-neutral result returned to the router and tools. */
export interface Observation {
  readonly version: 1
  readonly id: ObservationId
  readonly providerId: string
  readonly model?: string
  readonly status: 'completed' | 'partial' | 'failed'
  readonly summary: string
  readonly text: string
  readonly stateToken?: string
  readonly validForState?: string
  readonly structured?: JsonValue
  readonly evidence: readonly EvidenceRef[]
}

/** Named implementation registered behind `ctx.multimodal`. */
export interface MultimodalProvider {
  readonly id: string
  available(): boolean
  inspect(request: InspectRequest, signal?: AbortSignal): Promise<Observation>
}


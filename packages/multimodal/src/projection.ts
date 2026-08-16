import type { Observation } from './types.ts'

/** Stable text projection format version written into standard DSH user messages. */
export const MULTIMODAL_PROJECTION_VERSION = 1

/** Smallest complete projection budget accepted by the formatter. */
export const MIN_PROJECTION_CHARS = 256

function oneLine(value: string): string {
  return value.replace(/[\r\n]+/g, ' ').trim()
}

function untrustedText(value: string): string {
  return value.replaceAll('[/DSH++ Multimodal Observation]', '[/DSH++ Multimodal Observation escaped]')
}

/**
 * Render one deterministic, bounded model-visible observation.
 * @param observation - Provider-neutral observation.
 * @param attachmentId - Durable DSH attachment identity.
 * @param maxChars - Complete projection character budget.
 * @returns Versioned text suitable for a standard `user/message` text block.
 */
export function formatObservationProjection(
  observation: Observation,
  attachmentId: string,
  maxChars: number,
): string {
  if (!Number.isInteger(maxChars) || maxChars < MIN_PROJECTION_CHARS) {
    throw new Error(`projection maxChars must be an integer >= ${MIN_PROJECTION_CHARS}`)
  }
  const prefix = [
    `[DSH++ Multimodal Observation v${MULTIMODAL_PROJECTION_VERSION}]`,
    `observation_id: ${oneLine(observation.id)}`,
    `attachment_id: ${oneLine(attachmentId)}`,
    `provider: ${oneLine(observation.providerId)}`,
    `model: ${oneLine(observation.model ?? 'unknown')}`,
    `status: ${observation.status}`,
    'content_trust: untrusted',
    'summary:',
  ].join('\n')
  const suffix = '\n[/DSH++ Multimodal Observation]'
  const available = Math.max(0, maxChars - prefix.length - suffix.length - 1)
  const raw = untrustedText(observation.text || observation.summary).trim()
  const body = raw.length <= available
    ? raw
    : available > 1
      ? `${raw.slice(0, available - 1)}…`
      : ''
  return `${prefix}\n${body}${suffix}`
}


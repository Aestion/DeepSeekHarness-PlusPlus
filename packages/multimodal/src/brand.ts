/** Opaque identity for one multimodal observation. */
export type ObservationId = string & { readonly __brand: 'ObservationId' }

/**
 * Validate and brand an observation identity received from a provider or store.
 * @param value - Candidate non-empty identity.
 * @returns The branded identity.
 */
export function ObservationId(value: string): ObservationId {
  if (value.length === 0) throw new Error('observation id must not be empty')
  return value as ObservationId
}


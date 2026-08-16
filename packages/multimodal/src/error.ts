/** Machine-routable multimodal failure. */
export class MultimodalError extends Error {
  /**
   * @param message - User-actionable failure summary.
   * @param code - Stable routing code.
   * @param options - Optional error cause.
   */
  constructor(
    message: string,
    readonly code: string,
    options?: ErrorOptions,
  ) {
    super(message, options)
    this.name = 'MultimodalError'
  }
}


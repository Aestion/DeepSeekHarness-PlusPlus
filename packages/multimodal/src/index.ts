/** Multimodal provider registry and observation projection for DSH++. */

import { Context, Service } from '@deepseek-ai/cordis'
import z from '@deepseek-ai/schemastery'
import { MultimodalError } from './error.ts'
import { ObservationStore } from './store.ts'
import type { StoredObservation } from './store.ts'
import type { InspectRequest, MultimodalProvider, Observation } from './types.ts'

export { ObservationId } from './brand.ts'
export type { ObservationId as ObservationIdType } from './brand.ts'
export { MultimodalError } from './error.ts'
export { formatObservationProjection, MIN_PROJECTION_CHARS, MULTIMODAL_PROJECTION_VERSION } from './projection.ts'
export { ObservationStore } from './store.ts'
export type { StoredObservation } from './store.ts'
export type {
  EvidenceRef,
  ImageInspectSource,
  InspectRequest,
  JsonValue,
  MultimodalProvider,
  Observation,
} from './types.ts'

declare module '@deepseek-ai/cordis' {
  interface Context {
    multimodal: MultimodalRuntime
  }
}

/** Provider-selection settings for `ctx.multimodal`. */
export interface MultimodalRuntimeConfig {
  /** Explicit provider id; omitted only works when exactly one provider is available. */
  readonly provider?: string
  /**
   * 可选的结构化 Observation 落库根目录（如 `$DSH_HOME/dshplusplus/observations`）。
   * 配置后每次 inspect 完成都会追加一条 JSONL 记录；未配置时不做持久化。
   */
  readonly storeRoot?: string
}

/** Provider-selecting multimodal observation service. */
export class MultimodalRuntime extends Service {
  static Config: z<MultimodalRuntimeConfig> = z.object({
    provider: z.string(),
    storeRoot: z.string(),
  })

  private readonly providers = new Map<string, MultimodalProvider>()
  private readonly configuredProvider: string | undefined
  private readonly store: ObservationStore | undefined

  constructor(ctx: Context, config: MultimodalRuntimeConfig = {}) {
    super(ctx, 'multimodal')
    this.configuredProvider = config.provider
    this.store = config.storeRoot !== undefined && config.storeRoot !== ''
      ? new ObservationStore(config.storeRoot)
      : undefined
  }

  /**
   * Register one provider until the owning Cordis fiber is disposed.
   *
   * The registration is bound to the CALLING plugin's context (`owner`), not the
   * service's own long-lived context — otherwise a reloaded plugin's provider
   * lingers after the plugin is disposed and a re-registration throws
   * DUPLICATE_PROVIDER while the stale provider (with a dead ctx) still answers
   * inspect. Defaulting to the service ctx keeps backwards compatibility for
   * callers that do not pass an owner.
   *
   * @param provider - Named provider implementation.
   * @param owner - Context the registration should be tied to (the registering plugin).
   * @returns A disposer for early removal.
   */
  registerProvider(provider: MultimodalProvider, owner?: Context): () => void {
    if (provider.id.length === 0) throw new MultimodalError('provider id must not be empty', 'INVALID_PROVIDER')
    if (this.providers.has(provider.id)) {
      throw new MultimodalError(`multimodal provider "${provider.id}" is already registered`, 'DUPLICATE_PROVIDER')
    }
    const providers = this.providers
    const scope = owner ?? this.ctx
    const dispose = scope.effect(function* () {
      providers.set(provider.id, provider)
      yield () => providers.delete(provider.id)
    }, 'multimodal.registerProvider()')
    return () => void dispose()
  }

  /**
   * List detached provider status for diagnostics and configuration surfaces.
   * @returns Providers in registration order.
   */
  listProviders(): Array<{ id: string; available: boolean }> {
    return [...this.providers.values()].map(provider => ({
      id: provider.id,
      available: provider.available(),
    }))
  }

  /**
   * Inspect one source through the selected provider. When a store root is
   * configured, every completed observation (including failures) is appended
   * to the durable JSONL store; a store failure never fails the inspection.
   * @param request - Content reference and observation task.
   * @param signal - Optional cancellation signal.
   * @returns Provider-neutral observation.
   */
  async inspect(request: InspectRequest, signal?: AbortSignal): Promise<Observation> {
    const observation = await this.resolveProvider().inspect(request, signal)
    if (this.store !== undefined) {
      try {
        await this.store.append({
          ...(request.sessionId !== undefined ? { sessionId: request.sessionId } : {}),
          ...(request.messageId !== undefined ? { messageId: request.messageId } : {}),
          observation,
        })
      } catch (error) {
        console.warn('[dshplusplus/multimodal] observation store append failed:', error)
      }
    }
    return observation
  }

  /**
   * 查询持久化的 Observation 记录（未配置 storeRoot 时恒为空）。
   * @param sessionId - 可选按会话过滤。
   */
  async observations(sessionId?: string): Promise<StoredObservation[]> {
    if (this.store === undefined) return []
    return this.store.list(sessionId)
  }

  private resolveProvider(): MultimodalProvider {
    if (this.configuredProvider !== undefined) {
      const provider = this.providers.get(this.configuredProvider)
      if (provider === undefined) {
        throw new MultimodalError(
          `configured multimodal provider "${this.configuredProvider}" is not registered`,
          'PROVIDER_CONFIGURED_MISSING',
        )
      }
      if (!provider.available()) {
        throw new MultimodalError(
          `configured multimodal provider "${this.configuredProvider}" is unavailable`,
          'PROVIDER_CONFIGURED_UNAVAILABLE',
        )
      }
      return provider
    }
    const available = [...this.providers.values()].filter(provider => provider.available())
    if (available.length === 0) {
      throw new MultimodalError('no usable multimodal provider is registered', 'PROVIDER_UNAVAILABLE')
    }
    if (available.length > 1) {
      throw new MultimodalError(
        `multiple multimodal providers are available (${available.map(provider => provider.id).join(', ')})`,
        'PROVIDER_AMBIGUOUS',
      )
    }
    return available[0]!
  }
}

export default MultimodalRuntime

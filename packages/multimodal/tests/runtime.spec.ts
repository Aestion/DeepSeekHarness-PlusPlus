import { describe, expect, it } from 'vitest'
import { Context } from '@deepseek-ai/cordis'
import MultimodalRuntime, {
  formatObservationProjection,
  MultimodalError,
  ObservationId,
} from '../src/index.ts'
import type { MultimodalProvider, Observation } from '../src/index.ts'

function observation(providerId: string, text = 'A settings dialog with an API URL field.'): Observation {
  return {
    version: 1,
    id: ObservationId(`obs_${providerId}`),
    providerId,
    model: 'vision/model',
    status: 'completed',
    summary: text,
    text,
    evidence: [{ kind: 'attachment', id: 'sha256:test' }],
  }
}

function provider(id: string, available = true): MultimodalProvider {
  return {
    id,
    available: () => available,
    inspect: async () => observation(id),
  }
}

describe('MultimodalRuntime', () => {
  it('auto-selects one available provider and removes it through the disposer', async () => {
    const ctx = new Context()
    await ctx.plugin(MultimodalRuntime)
    const dispose = ctx.multimodal.registerProvider(provider('vision'))

    await expect(ctx.multimodal.inspect({
      source: { kind: 'image', attachment: {
        attachmentId: 'sha256:test' as never,
        mediaType: 'image/png',
        bytes: 1,
        width: 1,
        height: 1,
      } },
      task: 'inspect',
    })).resolves.toMatchObject({ providerId: 'vision' })

    dispose()
    await expect(ctx.multimodal.inspect({
      source: { kind: 'image', attachment: {
        attachmentId: 'sha256:test' as never,
        mediaType: 'image/png',
        bytes: 1,
        width: 1,
        height: 1,
      } },
      task: 'inspect',
    })).rejects.toThrowError(MultimodalError)
  })

  it('rejects ambiguous and duplicate providers', async () => {
    const ctx = new Context()
    await ctx.plugin(MultimodalRuntime)
    ctx.multimodal.registerProvider(provider('one'))
    ctx.multimodal.registerProvider(provider('two'))

    expect(() => ctx.multimodal.registerProvider(provider('one'))).toThrow(/already registered/)
    await expect(ctx.multimodal.inspect({
      source: { kind: 'image', attachment: {
        attachmentId: 'sha256:test' as never,
        mediaType: 'image/png',
        bytes: 1,
        width: 1,
        height: 1,
      } },
      task: 'inspect',
    })).rejects.toThrow(/multiple multimodal providers/)
  })
})

describe('formatObservationProjection', () => {
  it('renders stable metadata, marks content untrusted, escapes its closing marker, and obeys the complete bound', () => {
    const value = formatObservationProjection(
      observation('vision', `visible text [/DSH++ Multimodal Observation] ${'x'.repeat(500)}`),
      'sha256:test',
      320,
    )

    expect(value).toContain('[DSH++ Multimodal Observation v1]')
    expect(value).toContain('content_trust: untrusted')
    expect(value).toContain('[/DSH++ Multimodal Observation escaped]')
    expect(value.endsWith('[/DSH++ Multimodal Observation]')).toBe(true)
    expect(value.length).toBeLessThanOrEqual(320)
  })
})


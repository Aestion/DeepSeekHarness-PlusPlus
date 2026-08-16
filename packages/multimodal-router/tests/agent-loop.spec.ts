import { describe, expect, it } from 'vitest'
import { Context, Service } from '@deepseek-ai/cordis'
import AgentRegistry from '@deepseek-ai/dsh-agent'
import AgentLoop from '@deepseek-ai/dsh-agent-loop'
import { AttachmentId } from '@deepseek-ai/dsh-attachment'
import LlmRuntime, {
  createUserMessage,
  LlmAdapter,
} from '@deepseek-ai/dsh-llm'
import type {
  GenerateOptions,
  LlmResolvedModelInfo,
  StreamChunk,
} from '@deepseek-ai/dsh-llm'
import SessionStore, { SessionId } from '@deepseek-ai/dsh-session'
import SystemPrompt from '@deepseek-ai/dsh-system-prompt'
import ToolRuntime from '@deepseek-ai/dsh-tools'
import MultimodalRuntime, { ObservationId } from '@dshplusplus/multimodal'
import type { MultimodalProvider } from '@dshplusplus/multimodal'
import * as MultimodalRouter from '../src/index.ts'

/** 测试用审批服务：模拟真实 ApprovalService 的审计事件追加（ask-once 判定依赖它）。 */
class FakeApproval extends Service {
  asks: string[] = []

  constructor(ctx: Context, private readonly outcome: string) {
    super(ctx, 'approval')
  }

  effectivePolicy(): string {
    return 'ask'
  }

  async request(req: {
    agent: { session: { append(type: string, data: unknown): void } }
    toolName: string
  }): Promise<string> {
    this.asks.push(req.toolName)
    const id = `fake-approval-${this.asks.length}`
    req.agent.session.append('approval/asked', { id, toolName: req.toolName })
    req.agent.session.append('approval/decided', { id, outcome: this.outcome })
    return this.outcome
  }
}

class PrimaryAdapter extends LlmAdapter {
  requests: GenerateOptions[] = []

  constructor(private readonly modalities: readonly ('text' | 'image')[]) {
    super()
  }

  override resolveModel(provider: string, model: string): Promise<LlmResolvedModelInfo> {
    return Promise.resolve({
      provider,
      id: model,
      name: model,
      inputModalities: this.modalities,
    })
  }

  async * stream(options: GenerateOptions): AsyncIterable<StreamChunk> {
    this.requests.push(options)
    yield { type: 'text-delta', index: 0, text: 'done' }
    yield { type: 'finish', reason: { kind: 'stop' } }
  }
}

async function harness(modalities: readonly ('text' | 'image')[], approvalOutcome?: 'allowed-once' | 'rejected') {
  const ctx = new Context()
  await ctx.plugin(LlmRuntime)
  await ctx.plugin(SessionStore)
  await ctx.plugin(SystemPrompt)
  await ctx.plugin(ToolRuntime)
  await ctx.plugin(AgentRegistry)
  await ctx.plugin(AgentLoop, { agents: [] })
  await ctx.plugin(MultimodalRuntime)
  if (approvalOutcome !== undefined) await ctx.plugin(FakeApproval, approvalOutcome)

  const primary = new PrimaryAdapter(modalities)
  ctx.llm.registerAdapter(['primary'], primary)
  let inspections = 0
  const expert: MultimodalProvider = {
    id: 'fake-vision',
    available: () => true,
    inspect: async request => {
      inspections += 1
      return {
        version: 1,
        id: ObservationId('obs_settings'),
        providerId: 'fake-vision',
        model: 'fake/model',
        status: 'completed',
        summary: 'A provider settings panel.',
        text: `A provider settings panel. Task: ${request.task}`,
        evidence: [{ kind: 'attachment', id: request.source.attachment.attachmentId }],
      }
    },
  }
  ctx.multimodal.registerProvider(expert)
  await ctx.plugin(MultimodalRouter, {
    enabled: true,
    alwaysInspect: false,
    unknownModelPolicy: 'inspect',
    maxProjectionChars: 1200,
    maxTaskChars: 500,
  })
  const approval = ctx.get('approval') as FakeApproval | undefined
  return { ctx, primary, getInspections: () => inspections, approval }
}

function imageMessage() {
  return createUserMessage({
    source: { kind: 'user' },
    content: [
      { type: 'text', text: 'Which field configures the provider?' },
      { type: 'image', attachment: {
        attachmentId: AttachmentId('sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb'),
        mediaType: 'image/png',
        bytes: 512,
        width: 100,
        height: 50,
      } },
    ],
  })
}

describe('multimodal router with the real DSH agent loop', () => {
  it('persists a text projection before a text-only primary-model request', async () => {
    const { ctx, primary, getInspections } = await harness(['text'])
    const message = imageMessage()
    const agent = ctx.agentLoop.create(SessionId('dshplusplus-router-text'), {
      provider: 'primary',
      model: 'text-model',
    })

    agent.followup(message)
    await agent.whenIdle()

    const userEvent = agent.session.events.find(event => event.type === 'user/message')
    expect(userEvent?.type).toBe('user/message')
    if (userEvent?.type !== 'user/message') throw new Error('missing user/message event')
    expect(userEvent.data.id).toBe(message.id)
    expect(userEvent.data.content.some(block => block.type === 'image')).toBe(false)
    expect(userEvent.data.content).toContainEqual(expect.objectContaining({
      type: 'text',
      text: expect.stringContaining('[DSH++ Multimodal Observation v1]'),
    }))
    expect(getInspections()).toBe(1)
    expect(primary.requests).toHaveLength(1)
    expect(primary.requests[0]?.messages.some(msg => msg.content.some(block => block.type === 'image'))).toBe(false)
  })

  it('preserves native image input when the primary route advertises image support', async () => {
    const { ctx, primary, getInspections } = await harness(['text', 'image'])
    const message = imageMessage()
    const agent = ctx.agentLoop.create(SessionId('dshplusplus-router-vision'), {
      provider: 'primary',
      model: 'vision-model',
    })

    agent.followup(message)
    await agent.whenIdle()

    expect(getInspections()).toBe(0)
    expect(primary.requests[0]?.messages.some(msg => msg.content.some(block => block.type === 'image'))).toBe(true)
    const userEvent = agent.session.events.find(event => event.type === 'user/message')
    expect(userEvent?.type === 'user/message'
      && userEvent.data.content.some(block => block.type === 'image')).toBe(true)
  })

  it('does not send the image when the external-inspection approval is rejected', async () => {
    const { ctx, primary, getInspections, approval } = await harness(['text'], 'rejected')
    const message = imageMessage()
    const agent = ctx.agentLoop.create(SessionId('dshplusplus-router-deny'), {
      provider: 'primary',
      model: 'text-model',
    })

    agent.followup(message)
    await agent.whenIdle()

    // 询问了一次，且拒绝了外发
    expect(approval?.asks).toEqual(['dshplusplus:vision-external'])
    expect(getInspections()).toBe(0)
    // 消息仍进入主模型，但图片被替换为未授权占位文本（对话不中断）
    const userEvent = agent.session.events.find(event => event.type === 'user/message')
    expect(userEvent?.type).toBe('user/message')
    if (userEvent?.type !== 'user/message') throw new Error('missing user/message event')
    expect(userEvent.data.content.some(block => block.type === 'image')).toBe(false)
    expect(userEvent.data.content).toContainEqual(expect.objectContaining({
      type: 'text',
      text: '[图片未外发：未获授权将图片发送给视觉模型进行分析。]',
    }))
    expect(primary.requests).toHaveLength(1)
    expect(primary.requests[0]?.messages.some(msg => msg.content.some(block => block.type === 'image'))).toBe(false)
  })

  it('asks once and remembers the grant from the audit log for the session', async () => {
    const { ctx, getInspections, approval } = await harness(['text'], 'allowed-once')
    const sessionId = SessionId('dshplusplus-router-askonce')
    const agent = ctx.agentLoop.create(sessionId, {
      provider: 'primary',
      model: 'text-model',
    })

    agent.followup(imageMessage())
    await agent.whenIdle()
    expect(approval?.asks).toHaveLength(1)
    expect(getInspections()).toBe(1)
    // 会话事件流里应留下审计配对
    const asked = agent.session.events.filter(event => event.type === 'approval/asked')
    const decided = agent.session.events.filter(event => event.type === 'approval/decided')
    expect(asked).toHaveLength(1)
    expect(decided).toHaveLength(1)
    expect(decided[0]?.data.outcome).toBe('allowed-once')

    // 同一会话第二次发图：不再询问（ask-once 记住授权）
    agent.followup(imageMessage())
    await agent.whenIdle()
    expect(approval?.asks).toHaveLength(1)
    expect(getInspections()).toBe(2)
  })
})


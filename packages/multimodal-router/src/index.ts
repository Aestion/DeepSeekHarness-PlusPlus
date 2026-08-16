/** Automatic image preprocessing before DSH persists a proposed agent step. */

import { createHash } from 'node:crypto'
import type { Context } from '@deepseek-ai/cordis'
import z from '@deepseek-ai/schemastery'
import type {} from '@deepseek-ai/dsh-agent'
import {
  freezeMessage,
} from '@deepseek-ai/dsh-llm'
import type {
  ContentBlock,
  UserMessage,
} from '@deepseek-ai/dsh-llm'
import {
  formatObservationProjection,
  MIN_PROJECTION_CHARS,
  ObservationId,
} from '@dshplusplus/multimodal'
import type {
  Observation,
} from '@dshplusplus/multimodal'

/** Cordis plugin name used by loader diagnostics and durable message provenance. */
export const name = 'dshplusplus-multimodal-router'

/** Services required by automatic routing. */
export const inject = ['multimodal', 'llm']

/** Automatic routing and model-visible projection settings. */
export interface Config {
  /** Enable image preprocessing. */
  readonly enabled?: boolean
  /** Inspect even when the primary route explicitly supports images. */
  readonly alwaysInspect?: boolean
  /** Behavior when exact model modality metadata is absent or cannot be read. */
  readonly unknownModelPolicy?: 'inspect' | 'pass'
  /** Complete character budget for each projected observation. */
  readonly maxProjectionChars?: number
  /** Maximum nearby user-text characters supplied as the visual task. */
  readonly maxTaskChars?: number
  /**
   * Approval policy for sending image content to the external vision model.
   * `'ask-once'` asks through the DSH approval channel the first time a
   * session sends an image out, then remembers the grant from the durable
   * audit log (`approval/asked` + `approval/decided` pair); `'off'` never
   * asks (legacy behavior). The session's own approval policy (`never`)
   * bypasses the ask entirely.
   */
  readonly externalInspectionApproval?: 'ask-once' | 'off'
}

export const Config: z<Config> = z.object({
  enabled: z.boolean().default(true),
  alwaysInspect: z.boolean().default(false),
  unknownModelPolicy: z.union(['inspect', 'pass'] as const).default('inspect'),
  maxProjectionChars: z.number().default(6000),
  maxTaskChars: z.number().default(2000),
  externalInspectionApproval: z.union(['ask-once', 'off'] as const).default('ask-once'),
})

type ResolvedConfig = Required<Config>

function assertPositiveInteger(label: string, value: number, minimum: number): void {
  if (!Number.isInteger(value) || value < minimum) {
    throw new Error(`multimodal-router: ${label} must be an integer >= ${minimum}`)
  }
}

function hasImage(blocks: readonly ContentBlock[]): boolean {
  return blocks.some(block => block.type === 'image'
    || (block.type === 'tool-result' && hasImage(block.content)))
}

function taskFromMessage(message: UserMessage, maxChars: number): string {
  const text = message.content
    .filter(block => block.type === 'text')
    .map(block => block.text)
    .join('\n')
    .trim()
  const task = text.length === 0 ? 'Describe this image for the primary agent.' : text
  return task.length <= maxChars ? task : `${task.slice(0, maxChars - 1)}…`
}

async function primaryRouteNeedsInspection(
  ctx: Context,
  agent: { readonly options: { readonly provider?: string; readonly model?: string } },
  config: ResolvedConfig,
  signal: AbortSignal,
): Promise<boolean> {
  if (config.alwaysInspect) return true
  const { provider, model } = agent.options
  if (provider === undefined || model === undefined) return config.unknownModelPolicy === 'inspect'
  try {
    const info = await ctx.llm.resolveModelInfo(provider, model, signal)
    if (info.inputModalities === undefined) return config.unknownModelPolicy === 'inspect'
    return !info.inputModalities.includes('image')
  } catch (error: unknown) {
    if (signal.aborted) throw signal.reason
    return config.unknownModelPolicy === 'inspect'
  }
}

function failedObservation(attachmentId: string, error: unknown): Observation {
  const code = typeof error === 'object' && error !== null && 'code' in error
    ? String((error as { code: unknown }).code)
    : 'INSPECTION_FAILED'
  const message = error instanceof Error ? error.message : String(error)
  const id = createHash('sha256')
    .update(attachmentId)
    .update('\0')
    .update(code)
    .digest('hex')
    .slice(0, 24)
  return {
    version: 1,
    id: ObservationId(`obs_failed_${id}`),
    providerId: name,
    status: 'failed',
    summary: message,
    text: `Image inspection failed (${code}): ${message}`,
    structured: { code },
    evidence: [{ kind: 'attachment', id: attachmentId }],
  }
}

/** Approval tool identity for external image inspection (Provider Origin 维度见 reason）。 */
const VISION_EXTERNAL_TOOL = 'dshplusplus:vision-external'
/** 未获授权时的模型可见占位文本。 */
const VISION_UNAUTHORIZED_TEXT = '[图片未外发：未获授权将图片发送给视觉模型进行分析。]'

/** 会话事件流中的 `approval/asked`（id → toolName）。 */
type AskedEvent = { id: string; toolName: string }
type DecidedEvent = { id: string; outcome: string }

/**
 * ask-once 判定：该会话的事件流里是否已存在同一外发动作的已允许审批
 * （`approval/asked` + `approval/decided` 配对，outcome 为唯一 grant
 * `allowed-once`）。审计日志是持久化的，因此授权记忆随会话恢复而保留，
 * 无需新增任何持久化结构。
 */
function visionApprovalGranted(events: readonly unknown[], toolName: string): boolean {
  const asked = new Map<string, string>()
  for (const event of events) {
    const record = event as { type?: string; data?: unknown }
    if (record.type === 'approval/asked') {
      const data = record.data as Partial<AskedEvent>
      if (typeof data.id === 'string' && typeof data.toolName === 'string') {
        asked.set(data.id, data.toolName)
      }
    } else if (record.type === 'approval/decided') {
      const data = record.data as Partial<DecidedEvent>
      if (typeof data.id === 'string' && asked.get(data.id) === toolName && data.outcome === 'allowed-once') {
        return true
      }
    }
  }
  return false
}

/**
 * 图片外发前的 ask-once 审批：
 * - 无审批服务（部署未挂载 dsh-user-approval）→ 放行（兼容旧行为）；
 * - 会话审批策略为 `never` → 放行（用户明确要求不询问）；
 * - 历史审计日志已有该动作的 `allowed-once` → 放行（记住授权）；
 * - 否则走 DSH approval 通道询问；拒绝/取消/无回答者 → 抛错（调用方降级
 *   为“不发送图片”，fail closed）。
 */
async function ensureVisionApproval(
  ctx: Context,
  agent: { readonly session: { readonly events: readonly unknown[] } },
  signal: AbortSignal,
): Promise<void> {
  const approval = ctx.get('approval') as
    | {
        effectivePolicy(session: unknown): string
        request(req: {
          agent: unknown
          toolName: string
          reason?: string
          signal?: AbortSignal
        }): Promise<string>
      }
    | undefined
  if (approval === undefined) return
  if (approval.effectivePolicy(agent.session) === 'never') return
  if (visionApprovalGranted(agent.session.events, VISION_EXTERNAL_TOOL)) return
  const outcome = await approval.request({
    agent,
    toolName: VISION_EXTERNAL_TOOL,
    reason: '将图片内容发送给视觉模型生成观察描述（图片会发送到视觉模型服务进行分析）',
    signal,
  })
  if (outcome !== 'allowed-once') {
    const error = new Error(`图片外发未获授权（${outcome}）`)
    ;(error as { code?: string }).code = `VISION_EXTERNAL_${outcome.toUpperCase()}`
    throw error
  }
}

async function rewriteBlocks(
  ctx: Context,
  agent: { readonly session: { readonly events: readonly unknown[] } },
  blocks: readonly ContentBlock[],
  task: string,
  message: UserMessage,
  sessionId: string,
  config: ResolvedConfig,
  signal: AbortSignal,
  cache: Map<string, Promise<Observation>>,
): Promise<ContentBlock[]> {
  const rewritten: ContentBlock[] = []
  for (const block of blocks) {
    if (block.type === 'image') {
      // 外发审批：ask-once 询问（或按配置关闭）；未获授权时不发送图片，
      // 以模型可见的占位文本降级（对话继续，主模型知道存在图片但未获准查看）。
      if (config.externalInspectionApproval === 'ask-once') {
        try {
          await ensureVisionApproval(ctx, agent, signal)
        } catch (error: unknown) {
          if (signal.aborted) throw signal.reason
          rewritten.push({ type: 'text', text: VISION_UNAUTHORIZED_TEXT })
          continue
        }
      }
      const attachmentId = String(block.attachment.attachmentId)
      const key = `${attachmentId}\0${task}`
      let pending = cache.get(key)
      if (pending === undefined) {
        pending = ctx.multimodal.inspect({
          source: { kind: 'image', attachment: block.attachment },
          task,
          sessionId,
          messageId: String(message.id),
        }, signal).catch((error: unknown) => {
          if (signal.aborted) throw signal.reason
          return failedObservation(attachmentId, error)
        })
        cache.set(key, pending)
      }
      const observation = await pending
      rewritten.push({
        type: 'text',
        text: formatObservationProjection(observation, attachmentId, config.maxProjectionChars),
      })
      continue
    }
    if (block.type === 'tool-result' && hasImage(block.content)) {
      rewritten.push({
        ...block,
        content: await rewriteBlocks(ctx, agent, block.content, task, message, sessionId, config, signal, cache),
      })
      continue
    }
    rewritten.push(block)
  }
  return rewritten
}

/**
 * Register the cooperative `agent/pre-step` listener.
 * @param ctx - Cordis context with agent event types, ctx.llm, and ctx.multimodal.
 * @param config - Loader-resolved routing configuration.
 */
export function apply(ctx: Context, config: Config): void {
  const resolved = config as ResolvedConfig
  assertPositiveInteger('maxProjectionChars', resolved.maxProjectionChars, MIN_PROJECTION_CHARS)
  assertPositiveInteger('maxTaskChars', resolved.maxTaskChars, 1)
  if (!resolved.enabled) return

  ctx.on('agent/pre-step', async ({ agent, signal }, next) => {
    const decision = await next()
    if (decision.kind === 'reject' || !decision.messages.some(message => hasImage(message.content))) return decision
    if (!await primaryRouteNeedsInspection(ctx, agent, resolved, signal)) return decision

    const cache = new Map<string, Promise<Observation>>()
    const messages = await Promise.all(decision.messages.map(async (message): Promise<UserMessage> => {
      if (!hasImage(message.content)) return message
      const content = await rewriteBlocks(
        ctx,
        agent,
        message.content,
        taskFromMessage(message, resolved.maxTaskChars),
        message,
        String(agent.id),
        resolved,
        signal,
        cache,
      )
      return freezeMessage({ ...message, content })
    }))
    return { kind: 'enter', messages }
  })
}

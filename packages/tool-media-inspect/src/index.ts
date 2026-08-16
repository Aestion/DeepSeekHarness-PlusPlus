/** Explicit `media_inspect` tool: model-driven inspection of an image attachment. */

import { AttachmentId } from '@deepseek-ai/dsh-attachment'
import type { ImageMediaType } from '@deepseek-ai/dsh-attachment'
import type { ToolResult, ToolRunContext } from '@deepseek-ai/dsh-tools'
import { defineTool } from '@deepseek-ai/dsh-tools'
import type { Context } from '@deepseek-ai/cordis'
import type { ImageInspectSource } from '@dshplusplus/multimodal'

/** Cordis plugin name used by loader diagnostics. */
export const name = 'dshplusplus-media-inspect'

/** Services required by the tool. */
export const inject = ['multimodal', 'tools']

/**
 * Register the `media_inspect` tool. The model passes back the attachment
 * reference it saw in the conversation plus a task; the tool runs the same
 * multimodal seam as automatic routing, and the observation is persisted to
 * the configured store (when one exists).
 */
export function apply(ctx: Context): void {
  ctx.tools.register(defineTool({
    name: 'media_inspect',
    description: '显式检查一张图片：调用视觉模型生成观察描述，返回结构化 Observation。图片内容会发送给视觉模型服务进行分析。',
    parameters: {
      attachment: {
        type: 'object',
        description: '会话中图片附件的引用（与对话里出现的附件一致）',
        properties: {
          attachmentId: { type: 'string', description: '附件 id（形如 sha256:...）', required: true },
          mediaType: { type: 'string', description: '媒体类型，如 image/png' },
          bytes: { type: 'integer', description: '附件字节数' },
          width: { type: 'integer', description: '图片宽度（像素）' },
          height: { type: 'integer', description: '图片高度（像素）' },
        },
        additionalProperties: false,
        required: true,
      },
      task: { type: 'string', description: '检查任务：希望从图片中提取或确认什么信息', required: true },
    },
    output: {
      schema: {
        type: 'object',
        properties: {
          observationId: { type: 'string', description: 'Observation id' },
          provider: { type: 'string', description: '视觉模型 provider' },
          model: { type: 'string', description: '视觉模型' },
          status: { type: 'string', description: 'completed / partial / failed' },
          summary: { type: 'string', description: '观察摘要' },
          text: { type: 'string', description: '完整观察文本' },
        },
        additionalProperties: false,
      },
      render(_args, value) {
        return [{ type: 'text', text: `[media_inspect] ${String(value.summary)}` }]
      },
    },
    isConcurrencySafe() {
      return false
    },
    async execute(args, exec: ToolRunContext) {
      // 参数已经过 schema 校验；模型传回的附件字段与对话内附件一致。
      const attachment = args.attachment as {
        attachmentId: string
        mediaType?: string
        bytes?: number
        width?: number
        height?: number
      }
      const task = String(args.task)
      const source: ImageInspectSource = {
        kind: 'image',
        attachment: {
          attachmentId: AttachmentId(attachment.attachmentId),
          mediaType: (attachment.mediaType ?? 'image/png') as ImageMediaType,
          bytes: attachment.bytes ?? 0,
          width: attachment.width ?? 0,
          height: attachment.height ?? 0,
        },
      }
      const sessionId = exec.agent === undefined ? undefined : String(exec.agent.id)
      const observation = await ctx.multimodal.inspect({
        source,
        task,
        ...(sessionId !== undefined ? { sessionId } : {}),
      }, exec.signal)
      return {
        observationId: observation.id,
        provider: observation.providerId,
        ...observation.model === undefined ? {} : { model: observation.model },
        status: observation.status,
        summary: observation.summary,
        text: observation.text,
      }
    },
    presentCall(args) {
      return {
        card: 'generic',
        title: 'media_inspect',
        kind: 'read',
        rawInput: { attachmentId: (args.attachment as { attachmentId: string }).attachmentId, task: String(args.task) },
      }
    },
    presentResult(_args, result: ToolResult) {
      return {
        card: 'generic',
        title: 'media_inspect',
        kind: 'read',
        content: result.content,
      }
    },
  }))
}

export default { name, inject, apply }

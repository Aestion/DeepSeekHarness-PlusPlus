import { spawn } from 'node:child_process'
import { fileURLToPath } from 'node:url'
import { describe, expect, it } from 'vitest'

describe('browser native host shutdown', () => {
  it('exits quietly when the DSH++ browser gateway is gone', async () => {
    const script = fileURLToPath(new URL('../native-host/native-host.mjs', import.meta.url))
    const child = spawn(process.execPath, [script], {
      env: {
        ...process.env,
        DSHPLUSPLUS_GATEWAY: 'http://127.0.0.1:9',
        DSHPLUSPLUS_NATIVE_HOST_REG_KEY:
          'HKCU\\Software\\DSHPlusPlus\\Tests\\MissingNativeHost',
      },
      stdio: ['pipe', 'pipe', 'pipe'],
      windowsHide: true,
    })

    let stderr = ''
    child.stderr.setEncoding('utf8')
    child.stderr.on('data', (chunk: string) => {
      stderr += chunk
    })

    const exitCode = await new Promise<number | null>((resolve, reject) => {
      const timeout = setTimeout(() => {
        child.kill()
        reject(new Error('native host did not exit after the gateway became unavailable'))
      }, 8_000)
      child.once('error', reject)
      child.once('exit', (code) => {
        clearTimeout(timeout)
        resolve(code)
      })
    })

    expect(exitCode).toBe(0)
    expect(stderr).toBe('')
  })
})

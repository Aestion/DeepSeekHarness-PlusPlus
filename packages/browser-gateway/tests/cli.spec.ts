import { describe, expect, it } from 'vitest'
import { parseArgs } from '../src/cli.js'

describe('parseArgs — host binding', () => {
  it('rejects binding to a non-loopback host', () => {
    expect(() => parseArgs(['--host', '0.0.0.0'])).toThrow(/回环/)
    expect(() => parseArgs(['--host', '192.168.1.5'])).toThrow(/回环/)
  })

  it('accepts loopback hosts', () => {
    expect(parseArgs(['--host', '127.0.0.1']).host).toBe('127.0.0.1')
    expect(parseArgs(['--host', 'localhost']).host).toBe('localhost')
    expect(parseArgs([]).host).toBe('127.0.0.1')
  })

  it('does not treat a following flag as a value', () => {
    const options = parseArgs(['--host', '--port', '9000'])
    expect(options.host).toBe('127.0.0.1')
    expect(options.port).toBe(9000)
  })
})

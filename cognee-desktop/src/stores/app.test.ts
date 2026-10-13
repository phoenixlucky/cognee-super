import { beforeEach, describe, expect, it } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { useAppStore } from './app'

describe('app status', () => {
  beforeEach(() => setActivePinia(createPinia()))

  it('keeps API and MCP failures independent and derives the configured endpoint', () => {
    const app = useAppStore()
    app.apiStatus = 'failed'
    app.mcpStatus = 'running'
    app.config.mcpHost = '127.0.0.1'
    app.config.mcpPort = 12316
    expect(app.apiStatusLabel).toBe('启动失败')
    expect(app.mcpStatusLabel).toBe('运行中')
    expect(app.mcpUrl).toBe('http://127.0.0.1:12316/mcp-new')
    expect(app.legacyMcpUrl).toBe('http://127.0.0.1:12316/mcp')
  })

  it('never stores generated or provider secrets in the serializable config', () => {
    const app = useAppStore()
    app.secretInputs.llm = 'temporary-write-only-value'
    const config = JSON.stringify(app.config)
    expect(config).not.toContain('temporary-write-only-value')
    expect(app.secrets.llm).toBe(false)
  })

  it('preserves unsaved settings while periodic service status refreshes', () => {
    const app = useAppStore()
    app.configLoaded = true
    app.config.apiPort = 12345
    app.applyStatus({
      apiStatus: 'running', mcpStatus: 'stopped', runtimeStatus: 'ready', activeWork: false,
      config: { ...app.config, apiPort: 12315 },
    })
    expect(app.config.apiPort).toBe(12345)
  })
})

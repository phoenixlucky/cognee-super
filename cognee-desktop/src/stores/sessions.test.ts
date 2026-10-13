import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { invoke } from '@tauri-apps/api/core'
import { useSessionsStore } from './sessions'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))

describe('session browsing', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    vi.mocked(invoke).mockResolvedValue({ items: [{ sessionId: 's-1', summary: 'safe summary', status: 'completed', updatedAt: 'now' }], total: 51, hasMore: true })
  })

  it('uses API filters and offsets and consumes the paged sessions envelope', async () => {
    const sessions = useSessionsStore()
    sessions.range = '30d'
    sessions.status = 'completed'
    await sessions.load(2)
    expect(invoke).toHaveBeenCalledWith('list_sessions', { range: '30d', status: 'completed', limit: 50, offset: 50 })
    expect(sessions.items).toHaveLength(1)
    expect(sessions.total).toBe(51)
    expect(sessions.hasMore).toBe(true)
  })
})

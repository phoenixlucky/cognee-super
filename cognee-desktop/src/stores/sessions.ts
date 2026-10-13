import { defineStore } from 'pinia'
import { invoke } from '@tauri-apps/api/core'
import type { SessionDetailDto, SessionDto, SessionPageDto } from '../types/sessions'

export const useSessionsStore = defineStore('sessions', {
  state: () => ({
    items: [] as SessionDto[],
    selected: null as SessionDetailDto | null,
    range: '7d' as '24h' | '7d' | '30d' | 'all',
    status: '' as '' | 'running' | 'completed' | 'failed' | 'abandoned',
    page: 1,
    pageSize: 50,
    total: 0,
    hasMore: false,
    loading: false,
    error: '',
  }),
  actions: {
    async load(page = 1) {
      this.loading = true
      this.error = ''
      this.page = page
      try {
        const result = await invoke<SessionPageDto>('list_sessions', {
          range: this.range,
          status: this.status || null,
          limit: this.pageSize,
          offset: (page - 1) * this.pageSize,
        })
        this.items = result.items
        this.total = result.total
        this.hasMore = result.hasMore
      } catch (error) {
        this.error = String(error)
        this.items = []
      } finally {
        this.loading = false
      }
    },
    async loadDetail(row: SessionDto) {
      try {
        this.selected = await invoke<SessionDetailDto>('get_session', { sessionId: row.sessionId })
      } catch (error) {
        this.error = String(error)
      }
    },
  },
})

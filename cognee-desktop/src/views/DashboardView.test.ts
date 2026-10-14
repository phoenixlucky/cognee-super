import { mount } from '@vue/test-utils'
import { createPinia, setActivePinia } from 'pinia'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import DashboardView from './DashboardView.vue'
import { useAppStore } from '../stores/app'
import { useMemoryStore } from '../stores/memory'

describe('dashboard runtime recovery guidance', () => {
  let app: ReturnType<typeof useAppStore>
  let pinia: ReturnType<typeof createPinia>

  beforeEach(() => {
    pinia = createPinia()
    setActivePinia(pinia)
    app = useAppStore()
    vi.spyOn(app, 'refreshStatus').mockResolvedValue()
    vi.spyOn(useMemoryStore(), 'loadDatasets').mockResolvedValue()
  })

  function mountDashboard() {
    return mount(DashboardView, {
      global: {
        plugins: [pinia],
        stubs: { RouterLink: { template: '<a :href="to"><slot /></a>', props: ['to'] } },
      },
    })
  }

  it('keeps a repair action available for a ready runtime', () => {
    app.runtimeStatus = 'ready'

    const wrapper = mountDashboard()

    expect(wrapper.get('[data-testid="runtime-repair"]').text()).toContain('修复运行环境')
  })

  it('uses the existing runtime preparation action when repair is selected', async () => {
    app.runtimeStatus = 'ready'
    const prepareRuntime = vi.spyOn(app, 'prepareRuntime').mockResolvedValue()

    const wrapper = mountDashboard()
    await wrapper.get('[data-testid="runtime-repair"]').trigger('click')

    expect(prepareRuntime).toHaveBeenCalledOnce()
  })

  it('links to Settings when the default LLM key is missing', async () => {
    app.config.llmProvider = ''
    app.secrets.llm = false
    const wrapper = mountDashboard()

    expect(wrapper.get('[data-testid="llm-settings-link"]').attributes('href')).toBe('/settings')

    app.secrets.llm = true
    await wrapper.vm.$nextTick()
    expect(wrapper.find('[data-testid="llm-settings-link"]').exists()).toBe(false)
  })
})

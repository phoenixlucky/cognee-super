import { defineStore } from 'pinia'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import type { AppConfig, AppStatus, LogEntry, McpTool, RunState, RuntimeState } from '../types/app'

const defaults: AppConfig = { apiHost: '127.0.0.1', apiPort: 12315, mcpHost: '127.0.0.1', mcpPort: 12316, dataRoot: '', llmProvider: '', llmModel: '', llmEndpoint: '', embeddingProvider: '', embeddingModel: '', graphDatabaseProvider: 'ladybug', vectorDbProvider: 'lancedb', vectorDbUrl: '' }
async function command<T>(name: string, args?: Record<string, unknown>): Promise<T> { return invoke<T>(name, args) }

export const useAppStore = defineStore('app', {
  state: () => ({ config: { ...defaults } as AppConfig, apiStatus: 'stopped' as RunState, mcpStatus: 'stopped' as RunState, runtimeStatus: 'missing' as RuntimeState, busy: false, lastError: '', copied: false, tools: [] as McpTool[], logs: [] as string[], bootstrapPercent: 0, bootstrapMessage: '', secrets: { llm: false, embedding: false, vector: false } as Record<string, boolean>, secretInputs: { llm: '', embedding: '', vector: '' } as Record<'llm'|'embedding'|'vector', string> }),
  getters: {
    mcpUrl: (s) => `http://${s.config.mcpHost}:${s.config.mcpPort}/mcp-new`,
    legacyMcpUrl: (s) => `http://${s.config.mcpHost}:${s.config.mcpPort}/mcp`,
    apiStatusLabel: (s) => ({ running: '运行中', starting: '启动中', stopping: '正在停止', failed: '启动失败', stopped: '已停止' })[s.apiStatus],
    mcpStatusLabel: (s) => ({ running: '运行中', starting: '启动中', stopping: '正在停止', failed: '启动失败', stopped: '已停止' })[s.mcpStatus],
    runtimeLabel: (s) => ({ ready: '已就绪', preparing: '准备中', failed: '准备失败', missing: '未初始化' })[s.runtimeStatus],
  },
  actions: {
    applyStatus(status: AppStatus) { this.apiStatus = status.apiStatus; this.mcpStatus = status.mcpStatus; this.runtimeStatus = status.runtimeStatus; this.config = { ...this.config, ...status.config }; this.lastError = status.apiError || status.mcpError || '' },
    async refreshStatus() { try { this.applyStatus(await command<AppStatus>('get_app_status')); await this.refreshSecrets() } catch (e) { this.lastError = String(e) } },
    async refreshSecrets() { try { this.secrets = await command<Record<string,boolean>>('get_settings_summary') } catch {} },
    async run(name: string, service: 'api'|'mcp'|'all' = 'api') { this.busy = true; this.lastError = ''; try { const s = await command<AppStatus>(name); this.applyStatus(s); if(name==='start_mcp') await this.refreshTools() } catch(e) { this.lastError = String(e); if(service==='api')this.apiStatus='failed'; if(service==='mcp')this.mcpStatus='failed' } finally { this.busy = false } },
    startApi() { return this.run('start_api', 'api') }, startMcp() { return this.run('start_mcp', 'mcp') }, stopApi() { return this.run('stop_api', 'api') }, stopMcp() { return this.run('stop_mcp', 'mcp') }, stopAll() { return this.run('stop_all', 'all') }, prepareRuntime() { return this.run('prepare_runtime') },
    async cancelRuntime() { try { await command('cancel_runtime') } catch(e) { this.lastError=String(e) } },
    async saveSettings() { const restartApi=this.apiStatus==='running';const restartMcp=this.mcpStatus==='running';try { if(restartApi||restartMcp){const {ElMessageBox}=await import('element-plus');await ElMessageBox.confirm('应用新的提供方或端口设置需要重启本地服务。正在进行的记忆处理可能会中断。','重启本地服务',{type:'warning'});await this.stopAll()} await command('save_settings', { input: this.config }); for (const [kind,value] of Object.entries(this.secretInputs) as [keyof typeof this.secretInputs,string][]) if(value){ await command('set_provider_secret',{kind, value}); this.secrets[kind]=true; this.secretInputs[kind]='' } this.lastError=''; await this.refreshStatus();if(restartApi){await this.startApi();if(restartMcp)await this.startMcp()} } catch(e) { if(e!=='cancel'&&e!=='close')this.lastError=String(e) } },
    async refreshTools() { try { this.tools = await command<McpTool[]>('list_mcp_tools') } catch(e) { this.lastError=String(e) } },
    async refreshLogs() { try { const entries = await command<LogEntry[]>('get_recent_logs'); this.logs = entries.map(x=>`${x.timestamp} [${x.service}] ${x.line}`) } catch(e) { this.lastError=String(e) } },
    async listenBootstrap() { await listen<{percent:number;message:string}>('bootstrap-progress',event=>{this.bootstrapPercent=event.payload.percent;this.bootstrapMessage=event.payload.message}) },
  },
})

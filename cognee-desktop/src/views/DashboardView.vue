<script setup lang="ts">
import { computed, onMounted } from 'vue'
import { Activity, ArrowUpRight, Brain, Check, ChevronRight, CircleAlert, Copy, FileText, LoaderCircle, Play, Radio, RotateCw, Square, Workflow } from '@lucide/vue'
import { useAppStore } from '../stores/app'
import { useMemoryStore } from '../stores/memory'

const app = useAppStore()
const memory = useMemoryStore()
const needsLlmKey = computed(() => {
  const provider = app.config.llmProvider.trim().toLowerCase()
  return (!provider || provider === 'openai') && !app.secrets.llm
})
onMounted(async () => { await app.refreshStatus(); if(app.apiStatus==='running') void memory.loadDatasets() })
async function copyEndpoint() { await navigator.clipboard.writeText(app.mcpUrl); app.copied = true; setTimeout(() => app.copied = false, 1600) }
</script>

<template>
  <div class="page-heading"><div><div class="eyebrow">{{ new Date().toLocaleDateString('zh-CN', { year: 'numeric', month: 'long', day: 'numeric', weekday: 'long' }) }}</div><h1>你的本地记忆空间</h1><p>管理 Cognee 服务、浏览记忆文件，并连接你的 MCP 客户端。</p></div><div class="heading-actions"><button class="button button-quiet" @click="app.refreshStatus"><RotateCw :size="15" />刷新状态</button><button v-if="app.apiStatus !== 'running'" class="button button-primary" :disabled="app.busy" @click="app.startApi"><LoaderCircle v-if="app.busy" class="spin" :size="16" /><Play v-else :size="15" />{{ app.busy ? '正在启动…' : '启动服务' }}</button><button v-else class="button button-danger-quiet" @click="app.stopAll"><Square :size="14" />停止服务</button></div></div>
  <div v-if="app.lastError" class="alert-banner"><CircleAlert :size="18" /><div><strong>服务操作未完成</strong><p>{{ app.lastError }}</p></div><button @click="app.lastError = ''">×</button></div>
  <section class="service-grid">
    <article class="surface service-card"><div class="card-top"><div class="service-icon api-icon"><Activity :size="19" /></div><span class="status-chip" :class="app.apiStatus"><i></i>{{ app.apiStatusLabel }}</span></div><div class="service-title">Cognee API</div><div class="service-desc">本地记忆引擎与数据管理接口</div><div class="service-meta"><code>127.0.0.1:{{ app.config.apiPort }}</code><span>REST API</span></div><button class="card-action" @click="app.apiStatus === 'running' ? app.stopApi() : app.startApi()"><component :is="app.apiStatus === 'running' ? Square : Play" :size="14" />{{ app.apiStatus === 'running' ? '停止 API' : '启动 API' }}<ChevronRight :size="15" /></button></article>
    <article class="surface service-card"><div class="card-top"><div class="service-icon mcp-icon"><Radio :size="19" /></div><span class="status-chip" :class="app.mcpStatus"><i></i>{{ app.mcpStatusLabel }}</span></div><div class="service-title">MCP 服务</div><div class="service-desc">向 AI 客户端提供 Cognee 记忆工具</div><div class="service-meta"><code>127.0.0.1:{{ app.config.mcpPort }}/mcp-new</code><span>Streamable HTTP</span></div><button class="card-action" :disabled="app.mcpStatus === 'running' || app.apiStatus !== 'running'" @click="app.startMcp"><Play :size="14" />{{ app.mcpStatus === 'running' ? '服务运行中' : '启动 MCP' }}<ChevronRight :size="15" /></button></article>
    <article class="surface service-card runtime-card"><div class="card-top"><div class="service-icon runtime-icon"><Workflow :size="19" /></div><span class="status-chip" :class="app.runtimeStatus"><i></i>{{ app.runtimeLabel }}</span></div><div class="service-title">Cognee 运行环境</div><div class="service-desc">Python 与依赖由应用独立管理</div><div class="runtime-track"><span :style="{ width: `${app.bootstrapPercent}%` }"></span></div><div class="runtime-note">{{ app.bootstrapMessage || '环境文件保存在应用数据目录' }}</div><button v-if="app.runtimeStatus === 'preparing'" class="card-action" @click="app.cancelRuntime"><Square :size="14"/>取消准备<ChevronRight :size="15" /></button><button v-else-if="app.runtimeStatus !== 'ready'" class="card-action" :disabled="app.busy" @click="app.prepareRuntime"><component :is="app.busy ? LoaderCircle : RotateCw" :size="14" :class="{ spin: app.busy }" />{{ app.busy ? '正在准备…' : '准备运行环境' }}<ChevronRight :size="15" /></button><template v-else><div class="runtime-ready"><Check :size="14" />已就绪，可启动服务</div><button class="card-action" data-testid="runtime-repair" :disabled="app.busy" @click="app.prepareRuntime"><component :is="app.busy ? LoaderCircle : RotateCw" :size="14" :class="{ spin: app.busy }" />{{ app.busy ? '正在修复…' : '修复运行环境' }}<ChevronRight :size="15" /></button></template><p v-if="needsLlmKey" class="runtime-credential-hint">LLM 记忆构建需要密钥，本地向量嵌入可继续使用。<RouterLink data-testid="llm-settings-link" to="/settings">前往设置</RouterLink></p></article>
  </section>

  <section class="dashboard-lower"><article class="surface endpoint-panel"><div class="section-title-row"><div><div class="eyebrow">连接入口</div><h2>MCP 客户端地址</h2></div><span class="endpoint-tag"><span></span>新版协议</span></div><p>在支持 Streamable HTTP 的 MCP 客户端中添加此地址，即可调用 Cognee 的记忆工具。</p><div class="endpoint-copy"><code>{{ app.mcpUrl }}</code><button class="button button-small" @click="copyEndpoint"><Check v-if="app.copied" :size="14" /><Copy v-else :size="14" />{{ app.copied ? '已复制' : '复制地址' }}</button></div><div class="endpoint-foot"><span><span class="tiny-dot"></span>监听地址仅对本机开放</span><RouterLink to="/mcp">查看配置说明 <ArrowUpRight :size="13" /></RouterLink></div></article>
    <article class="surface activity-panel"><div class="section-title-row"><div><div class="eyebrow">快速访问</div><h2>记忆管理 <span class="muted">· {{ memory.datasets.length }} 个数据集</span></h2></div><RouterLink class="text-link" to="/datasets">全部记忆 <ArrowUpRight :size="14" /></RouterLink></div><div class="quick-links"><RouterLink to="/datasets" class="quick-link"><span class="quick-icon files"><FileText :size="17" /></span><span><strong>原始文件与数据集</strong><small>浏览、添加或删除来源文件</small></span><ChevronRight :size="16" /></RouterLink><RouterLink to="/graph" class="quick-link"><span class="quick-icon graph"><Brain :size="17" /></span><span><strong>知识图谱</strong><small>探索 Cognee 构建的关联记忆</small></span><ChevronRight :size="16" /></RouterLink></div></article></section>
</template>

<style scoped>
.runtime-credential-hint { margin: 8px 0 0; color: #8792a2; font-size: 10px; line-height: 1.5; }
.runtime-credential-hint a { color: #148b7e; margin-left: 4px; text-decoration: none; }
.runtime-credential-hint a:hover { text-decoration: underline; }
</style>

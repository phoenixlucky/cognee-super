<script setup lang="ts">
import { onMounted } from 'vue'
import { Save, ShieldCheck } from '@lucide/vue'
import { useAppStore } from '../stores/app'

const app = useAppStore()
onMounted(() => { void app.refreshStatus(); void app.refreshSecrets() })
</script>

<template>
  <div class="page-heading">
    <div><div class="eyebrow">本地运行环境</div><h1>运行设置</h1><p>配置服务端口、数据目录和模型提供方。</p></div>
    <button class="button button-primary" :disabled="app.busy" @click="app.saveSettings"><Save :size="14"/>保存设置</button>
  </div>
  <div v-if="app.lastError" class="alert-banner">{{ app.lastError }}</div>
  <article class="surface content-card">
    <div class="content-toolbar"><div><h2>服务地址</h2><span class="muted">服务只绑定 127.0.0.1，本机应用可访问</span></div></div>
    <div class="two-col"><div class="field"><label>Cognee API 端口</label><input v-model.number="app.config.apiPort" type="number" min="1024" max="65535"></div><div class="field"><label>MCP 服务端口</label><input v-model.number="app.config.mcpPort" type="number" min="1024" max="65535"></div></div>
    <div class="field"><label>记忆数据目录</label><input v-model="app.config.dataRoot" placeholder="默认使用应用数据目录"></div>
  </article>
  <article class="surface content-card" style="margin-top:15px">
    <div class="content-toolbar"><div><h2>模型提供方</h2><span class="muted">密钥写入 Windows 凭据管理器，不会回显</span></div><ShieldCheck :size="19" color="#138f7e"/></div>
    <div class="two-col"><div class="field"><label>LLM 提供方</label><input v-model="app.config.llmProvider" placeholder="例如 openai、ollama"></div><div class="field"><label>LLM 模型</label><input v-model="app.config.llmModel" placeholder="模型名称"></div></div>
    <div class="field"><label>LLM API 地址（可选）</label><input v-model="app.config.llmEndpoint" placeholder="https://... 或本地模型地址"></div>
    <div class="field"><label>LLM API 密钥 {{ app.secrets.llm ? '· 已保存' : '' }}</label><div class="secret-row"><input v-model="app.secretInputs.llm" type="password" autocomplete="new-password" placeholder="留空表示不更改"><button v-if="app.secrets.llm" class="button button-quiet" @click="app.clearSecret('llm')">清除密钥</button></div><small class="muted">{{ app.secrets.llm ? '密钥保存在 Windows 凭据库' : '尚未配置密钥' }}</small></div>
    <div class="two-col"><div class="field"><label>Embedding 提供方</label><input v-model="app.config.embeddingProvider" placeholder="留空时使用 Cognee 默认策略"></div><div class="field"><label>Embedding 模型</label><input v-model="app.config.embeddingModel" placeholder="模型名称"></div></div>
    <div class="field"><label>Embedding API 密钥 {{ app.secrets.embedding ? '· 已保存' : '' }}</label><div class="secret-row"><input v-model="app.secretInputs.embedding" type="password" autocomplete="new-password" placeholder="留空表示不更改"><button v-if="app.secrets.embedding" class="button button-quiet" @click="app.clearSecret('embedding')">清除密钥</button></div><small class="muted">不配置远程 Embedding 时，Cognee 可使用本地 FastEmbed。</small></div>
    <div class="two-col"><div class="field"><label>图数据库</label><input v-model="app.config.graphDatabaseProvider" placeholder="ladybug"></div><div class="field"><label>向量数据库</label><input v-model="app.config.vectorDbProvider" placeholder="lancedb"></div></div>
    <div class="field"><label>向量数据库地址</label><input v-model="app.config.vectorDbUrl" placeholder="本地存储可留空"></div>
    <div class="field"><label>向量数据库密钥 {{ app.secrets.vector ? '· 已保存' : '' }}</label><div class="secret-row"><input v-model="app.secretInputs.vector" type="password" autocomplete="new-password" placeholder="留空表示不更改"><button v-if="app.secrets.vector" class="button button-quiet" @click="app.clearSecret('vector')">清除密钥</button></div></div>
    <p class="muted">修改运行中 API 的配置后，需要重启 API 与 MCP 才会生效；运行中的任务可能被中断。</p>
  </article>
</template>

<style scoped>.secret-row{display:flex;gap:8px;align-items:center}.secret-row input{flex:1;min-width:0}</style>

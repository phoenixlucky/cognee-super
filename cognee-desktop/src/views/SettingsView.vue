<script setup lang="ts">
import { onMounted } from 'vue'
import { Save, ShieldCheck } from '@lucide/vue'
import { useAppStore } from '../stores/app'

const app = useAppStore()
const modelTiers = [
  { name: '低', model: 'qwen2.5:14b-instruct-q2_K', size: '约 5.8 GB', memory: '建议 16 GB 内存起' },
  { name: '中', model: 'qwen2.5:14b-instruct-q4_K_M', size: '约 9 GB', memory: '建议 24 GB 内存起' },
  { name: '高', model: 'qwen2.5:32b-instruct-q4_K_M', size: '约 20 GB', memory: '建议 32 GB 内存起' },
]
onMounted(() => { void app.refreshStatus(); void app.refreshSecrets(); void app.refreshOllamaStatus() })
</script>

<template>
  <div class="page-heading">
    <div><div class="eyebrow">本地运行环境</div><h1>运行设置</h1><p>配置服务端口、数据目录和模型提供方。</p></div>
    <button class="button button-primary" :disabled="app.busy" @click="app.saveSettings"><Save :size="14"/>保存设置</button>
  </div>
  <div v-if="app.lastError" class="alert-banner">{{ app.lastError }}</div>
  <article class="surface content-card setup-guide">
    <div class="content-toolbar"><div><h2>首次配置：安装 Ollama</h2><span class="muted">本地运行 LLM，无需云端 API 账号</span></div><span class="ollama-state" :class="{ ready: app.ollamaAvailable }">{{ app.ollamaChecking ? '正在检查…' : app.ollamaAvailable ? '已检测到 Ollama' : '尚未检测到 Ollama' }}</span></div>
    <ol class="setup-steps">
      <li>下载安装 <a href="https://ollama.com/download/windows" target="_blank" rel="noreferrer">Ollama for Windows</a>，安装后保持 Ollama 在后台运行。</li>
      <li>选择下方低、中或高档，点击“下载所选模型”。模型文件较大，首次下载需要一些时间。</li>
      <li>下载完成后保存设置并启动 Cognee；首次使用中文向量模型时，FastEmbed 会自动下载约 90 MB 的模型文件。</li>
    </ol>
    <div class="guide-actions"><button class="button button-quiet" :disabled="app.ollamaChecking" @click="app.refreshOllamaStatus">{{ app.ollamaChecking ? '正在检查…' : '重新检查 Ollama' }}</button><span v-if="!app.ollamaAvailable" class="muted">安装后若仍未检测到，请重启 Cognee Desktop。</span></div>
  </article>
  <article class="surface content-card">
    <div class="content-toolbar"><div><h2>服务地址</h2><span class="muted">服务只绑定 127.0.0.1，本机应用可访问</span></div></div>
    <div class="two-col"><div class="field"><label>Cognee API 端口</label><input v-model.number="app.config.apiPort" type="number" min="1024" max="65535"></div><div class="field"><label>MCP 服务端口</label><input v-model.number="app.config.mcpPort" type="number" min="1024" max="65535"></div></div>
    <div class="field"><label>记忆数据目录</label><input v-model="app.config.dataRoot" placeholder="默认使用应用数据目录"></div>
  </article>
  <article class="surface content-card" style="margin-top:15px">
    <div class="content-toolbar"><div><h2>模型提供方</h2><span class="muted">密钥写入 Windows 凭据管理器，不会回显</span></div><ShieldCheck :size="19" color="#138f7e"/></div>
    <div class="two-col"><div class="field"><label>LLM 提供方</label><input v-model="app.config.llmProvider" placeholder="ollama"></div><div class="field"><label>LLM 模型档位</label><select v-model="app.config.llmModel"><option v-for="tier in modelTiers" :key="tier.model" :value="tier.model">{{ tier.name }}档 · {{ tier.model }} · {{ tier.size }} · {{ tier.memory }}</option></select></div></div>
    <div class="model-download"><button class="button button-primary" :disabled="!app.ollamaAvailable || !!app.pullingOllamaModel || app.busy" @click="app.pullOllamaModel">{{ app.pullingOllamaModel ? `正在下载 ${app.pullingOllamaModel}…` : '下载所选模型' }}</button><span class="muted">下载输出可在“服务日志”查看。切换档位只更换 LLM，Embedding 保持不变。</span></div>
    <div class="field"><label>LLM API 地址（Ollama 默认地址）</label><input v-model="app.config.llmEndpoint" placeholder="http://localhost:11434"></div>
    <div v-if="app.config.llmProvider === 'ollama'" class="field"><label>LLM API 密钥</label><div class="local-key-note">Ollama 本地调用不校验密钥；桌面端会自动使用 `ollama`，无需手动填写。</div></div>
    <div v-else class="field"><label>LLM API 密钥 {{ app.secrets.llm ? '· 已保存' : '' }}</label><div class="secret-row"><input v-model="app.secretInputs.llm" type="password" autocomplete="new-password" placeholder="留空表示不更改"><button v-if="app.secrets.llm" class="button button-quiet" @click="app.clearSecret('llm')">清除密钥</button></div><small class="muted">{{ app.secrets.llm ? '密钥保存在 Windows 凭据库' : '尚未配置密钥' }}</small></div>
    <div class="two-col"><div class="field"><label>Embedding 提供方（固定）</label><input v-model="app.config.embeddingProvider" readonly></div><div class="field"><label>Embedding 模型（固定）</label><input v-model="app.config.embeddingModel" readonly></div></div>
    <div class="field"><label>Embedding API 密钥 {{ app.secrets.embedding ? '· 已保存' : '' }}</label><div class="secret-row"><input v-model="app.secretInputs.embedding" type="password" autocomplete="new-password" placeholder="FastEmbed 本地运行，无需密钥"><button v-if="app.secrets.embedding" class="button button-quiet" @click="app.clearSecret('embedding')">清除密钥</button></div><small class="muted">Embedding 模型保持固定，避免切换后与已有向量维度不匹配。</small></div>
    <div class="two-col"><div class="field"><label>图数据库</label><input v-model="app.config.graphDatabaseProvider" placeholder="ladybug"></div><div class="field"><label>向量数据库</label><input v-model="app.config.vectorDbProvider" placeholder="lancedb"></div></div>
    <div class="field"><label>向量数据库地址</label><input v-model="app.config.vectorDbUrl" placeholder="本地存储可留空"></div>
    <div class="field"><label>向量数据库密钥 {{ app.secrets.vector ? '· 已保存' : '' }}</label><div class="secret-row"><input v-model="app.secretInputs.vector" type="password" autocomplete="new-password" placeholder="留空表示不更改"><button v-if="app.secrets.vector" class="button button-quiet" @click="app.clearSecret('vector')">清除密钥</button></div></div>
    <p class="muted">修改运行中 API 的配置后，需要重启 API 与 MCP 才会生效；运行中的任务可能被中断。</p>
  </article>
</template>

<style scoped>
.secret-row{display:flex;gap:8px;align-items:center}.secret-row input{flex:1;min-width:0}
.setup-guide{margin-bottom:15px}.setup-steps{margin:0 0 12px;padding-left:20px;color:#667486;font-size:11px;line-height:1.9}.setup-steps a{color:#128b7d;font-weight:600}.guide-actions,.model-download{display:flex;align-items:center;gap:12px;flex-wrap:wrap}.ollama-state{font-size:11px;color:#a06c12}.ollama-state.ready{color:#138f7e}.local-key-note{padding:10px;border-radius:6px;background:#f5f8fa;color:#667486;font-size:11px}.model-download{margin:-2px 0 14px}
</style>

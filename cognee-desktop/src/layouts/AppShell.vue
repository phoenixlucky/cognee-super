<script setup lang="ts">
import { computed, onMounted, onUnmounted } from 'vue'
import { useRoute, RouterLink } from 'vue-router'
import { Activity, Boxes, Brain, CircleHelp, Command, FileStack, Settings2, Workflow, ScrollText, Radio } from '@lucide/vue'
import { useAppStore } from '../stores/app'

const route = useRoute()
const app = useAppStore()
let statusTimer: ReturnType<typeof setInterval>
onMounted(() => { void app.refreshStatus(); statusTimer = setInterval(() => { void app.refreshStatus() }, 5000) })
onUnmounted(() => clearInterval(statusTimer))
const title = computed(() => String(route.meta.title ?? '总览'))
const nav = [
  { to: '/', label: '工作台', icon: Boxes, group: '管理' },
  { to: '/mcp', label: 'MCP 服务', icon: Radio, group: '管理' },
  { to: '/datasets', label: '记忆文件', icon: FileStack, group: '记忆' },
  { to: '/graph', label: '知识图谱', icon: Workflow, group: '记忆' },
  { to: '/sessions', label: '会话记录', icon: Brain, group: '记忆' },
  { to: '/settings', label: '运行设置', icon: Settings2, group: '系统' },
  { to: '/logs', label: '服务日志', icon: ScrollText, group: '系统' },
]
</script>

<template>
  <div class="app-shell">
    <aside class="sidebar">
      <div class="brand"><div class="brand-mark"><Command :size="19" /></div><div><strong>cognee</strong><span>LOCAL MEMORY</span></div></div>
      <div class="workspace-pill"><span class="workspace-dot"></span><span>本地工作区</span><span class="chevron">⌄</span></div>
      <nav class="side-nav">
        <template v-for="group in ['管理', '记忆', '系统']" :key="group">
          <p class="nav-label">{{ group }}</p>
          <RouterLink v-for="item in nav.filter((n) => n.group === group)" :key="item.to" :to="item.to" class="nav-item" active-class="active">
            <component :is="item.icon" :size="17" :stroke-width="1.8" /><span>{{ item.label }}</span><span v-if="item.to === '/mcp' && app.mcpStatus === 'running'" class="nav-live"></span>
          </RouterLink>
        </template>
      </nav>
      <div class="sidebar-bottom"><div class="connection-state"><span class="pulse" :class="app.apiStatus"></span><span>{{ app.apiStatus === 'running' ? 'Cognee 已连接' : '服务未启动' }}</span><button aria-label="检查状态" @click="app.refreshStatus"><Activity :size="15" /></button></div><div class="profile"><div class="avatar">C</div><div><strong>本地实例</strong><small>Windows · x64</small></div><CircleHelp :size="16" class="profile-help" /></div></div>
    </aside>
    <main class="main-area">
      <header class="topbar"><div class="breadcrumbs"><span>Cognee</span><span class="slash">/</span><strong>{{ title }}</strong></div><div class="top-actions"><span class="secure-label"><span class="secure-dot"></span>仅本机访问</span><button class="icon-button" aria-label="服务日志" @click="$router.push('/logs')"><Activity :size="17" /></button><div class="top-avatar">C</div></div></header>
      <section class="page-content"><RouterView /></section>
    </main>
  </div>
</template>

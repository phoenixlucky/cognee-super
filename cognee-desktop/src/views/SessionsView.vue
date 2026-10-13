<script setup lang="ts">
import { onMounted } from 'vue'
import { RotateCw, Brain } from '@lucide/vue'
import { useSessionsStore } from '../stores/sessions'
const sessions = useSessionsStore()
onMounted(() => sessions.load())
</script>
<template>
  <div class="page-heading"><div><div class="eyebrow">交互记忆</div><h1>会话记录</h1><p>浏览 Cognee 保存的会话摘要与处理状态。</p></div><button class="button button-quiet" @click="sessions.load"><RotateCw :size="14"/>刷新</button></div>
  <article class="surface content-card"><div class="content-toolbar"><h2>最近会话</h2><el-select v-model="sessions.range" style="width:120px" @change="sessions.load"><el-option label="全部时间" value="all"/><el-option label="最近 24 小时" value="24h"/><el-option label="最近 7 天" value="7d"/><el-option label="最近 30 天" value="30d"/></el-select></div>
    <el-table :data="sessions.items" stripe @row-click="sessions.loadDetail"><el-table-column prop="sessionId" label="会话 ID" min-width="180"/><el-table-column prop="summary" label="摘要"/><el-table-column prop="status" label="状态" width="120"/><el-table-column prop="updatedAt" label="更新时间" width="180"/></el-table>
    <div v-if="!sessions.items.length" class="empty-state"><div class="empty-icon"><Brain :size="22"/></div><strong>{{ sessions.loading?'正在加载会话…':'暂无会话记录' }}</strong><p>已清理敏感 trace 内容，只展示安全的会话元信息。</p></div>
  </article>
  <el-dialog :model-value="!!sessions.selected" title="会话摘要" width="520px" @close="sessions.selected=null"><template v-if="sessions.selected"><p><strong>状态：</strong>{{ sessions.selected.status }}</p><p><strong>开始：</strong>{{ sessions.selected.startedAt || '—' }}</p><p><strong>结束：</strong>{{ sessions.selected.endedAt || '仍在运行或未记录' }}</p><p><strong>摘要：</strong>{{ sessions.selected.summary || '暂无安全摘要' }}</p></template></el-dialog>
</template>

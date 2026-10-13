<script setup lang="ts">
import { onMounted } from 'vue'
import { Brain, RotateCw } from '@lucide/vue'
import { useSessionsStore } from '../stores/sessions'

const sessions = useSessionsStore()
onMounted(() => sessions.load())
</script>

<template>
  <div class="page-heading">
    <div><div class="eyebrow">交互记忆</div><h1>会话记录</h1><p>浏览 Cognee 保存的会话摘要与处理状态。</p></div>
    <button class="button button-quiet" :disabled="sessions.loading" @click="sessions.load(sessions.page)"><RotateCw :size="14"/>刷新</button>
  </div>
  <article class="surface content-card">
    <div class="content-toolbar">
      <h2>会话 <span class="muted">· {{ sessions.total }} 条</span></h2>
      <div class="toolbar-actions">
        <el-select v-model="sessions.status" style="width:140px" @change="sessions.load(1)">
          <el-option label="全部状态" value=""/><el-option label="运行中" value="running"/><el-option label="已完成" value="completed"/><el-option label="失败" value="failed"/><el-option label="已归档" value="abandoned"/>
        </el-select>
        <el-select v-model="sessions.range" style="width:140px" @change="sessions.load(1)">
          <el-option label="全部时间" value="all"/><el-option label="最近 24 小时" value="24h"/><el-option label="最近 7 天" value="7d"/><el-option label="最近 30 天" value="30d"/>
        </el-select>
      </div>
    </div>
    <div v-if="sessions.error" class="alert-banner">{{ sessions.error }}</div>
    <el-table :data="sessions.items" stripe v-loading="sessions.loading" @row-click="sessions.loadDetail">
      <el-table-column prop="sessionId" label="会话 ID" min-width="180"/><el-table-column prop="summary" label="摘要"/><el-table-column prop="status" label="状态" width="120"/><el-table-column prop="updatedAt" label="更新时间" width="180"/>
    </el-table>
    <div v-if="!sessions.items.length && !sessions.loading" class="empty-state"><div class="empty-icon"><Brain :size="22"/></div><strong>暂无会话记录</strong><p>这里只展示安全的会话摘要与状态，不包含内部 trace。</p></div>
    <div class="content-toolbar" style="margin-top:16px">
      <span class="muted">第 {{ sessions.page }} 页</span>
      <div class="toolbar-actions">
        <button class="button button-quiet" :disabled="sessions.page<=1 || sessions.loading" @click="sessions.load(sessions.page-1)">上一页</button>
        <button class="button button-quiet" :disabled="!sessions.hasMore || sessions.loading" @click="sessions.load(sessions.page+1)">下一页</button>
      </div>
    </div>
  </article>
  <el-dialog :model-value="!!sessions.selected" title="会话摘要" width="520px" @close="sessions.selected=null">
    <template v-if="sessions.selected"><p><strong>状态：</strong>{{ sessions.selected.status }}</p><p><strong>开始：</strong>{{ sessions.selected.startedAt || '—' }}</p><p><strong>结束：</strong>{{ sessions.selected.endedAt || '仍在运行或未记录' }}</p><p><strong>摘要：</strong>{{ sessions.selected.summary || '暂无安全摘要' }}</p></template>
  </el-dialog>
</template>

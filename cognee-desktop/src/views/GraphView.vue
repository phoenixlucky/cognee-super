<script setup lang="ts">
import { computed, onMounted } from 'vue'
import { Search, Workflow } from '@lucide/vue'
import { useGraphStore } from '../stores/graph'
import { useMemoryStore } from '../stores/memory'
import { VueFlow } from '@vue-flow/core'
import { Background } from '@vue-flow/background'
import { Controls } from '@vue-flow/controls'
const graph = useGraphStore(); const memory = useMemoryStore()
const selectedDetails = computed(() => {
  const value = graph.selectedNode?.data ?? graph.selectedEdge
  return value ? JSON.stringify(value, null, 2).slice(0, 5000) : ''
})
onMounted(() => { if (!memory.datasets.length) memory.loadDatasets() })
</script>
<template><div class="page-heading"><div><div class="eyebrow">关联记忆</div><h1>知识图谱</h1><p>查看 Cognee 从来源文件中提取的实体与关系。</p></div><div class="toolbar-actions"><el-select v-model="graph.datasetId" placeholder="选择数据集" style="width:190px"><el-option v-for="d in memory.datasets" :key="d.id" :label="d.name" :value="d.id"/></el-select><el-input v-model="graph.query" placeholder="筛选或搜索" style="width:190px"><template #prefix><Search :size="14"/></template></el-input><button class="button button-primary" :disabled="!graph.datasetId || graph.loading" @click="graph.load"><Workflow :size="14"/>加载图谱</button></div></div><article class="surface content-card"><div v-if="graph.error" class="alert-banner">{{ graph.error }}</div><div v-if="graph.nodes.length" class="graph-canvas"><VueFlow v-model:nodes="graph.nodes" v-model:edges="graph.edges" fit-view-on-init @node-click="({node})=>graph.selectNode(node)" @edge-click="({edge})=>graph.selectEdge(edge)"><Background/><Controls/></VueFlow></div><div v-else class="empty-state"><div class="empty-icon"><Workflow :size="23"/></div><strong>{{ graph.loading ? '正在构建图谱视图…' : '选择数据集以浏览图谱' }}</strong><p>默认最多展示 500 个节点，避免大图影响界面响应。</p></div><div v-if="graph.nodes.length" class="muted" style="margin-top:10px">显示 {{ graph.nodes.length }} 个节点、{{ graph.edges.length }} 条关系<span v-if="graph.truncated"> · 结果已截断</span></div><div v-if="selectedDetails" class="node-details"><strong>{{ graph.selectedNode ? `节点：${graph.selectedNode.label}` : `关系：${graph.selectedEdge?.label || '关联'}` }}</strong><pre>{{ selectedDetails }}</pre></div></article></template>
<style scoped>.graph-canvas{height:560px;border-radius:8px;overflow:hidden;border:1px solid #e9eef3;background:#f9fbfc}.node-details{margin-top:18px;padding-top:15px;border-top:1px solid #e9eef3}.node-details pre{max-height:260px;overflow:auto;padding:12px;border-radius:7px;background:#f5f7f8;color:#52616b;font-size:12px;white-space:pre-wrap}</style>

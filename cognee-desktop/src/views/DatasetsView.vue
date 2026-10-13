<script setup lang="ts">
import { onMounted } from 'vue'
import { FilePlus2, FolderOpen, RotateCw, Trash2 } from '@lucide/vue'
import { useMemoryStore } from '../stores/memory'

const memory = useMemoryStore()
onMounted(() => memory.loadDatasets())
function formatSize(value?: number | null) {
  if (value == null) return '—'
  if (value < 1024) return `${value} B`
  if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} KB`
  return `${(value / (1024 * 1024)).toFixed(1)} MB`
}
</script>

<template>
  <div class="page-heading">
    <div><div class="eyebrow">数据集与来源</div><h1>记忆文件</h1><p>管理进入 Cognee 的原始文件与已处理的数据集。</p></div>
    <div class="heading-actions"><button class="button button-quiet" @click="memory.loadDatasets"><RotateCw :size="14"/>刷新</button><button class="button button-primary" @click="memory.createDataset"><FilePlus2 :size="15"/>新建数据集</button></div>
  </div>
  <div v-if="memory.error" class="alert-banner"><span>{{ memory.error }}</span></div>
  <article class="surface content-card">
    <div class="content-toolbar"><h2>数据集 <span class="muted">{{ memory.datasets.length }} 个</span></h2></div>
    <el-table v-if="memory.datasets.length" :data="memory.datasets" stripe v-loading="memory.loading" @row-click="memory.selectDataset">
      <el-table-column prop="name" label="名称"/><el-table-column prop="id" label="ID" min-width="210"/><el-table-column prop="createdAt" label="创建时间"/>
      <el-table-column label="操作" width="90"><template #default="{row}"><button class="link-button" @click.stop="memory.deleteDataset(row)"><Trash2 :size="14"/> 删除</button></template></el-table-column>
    </el-table>
    <div v-else class="empty-state"><div class="empty-icon"><FolderOpen :size="22"/></div><strong>{{ memory.loading ? '正在读取数据集…' : '还没有数据集' }}</strong><p>新建数据集后，导入来源文件并构建可检索的图谱记忆。</p><button class="button button-primary" @click="memory.createDataset"><FilePlus2 :size="14"/>新建第一个数据集</button></div>
  </article>
  <article v-if="memory.selected" class="surface content-card" style="margin-top:15px">
    <div class="content-toolbar">
      <div><h2>{{ memory.selected.name }} · 来源文件</h2><span class="muted">数据集 ID {{ memory.selected.id }}</span></div>
      <div class="toolbar-actions"><button class="button button-primary" :disabled="memory.loading || memory.building" @click="memory.uploadSource"><FilePlus2 :size="14"/>导入文件</button><button class="button button-quiet" :disabled="memory.building" @click="memory.buildMemory">{{ memory.building ? '正在构建记忆…' : '构建图谱记忆' }}</button><button class="button button-quiet" @click="memory.loadDataPage(memory.selected.id,50,0);memory.loadProgress()"><RotateCw :size="14"/>刷新</button></div>
    </div>
    <div v-if="memory.building || memory.progress.status !== 'idle'" class="pipeline-status" :class="memory.progress.status">
      <span>记忆构建：{{ memory.progress.status === 'running' ? '处理中' : memory.progress.status === 'completed' ? '已完成' : '失败' }}</span>
      <span v-if="memory.progress.totalItems">{{ memory.progress.completedItems || 0 }} / {{ memory.progress.totalItems }}</span>
      <span v-if="memory.progress.currentStage" class="muted">{{ memory.progress.currentStage }}</span>
    </div>
    <el-table :data="memory.records" stripe v-loading="memory.loading">
      <el-table-column prop="name" label="文件名" min-width="190"/><el-table-column prop="fileType" label="类型" width="110"/><el-table-column label="大小" width="110"><template #default="{row}">{{ formatSize(row.size) }}</template></el-table-column><el-table-column prop="createdAt" label="添加时间" width="180"/>
      <el-table-column label="操作" width="160"><template #default="{row}"><button v-if="/\.(txt|md|csv|json|log|yaml|yml|xml)$/i.test(row.name)" class="link-button" @click="memory.previewRaw(row)">预览</button><button class="link-button" @click="memory.downloadRaw(row)">下载</button><button class="link-button" @click="memory.deleteData(row)"><Trash2 :size="13"/></button></template></el-table-column>
    </el-table>
    <div v-if="!memory.records.length && !memory.loading" class="empty-state" style="min-height:150px"><div class="empty-icon"><FilePlus2 :size="20"/></div><strong>数据集还没有来源文件</strong><p>选择本地原始文件进行导入。</p></div>
  </article>
</template>

<style scoped>.pipeline-status{display:flex;gap:16px;align-items:center;margin:0 0 12px;padding:10px 12px;background:#f1f7f5;border-radius:7px;color:#46645a;font-size:13px}.pipeline-status.failed{background:#fff4f1;color:#9c4738}</style>

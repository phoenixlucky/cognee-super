import { createRouter, createWebHashHistory } from 'vue-router'
import AppShell from '../layouts/AppShell.vue'
import DashboardView from '../views/DashboardView.vue'
import McpView from '../views/McpView.vue'
import DatasetsView from '../views/DatasetsView.vue'
import GraphView from '../views/GraphView.vue'
import SessionsView from '../views/SessionsView.vue'
import SettingsView from '../views/SettingsView.vue'
import LogsView from '../views/LogsView.vue'

export default createRouter({
  history: createWebHashHistory(),
  routes: [{ path: '/', component: AppShell, children: [
    { path: '', name: 'dashboard', component: DashboardView, meta: { title: '总览', eyebrow: '工作台' } },
    { path: 'mcp', name: 'mcp', component: McpView, meta: { title: 'MCP 服务', eyebrow: '连接与工具' } },
    { path: 'datasets', name: 'datasets', component: DatasetsView, meta: { title: '记忆文件', eyebrow: '数据集与来源' } },
    { path: 'graph', name: 'graph', component: GraphView, meta: { title: '知识图谱', eyebrow: '关联记忆' } },
    { path: 'sessions', name: 'sessions', component: SessionsView, meta: { title: '会话记录', eyebrow: '交互记忆' } },
    { path: 'settings', name: 'settings', component: SettingsView, meta: { title: '设置', eyebrow: '本地运行环境' } },
    { path: 'logs', name: 'logs', component: LogsView, meta: { title: '运行日志', eyebrow: '诊断信息' } },
  ] }],
})

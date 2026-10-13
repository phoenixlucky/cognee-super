import { defineStore } from 'pinia'
import { invoke } from '@tauri-apps/api/core'
import { ElMessage, ElMessageBox } from 'element-plus'
import type { Dataset, DataRecord } from '../types/memory'

interface DatasetProgress { status: 'idle' | 'running' | 'completed' | 'failed'; completedItems?: number | null; totalItems?: number | null; currentStage?: string | null }
const pause = (ms: number) => new Promise(resolve => setTimeout(resolve, ms))

export const useMemoryStore = defineStore('memory', {
  state: () => ({ datasets: [] as Dataset[], records: [] as DataRecord[], selected: null as Dataset | null, loading: false, building: false, progress: { status: 'idle' } as DatasetProgress, error: '' }),
  actions: {
    async loadDatasets() {
      this.loading = true
      this.error = ''
      try { this.datasets = await invoke<Dataset[]>('list_datasets') }
      catch (error) { this.error = String(error) }
      finally { this.loading = false }
    },
    async selectDataset(row: Dataset) {
      this.selected = row
      await Promise.all([this.loadDataPage(row.id, 50, 0), this.loadProgress()])
    },
    async loadDataPage(datasetId: string, limit: number, offset: number) {
      this.loading = true
      try { this.records = await invoke<DataRecord[]>('list_data', { datasetId, limit, offset }) }
      catch (error) { this.error = String(error) }
      finally { this.loading = false }
    },
    async loadProgress() {
      if (!this.selected) return
      try { this.progress = await invoke<DatasetProgress>('get_dataset_progress', { datasetId: this.selected.id }) }
      catch (error) { this.error = String(error) }
    },
    async createDataset() {
      try {
        const { value } = await ElMessageBox.prompt('为新的记忆集合命名', '新建数据集', { inputPlaceholder: '例如：项目文档' })
        await invoke('create_dataset', { name: value })
        await this.loadDatasets()
      } catch (error) { if (error !== 'cancel' && error !== 'close') this.error = String(error) }
    },
    async uploadSource() {
      if (!this.selected) return
      try {
        await invoke('upload_source', { datasetId: this.selected.id })
        await Promise.all([this.loadDataPage(this.selected.id, 50, 0), this.loadProgress()])
      } catch (error) { this.error = String(error) }
    },
    async buildMemory() {
      if (!this.selected || this.building) return
      this.building = true
      this.error = ''
      this.progress = { status: 'running' }
      const operation = invoke('cognify_dataset', { datasetId: this.selected.id })
      const completion = operation.then(() => true, () => false)
      const deadline = Date.now() + 60 * 60 * 1000
      try {
        let done = false
        while (!done && Date.now() < deadline) {
          const result = await Promise.race([completion.then(ok => ({ done: true, ok })), pause(1200).then(() => ({ done: false, ok: true }))])
          done = result.done
          await this.loadProgress()
          if (this.progress.status === 'idle') this.progress.status = 'running'
          if (this.progress.status === 'failed') break
        }
        if (!done && this.progress.status !== 'failed') throw new Error('记忆构建等待超时，请检查服务日志后重试。')
        await operation
        await this.loadProgress()
        if (this.progress.status !== 'failed') this.progress.status = 'completed'
      } catch (error) {
        this.error = String(error)
        this.progress.status = 'failed'
      } finally { this.building = false }
    },
    async previewRaw(row: DataRecord) {
      if (!this.selected) return
      try {
        const text = await invoke<string>('preview_raw', { datasetId: this.selected.id, dataId: row.id, filename: row.name })
        await ElMessageBox.alert(text, row.name, { confirmButtonText: '关闭', customClass: 'preview-dialog' })
      } catch (error) { if (error !== 'cancel') this.error = String(error) }
    },
    async downloadRaw(row: DataRecord) {
      if (!this.selected) return
      try {
        const path = await invoke<string | null>('download_raw', { datasetId: this.selected.id, dataId: row.id, filename: row.name })
        if (path) ElMessage.success(`文件已保存：${path}`)
      } catch (error) { this.error = String(error) }
    },
    async deleteData(row: DataRecord) {
      if (!this.selected) return
      try {
        await ElMessageBox.confirm(`确定删除「${row.name}」吗？此操作无法撤销。`, '删除来源文件', { type: 'warning' })
        await invoke('delete_data', { datasetId: this.selected.id, dataId: row.id })
        await this.loadDataPage(this.selected.id, 50, 0)
      } catch (error) { if (error !== 'cancel' && error !== 'close') this.error = String(error) }
    },
    async deleteDataset(row: Dataset) {
      try {
        await ElMessageBox.confirm(`确定删除数据集「${row.name}」及其记忆吗？此操作无法撤销。`, '删除数据集', { type: 'warning' })
        await invoke('delete_dataset', { datasetId: row.id })
        if (this.selected?.id === row.id) { this.selected = null; this.records = [] }
        await this.loadDatasets()
      } catch (error) { if (error !== 'cancel' && error !== 'close') this.error = String(error) }
    },
  },
})

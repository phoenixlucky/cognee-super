import { defineStore } from 'pinia'
import { invoke } from '@tauri-apps/api/core'
import { ElMessage, ElMessageBox } from 'element-plus'
import type { Dataset, DataRecord } from '../types/memory'
export const useMemoryStore = defineStore('memory', { state:()=>({datasets:[] as Dataset[],records:[] as DataRecord[],selected:null as Dataset|null,loading:false,error:''}), actions:{
 async loadDatasets(){this.loading=true;this.error='';try{this.datasets=await invoke<Dataset[]>('list_datasets')}catch(e){this.error=String(e)}finally{this.loading=false}},
 async selectDataset(row:Dataset){this.selected=row;await this.loadDataPage(row.id,50,0)},
 async loadDataPage(datasetId:string,limit:number,offset:number){this.loading=true;try{this.records=await invoke<DataRecord[]>('list_data',{datasetId,limit,offset})}catch(e){this.error=String(e)}finally{this.loading=false}},
 async createDataset(){try{const {ElMessageBox}=await import('element-plus');const {value}=await ElMessageBox.prompt('为新的记忆集合命名','新建数据集',{inputPlaceholder:'例如：项目文档'});await invoke('create_dataset',{name:value});await this.loadDatasets()}catch(e){if(e!=='cancel'&&e!=='close')this.error=String(e)}},
 async uploadSource(){if(!this.selected)return;try{const {open}=await import('@tauri-apps/plugin-dialog');const paths=await open({multiple:true,directory:false});if(paths)for(const path of Array.isArray(paths)?paths:[paths])await invoke('upload_source',{datasetId:this.selected.id,path});await this.loadDataPage(this.selected.id,50,0)}catch(e){this.error=String(e)}},
 async previewRaw(row:DataRecord){if(!this.selected)return;try{const text=await invoke<string>('preview_raw',{datasetId:this.selected.id,dataId:row.id,filename:row.name});await ElMessageBox.alert(text,row.name,{confirmButtonText:'关闭',customClass:'preview-dialog'})}catch(e){if(e!=='cancel')this.error=String(e)}},
 async downloadRaw(row:DataRecord){if(!this.selected)return;try{const path=await invoke<string>('download_raw',{datasetId:this.selected.id,dataId:row.id});ElMessage.success(`文件已保存：${path}`)}catch(e){this.error=String(e)}},
 async deleteData(row:DataRecord){if(!this.selected)return;try{const {ElMessageBox}=await import('element-plus');await ElMessageBox.confirm(`确定删除「${row.name}」吗？此操作无法撤销。`,'删除来源文件',{type:'warning'});await invoke('delete_data',{datasetId:this.selected.id,dataId:row.id});await this.loadDataPage(this.selected.id,50,0)}catch(e){if(e!=='cancel'&&e!=='close')this.error=String(e)}},
 async deleteDataset(row:Dataset){try{const {ElMessageBox}=await import('element-plus');await ElMessageBox.confirm(`确定删除数据集「${row.name}」及其记忆吗？此操作无法撤销。`,'删除数据集',{type:'warning'});await invoke('delete_dataset',{datasetId:row.id});if(this.selected?.id===row.id)this.selected=null;await this.loadDatasets()}catch(e){if(e!=='cancel'&&e!=='close')this.error=String(e)}}
}})

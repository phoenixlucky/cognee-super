import { defineStore } from 'pinia'
import { invoke } from '@tauri-apps/api/core'
import type { GraphDto } from '../types/graph'
export const useGraphStore=defineStore('graph',{state:()=>({datasetId:'',query:'',nodes:[] as GraphDto['nodes'],edges:[] as GraphDto['edges'],truncated:false,loading:false,error:''}),actions:{async load(){if(!this.datasetId)return;this.loading=true;this.error='';try{const g=await invoke<GraphDto>('get_graph',{datasetId:this.datasetId,query:this.query||null,maxNodes:500});this.nodes=g.nodes;this.edges=g.edges;this.truncated=g.truncated}catch(e){this.error=String(e)}finally{this.loading=false}}}})

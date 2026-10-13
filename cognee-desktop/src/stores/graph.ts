import { defineStore } from 'pinia'
import { invoke } from '@tauri-apps/api/core'
import type { GraphDto } from '../types/graph'
type SelectedNode = { id: string; label?: unknown; data?: unknown }
type SelectedEdge = { id: string; label?: unknown; source?: string; target?: string }
export const useGraphStore=defineStore('graph',{state:()=>({datasetId:'',query:'',nodes:[] as GraphDto['nodes'],edges:[] as GraphDto['edges'],selectedNode:null as SelectedNode|null,selectedEdge:null as SelectedEdge|null,truncated:false,loading:false,error:''}),actions:{async load(){if(!this.datasetId)return;this.loading=true;this.error='';this.selectedNode=null;this.selectedEdge=null;try{const g=await invoke<GraphDto>('get_graph',{datasetId:this.datasetId,query:this.query||null,maxNodes:500});this.nodes=g.nodes;this.edges=g.edges;this.truncated=g.truncated}catch(e){this.error=String(e)}finally{this.loading=false}},selectNode(node:SelectedNode){this.selectedNode=node;this.selectedEdge=null},selectEdge(edge:SelectedEdge){this.selectedEdge=edge;this.selectedNode=null}}})

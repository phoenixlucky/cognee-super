import { defineStore } from 'pinia'
import { invoke } from '@tauri-apps/api/core'
import type { SessionDetailDto, SessionDto } from '../types/sessions'
export const useSessionsStore=defineStore('sessions',{state:()=>({items:[] as SessionDto[],selected:null as SessionDetailDto|null,range:'7d' as '24h'|'7d'|'30d'|'all',loading:false}),actions:{async load(){this.loading=true;try{this.items=await invoke<SessionDto[]>('list_sessions',{range:this.range,status:null,limit:50,offset:0})}catch{this.items=[]}finally{this.loading=false}},async loadDetail(row:SessionDto){try{this.selected=await invoke<SessionDetailDto>('get_session',{sessionId:row.sessionId})}catch{this.selected={...row,startedAt:'',endedAt:''}}}}})

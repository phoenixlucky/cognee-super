export interface SessionDto { sessionId:string; summary:string; status:string; updatedAt:string }
export interface SessionDetailDto extends SessionDto { startedAt:string; endedAt:string }

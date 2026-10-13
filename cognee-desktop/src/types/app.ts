export type RunState = 'stopped' | 'starting' | 'running' | 'stopping' | 'failed'
export type RuntimeState = 'missing' | 'preparing' | 'ready' | 'failed'
export interface AppConfig {
  apiPort: number; mcpPort: number; dataRoot: string; llmProvider: string; llmModel: string; llmEndpoint: string;
  embeddingProvider: string; embeddingModel: string; apiHost: string; mcpHost: string;
  graphDatabaseProvider: string; vectorDbProvider: string; vectorDbUrl: string
}
export interface AppStatus { apiStatus: RunState; mcpStatus: RunState; runtimeStatus: RuntimeState; activeWork: boolean; config: AppConfig; apiError?: string; mcpError?: string }
export interface McpTool { name: string; description: string; inputSchema?: unknown }
export interface LogEntry { service: string; line: string; timestamp: string }

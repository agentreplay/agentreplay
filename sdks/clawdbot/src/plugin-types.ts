/**
 * Clawdbot Plugin API Types
 * 
 * These types are compatible with clawdbot's plugin system.
 * They are defined here to avoid requiring clawdbot as a build-time dependency.
 */

export interface PluginLogger {
  debug?: (message: string) => void;
  info: (message: string) => void;
  warn: (message: string) => void;
  error: (message: string) => void;
}

export interface PluginCommandResult {
  text: string;
}

export interface PluginCommandContext {
  senderId?: string;
  channel: string;
  isAuthorizedSender: boolean;
  args?: string;
  commandBody: string;
}

export interface PluginCommandDefinition {
  name: string;
  description: string;
  acceptsArgs?: boolean;
  requireAuth?: boolean;
  handler: (ctx: PluginCommandContext) => PluginCommandResult | Promise<PluginCommandResult>;
}

export interface PluginServiceContext {
  config: Record<string, unknown>;
  workspaceDir?: string;
  stateDir: string;
  logger: PluginLogger;
}

export interface PluginService {
  id: string;
  start: (ctx: PluginServiceContext) => void | Promise<void>;
  stop?: (ctx: PluginServiceContext) => void | Promise<void>;
}

export interface PluginConfigSchema {
  jsonSchema?: Record<string, unknown>;
  uiHints?: Record<string, { label?: string; help?: string; placeholder?: string }>;
}

export interface PluginHookContext {
  agentId?: string;
  sessionKey?: string;
  workspaceDir?: string;
  messageProvider?: string;
}

export interface PluginHookToolContext extends PluginHookContext {}

export interface BeforeAgentStartEvent {
  prompt: string;
  messages?: unknown[];
}

export interface AgentEndEvent {
  messages: unknown[];
  success: boolean;
  error?: string;
  durationMs?: number;
}

export interface BeforeToolCallEvent {
  toolName: string;
  toolInput: unknown;
}

export interface AfterToolCallEvent {
  toolName: string;
  toolInput: unknown;
  toolResult: unknown;
  error?: string;
}

export type HookHandler<E, C, R = void> = (event: E, ctx: C) => R | Promise<R>;

export interface PluginApi {
  id: string;
  name: string;
  version?: string;
  description?: string;
  source: string;
  config: Record<string, unknown>;
  pluginConfig?: Record<string, unknown>;
  logger: PluginLogger;
  
  registerService: (service: PluginService) => void;
  registerCommand: (command: PluginCommandDefinition) => void;
  
  on(hookName: "before_agent_start", handler: HookHandler<BeforeAgentStartEvent, PluginHookContext, { prependContext?: string } | undefined>): void;
  on(hookName: "agent_end", handler: HookHandler<AgentEndEvent, PluginHookContext>): void;
  on(hookName: "before_tool_call", handler: HookHandler<BeforeToolCallEvent, PluginHookToolContext, { skip?: boolean } | undefined>): void;
  on(hookName: "after_tool_call", handler: HookHandler<AfterToolCallEvent, PluginHookToolContext>): void;
  on(hookName: string, handler: HookHandler<unknown, unknown, unknown>): void;
}

export interface PluginDefinition {
  id: string;
  name?: string;
  description?: string;
  version?: string;
  configSchema?: PluginConfigSchema;
  register?: (api: PluginApi) => void | Promise<void>;
  activate?: (api: PluginApi) => void | Promise<void>;
}

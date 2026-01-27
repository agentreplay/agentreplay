/**
 * Flowtrace Integration Plugin for Clawdbot
 *
 * Automatically traces all agent activities to Flowtrace for observability.
 *
 * Installation:
 *   npm install @flowtrace/clawdbot-plugin
 *
 * Configuration (in clawdbot.json):
 * {
 *   "plugins": {
 *     "@flowtrace/clawdbot-plugin": {
 *       "enabled": true,
 *       "url": "http://localhost:9600",
 *       "tenant_id": 1,
 *       "project_id": 1
 *     }
 *   }
 * }
 *
 * Or via environment:
 * - FLOWTRACE_URL: Flowtrace server URL (default: http://localhost:9600)
 * - FLOWTRACE_TENANT_ID: Tenant ID (default: 1)
 * - FLOWTRACE_PROJECT_ID: Project ID (default: 1)
 */

import type { PluginApi, PluginDefinition } from "./src/plugin-types.js";

import { createFlowtraceService } from "./src/service.js";
import { flowtraceConfigSchema } from "./src/config.js";

const plugin: PluginDefinition = {
  id: "flowtrace",
  name: "Flowtrace Observability",
  description: "Automatic tracing of agent activities to Flowtrace for observability and monitoring",
  version: "0.1.0",
  configSchema: flowtraceConfigSchema,

  register(api: PluginApi) {
    // Register service for lifecycle management
    api.registerService(createFlowtraceService(api));

    // Register /flowtrace command for status
    api.registerCommand({
      name: "flowtrace",
      description: "Show Flowtrace integration status",
      acceptsArgs: false,
      requireAuth: true,
      handler: () => {
        const cfg = getFlowtraceConfig(api);
        return {
          text:
            `📊 **Flowtrace Status**\n\n` +
            `• Enabled: ${cfg.enabled ? "Yes" : "No"}\n` +
            `• Server: ${cfg.url}\n` +
            `• Tenant: ${cfg.tenantId}\n` +
            `• Project: ${cfg.projectId}`,
        };
      },
    });
  },
};

export function getFlowtraceConfig(api: PluginApi) {
  const pluginCfg = (api.pluginConfig || {}) as Record<string, unknown>;
  return {
    enabled:
      pluginCfg.enabled !== undefined
        ? Boolean(pluginCfg.enabled)
        : process.env.FLOWTRACE_ENABLED !== "false",
    url: (pluginCfg.url as string) || process.env.FLOWTRACE_URL || "http://localhost:9600",
    tenantId:
      typeof pluginCfg.tenant_id === "number"
        ? pluginCfg.tenant_id
        : parseInt(process.env.FLOWTRACE_TENANT_ID || "1", 10),
    projectId:
      typeof pluginCfg.project_id === "number"
        ? pluginCfg.project_id
        : parseInt(process.env.FLOWTRACE_PROJECT_ID || "1", 10),
  };
}

export default plugin;

/**
 * Agentreplay Integration Plugin for Clawdbot
 *
 * Automatically traces all agent activities to Agentreplay for observability.
 *
 * Installation:
 *   npm install @agentreplay/clawdbot-plugin
 *
 * Configuration (in clawdbot.json):
 * {
 *   "plugins": {
 *     "@agentreplay/clawdbot-plugin": {
 *       "enabled": true,
 *       "url": "http://localhost:9600",
 *       "tenant_id": 1,
 *       "project_id": 1
 *     }
 *   }
 * }
 *
 * Or via environment:
 * - AGENTREPLAY_URL: Agentreplay server URL (default: http://localhost:9600)
 * - AGENTREPLAY_TENANT_ID: Tenant ID (default: 1)
 * - AGENTREPLAY_PROJECT_ID: Project ID (default: 1)
 */

import type { PluginApi, PluginDefinition } from "./src/plugin-types.js";

import { createAgentreplayService } from "./src/service.js";
import { agentreplayConfigSchema } from "./src/config.js";

const plugin: PluginDefinition = {
  id: "agentreplay",
  name: "Agentreplay Observability",
  description: "Automatic tracing of agent activities to Agentreplay for observability and monitoring",
  version: "0.1.0",
  configSchema: agentreplayConfigSchema,

  register(api: PluginApi) {
    // Register service for lifecycle management
    api.registerService(createAgentreplayService(api));

    // Register /agentreplay command for status
    api.registerCommand({
      name: "agentreplay",
      description: "Show Agentreplay integration status",
      acceptsArgs: false,
      requireAuth: true,
      handler: () => {
        const cfg = getAgentreplayConfig(api);
        return {
          text:
            `📊 **Agentreplay Status**\n\n` +
            `• Enabled: ${cfg.enabled ? "Yes" : "No"}\n` +
            `• Server: ${cfg.url}\n` +
            `• Tenant: ${cfg.tenantId}\n` +
            `• Project: ${cfg.projectId}`,
        };
      },
    });
  },
};

export function getAgentreplayConfig(api: PluginApi) {
  const pluginCfg = (api.pluginConfig || {}) as Record<string, unknown>;
  return {
    enabled:
      pluginCfg.enabled !== undefined
        ? Boolean(pluginCfg.enabled)
        : process.env.AGENTREPLAY_ENABLED !== "false",
    url: (pluginCfg.url as string) || process.env.AGENTREPLAY_URL || "http://localhost:9600",
    tenantId:
      typeof pluginCfg.tenant_id === "number"
        ? pluginCfg.tenant_id
        : parseInt(process.env.AGENTREPLAY_TENANT_ID || "1", 10),
    projectId:
      typeof pluginCfg.project_id === "number"
        ? pluginCfg.project_id
        : parseInt(process.env.AGENTREPLAY_PROJECT_ID || "1", 10),
  };
}

export default plugin;

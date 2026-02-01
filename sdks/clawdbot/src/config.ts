/**
 * Agentreplay configuration schema
 */

export const agentreplayConfigSchema = {
  jsonSchema: {
    type: "object",
    properties: {
      enabled: {
        type: "boolean",
        default: true,
        description: "Enable or disable Agentreplay tracing",
      },
      url: {
        type: "string",
        default: "http://localhost:47100",
        description: "Agentreplay server URL",
      },
      tenant_id: {
        type: "number",
        default: 1,
        description: "Agentreplay tenant identifier",
      },
      project_id: {
        type: "number",
        default: 1,
        description: "Agentreplay project identifier",
      },
    },
  },
  uiHints: {
    enabled: {
      label: "Enable Agentreplay",
      help: "Enable or disable tracing to Agentreplay server",
    },
    url: {
      label: "Agentreplay URL",
      placeholder: "http://localhost:47100",
      help: "The URL of your Agentreplay server",
    },
    tenant_id: {
      label: "Tenant ID",
      help: "Agentreplay tenant identifier for multi-tenant deployments",
    },
    project_id: {
      label: "Project ID",
      help: "Agentreplay project identifier for organizing traces",
    },
  },
};

export interface AgentreplayConfig {
  enabled: boolean;
  url: string;
  tenantId: number;
  projectId: number;
}

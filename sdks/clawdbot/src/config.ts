/**
 * Flowtrace configuration schema
 */

export const flowtraceConfigSchema = {
  jsonSchema: {
    type: "object",
    properties: {
      enabled: {
        type: "boolean",
        default: true,
        description: "Enable or disable Flowtrace tracing",
      },
      url: {
        type: "string",
        default: "http://localhost:9600",
        description: "Flowtrace server URL",
      },
      tenant_id: {
        type: "number",
        default: 1,
        description: "Flowtrace tenant identifier",
      },
      project_id: {
        type: "number",
        default: 1,
        description: "Flowtrace project identifier",
      },
    },
  },
  uiHints: {
    enabled: {
      label: "Enable Flowtrace",
      help: "Enable or disable tracing to Flowtrace server",
    },
    url: {
      label: "Flowtrace URL",
      placeholder: "http://localhost:9600",
      help: "The URL of your Flowtrace server",
    },
    tenant_id: {
      label: "Tenant ID",
      help: "Flowtrace tenant identifier for multi-tenant deployments",
    },
    project_id: {
      label: "Project ID",
      help: "Flowtrace project identifier for organizing traces",
    },
  },
};

export interface FlowtraceConfig {
  enabled: boolean;
  url: string;
  tenantId: number;
  projectId: number;
}

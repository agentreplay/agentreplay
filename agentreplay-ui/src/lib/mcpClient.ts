
export interface McpRequest {
    jsonrpc: "2.0";
    method: string;
    params?: any;
    id: number | string;
}

export interface McpResponse<T = any> {
    jsonrpc: "2.0";
    result?: T;
    error?: {
        code: number;
        message: string;
        data?: any;
    };
    id: number | string;
}

const MCP_ENDPOINT = "http://127.0.0.1:9601/mcp";

export async function mcpCall<T = any>(
    method: string,
    params?: any
): Promise<T> {
    const request: McpRequest = {
        jsonrpc: "2.0",
        method,
        params,
        id: Date.now(),
    };

    const response = await fetch(MCP_ENDPOINT, {
        method: "POST",
        headers: {
            "Content-Type": "application/json",
        },
        body: JSON.stringify(request),
    });

    if (!response.ok) {
        throw new Error(`MCP request failed: ${response.status} ${response.statusText}`);
    }

    const data: McpResponse<T> = await response.json();

    if (data.error) {
        throw new Error(data.error.message);
    }

    return data.result as T;
}

// Helper functions for specific tools

export interface TraceSearchResult {
    edge_id: string;
    timestamp_us: number;
    operation: string;
    duration_ms: number;
    tokens: number;
    cost: number;
    relevance_score: number;
    payload_summary?: string;
    related_traces?: string[];
}

export async function searchTraces(
    query: string,
    limit: number = 20,
    spanTypes?: string[]
): Promise<{ results: TraceSearchResult[]; count: number }> {
    const response = await mcpCall('tools/call', {
        name: 'search_traces',
        arguments: {
            query,
            limit,
            span_types: spanTypes,
            include_payload: true,
            include_related: false // Can be toggled
        }
    });

    // The tool returns a text content with JSON string
    const content = response.content[0].text;
    return JSON.parse(content);
}

export async function getTraceDetails(edgeId: string): Promise<any> {
    const response = await mcpCall('tools/call', {
        name: 'get_trace_details',
        arguments: {
            edge_id: edgeId
        }
    });

    const content = response.content[0].text;
    return JSON.parse(content);
}

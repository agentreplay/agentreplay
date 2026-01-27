import { useState, useEffect } from 'react';
import { Terminal, Copy, Check, Info } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';

type UsageContext = 'observability' | 'memory' | 'claude';

interface EnvironmentConfigProps {
    projectId: string;
    projectName?: string;
    onCopy?: (text: string, label: string) => void;
    envVars?: Record<string, string>;
}

export function EnvironmentConfig({ projectId, envVars, onCopy }: EnvironmentConfigProps) {
    const [usageContext, setUsageContext] = useState<UsageContext>('observability');
    const [copiedEnvVar, setCopiedEnvVar] = useState<string | null>(null);
    const [bridgePath, setBridgePath] = useState<string>('');
    const [autoDetectError, setAutoDetectError] = useState<string | null>(null);

    // Default env vars if not provided
    const effectiveEnvVars = envVars || {
        FLOWTRACE_URL: 'http://localhost:9600',
        FLOWTRACE_TENANT_ID: 'default',
        FLOWTRACE_PROJECT_ID: projectId,
    };

    // Auto-detect bridge path when Claude context is selected
    useEffect(() => {
        if (usageContext === 'claude') {
            const detectPath = async () => {
                try {
                    // Use Tauri invoke to get the path
                    // @ts-ignore - invoke might not be fully typed in all contexts
                    if (typeof window !== 'undefined' && '__TAURI__' in window) {
                        try {
                            // @ts-ignore
                            const path = await window.__TAURI__.core.invoke('get_bridge_path');
                            setBridgePath(path);
                            setAutoDetectError(null);
                        } catch (e) {
                            console.warn("Using default path, failed to auto-detect:", e);
                            setAutoDetectError("Could not auto-detect path. Please verify installation.");
                        }
                    }
                } catch (error) {
                    console.error("Failed to detect bridge path:", error);
                }
            };
            detectPath();
        }
    }, [usageContext]);

    const handleCopy = (text: string, label: string) => {
        if (onCopy) {
            onCopy(text, label);
        } else {
            navigator.clipboard.writeText(text);
        }
        setCopiedEnvVar(label);
        setTimeout(() => setCopiedEnvVar(null), 2000);
    };

    return (
        <div className="space-y-6">
            {/* Context Selector */}
            <div className="flex p-1 bg-surface-elevated rounded-lg border border-border">
                {(['observability', 'memory', 'claude'] as const).map((context) => (
                    <button
                        key={context}
                        onClick={() => setUsageContext(context)}
                        className={`flex-1 py-2 px-4 text-sm font-medium rounded-md transition-all ${usageContext === context
                            ? 'bg-primary text-white shadow-sm'
                            : 'text-textSecondary hover:text-textPrimary hover:bg-surface-hover'
                            }`}
                    >
                        {context === 'observability'
                            ? 'Observability'
                            : context === 'memory'
                                ? 'Memory'
                                : 'Claude Code'}
                    </button>
                ))}
            </div>

            {usageContext === 'claude' ? (
                /* Claude Code Configuration */
                <div className="space-y-4">
                    <div className="p-4 bg-blue-500/10 border border-blue-500/20 rounded-lg">
                        <div className="flex items-start gap-3">
                            <Terminal className="w-5 h-5 text-blue-500 mt-0.5 flex-shrink-0" />
                            <div>
                                <h4 className="font-semibold text-blue-500 mb-1">
                                    Configure Claude Code
                                </h4>
                                <p className="text-sm text-textSecondary">
                                    Add this configuration to your <code>settings.json</code> (VS Code) or <code>claude_desktop_config.json</code> (Claude Desktop).
                                </p>
                                <div className="mt-2 text-xs text-textTertiary space-y-1 bg-surface rounded p-2 border border-border-subtle">
                                    <p className="font-medium text-textSecondary">Config Locations (macOS):</p>
                                    <ul className="list-disc pl-4 space-y-0.5">
                                        <li>
                                            <span className="font-medium">VS Code:</span> <code>Cmd+Shift+P</code> &gt; "Open User Settings (JSON)"
                                        </li>
                                        <li>
                                            <span className="font-medium">Claude Desktop:</span> <code>~/Library/Application Support/Claude/claude_desktop_config.json</code>
                                        </li>
                                    </ul>
                                </div>
                                {autoDetectError && (
                                    <p className="text-xs text-yellow-500 mt-2">Warning: {autoDetectError}</p>
                                )}
                            </div>
                        </div>

                        <div className="space-y-2">
                            <label className="block text-xs font-medium text-textSecondary uppercase tracking-wider">
                                Bridge Script Path (index.js)
                            </label>
                            <div className="flex gap-2">
                                <input
                                    type="text"
                                    value={bridgePath}
                                    onChange={(e) => setBridgePath(e.target.value)}
                                    className="flex-1 px-3 py-2 bg-surface text-sm border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/50 font-mono text-textSecondary"
                                    placeholder="/path/to/flowtrace-claude-bridge/dist/index.js"
                                />
                                <button
                                    onClick={async () => {
                                        try {
                                            const selected = await open({
                                                multiple: false,
                                                filters: [{
                                                    name: 'JavaScript',
                                                    extensions: ['js']
                                                }]
                                            });
                                            if (selected) {
                                                setBridgePath(selected as string);
                                                setAutoDetectError(null);
                                            }
                                        } catch (err) {
                                            console.error("Failed to open dialog:", err);
                                        }
                                    }}
                                    className="px-4 py-2 bg-surface-hover hover:bg-surface-elevated border border-border rounded-lg text-sm font-medium text-textPrimary transition-colors"
                                >
                                    Browse...
                                </button>
                            </div>
                            <p className="text-xs text-textTertiary">
                                Select the <code>dist/index.js</code> file from the installed <code>flowtrace-claude-bridge</code> package. This assumes you have cloned the repo or have it installed locally.
                            </p>
                        </div>
                    </div>

                    <div className="relative">
                        <div className="flex items-center justify-between mb-2">
                            <span className="text-sm font-semibold text-textPrimary">
                                MCP Configuration
                            </span>
                            <button
                                onClick={() => {
                                    const config = {
                                        "flowtrace-memory": {
                                            "command": "node",
                                            "args": [bridgePath],
                                            "env": {
                                                "FLOWTRACE_URL": "http://127.0.0.1:9601/mcp"
                                            }
                                        }
                                    };
                                    handleCopy(JSON.stringify(config, null, 2), 'mcp-config');
                                }}
                                className="flex items-center gap-2 px-3 py-1.5 rounded-lg bg-surface-hover hover:bg-surface-elevated transition-colors text-sm text-textSecondary"
                            >
                                {copiedEnvVar === 'mcp-config' ? (
                                    <>
                                        <Check className="w-4 h-4 text-green-500" />
                                        Copied!
                                    </>
                                ) : (
                                    <>
                                        <Copy className="w-4 h-4" />
                                        Copy
                                    </>
                                )}
                            </button>
                        </div>
                        <pre className="bg-surface-elevated rounded-lg p-4 overflow-x-auto border border-border-subtle text-xs">
                            <code className="text-textSecondary font-mono">
                                {`"flowtrace-memory": {
  "command": "node",
  "args": ["${bridgePath}"],
  "env": {
    "FLOWTRACE_URL": "http://127.0.0.1:9601/mcp"
  }
}`}
                            </code>
                        </pre>
                    </div>
                </div>
            ) : (
                /* Observability & Memory Configuration */
                <div className="space-y-4">
                    <div className="flex justify-end">
                        <button
                            onClick={() => {
                                const envBlock = Object.entries(effectiveEnvVars)
                                    .map(([k, v]) => `${k}="${String(v)}"`)
                                    .join('\n');
                                handleCopy(envBlock, 'all-env');
                            }}
                            className="text-sm font-medium text-primary hover:text-primary/80 transition-colors flex items-center gap-2"
                        >
                            {copiedEnvVar === 'all-env' ? (
                                <>
                                    <Check className="w-4 h-4" />
                                    Copied all!
                                </>
                            ) : (
                                <>
                                    <Copy className="w-4 h-4" />
                                    Copy all as .env
                                </>
                            )}
                        </button>
                    </div>

                    {Object.entries(effectiveEnvVars).map(([key, value]) => (
                        <div key={key}>
                            <div className="flex items-center justify-between mb-2">
                                <span className="text-sm font-semibold text-textPrimary uppercase">
                                    {key}
                                </span>
                                <button
                                    onClick={() => handleCopy(`${key}="${value}"`, key)}
                                    className="flex items-center gap-2 px-3 py-1.5 rounded-lg bg-surface-hover hover:bg-surface-elevated transition-colors text-sm text-textSecondary"
                                >
                                    {copiedEnvVar === key ? (
                                        <>
                                            <Check className="w-4 h-4 text-green-500" />
                                            Copied!
                                        </>
                                    ) : (
                                        <>
                                            <Copy className="w-4 h-4" />
                                            Copy
                                        </>
                                    )}
                                </button>
                            </div>
                            <pre className="bg-surface-elevated rounded-lg p-4 overflow-x-auto border border-border-subtle">
                                <code className="text-sm text-textSecondary font-mono">
                                    {key}=&quot;{String(value)}&quot;
                                </code>
                            </pre>
                        </div>
                    ))}

                    <div className="flex items-start gap-3 p-3 bg-primary/5 border border-primary/10 rounded-lg mt-4">
                        <div className="p-1 bg-primary/10 rounded-full mt-0.5">
                            <Info className="w-3.5 h-3.5 text-primary" />
                        </div>
                        <div>
                            <h4 className="text-xs font-semibold text-textPrimary mb-0.5">Pro Tip</h4>
                            <p className="text-xs text-textSecondary leading-relaxed">
                                For local development, you can add these variables to a <code className="px-1 py-0.5 bg-background border border-border rounded text-[10px]">.env</code> file in your project root.
                            </p>
                        </div>
                    </div>
                </div>
            )}
        </div>
    );
}

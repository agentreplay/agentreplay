import { useState } from 'react';
import { Terminal, Copy, Check, Info } from 'lucide-react';

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

    // Default env vars if not provided
    const effectiveEnvVars = envVars || {
        AGENTREPLAY_URL: 'http://localhost:47100',
        AGENTREPLAY_TENANT_ID: 'default',
        AGENTREPLAY_PROJECT_ID: projectId,
    };

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
                /* Claude Code Configuration - Marketplace Install */
                <div className="space-y-4">
                    <div className="p-4 bg-blue-500/10 border border-blue-500/20 rounded-lg">
                        <div className="flex items-start gap-3">
                            <Terminal className="w-5 h-5 text-blue-500 mt-0.5 flex-shrink-0" />
                            <div className="flex-1">
                                <h4 className="font-semibold text-blue-500 mb-1">
                                    Install Agent Replay Plugin
                                </h4>
                                <p className="text-sm text-textSecondary mb-4">
                                    Install the official Agent Replay plugin for Claude Code from the marketplace. This provides automatic tracing and persistent memory for your coding sessions.
                                </p>
                                
                                <div className="space-y-3">
                                    <div>
                                        <label className="block text-xs font-medium text-textSecondary uppercase tracking-wider mb-2">
                                            Step 1: Run in Claude Code
                                        </label>
                                        <div className="flex gap-2">
                                            <pre className="flex-1 bg-surface-elevated rounded-lg p-3 overflow-x-auto border border-border-subtle">
                                                <code className="text-green-400 font-mono text-sm">
                                                    /plugin marketplace add agentreplay/agentreplay-claude-plugin
                                                </code>
                                            </pre>
                                            <button
                                                onClick={() => handleCopy('/plugin marketplace add agentreplay/agentreplay-claude-plugin', 'marketplace-cmd')}
                                                className="px-3 py-2 bg-surface-hover hover:bg-surface-elevated border border-border rounded-lg text-sm font-medium text-textPrimary transition-colors flex items-center gap-2"
                                            >
                                                {copiedEnvVar === 'marketplace-cmd' ? (
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
                                    </div>

                                    <div>
                                        <label className="block text-xs font-medium text-textSecondary uppercase tracking-wider mb-2">
                                            Step 2: Restart Claude Code
                                        </label>
                                        <p className="text-sm text-textTertiary">
                                            After installation, restart Claude Code to activate the plugin. Sessions will automatically appear in the <strong>Claude Code</strong> project.
                                        </p>
                                    </div>
                                </div>
                            </div>
                        </div>
                    </div>

                    <div className="p-4 bg-surface rounded-lg border border-border">
                        <h4 className="font-semibold text-textPrimary mb-3">What's Included</h4>
                        <div className="grid grid-cols-2 gap-3 text-sm">
                            <div className="flex items-start gap-2">
                                <Check className="w-4 h-4 text-green-500 mt-0.5 flex-shrink-0" />
                                <span className="text-textSecondary">Session tracing</span>
                            </div>
                            <div className="flex items-start gap-2">
                                <Check className="w-4 h-4 text-green-500 mt-0.5 flex-shrink-0" />
                                <span className="text-textSecondary">Tool call tracking</span>
                            </div>
                            <div className="flex items-start gap-2">
                                <Check className="w-4 h-4 text-green-500 mt-0.5 flex-shrink-0" />
                                <span className="text-textSecondary">Persistent memory</span>
                            </div>
                            <div className="flex items-start gap-2">
                                <Check className="w-4 h-4 text-green-500 mt-0.5 flex-shrink-0" />
                                <span className="text-textSecondary">Context injection</span>
                            </div>
                        </div>
                    </div>

                    <div className="p-3 bg-yellow-500/10 border border-yellow-500/20 rounded-lg">
                        <p className="text-xs text-yellow-600">
                            <strong>Note:</strong> Make sure Agent Replay server is running on <code>http://localhost:47100</code> before using Claude Code.
                        </p>
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

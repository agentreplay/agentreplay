/**
 * MCP Tester Page — Main Entry Point
 *
 * Three-panel IDE layout:
 * - Left Sidebar (256px): Method catalog, preset sequences
 * - Center Main (flex): Request Composer + Response Inspector + History
 * - Right Sidebar (224px, xl+): Quick Reference
 * - Top Bar: Transport selector, endpoint input, connection status
 * - Footer: Connection dot, transport + endpoint, request count, MCP version
 */

import { useState, useEffect, useCallback, useRef, useMemo } from 'react';
import { AnimatePresence, motion } from 'framer-motion';
import {
  Play, Send, RotateCcw, Trash2, Download, Upload,
  ChevronRight, ChevronDown, Copy, Check, AlertCircle,
  Wifi, WifiOff, Clock, Zap, FileJson, History as HistoryIcon,
  RefreshCw, Settings, ArrowRight, CheckCircle, XCircle,
  Code, Layers, Network, Terminal as TerminalIcon,
  BookOpen, Braces, ArrowUpDown, PanelRightClose, PanelRight,
} from 'lucide-react';
import { cn } from '@/lib/utils';

// Protocol
import {
  buildRequest, buildNotification, serialize,
  parseResponse,
  isErrorResponse, getErrorLabel, getErrorDescription,
  JSON_RPC_ERROR_CODES,
  type JsonRpcRequest, type JsonRpcResponse,
} from '../mcp-tester/protocol/codec';

// Transport
import { createTransport, type McpTransport, type TransportType, type ConnectionState } from '../mcp-tester/transport';

// Catalog
import {
  MCP_METHODS, CATEGORY_COLORS, SPAN_TYPES,
  getMethodDefinition, getMethodsByCategory, getAllCategories,
  getToolArgumentSchema, validateParams, generateTemplate,
  type McpMethodDefinition, type MethodCategory,
} from '../mcp-tester/catalog/registry';

// Sequences
import {
  PRESET_SEQUENCES, extractJsonPath, substituteVariables,
  type McpSequence, type SequenceStep,
} from '../mcp-tester/composer/sequences';

// History
import {
  loadHistory, addEntry, getEntries, clearHistory,
  filterEntries, exportSession, importSession,
  type HistoryEntry, type HistoryFilter,
} from '../mcp-tester/history/store';

// Health
import { HealthMonitor, type HealthSnapshot, type McpCapabilities } from '../mcp-tester/health/monitor';

// Assertions
import {
  evaluateAssertion, parseAssertionDsl, BUILTIN_SUITES,
  type Assertion, type AssertionResult,
} from '../mcp-tester/assertions/engine';


// ═══════════════════════════════════════════════════════════════════════════════
// MCP Tester Page Component
// ═══════════════════════════════════════════════════════════════════════════════

export default function McpTesterPage() {
  // ── State ──
  const [transportType, setTransportType] = useState<TransportType>('http');
  const [endpoint, setEndpoint] = useState('http://localhost:47100/mcp');
  const [connectionState, setConnectionState] = useState<ConnectionState>('disconnected');
  const [transport, setTransport] = useState<McpTransport | null>(null);

  const [selectedMethod, setSelectedMethod] = useState<McpMethodDefinition | null>(MCP_METHODS[0]);
  const [requestJson, setRequestJson] = useState('{}');
  const [responseJson, setResponseJson] = useState<string | null>(null);
  const [responseObj, setResponseObj] = useState<JsonRpcResponse | null>(null);
  const [sending, setSending] = useState(false);
  const [lastDuration, setLastDuration] = useState<number | null>(null);
  const [lastResponseSize, setLastResponseSize] = useState<number | null>(null);
  const [lastHttpStatus, setLastHttpStatus] = useState<number | null>(null);

  const [activeTab, setActiveTab] = useState<'composer' | 'history' | 'assertions' | 'sequences'>('composer');
  const [inspectorTab, setInspectorTab] = useState<'response' | 'request' | 'headers' | 'timing'>('response');
  const [showRightSidebar, setShowRightSidebar] = useState(true);
  const [expandedCategories, setExpandedCategories] = useState<Set<string>>(new Set(['lifecycle', 'tools', 'resources', 'prompts']));
  const [expandedSequences, setExpandedSequences] = useState(true);

  const [history, setHistory] = useState<HistoryEntry[]>([]);
  const [historyFilter, setHistoryFilter] = useState<HistoryFilter>({});
  const [requestCount, setRequestCount] = useState(0);

  const [healthSnapshot, setHealthSnapshot] = useState<HealthSnapshot | null>(null);
  const healthMonitorRef = useRef<HealthMonitor | null>(null);

  const [copied, setCopied] = useState(false);
  const [validationErrors, setValidationErrors] = useState<string[]>([]);

  // Sequence execution
  const [runningSequence, setRunningSequence] = useState<string | null>(null);
  const [sequenceResults, setSequenceResults] = useState<Array<{ step: string; response: JsonRpcResponse | null; duration: number; error?: string }>>([]);

  // Assertions
  const [assertions, setAssertions] = useState<Assertion[]>([]);
  const [assertionResults, setAssertionResults] = useState<AssertionResult[]>([]);
  const [assertionDsl, setAssertionDsl] = useState('');

  // ── Initialize ──
  useEffect(() => {
    const loaded = loadHistory();
    setHistory(loaded);
    setRequestCount(loaded.length);
  }, []);

  // Health monitor
  useEffect(() => {
    const monitor = new HealthMonitor(endpoint);
    healthMonitorRef.current = monitor;
    monitor.onChange(setHealthSnapshot);
    monitor.start();
    return () => monitor.stop();
  }, [endpoint]);

  // ── Transport Management ──
  const connectTransport = useCallback(async () => {
    setConnectionState('connecting');
    try {
      const t = createTransport(transportType);
      t.on((event) => {
        if (event.type === 'state-change') {
          setConnectionState(event.data as ConnectionState);
        }
      });
      await t.connect(endpoint);
      setTransport(t);
      setConnectionState('connected');
    } catch (err) {
      console.warn('[MCP Tester] Connection failed:', err instanceof Error ? err.message : err);
      setConnectionState('error');
    }
  }, [transportType, endpoint]);

  const disconnectTransport = useCallback(async () => {
    if (transport) {
      try {
        await transport.disconnect();
      } catch { /* ignore */ }
      setTransport(null);
      setConnectionState('disconnected');
    }
  }, [transport]);

  // Auto-connect on mount
  useEffect(() => {
    let cancelled = false;
    const doConnect = async () => {
      try {
        const t = createTransport(transportType);
        t.on((event) => {
          if (!cancelled && event.type === 'state-change') {
            setConnectionState(event.data as ConnectionState);
          }
        });
        await t.connect(endpoint);
        if (!cancelled) {
          setTransport(t);
          setConnectionState('connected');
        } else {
          t.disconnect();
        }
      } catch {
        if (!cancelled) {
          setConnectionState('disconnected');
        }
      }
    };
    doConnect();
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // ── Method Selection ──
  const handleSelectMethod = useCallback((method: McpMethodDefinition) => {
    setSelectedMethod(method);
    setActiveTab('composer');
    const params = method.defaultParams || {};
    setRequestJson(JSON.stringify(params, null, 2));
    setValidationErrors([]);
    setResponseJson(null);
    setResponseObj(null);
  }, []);

  // ── Send Request ──
  const handleSend = useCallback(async () => {
    if (!selectedMethod || sending) return;

    let params: Record<string, unknown>;
    try {
      params = JSON.parse(requestJson);
    } catch {
      setValidationErrors(['Invalid JSON in request body.']);
      return;
    }

    // Validate params
    if (selectedMethod.paramsSchema) {
      const errors = validateParams(params, selectedMethod.paramsSchema);
      if (errors.length > 0) {
        setValidationErrors(errors.map((e) => `${e.path}: ${e.message}`));
        return;
      }
    }
    setValidationErrors([]);

    setSending(true);
    const start = performance.now();

    try {
      let response: JsonRpcResponse;

      if (selectedMethod.isNotification) {
        // Send as a request so we can get a response back
        const req = buildRequest(selectedMethod.method, params);
        const serialized = serialize(req);

        const resp = await fetch(endpoint, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: serialized,
        });
        const text = await resp.text();
        const elapsed = performance.now() - start;
        setLastDuration(elapsed);
        setLastResponseSize(new Blob([text]).size);
        setLastHttpStatus(resp.status);

        try {
          response = JSON.parse(text);
        } catch {
          response = { jsonrpc: '2.0', result: { acknowledged: true }, id: null };
        }
      } else if (transport && transport.isConnected()) {
        const tracked = buildRequest(selectedMethod.method, params);
        response = await transport.send(tracked.message);
        const elapsed = performance.now() - start;
        setLastDuration(elapsed);
        const respText = JSON.stringify(response, null, 2);
        setLastResponseSize(new Blob([respText]).size);
        setLastHttpStatus(200);
      } else {
        // Fallback: direct HTTP
        const tracked = buildRequest(selectedMethod.method, params);
        const body = serialize(tracked);

        const resp = await fetch(endpoint, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body,
        });
        const text = await resp.text();
        const elapsed = performance.now() - start;
        setLastDuration(elapsed);
        setLastResponseSize(new Blob([text]).size);
        setLastHttpStatus(resp.status);
        if (!text || text.trim() === '') {
          response = { jsonrpc: '2.0', result: { acknowledged: true }, id: null };
        } else {
          try {
            response = JSON.parse(text);
          } catch {
            response = { jsonrpc: '2.0', result: { raw: text }, id: null };
          }
        }
      }

      setResponseObj(response);
      setResponseJson(JSON.stringify(response, null, 2));

      // Run assertions
      if (assertions.length > 0) {
        const results = assertions
          .filter((a) => a.enabled)
          .map((a) => evaluateAssertion(a, response, { durationMs: lastDuration || 0 }));
        setAssertionResults(results);
      }

      // Add to history
      const entry = addEntry({
        timestamp: Date.now(),
        method: selectedMethod.method,
        category: selectedMethod.category,
        request: params,
        response,
        error: isErrorResponse(response) ? response.error?.message : undefined,
        httpStatus: lastHttpStatus || 200,
        durationMs: performance.now() - start,
        responseSize: lastResponseSize || 0,
        transport: transportType,
      });

      setHistory(getEntries());
      setRequestCount((c) => c + 1);
      setInspectorTab('response');
    } catch (err) {
      const elapsed = performance.now() - start;
      setLastDuration(elapsed);
      setResponseJson(JSON.stringify({ error: err instanceof Error ? err.message : 'Request failed' }, null, 2));
      setResponseObj(null);
    } finally {
      setSending(false);
    }
  }, [selectedMethod, requestJson, endpoint, transport, transportType, sending, assertions, lastDuration, lastResponseSize, lastHttpStatus]);

  // ── Sequence Execution ──
  const handleRunSequence = useCallback(async (sequence: McpSequence) => {
    setRunningSequence(sequence.id);
    setSequenceResults([]);
    setActiveTab('sequences');

    const variables: Record<string, unknown> = {};
    const results: typeof sequenceResults = [];

    for (const step of sequence.steps) {
      const params = substituteVariables(step.params, variables);

      if (step.delayMs) {
        await new Promise((r) => setTimeout(r, step.delayMs));
      }

      const start = performance.now();
      try {
        const tracked = step.isNotification
          ? buildNotification(step.method, params)
          : buildRequest(step.method, params);

        const body = serialize(tracked);
        const resp = await fetch(endpoint, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body,
        });
        const text = await resp.text();
        const elapsed = performance.now() - start;
        let response: JsonRpcResponse;
        if (!text || text.trim() === '') {
          response = { jsonrpc: '2.0', result: { acknowledged: true }, id: null };
        } else {
          try {
            response = JSON.parse(text) as JsonRpcResponse;
          } catch {
            response = { jsonrpc: '2.0', result: { raw: text }, id: null };
          }
        }

        // Extract values
        if (step.extractions) {
          for (const extraction of step.extractions) {
            const value = extractJsonPath(response, extraction.path);
            if (value !== undefined) {
              variables[extraction.variable] = value;
            }
          }
        }

        results.push({ step: step.id, response, duration: elapsed });
        setSequenceResults([...results]);

        // Add to history
        addEntry({
          timestamp: Date.now(),
          method: step.method,
          category: 'lifecycle',
          request: params,
          response,
          error: isErrorResponse(response) ? response.error?.message : undefined,
          durationMs: elapsed,
          responseSize: new Blob([text]).size,
          transport: transportType,
        });
      } catch (err) {
        const elapsed = performance.now() - start;
        results.push({
          step: step.id,
          response: null,
          duration: elapsed,
          error: err instanceof Error ? err.message : 'Failed',
        });
        setSequenceResults([...results]);
        break; // Stop sequence on error
      }
    }

    setHistory(getEntries());
    setRequestCount(getEntries().length);
    setRunningSequence(null);
  }, [endpoint, transportType]);

  // ── Copy ──
  const handleCopy = useCallback((text: string) => {
    navigator.clipboard.writeText(text);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  }, []);

  // ── History Export/Import ──
  const handleExport = useCallback(() => {
    const data = exportSession();
    const blob = new Blob([data], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `mcp-test-session-${new Date().toISOString().slice(0, 10)}.json`;
    a.click();
    URL.revokeObjectURL(url);
  }, []);

  const handleImport = useCallback(() => {
    const input = document.createElement('input');
    input.type = 'file';
    input.accept = '.json';
    input.onchange = async (e) => {
      const file = (e.target as HTMLInputElement).files?.[0];
      if (!file) return;
      const text = await file.text();
      try {
        importSession(text);
        setHistory(getEntries());
      } catch (err) {
        alert('Failed to import session');
      }
    };
    input.click();
  }, []);

  // ── Replay from History ──
  const handleReplay = useCallback((entry: HistoryEntry) => {
    const method = getMethodDefinition(entry.method);
    if (method) {
      handleSelectMethod(method);
      setRequestJson(JSON.stringify(entry.request, null, 2));
    }
  }, [handleSelectMethod]);

  // ── Assertion DSL ──
  const handleAddAssertion = useCallback(() => {
    if (!assertionDsl.trim()) return;
    const parsed = parseAssertionDsl(assertionDsl);
    if (parsed) {
      setAssertions((prev) => [...prev, parsed]);
      setAssertionDsl('');
    }
  }, [assertionDsl]);

  // ── Filtered History ──
  const filteredHistory = useMemo(() => {
    if (Object.keys(historyFilter).length === 0) return history;
    return filterEntries(historyFilter);
  }, [history, historyFilter]);

  // ── Connection Status Color ──
  const connectionDot = connectionState === 'connected'
    ? 'bg-emerald-400' : connectionState === 'connecting' || connectionState === 'reconnecting'
      ? 'bg-amber-400 animate-pulse' : connectionState === 'error'
        ? 'bg-red-400' : 'bg-muted-foreground';

  const healthDot = healthSnapshot?.state === 'healthy'
    ? 'bg-emerald-400' : healthSnapshot?.state === 'degraded'
      ? 'bg-amber-400' : healthSnapshot?.state === 'unhealthy'
        ? 'bg-red-400' : 'bg-muted-foreground';

  // ═══════════════════════════════════════════════════════════════════════════
  // RENDER
  // ═══════════════════════════════════════════════════════════════════════════

  return (
    <div className="flex flex-col h-full bg-background text-foreground overflow-hidden">
      {/* ── Top Bar ── */}
      <div className="flex items-center gap-3 px-4 py-2 border-b border-border bg-muted/50">
        <div className="flex items-center gap-2">
          <Braces className="w-5 h-5 text-primary" />
          <span className="font-semibold text-sm">MCP Tester</span>
        </div>

        <div className="flex items-center gap-2 ml-4">
          {/* Transport Selector */}
          <select
            value={transportType}
            onChange={(e) => {
              setTransportType(e.target.value as TransportType);
              disconnectTransport();
            }}
            className="bg-muted border border-border rounded-md px-2 py-1 text-xs font-mono text-foreground focus:outline-none focus:ring-1 focus:ring-ring"
          >
            <option value="http">HTTP POST</option>
            <option value="websocket">WebSocket</option>
            <option value="sse">SSE</option>
            <option value="stdio">stdio</option>
          </select>

          {/* Endpoint Input */}
          <input
            type="text"
            value={endpoint}
            onChange={(e) => setEndpoint(e.target.value)}
            className="bg-muted border border-border rounded-md px-3 py-1 text-xs font-mono text-foreground w-72 focus:outline-none focus:ring-1 focus:ring-ring"
            placeholder="http://localhost:47100/mcp"
          />

          {/* Connect/Disconnect Button */}
          <button
            onClick={connectionState === 'connected' ? disconnectTransport : connectTransport}
            className={cn(
              'px-3 py-1 rounded-md text-xs font-medium transition-colors',
              connectionState === 'connected'
                ? 'bg-red-500/20 text-red-300 hover:bg-red-500/30'
                : 'bg-primary/20 text-primary hover:bg-primary/30'
            )}
          >
            {connectionState === 'connected' ? 'Disconnect' : 'Connect'}
          </button>
        </div>

        {/* Health Status */}
        <div className="flex items-center gap-2 ml-auto">
          {healthSnapshot && (
            <div className="flex items-center gap-1.5 text-xs text-muted-foreground">
              <div className={cn('w-2 h-2 rounded-full', healthDot)} />
              {healthSnapshot.state === 'healthy' && healthSnapshot.status && (
                <span>{healthSnapshot.status.server_name} v{healthSnapshot.status.server_version}</span>
              )}
              {healthSnapshot.emaLatencyMs > 0 && (
                <span className="text-muted-foreground">({Math.round(healthSnapshot.emaLatencyMs)}ms avg)</span>
              )}
            </div>
          )}
          <button
            onClick={() => setShowRightSidebar(!showRightSidebar)}
            className="p-1 text-muted-foreground hover:text-foreground transition-colors"
          >
            {showRightSidebar ? <PanelRightClose className="w-4 h-4" /> : <PanelRight className="w-4 h-4" />}
          </button>
        </div>
      </div>

      {/* ── Main Content (3-panel) ── */}
      <div className="flex flex-1 overflow-hidden">
        {/* ── Left Sidebar: Catalog ── */}
        <div className="w-64 border-r border-border bg-muted/30 overflow-y-auto flex-shrink-0">
          <div className="p-3">
            <div className="text-[10px] font-semibold text-muted-foreground uppercase tracking-wider mb-2">Method Catalog</div>

            {getAllCategories().map((category) => {
              const methods = getMethodsByCategory(category);
              const colors = CATEGORY_COLORS[category];
              const isExpanded = expandedCategories.has(category);

              return (
                <div key={category} className="mb-1">
                  <button
                    onClick={() => {
                      const next = new Set(expandedCategories);
                      if (isExpanded) next.delete(category);
                      else next.add(category);
                      setExpandedCategories(next);
                    }}
                    className="flex items-center gap-1.5 w-full px-2 py-1 text-xs font-medium text-muted-foreground hover:text-foreground transition-colors"
                  >
                    {isExpanded ? <ChevronDown className="w-3 h-3" /> : <ChevronRight className="w-3 h-3" />}
                    <span className="capitalize">{category}</span>
                    <span className="text-[10px] text-muted-foreground/70 ml-auto">{methods.length}</span>
                  </button>

                  <AnimatePresence>
                    {isExpanded && (
                      <motion.div
                        initial={{ height: 0, opacity: 0 }}
                        animate={{ height: 'auto', opacity: 1 }}
                        exit={{ height: 0, opacity: 0 }}
                        transition={{ duration: 0.15 }}
                        className="overflow-hidden"
                      >
                        {methods.map((method) => (
                          <button
                            key={method.method}
                            onClick={() => handleSelectMethod(method)}
                            className={cn(
                              'flex items-center gap-2 w-full px-3 py-1.5 text-xs rounded-md transition-colors ml-2',
                              selectedMethod?.method === method.method
                                ? `${colors.bg} ${colors.text}`
                                : 'text-muted-foreground hover:text-foreground hover:bg-muted'
                            )}
                          >
                            <div className={cn('w-1.5 h-1.5 rounded-full', colors.dot)} />
                            <span className="font-mono truncate">{method.method}</span>
                            {method.isNotification && (
                              <span className="text-[9px] text-muted-foreground/70 ml-auto">notif</span>
                            )}
                          </button>
                        ))}
                      </motion.div>
                    )}
                  </AnimatePresence>
                </div>
              );
            })}

            {/* ── Sequences ── */}
            <div className="mt-4 pt-3 border-t border-border">
              <button
                onClick={() => setExpandedSequences(!expandedSequences)}
                className="flex items-center gap-1.5 w-full px-2 py-1 text-xs font-medium text-muted-foreground hover:text-foreground"
              >
                {expandedSequences ? <ChevronDown className="w-3 h-3" /> : <ChevronRight className="w-3 h-3" />}
                <span className="uppercase tracking-wider text-[10px] font-semibold text-muted-foreground">Sequences</span>
              </button>

              <AnimatePresence>
                {expandedSequences && (
                  <motion.div
                    initial={{ height: 0, opacity: 0 }}
                    animate={{ height: 'auto', opacity: 1 }}
                    exit={{ height: 0, opacity: 0 }}
                    transition={{ duration: 0.15 }}
                    className="overflow-hidden"
                  >
                    {PRESET_SEQUENCES.map((seq) => (
                      <button
                        key={seq.id}
                        onClick={() => handleRunSequence(seq)}
                        disabled={!!runningSequence}
                        className={cn(
                          'flex items-center gap-2 w-full px-3 py-1.5 text-xs rounded-md transition-colors ml-2',
                          runningSequence === seq.id
                            ? 'bg-rose-500/15 text-rose-300'
                            : 'text-muted-foreground hover:text-foreground hover:bg-muted'
                        )}
                      >
                        <RefreshCw className={cn('w-3 h-3', runningSequence === seq.id && 'animate-spin')} />
                        <span className="truncate">{seq.name}</span>
                        <span className="text-[9px] text-muted-foreground/70 ml-auto">{seq.steps.length}s</span>
                      </button>
                    ))}
                  </motion.div>
                )}
              </AnimatePresence>
            </div>
          </div>
        </div>

        {/* ── Center: Composer + Inspector ── */}
        <div className="flex-1 flex flex-col overflow-hidden">
          {/* Tabs */}
          <div className="flex items-center gap-0 px-4 border-b border-border bg-muted/30">
            {(['composer', 'history', 'assertions', 'sequences'] as const).map((tab) => (
              <button
                key={tab}
                onClick={() => setActiveTab(tab)}
                className={cn(
                  'px-4 py-2 text-xs font-medium border-b-2 transition-colors capitalize',
                  activeTab === tab
                    ? 'border-primary text-primary'
                    : 'border-transparent text-muted-foreground hover:text-foreground'
                )}
              >
                {tab}
              </button>
            ))}
          </div>

          {/* Tab Content */}
          <div className="flex-1 overflow-hidden">
            {activeTab === 'composer' && (
              <div className="flex h-full">
                {/* Request Editor */}
                <div className="flex-1 flex flex-col border-r border-border overflow-hidden">
                  <div className="flex items-center justify-between px-4 py-2 border-b border-border/50">
                    <div className="flex items-center gap-2">
                      <Code className="w-3.5 h-3.5 text-muted-foreground" />
                      <span className="text-xs font-medium text-muted-foreground">Request</span>
                      {selectedMethod && (
                        <span className={cn(
                          'text-[10px] px-1.5 py-0.5 rounded-full font-mono',
                          CATEGORY_COLORS[selectedMethod.category].bg,
                          CATEGORY_COLORS[selectedMethod.category].text
                        )}>
                          {selectedMethod.method}
                        </span>
                      )}
                    </div>
                    <div className="flex items-center gap-1">
                      <button
                        onClick={() => {
                          if (selectedMethod?.defaultParams) {
                            setRequestJson(JSON.stringify(selectedMethod.defaultParams, null, 2));
                          }
                        }}
                        className="p-1 text-muted-foreground hover:text-foreground transition-colors"
                        title="Reset to default"
                      >
                        <RotateCcw className="w-3.5 h-3.5" />
                      </button>
                      <button
                        onClick={() => handleCopy(requestJson)}
                        className="p-1 text-muted-foreground hover:text-foreground transition-colors"
                        title="Copy request"
                      >
                        {copied ? <Check className="w-3.5 h-3.5 text-emerald-400" /> : <Copy className="w-3.5 h-3.5" />}
                      </button>
                    </div>
                  </div>

                  {/* Method description */}
                  {selectedMethod && (
                    <div className="px-4 py-2 text-[11px] text-muted-foreground border-b border-border/30">
                      {selectedMethod.description}
                    </div>
                  )}

                  {/* Validation errors */}
                  {validationErrors.length > 0 && (
                    <div className="mx-4 mt-2 p-2 rounded-md bg-red-500/10 border border-red-500/20">
                      {validationErrors.map((err, i) => (
                        <div key={i} className="flex items-start gap-1.5 text-[11px] text-red-300">
                          <AlertCircle className="w-3 h-3 mt-0.5 flex-shrink-0" />
                          <span>{err}</span>
                        </div>
                      ))}
                    </div>
                  )}

                  {/* JSON Editor */}
                  <div className="flex-1 overflow-auto p-4">
                    <textarea
                      value={requestJson}
                      onChange={(e) => {
                        setRequestJson(e.target.value);
                        setValidationErrors([]);
                      }}
                      className="w-full h-full bg-transparent text-xs font-mono text-foreground resize-none focus:outline-none leading-relaxed"
                      spellCheck={false}
                      placeholder="Enter request parameters as JSON..."
                    />
                  </div>

                  {/* Send Button */}
                  <div className="px-4 py-3 border-t border-border flex items-center gap-2">
                    <button
                      onClick={handleSend}
                      disabled={sending || !selectedMethod}
                      className={cn(
                        'flex items-center gap-2 px-4 py-2 rounded-md text-xs font-medium transition-all',
                        sending
                          ? 'bg-primary/30 text-primary cursor-wait'
                          : 'bg-primary text-white hover:bg-primary/90 active:scale-[0.98]'
                      )}
                    >
                      {sending ? (
                        <RefreshCw className="w-3.5 h-3.5 animate-spin" />
                      ) : (
                        <Send className="w-3.5 h-3.5" />
                      )}
                      {sending ? 'Sending...' : 'Send'}
                    </button>
                    <span className="text-[10px] text-muted-foreground/70">
                      {transportType.toUpperCase()} · {selectedMethod?.method ?? 'none'}
                    </span>
                  </div>
                </div>

                {/* Response Inspector */}
                <div className="flex-1 flex flex-col overflow-hidden">
                  {/* Inspector sub-tabs */}
                  <div className="flex items-center gap-0 px-4 border-b border-border/50">
                    {(['response', 'request', 'headers', 'timing'] as const).map((tab) => (
                      <button
                        key={tab}
                        onClick={() => setInspectorTab(tab)}
                        className={cn(
                          'px-3 py-2 text-[11px] font-medium border-b transition-colors capitalize',
                          inspectorTab === tab
                            ? 'border-emerald-400 text-emerald-300'
                            : 'border-transparent text-muted-foreground hover:text-foreground'
                        )}
                      >
                        {tab}
                      </button>
                    ))}
                    {lastDuration !== null && (
                      <div className="flex items-center gap-3 ml-auto text-[11px] text-muted-foreground">
                        {lastHttpStatus && (
                          <span className={cn(
                            lastHttpStatus < 300 ? 'text-emerald-400' : 'text-red-400'
                          )}>
                            {lastHttpStatus} {lastHttpStatus < 300 ? 'OK' : 'Error'}
                          </span>
                        )}
                        <span className="flex items-center gap-1">
                          <Clock className="w-3 h-3" />
                          {lastDuration.toFixed(0)}ms
                        </span>
                        {lastResponseSize && (
                          <span>{(lastResponseSize / 1024).toFixed(1)}KB</span>
                        )}
                      </div>
                    )}
                  </div>

                  {/* Inspector Content */}
                  <div className="flex-1 overflow-auto p-4">
                    {inspectorTab === 'response' && (
                      <ResponseView json={responseJson} response={responseObj} />
                    )}
                    {inspectorTab === 'request' && selectedMethod && (
                      <pre className="text-xs font-mono text-muted-foreground whitespace-pre-wrap">
                        {JSON.stringify(
                          {
                            jsonrpc: '2.0',
                            method: selectedMethod.method,
                            params: (() => { try { return JSON.parse(requestJson); } catch { return {}; } })(),
                            id: '(auto)',
                          },
                          null,
                          2
                        )}
                      </pre>
                    )}
                    {inspectorTab === 'headers' && (
                      <div className="space-y-1 text-xs font-mono">
                        <HeaderRow label="Content-Type" value="application/json" />
                        <HeaderRow label="Accept" value="application/json" />
                        <HeaderRow label="X-Protocol" value="JSON-RPC 2.0" />
                        <HeaderRow label="X-MCP-Version" value="2024-11-05" />
                        <HeaderRow label="X-Transport" value={transportType} />
                      </div>
                    )}
                    {inspectorTab === 'timing' && lastDuration !== null && (
                      <TimingView durationMs={lastDuration} size={lastResponseSize || 0} />
                    )}
                  </div>
                </div>
              </div>
            )}

            {activeTab === 'history' && (
              <HistoryView
                entries={filteredHistory}
                filter={historyFilter}
                onFilterChange={setHistoryFilter}
                onReplay={handleReplay}
                onClear={clearHistory}
                onExport={handleExport}
                onImport={handleImport}
              />
            )}

            {activeTab === 'assertions' && (
              <AssertionsView
                assertions={assertions}
                results={assertionResults}
                dsl={assertionDsl}
                onDslChange={setAssertionDsl}
                onAdd={handleAddAssertion}
                onRemove={(id) => setAssertions((prev) => prev.filter((a) => a.id !== id))}
                onToggle={(id) => setAssertions((prev) => prev.map((a) => a.id === id ? { ...a, enabled: !a.enabled } : a))}
                onLoadSuite={(suite) => setAssertions(suite.assertions)}
              />
            )}

            {activeTab === 'sequences' && (
              <SequenceResultsView results={sequenceResults} running={runningSequence} />
            )}
          </div>
        </div>

        {/* ── Right Sidebar: Quick Reference ── */}
        <AnimatePresence>
          {showRightSidebar && (
            <motion.div
              initial={{ width: 0, opacity: 0 }}
              animate={{ width: 224, opacity: 1 }}
              exit={{ width: 0, opacity: 0 }}
              transition={{ duration: 0.2 }}
              className="border-l border-border bg-muted/30 overflow-y-auto overflow-x-hidden flex-shrink-0 hidden xl:block"
            >
              <div className="p-3 w-56">
                {/* JSON-RPC Error Codes */}
                <div className="mb-4">
                  <div className="text-[10px] font-semibold text-muted-foreground uppercase tracking-wider mb-2">JSON-RPC Errors</div>
                  {Object.entries(JSON_RPC_ERROR_CODES).map(([code, info]) => (
                    <div key={code} className="flex items-start gap-2 py-1 text-[10px]">
                      <code className="text-red-400 font-mono flex-shrink-0">{code}</code>
                      <span className="text-muted-foreground">{info.label}</span>
                    </div>
                  ))}
                </div>

                {/* Span Types */}
                <div className="mb-4">
                  <div className="text-[10px] font-semibold text-muted-foreground uppercase tracking-wider mb-2">Span Types</div>
                  <div className="flex flex-wrap gap-1">
                    {SPAN_TYPES.map((type) => (
                      <span key={type} className="px-1.5 py-0.5 text-[9px] bg-muted text-muted-foreground rounded font-mono">
                        {type}
                      </span>
                    ))}
                  </div>
                </div>

                {/* Transport Paths */}
                <div className="mb-4">
                  <div className="text-[10px] font-semibold text-muted-foreground uppercase tracking-wider mb-2">Transports</div>
                  <div className="space-y-1 text-[10px]">
                    <div className="flex items-center gap-2">
                      <Network className="w-3 h-3 text-blue-400" />
                      <code className="text-muted-foreground font-mono">POST /mcp</code>
                    </div>
                    <div className="flex items-center gap-2">
                      <ArrowUpDown className="w-3 h-3 text-green-400" />
                      <code className="text-muted-foreground font-mono">GET /mcp/ws</code>
                    </div>
                    <div className="flex items-center gap-2">
                      <Zap className="w-3 h-3 text-amber-400" />
                      <code className="text-muted-foreground font-mono">GET /mcp/sse</code>
                    </div>
                    <div className="flex items-center gap-2">
                      <TerminalIcon className="w-3 h-3 text-purple-400" />
                      <code className="text-muted-foreground font-mono">stdio (4B LE)</code>
                    </div>
                  </div>
                </div>

                {/* Endpoints */}
                <div className="mb-4">
                  <div className="text-[10px] font-semibold text-muted-foreground uppercase tracking-wider mb-2">Endpoints</div>
                  <div className="space-y-1 text-[10px]">
                    <div className="text-muted-foreground font-mono">/mcp — JSON-RPC</div>
                    <div className="text-muted-foreground font-mono">/mcp/health — Health</div>
                    <div className="text-muted-foreground font-mono">/mcp/ws — WebSocket</div>
                    <div className="text-muted-foreground font-mono">/mcp/sse — Events</div>
                  </div>
                </div>

                {/* Connected Clients */}
                {healthSnapshot?.status && (
                  <div className="mb-4">
                    <div className="text-[10px] font-semibold text-muted-foreground uppercase tracking-wider mb-2">Server Info</div>
                    <div className="space-y-1 text-[10px] text-muted-foreground">
                      <div>Clients: {healthSnapshot.status.connected_clients}</div>
                      <div>Protocol: {healthSnapshot.status.protocol_version}</div>
                      <div>Latency: {Math.round(healthSnapshot.emaLatencyMs)}ms</div>
                      <div>Jitter: {healthSnapshot.jitterMs.toFixed(1)}ms</div>
                    </div>
                  </div>
                )}
              </div>
            </motion.div>
          )}
        </AnimatePresence>
      </div>

      {/* ── Footer ── */}
      <div className="flex items-center justify-between px-4 py-1.5 border-t border-border bg-muted/50 text-[10px] text-muted-foreground">
        <div className="flex items-center gap-3">
          <div className="flex items-center gap-1.5">
            <div className={cn('w-2 h-2 rounded-full', connectionDot)} />
            <span>{connectionState === 'connected' ? 'Connected' : connectionState}</span>
          </div>
          <span>·</span>
          <span className="font-mono">{transportType.toUpperCase()}</span>
          <span>·</span>
          <span className="font-mono">{endpoint}</span>
        </div>
        <div className="flex items-center gap-3">
          <span>{requestCount} requests</span>
          <span>·</span>
          <span>MCP v2024-11-05</span>
        </div>
      </div>
    </div>
  );
}


// ═══════════════════════════════════════════════════════════════════════════════
// Sub-Components
// ═══════════════════════════════════════════════════════════════════════════════

// ── Response View with Semantic Highlighting ────────────────────────────────

function ResponseView({ json, response }: { json: string | null; response: JsonRpcResponse | null }) {
  if (!json) {
    return (
      <div className="flex items-center justify-center h-full text-muted-foreground/70 text-sm">
        <div className="text-center">
          <Braces className="w-8 h-8 mx-auto mb-2 opacity-30" />
          <p>Send a request to see the response</p>
        </div>
      </div>
    );
  }

  const isError = response && isErrorResponse(response);

  return (
    <div className="space-y-3">
      {/* Error Banner */}
      {isError && response?.error && (
        <div className="p-3 rounded-md bg-red-500/10 border border-red-500/20">
          <div className="flex items-center gap-2 mb-1">
            <XCircle className="w-4 h-4 text-red-400" />
            <span className="text-sm font-medium text-red-300">
              {getErrorLabel(response.error.code)}
            </span>
            <code className="text-[10px] text-red-400/70 font-mono">({response.error.code})</code>
          </div>
          <p className="text-xs text-red-300/70">{response.error.message}</p>
          <p className="text-[10px] text-red-300/50 mt-1">{getErrorDescription(response.error.code)}</p>
        </div>
      )}

      {/* Success Banner */}
      {response && !isError && (
        <div className="flex items-center gap-2 p-2 rounded-md bg-emerald-500/10 border border-emerald-500/20">
          <CheckCircle className="w-3.5 h-3.5 text-emerald-400" />
          <span className="text-xs text-emerald-300">Success</span>
        </div>
      )}

      {/* Highlighted JSON */}
      <pre className="text-xs font-mono leading-relaxed whitespace-pre-wrap break-all">
        <HighlightedJson text={json} />
      </pre>
    </div>
  );
}

/**
 * Syntax-highlighted JSON renderer.
 * Single-pass O(n) regex transform over stringified JSON.
 * Keys: amber, Strings: emerald, Numbers: sky, Booleans: violet, Null: rose
 */
function HighlightedJson({ text }: { text: string }) {
  const highlighted = useMemo(() => {
    const parts: Array<{ text: string; className: string }> = [];

    // Match different JSON token types
    const regex = /("(?:[^"\\]|\\.)*")\s*:|("(?:[^"\\]|\\.)*")|(-?\d+\.?\d*(?:[eE][+-]?\d+)?)|(\btrue\b|\bfalse\b)|(\bnull\b)|([{}[\],:])/g;

    let lastIndex = 0;
    let match: RegExpExecArray | null;

    while ((match = regex.exec(text)) !== null) {
      // Add any text before this match
      if (match.index > lastIndex) {
        parts.push({ text: text.slice(lastIndex, match.index), className: 'text-muted-foreground' });
      }

      if (match[1]) {
        // Key (includes the colon)
        parts.push({ text: match[1], className: 'text-amber-300' });
      } else if (match[2]) {
        // String value
        parts.push({ text: match[2], className: 'text-emerald-300' });
      } else if (match[3]) {
        // Number
        parts.push({ text: match[3], className: 'text-sky-300' });
      } else if (match[4]) {
        // Boolean
        parts.push({ text: match[4], className: 'text-violet-300' });
      } else if (match[5]) {
        // Null
        parts.push({ text: match[5], className: 'text-rose-300' });
      } else if (match[6]) {
        // Punctuation
        parts.push({ text: match[6], className: 'text-muted-foreground/70' });
      }

      lastIndex = regex.lastIndex;
    }

    // Add remaining text
    if (lastIndex < text.length) {
      parts.push({ text: text.slice(lastIndex), className: 'text-muted-foreground' });
    }

    return parts;
  }, [text]);

  return (
    <>
      {highlighted.map((part, i) => (
        <span key={i} className={part.className}>{part.text}</span>
      ))}
    </>
  );
}

// ── Header Row ──────────────────────────────────────────────────────────────

function HeaderRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-center gap-2 py-1.5 border-b border-border/30">
      <span className="text-muted-foreground w-32">{label}:</span>
      <span className="text-foreground">{value}</span>
    </div>
  );
}

// ── Timing View ──────────────────────────────────────────────────────────────

function TimingView({ durationMs, size }: { durationMs: number; size: number }) {
  // Approximate phase breakdown for HTTP
  const dns = durationMs * 0.05;
  const tcp = durationMs * 0.1;
  const tls = durationMs * 0.1;
  const ttfb = durationMs * 0.5;
  const transfer = durationMs * 0.25;

  const phases = [
    { label: 'DNS Lookup', duration: dns, color: 'bg-sky-400' },
    { label: 'TCP Connect', duration: tcp, color: 'bg-emerald-400' },
    { label: 'TLS Handshake', duration: tls, color: 'bg-violet-400' },
    { label: 'Time to First Byte', duration: ttfb, color: 'bg-amber-400' },
    { label: 'Content Transfer', duration: transfer, color: 'bg-rose-400' },
  ];

  return (
    <div className="space-y-4">
      {/* Waterfall */}
      <div className="space-y-2">
        <div className="text-xs font-medium text-muted-foreground mb-3">Timing Waterfall</div>
        {phases.map((phase) => (
          <div key={phase.label} className="flex items-center gap-3">
            <span className="text-[10px] text-muted-foreground w-32 text-right">{phase.label}</span>
            <div className="flex-1 h-4 bg-muted rounded-sm overflow-hidden">
              <div
                className={cn('h-full rounded-sm transition-all', phase.color)}
                style={{ width: `${(phase.duration / durationMs) * 100}%` }}
              />
            </div>
            <span className="text-[10px] text-muted-foreground w-14 text-right">{phase.duration.toFixed(1)}ms</span>
          </div>
        ))}
        <div className="flex items-center gap-3 mt-2 pt-2 border-t border-border">
          <span className="text-[10px] text-muted-foreground w-32 text-right font-medium">Total</span>
          <div className="flex-1" />
          <span className="text-[10px] text-foreground w-14 text-right font-medium">{durationMs.toFixed(0)}ms</span>
        </div>
      </div>

      {/* Size */}
      <div className="p-3 bg-muted/50 rounded-md">
        <div className="text-xs text-muted-foreground">Response Size</div>
        <div className="text-lg font-mono text-foreground mt-1">
          {size < 1024 ? `${size} B` : `${(size / 1024).toFixed(1)} KB`}
        </div>
      </div>
    </div>
  );
}

// ── History View ─────────────────────────────────────────────────────────────

function HistoryView({
  entries,
  filter,
  onFilterChange,
  onReplay,
  onClear,
  onExport,
  onImport,
}: {
  entries: HistoryEntry[];
  filter: HistoryFilter;
  onFilterChange: (f: HistoryFilter) => void;
  onReplay: (entry: HistoryEntry) => void;
  onClear: () => void;
  onExport: () => void;
  onImport: () => void;
}) {
  return (
    <div className="flex flex-col h-full">
      {/* Filter Bar */}
      <div className="flex items-center gap-2 px-4 py-2 border-b border-border">
        <input
          type="text"
          placeholder="Search history..."
          value={filter.search || ''}
          onChange={(e) => onFilterChange({ ...filter, search: e.target.value || undefined })}
          className="bg-muted border border-border rounded-md px-2 py-1 text-xs text-foreground flex-1 max-w-xs focus:outline-none focus:ring-1 focus:ring-ring"
        />
        <select
          value={filter.category || ''}
          onChange={(e) => onFilterChange({ ...filter, category: e.target.value || undefined })}
          className="bg-muted border border-border rounded-md px-2 py-1 text-xs text-foreground focus:outline-none"
        >
          <option value="">All categories</option>
          <option value="lifecycle">Lifecycle</option>
          <option value="tools">Tools</option>
          <option value="resources">Resources</option>
          <option value="prompts">Prompts</option>
        </select>
        <select
          value={filter.status || 'all'}
          onChange={(e) => onFilterChange({ ...filter, status: e.target.value as HistoryFilter['status'] })}
          className="bg-muted border border-border rounded-md px-2 py-1 text-xs text-foreground focus:outline-none"
        >
          <option value="all">All status</option>
          <option value="success">Success</option>
          <option value="error">Error</option>
        </select>
        <div className="flex items-center gap-1 ml-auto">
          <button onClick={onExport} className="p-1 text-muted-foreground hover:text-foreground" title="Export session">
            <Download className="w-3.5 h-3.5" />
          </button>
          <button onClick={onImport} className="p-1 text-muted-foreground hover:text-foreground" title="Import session">
            <Upload className="w-3.5 h-3.5" />
          </button>
          <button onClick={onClear} className="p-1 text-muted-foreground hover:text-red-300" title="Clear history">
            <Trash2 className="w-3.5 h-3.5" />
          </button>
        </div>
      </div>

      {/* History Table */}
      <div className="flex-1 overflow-auto">
        {entries.length === 0 ? (
          <div className="flex items-center justify-center h-full text-muted-foreground/70 text-sm">
            <div className="text-center">
              <HistoryIcon className="w-8 h-8 mx-auto mb-2 opacity-30" />
              <p>No history yet</p>
            </div>
          </div>
        ) : (
          <table className="w-full text-xs">
            <thead>
              <tr className="text-muted-foreground border-b border-border sticky top-0 bg-background">
                <th className="text-left px-4 py-2 font-medium">Time</th>
                <th className="text-left px-4 py-2 font-medium">Method</th>
                <th className="text-left px-4 py-2 font-medium">Status</th>
                <th className="text-right px-4 py-2 font-medium">Duration</th>
                <th className="text-right px-4 py-2 font-medium">Size</th>
                <th className="text-center px-4 py-2 font-medium">Actions</th>
              </tr>
            </thead>
            <tbody>
              {entries.slice(-200).reverse().map((entry) => {
                const category = (['lifecycle', 'tools', 'resources', 'prompts'] as MethodCategory[])
                  .find((c) => c === entry.category) || 'lifecycle';
                const colors = CATEGORY_COLORS[category];
                return (
                  <tr key={entry.id} className="border-b border-border/30 hover:bg-muted/20 transition-colors">
                    <td className="px-4 py-2 text-muted-foreground font-mono">
                      {new Date(entry.timestamp).toLocaleTimeString()}
                    </td>
                    <td className="px-4 py-2">
                      <span className={cn('font-mono', colors.text)}>{entry.method}</span>
                    </td>
                    <td className="px-4 py-2">
                      {entry.error ? (
                        <span className="text-red-400 flex items-center gap-1">
                          <XCircle className="w-3 h-3" /> Error
                        </span>
                      ) : (
                        <span className="text-emerald-400 flex items-center gap-1">
                          <CheckCircle className="w-3 h-3" /> OK
                        </span>
                      )}
                    </td>
                    <td className="px-4 py-2 text-right text-muted-foreground font-mono">
                      {entry.durationMs.toFixed(0)}ms
                    </td>
                    <td className="px-4 py-2 text-right text-muted-foreground font-mono">
                      {entry.responseSize < 1024
                        ? `${entry.responseSize}B`
                        : `${(entry.responseSize / 1024).toFixed(1)}KB`}
                    </td>
                    <td className="px-4 py-2 text-center">
                      <button
                        onClick={() => onReplay(entry)}
                        className="p-1 text-muted-foreground hover:text-primary transition-colors"
                        title="Replay request"
                      >
                        <Play className="w-3 h-3" />
                      </button>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        )}
      </div>

      {/* Summary */}
      <div className="px-4 py-1.5 border-t border-border text-[10px] text-muted-foreground">
        {entries.length} entries · Session: {new Date().toLocaleDateString()}
      </div>
    </div>
  );
}

// ── Assertions View ──────────────────────────────────────────────────────────

function AssertionsView({
  assertions,
  results,
  dsl,
  onDslChange,
  onAdd,
  onRemove,
  onToggle,
  onLoadSuite,
}: {
  assertions: Assertion[];
  results: AssertionResult[];
  dsl: string;
  onDslChange: (v: string) => void;
  onAdd: () => void;
  onRemove: (id: string) => void;
  onToggle: (id: string) => void;
  onLoadSuite: (suite: { assertions: Assertion[] }) => void;
}) {
  return (
    <div className="flex flex-col h-full">
      {/* Add Assertion */}
      <div className="px-4 py-3 border-b border-border">
        <div className="text-xs font-medium text-muted-foreground mb-2">Add Assertion (DSL)</div>
        <div className="flex gap-2">
          <input
            type="text"
            value={dsl}
            onChange={(e) => onDslChange(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && onAdd()}
            placeholder='$.result.tools | length >= 5'
            className="flex-1 bg-muted border border-border rounded-md px-3 py-1.5 text-xs font-mono text-foreground focus:outline-none focus:ring-1 focus:ring-ring"
          />
          <button onClick={onAdd} className="px-3 py-1.5 bg-primary text-white rounded-md text-xs hover:bg-primary/90">
            Add
          </button>
        </div>
        <div className="flex gap-2 mt-2">
          <span className="text-[10px] text-muted-foreground/70">Presets:</span>
          {BUILTIN_SUITES.map((suite) => (
            <button
              key={suite.id}
              onClick={() => onLoadSuite(suite)}
              className="text-[10px] px-2 py-0.5 bg-muted text-muted-foreground rounded hover:text-foreground hover:bg-muted transition-colors"
            >
              {suite.name}
            </button>
          ))}
        </div>
      </div>

      {/* Assertions List */}
      <div className="flex-1 overflow-auto p-4">
        {assertions.length === 0 ? (
          <div className="text-center text-muted-foreground/70 text-sm py-8">
            <BookOpen className="w-8 h-8 mx-auto mb-2 opacity-30" />
            <p>No assertions defined</p>
            <p className="text-[10px] mt-1">Add assertions using the DSL above or load a preset suite</p>
          </div>
        ) : (
          <div className="space-y-2">
            {assertions.map((assertion) => {
              const result = results.find((r) => r.assertionId === assertion.id);
              return (
                <div
                  key={assertion.id}
                  className={cn(
                    'flex items-center gap-3 px-3 py-2 rounded-md border transition-colors',
                    result
                      ? result.passed
                        ? 'border-emerald-500/20 bg-emerald-500/5'
                        : 'border-red-500/20 bg-red-500/5'
                      : 'border-border bg-muted/30'
                  )}
                >
                  <button onClick={() => onToggle(assertion.id)} className="flex-shrink-0">
                    {assertion.enabled ? (
                      <CheckCircle className={cn('w-4 h-4', result?.passed ? 'text-emerald-400' : result ? 'text-red-400' : 'text-muted-foreground')} />
                    ) : (
                      <div className="w-4 h-4 rounded-full border border-border" />
                    )}
                  </button>
                  <div className="flex-1 min-w-0">
                    <div className="text-xs text-foreground truncate">{assertion.label}</div>
                    <div className="text-[10px] text-muted-foreground font-mono truncate">
                      {assertion.path} {assertion.operator} {assertion.expected !== undefined ? JSON.stringify(assertion.expected) : ''}
                    </div>
                    {result && !result.passed && (
                      <div className="text-[10px] text-red-300 mt-0.5">{result.message}</div>
                    )}
                  </div>
                  <button onClick={() => onRemove(assertion.id)} className="text-muted-foreground/70 hover:text-red-400 transition-colors flex-shrink-0">
                    <Trash2 className="w-3.5 h-3.5" />
                  </button>
                </div>
              );
            })}
          </div>
        )}
      </div>

      {/* Summary */}
      {results.length > 0 && (
        <div className="px-4 py-2 border-t border-border flex items-center gap-3 text-xs">
          <span className="text-emerald-400">{results.filter((r) => r.passed).length} passed</span>
          <span className="text-red-400">{results.filter((r) => !r.passed).length} failed</span>
          <span className="text-muted-foreground">of {results.length} assertions</span>
        </div>
      )}
    </div>
  );
}

// ── Sequence Results View ────────────────────────────────────────────────────

function SequenceResultsView({
  results,
  running,
}: {
  results: Array<{ step: string; response: JsonRpcResponse | null; duration: number; error?: string }>;
  running: string | null;
}) {
  if (results.length === 0 && !running) {
    return (
      <div className="flex items-center justify-center h-full text-muted-foreground/70 text-sm">
        <div className="text-center">
          <Layers className="w-8 h-8 mx-auto mb-2 opacity-30" />
          <p>Select a sequence from the sidebar to run</p>
        </div>
      </div>
    );
  }

  return (
    <div className="p-4 space-y-3 overflow-auto h-full">
      {running && (
        <div className="flex items-center gap-2 text-xs text-amber-300 mb-3">
          <RefreshCw className="w-3.5 h-3.5 animate-spin" />
          <span>Running sequence: {running}</span>
        </div>
      )}
      {results.map((result, i) => (
        <div
          key={i}
          className={cn(
            'p-3 rounded-md border',
            result.error
              ? 'border-red-500/20 bg-red-500/5'
              : 'border-emerald-500/20 bg-emerald-500/5'
          )}
        >
          <div className="flex items-center justify-between mb-2">
            <div className="flex items-center gap-2">
              <span className="text-xs font-medium text-foreground">Step {i + 1}: {result.step}</span>
              {result.error ? (
                <XCircle className="w-3.5 h-3.5 text-red-400" />
              ) : (
                <CheckCircle className="w-3.5 h-3.5 text-emerald-400" />
              )}
            </div>
            <span className="text-[10px] text-muted-foreground font-mono">{result.duration.toFixed(0)}ms</span>
          </div>
          {result.error && (
            <div className="text-[11px] text-red-300">{result.error}</div>
          )}
          {result.response && (
            <pre className="text-[10px] font-mono text-muted-foreground mt-2 max-h-32 overflow-auto whitespace-pre-wrap">
              {JSON.stringify(result.response, null, 2).slice(0, 500)}
              {JSON.stringify(result.response, null, 2).length > 500 ? '\n...' : ''}
            </pre>
          )}
        </div>
      ))}
    </div>
  );
}

// Copyright 2025 AgentReplay (https://github.com/agentreplay)
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0

import { useState, useCallback, useMemo, useEffect } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import {
  Shield,
  Play,
  Upload,
  FileText,
  AlertTriangle,
  CheckCircle2,
  XCircle,
  Loader2,
  TrendingUp,
  BarChart3,
  Lock,
  RefreshCw,
  ChevronDown,
  ChevronRight,
  Eye,
  Copy,
  Search,
  Brain,
  Database,
} from 'lucide-react';
import { cn } from '../../lib/utils';
import {
  loadSkill,
  runSkillTests,
  scanSkillSecurity,
  getSkillDrift,
  fetchSkillMemorySkills,
  type SkillManifest,
  type ValidationReport,
  type ValidationFinding,
  type TestResult,
  type OwaspScanResult,
  type OwaspFinding,
  type DriftResult,
  type SkillMemoryEntry,
} from '../../lib/api/skill-tester';

// ─── Tabs ────────────────────────────────────────────────────
type TabId = 'loader' | 'tests' | 'security' | 'drift';

const tabs: { id: TabId; label: string; icon: React.ElementType }[] = [
  { id: 'loader', label: 'Skill Loader', icon: Upload },
  { id: 'tests', label: 'Test Runner', icon: Play },
  { id: 'security', label: 'Security Scanner', icon: Shield },
  { id: 'drift', label: 'Drift Monitor', icon: TrendingUp },
];

// ─── Main Page ───────────────────────────────────────────────
export default function SkillTesterPage() {
  const [activeTab, setActiveTab] = useState<TabId>('loader');
  const [manifest, setManifest] = useState<SkillManifest | null>(null);
  const [validation, setValidation] = useState<ValidationReport | null>(null);
  const [skillContent, setSkillContent] = useState<string | null>(null);
  const [testResults, setTestResults] = useState<TestResult[]>([]);
  const [owaspResult, setOwaspResult] = useState<OwaspScanResult | null>(null);
  const [driftResults, setDriftResults] = useState<DriftResult[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleSkillLoaded = useCallback((m: SkillManifest, v: ValidationReport, content?: string) => {
    setManifest(m);
    setValidation(v);
    setSkillContent(content ?? null);
    setError(null);
  }, []);

  const handleTestsDone = useCallback((results: TestResult[]) => {
    setTestResults(results);
  }, []);

  const handleSecurityDone = useCallback((result: OwaspScanResult) => {
    setOwaspResult(result);
  }, []);

  const handleDriftDone = useCallback((results: DriftResult[]) => {
    setDriftResults(results);
  }, []);

  return (
    <div className="flex h-full flex-col bg-background text-foreground">
      {/* Header */}
      <div className="flex items-center justify-between border-b border-border px-6 py-4">
        <div className="flex items-center gap-3">
          <Shield className="h-6 w-6 text-blue-500" />
          <div>
            <h1 className="text-lg font-semibold">Skill Tester (Experimental)</h1>
            <p className="text-xs text-muted-foreground">
              Test, debug, and certify AI agent skills
            </p>
          </div>
        </div>
        {manifest && (
          <div className="flex items-center gap-2 rounded-lg bg-muted px-3 py-1.5 text-sm">
            <FileText className="h-4 w-4" />
            <span className="font-medium">{manifest.name}</span>
            <span className="text-muted-foreground">v{manifest.version}</span>
          </div>
        )}
      </div>

      {/* Tabs */}
      <div className="flex border-b border-border px-6">
        {tabs.map((tab) => {
          const Icon = tab.icon;
          return (
            <button
              key={tab.id}
              onClick={() => setActiveTab(tab.id)}
              className={cn(
                'flex items-center gap-2 border-b-2 px-4 py-3 text-sm font-medium transition-colors',
                activeTab === tab.id
                  ? 'border-blue-500 text-blue-500'
                  : 'border-transparent text-muted-foreground hover:text-foreground'
              )}
            >
              <Icon className="h-4 w-4" />
              {tab.label}
            </button>
          );
        })}
      </div>

      {/* Error banner */}
      <AnimatePresence>
        {error && (
          <motion.div
            initial={{ height: 0, opacity: 0 }}
            animate={{ height: 'auto', opacity: 1 }}
            exit={{ height: 0, opacity: 0 }}
            className="border-b border-red-500/20 bg-red-500/10 px-6 py-3 text-sm text-red-400"
          >
            <div className="flex items-center gap-2">
              <AlertTriangle className="h-4 w-4" />
              {error}
              <button onClick={() => setError(null)} className="ml-auto text-red-300 hover:text-red-200">
                Dismiss
              </button>
            </div>
          </motion.div>
        )}
      </AnimatePresence>

      {/* Tab content */}
      <div className="flex-1 overflow-y-auto p-6">
        {activeTab === 'loader' && (
          <SkillLoaderTab
            manifest={manifest}
            validation={validation}
            onLoaded={handleSkillLoaded}
            onError={setError}
          />
        )}
        {activeTab === 'tests' && (
          <TestRunnerTab
            manifest={manifest}
            skillContent={skillContent}
            testResults={testResults}
            onTestsDone={handleTestsDone}
            onError={setError}
          />
        )}
        {activeTab === 'security' && (
          <SecurityScannerTab
            manifest={manifest}
            skillContent={skillContent}
            owaspResult={owaspResult}
            onScanDone={handleSecurityDone}
            onError={setError}
          />
        )}
        {activeTab === 'drift' && (
          <DriftMonitorTab
            manifest={manifest}
            driftResults={driftResults}
            onDriftDone={handleDriftDone}
            onError={setError}
          />
        )}
      </div>
    </div>
  );
}

// ─── Skill Loader Tab ────────────────────────────────────────
function SkillLoaderTab({
  manifest,
  validation,
  onLoaded,
  onError,
}: {
  manifest: SkillManifest | null;
  validation: ValidationReport | null;
  onLoaded: (m: SkillManifest, v: ValidationReport, content?: string) => void;
  onError: (msg: string) => void;
}) {
  const [input, setInput] = useState('');
  const [loading, setLoading] = useState(false);
  const [inputMode, setInputMode] = useState<'path' | 'paste' | 'url' | 'memory'>('memory');

  // Skill Memory integration state
  const [memorySkills, setMemorySkills] = useState<SkillMemoryEntry[]>([]);
  const [memoryLoading, setMemoryLoading] = useState(false);
  const [selectedSkillId, setSelectedSkillId] = useState<string>('');
  const [memorySearch, setMemorySearch] = useState('');

  // Fetch skills from memory when switching to memory mode
  useEffect(() => {
    if (inputMode === 'memory' && memorySkills.length === 0) {
      setMemoryLoading(true);
      fetchSkillMemorySkills()
        .then((skills) => setMemorySkills(skills))
        .catch(() => onError('Failed to fetch skills from Skill Memory'))
        .finally(() => setMemoryLoading(false));
    }
  }, [inputMode]);

  const filteredMemorySkills = useMemo(() => {
    if (!memorySearch.trim()) return memorySkills;
    const q = memorySearch.toLowerCase();
    return memorySkills.filter(
      (s) =>
        s.name.toLowerCase().includes(q) ||
        s.description.toLowerCase().includes(q) ||
        s.category.toLowerCase().includes(q) ||
        s.tags.some((t) => t.toLowerCase().includes(q))
    );
  }, [memorySkills, memorySearch]);

  const handleLoad = useCallback(async () => {
    setLoading(true);
    try {
      let payload: { path?: string; content?: string; url?: string };

      if (inputMode === 'memory') {
        const skill = memorySkills.find((s) => s.skill_id === selectedSkillId);
        if (!skill) {
          onError('Please select a skill from memory');
          setLoading(false);
          return;
        }
        // Build proper SKILL.md content with YAML frontmatter from memory metadata.
        // The backend parser requires `---` delimited frontmatter with at least name,
        // description, version fields, followed by the instruction body.
        const def = (skill.definition || '').trim();
        const alreadyHasFrontmatter = def.startsWith('---');
        let skillMd: string;
        if (alreadyHasFrontmatter) {
          skillMd = def;
        } else {
          const yamlLines = [
            '---',
            `name: ${skill.name}`,
            `description: ${skill.description || skill.name}`,
            `version: "v${skill.version ?? 1}"`,
          ];
          if (skill.category) yamlLines.push(`category: ${skill.category}`);
          if (skill.tags?.length) yamlLines.push(`tags: [${skill.tags.join(', ')}]`);
          if (skill.status) yamlLines.push(`status: ${skill.status}`);
          yamlLines.push('---');
          yamlLines.push('');
          yamlLines.push(def || `# ${skill.name}`);
          skillMd = yamlLines.join('\n');
        }
        payload = { content: skillMd };
      } else {
        if (!input.trim()) {
          setLoading(false);
          return;
        }
        payload =
          inputMode === 'path' ? { path: input } :
          inputMode === 'url' ? { url: input } :
          { content: input };
      }

      const res = await loadSkill(payload);
      onLoaded(res.manifest, res.validation, payload.content);
    } catch (e: any) {
      onError(e.message || 'Failed to load skill');
    } finally {
      setLoading(false);
    }
  }, [input, inputMode, selectedSkillId, memorySkills, onLoaded, onError]);

  return (
    <div className="space-y-6">
      {/* Input controls */}
      <div className="rounded-lg border border-border bg-card p-6">
        <h2 className="mb-4 text-base font-semibold">Load SKILL.md</h2>
        <div className="mb-3 flex gap-2">
          {([
            { mode: 'memory' as const, label: 'From Memory', icon: Brain },
            { mode: 'path' as const, label: 'File Path', icon: FileText },
            { mode: 'paste' as const, label: 'Paste Content', icon: Copy },
            { mode: 'url' as const, label: 'URL', icon: Search },
          ]).map(({ mode, label, icon: ModeIcon }) => (
            <button
              key={mode}
              onClick={() => setInputMode(mode)}
              className={cn(
                'flex items-center gap-1.5 rounded-md px-3 py-1.5 text-xs font-medium transition-colors',
                inputMode === mode
                  ? mode === 'memory'
                    ? 'bg-purple-500/20 text-purple-400'
                    : 'bg-blue-500/20 text-blue-400'
                  : 'bg-muted text-muted-foreground hover:text-foreground'
              )}
            >
              <ModeIcon className="h-3.5 w-3.5" />
              {label}
            </button>
          ))}
        </div>

        {/* From Memory mode */}
        {inputMode === 'memory' && (
          <div className="space-y-3">
            {memoryLoading ? (
              <div className="flex items-center gap-2 py-8 justify-center text-sm text-muted-foreground">
                <Loader2 className="h-4 w-4 animate-spin" />
                Loading skills from memory...
              </div>
            ) : memorySkills.length === 0 ? (
              <div className="flex flex-col items-center gap-2 py-8 text-sm text-muted-foreground">
                <Database className="h-8 w-8 opacity-30" />
                <p>No skills in memory yet.</p>
                <p className="text-xs">Create skills in the Skill Memory page first.</p>
              </div>
            ) : (
              <>
                {/* Search within memory skills */}
                <div className="relative">
                  <Search className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
                  <input
                    value={memorySearch}
                    onChange={(e) => setMemorySearch(e.target.value)}
                    placeholder="Search skills by name, category, or tag..."
                    className="w-full rounded-md border border-border bg-background pl-9 pr-3 py-2 text-sm focus:border-purple-500 focus:outline-none"
                  />
                </div>

                {/* Skill list */}
                <div className="max-h-64 overflow-y-auto rounded-md border border-border divide-y divide-border">
                  {filteredMemorySkills.map((skill) => (
                    <button
                      key={skill.skill_id}
                      onClick={() => setSelectedSkillId(skill.skill_id)}
                      className={cn(
                        'flex w-full items-start gap-3 px-4 py-3 text-left transition-colors',
                        selectedSkillId === skill.skill_id
                          ? 'bg-purple-500/10 border-l-2 border-l-purple-500'
                          : 'hover:bg-muted/50'
                      )}
                    >
                      <Brain className={cn(
                        'mt-0.5 h-4 w-4 shrink-0',
                        selectedSkillId === skill.skill_id ? 'text-purple-400' : 'text-muted-foreground'
                      )} />
                      <div className="flex-1 min-w-0">
                        <div className="flex items-center gap-2">
                          <span className="font-medium text-sm truncate">{skill.name}</span>
                          <span className={cn(
                            'rounded px-1.5 py-0.5 text-[10px] font-medium',
                            skill.status === 'active' ? 'bg-green-500/20 text-green-400' :
                            skill.status === 'draft' ? 'bg-yellow-500/20 text-yellow-400' :
                            'bg-gray-500/20 text-gray-400'
                          )}>
                            {skill.status}
                          </span>
                          {skill.category && (
                            <span className="rounded px-1.5 py-0.5 text-[10px] bg-blue-500/10 text-blue-400">
                              {skill.category}
                            </span>
                          )}
                        </div>
                        <p className="mt-0.5 text-xs text-muted-foreground truncate">{skill.description}</p>
                        {skill.tags.length > 0 && (
                          <div className="mt-1 flex flex-wrap gap-1">
                            {skill.tags.slice(0, 4).map((tag) => (
                              <span key={tag} className="rounded bg-muted px-1.5 py-0.5 text-[10px] text-muted-foreground">
                                {tag}
                              </span>
                            ))}
                            {skill.tags.length > 4 && (
                              <span className="text-[10px] text-muted-foreground">+{skill.tags.length - 4}</span>
                            )}
                          </div>
                        )}
                      </div>
                      {selectedSkillId === skill.skill_id && (
                        <CheckCircle2 className="mt-0.5 h-4 w-4 shrink-0 text-purple-400" />
                      )}
                    </button>
                  ))}
                </div>
                <p className="text-xs text-muted-foreground">
                  {filteredMemorySkills.length} skill{filteredMemorySkills.length !== 1 ? 's' : ''} available
                  {selectedSkillId && ' — 1 selected'}
                </p>
              </>
            )}
          </div>
        )}

        {/* Paste mode */}
        {inputMode === 'paste' && (
          <textarea
            value={input}
            onChange={(e) => setInput(e.target.value)}
            placeholder="Paste SKILL.md content here..."
            rows={8}
            className="w-full rounded-md border border-border bg-background px-3 py-2 text-sm font-mono focus:border-blue-500 focus:outline-none"
          />
        )}

        {/* File path / URL mode */}
        {(inputMode === 'path' || inputMode === 'url') && (
          <input
            value={input}
            onChange={(e) => setInput(e.target.value)}
            placeholder={inputMode === 'path' ? '/path/to/SKILL.md' : 'https://...'}
            className="w-full rounded-md border border-border bg-background px-3 py-2 text-sm focus:border-blue-500 focus:outline-none"
            onKeyDown={(e) => e.key === 'Enter' && handleLoad()}
          />
        )}

        <button
          onClick={handleLoad}
          disabled={loading || (inputMode === 'memory' ? !selectedSkillId : !input.trim())}
          className={cn(
            'mt-3 flex items-center gap-2 rounded-md px-4 py-2 text-sm font-medium text-white transition-colors disabled:opacity-50',
            inputMode === 'memory'
              ? 'bg-purple-600 hover:bg-purple-500'
              : 'bg-blue-600 hover:bg-blue-500'
          )}
        >
          {loading ? <Loader2 className="h-4 w-4 animate-spin" /> : inputMode === 'memory' ? <Brain className="h-4 w-4" /> : <Upload className="h-4 w-4" />}
          {inputMode === 'memory' ? 'Load from Memory' : 'Load Skill'}
        </button>
      </div>

      {/* Manifest details */}
      {manifest && (
        <motion.div
          initial={{ opacity: 0, y: 10 }}
          animate={{ opacity: 1, y: 0 }}
          className="rounded-lg border border-border bg-card p-6"
        >
          <h2 className="mb-4 text-base font-semibold">Manifest</h2>
          <div className="grid grid-cols-2 gap-4 text-sm">
            <InfoRow label="Name" value={manifest.name} />
            <InfoRow label="Version" value={manifest.version} />
            <InfoRow label="Hash" value={manifest.version_hash.slice(0, 16) + '…'} mono />
            <InfoRow label="Description" value={manifest.description} span2 />
          </div>

          {manifest.requires && (
            <div className="mt-4">
              <h3 className="mb-2 text-sm font-medium text-muted-foreground">Requirements</h3>
              <div className="flex flex-wrap gap-2">
                {manifest.requires.mcp?.map((m) => (
                  <Badge key={m} text={`MCP: ${m}`} color="purple" />
                ))}
                {manifest.requires.bins?.map((b) => (
                  <Badge key={b} text={`Bin: ${b}`} color="orange" />
                ))}
                {manifest.requires.env?.map((e) => (
                  <Badge key={e} text={`Env: ${e}`} color="green" />
                ))}
              </div>
            </div>
          )}
        </motion.div>
      )}

      {/* Validation */}
      {validation && (
        <motion.div
          initial={{ opacity: 0, y: 10 }}
          animate={{ opacity: 1, y: 0 }}
          className="rounded-lg border border-border bg-card p-6"
        >
          <div className="mb-4 flex items-center justify-between">
            <h2 className="text-base font-semibold">Validation</h2>
            <div className="flex gap-3 text-xs">
              <span className="text-green-400">{validation.pass_count} pass</span>
              <span className="text-yellow-400">{validation.warn_count} warn</span>
              <span className="text-red-400">{validation.error_count} error</span>
            </div>
          </div>
          <div className="space-y-2">
            {validation.findings.map((f, i) => (
              <FindingRow key={i} finding={f} />
            ))}
          </div>
        </motion.div>
      )}
    </div>
  );
}

// ─── Test Runner Tab ─────────────────────────────────────────
function TestRunnerTab({
  manifest,
  skillContent,
  testResults,
  onTestsDone,
  onError,
}: {
  manifest: SkillManifest | null;
  skillContent: string | null;
  testResults: TestResult[];
  onTestsDone: (results: TestResult[]) => void;
  onError: (msg: string) => void;
}) {
  const [loading, setLoading] = useState(false);
  const [tagFilter, setTagFilter] = useState('');
  const [expandedTest, setExpandedTest] = useState<string | null>(null);

  const summary = useMemo(() => {
    const total = testResults.length;
    const passed = testResults.filter((r) => r.status === 'Passed').length;
    const failed = testResults.filter((r) => r.status === 'Failed').length;
    const skipped = testResults.filter((r) => r.status === 'Skipped').length;
    const duration = testResults.reduce((s, r) => s + r.duration_ms, 0);
    return { total, passed, failed, skipped, duration };
  }, [testResults]);

  const handleRun = useCallback(async () => {
    if (!manifest) return;
    setLoading(true);
    try {
      const tags = tagFilter.trim() ? tagFilter.split(',').map((t) => t.trim()) : undefined;
      const res = await runSkillTests({ skill: manifest.name, content: skillContent ?? undefined, tags });
      onTestsDone(res.results);
    } catch (e: any) {
      onError(e.message || 'Test run failed');
    } finally {
      setLoading(false);
    }
  }, [manifest, tagFilter, onTestsDone, onError, skillContent]);

  if (!manifest) {
    return <EmptyState message="Load a skill first to run tests" icon={Play} />;
  }

  return (
    <div className="space-y-6">
      {/* Controls */}
      <div className="flex items-center gap-4">
        <button
          onClick={handleRun}
          disabled={loading}
          className="flex items-center gap-2 rounded-md bg-green-600 px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-green-500 disabled:opacity-50"
        >
          {loading ? <Loader2 className="h-4 w-4 animate-spin" /> : <Play className="h-4 w-4" />}
          Run Tests
        </button>
        <input
          value={tagFilter}
          onChange={(e) => setTagFilter(e.target.value)}
          placeholder="Filter by tags (comma-separated)..."
          className="flex-1 rounded-md border border-border bg-background px-3 py-2 text-sm focus:border-blue-500 focus:outline-none"
        />
      </div>

      {/* Summary bar */}
      {testResults.length > 0 && (
        <motion.div
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          className="grid grid-cols-5 gap-3"
        >
          <StatCard label="Total" value={summary.total} />
          <StatCard label="Passed" value={summary.passed} color="green" />
          <StatCard label="Failed" value={summary.failed} color="red" />
          <StatCard label="Skipped" value={summary.skipped} color="yellow" />
          <StatCard label="Duration" value={`${summary.duration}ms`} />
        </motion.div>
      )}

      {/* Results list */}
      <div className="space-y-2">
        {testResults.map((result) => (
          <TestResultCard
            key={result.test_id}
            result={result}
            expanded={expandedTest === result.test_id}
            onToggle={() =>
              setExpandedTest((prev) =>
                prev === result.test_id ? null : result.test_id
              )
            }
          />
        ))}
      </div>
    </div>
  );
}

// ─── Security Scanner Tab ────────────────────────────────────
function SecurityScannerTab({
  manifest,
  skillContent,
  owaspResult,
  onScanDone,
  onError,
}: {
  manifest: SkillManifest | null;
  skillContent: string | null;
  owaspResult: OwaspScanResult | null;
  onScanDone: (result: OwaspScanResult) => void;
  onError: (msg: string) => void;
}) {
  const [loading, setLoading] = useState(false);

  const handleScan = useCallback(async () => {
    if (!manifest) return;
    setLoading(true);
    try {
      const res = await scanSkillSecurity(manifest.name, skillContent ?? undefined);
      onScanDone(res);
    } catch (e: any) {
      onError(e.message || 'Security scan failed');
    } finally {
      setLoading(false);
    }
  }, [manifest, onScanDone, onError, skillContent]);

  if (!manifest) {
    return <EmptyState message="Load a skill first to scan" icon={Shield} />;
  }

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <button
          onClick={handleScan}
          disabled={loading}
          className="flex items-center gap-2 rounded-md bg-purple-600 px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-purple-500 disabled:opacity-50"
        >
          {loading ? <Loader2 className="h-4 w-4 animate-spin" /> : <Lock className="h-4 w-4" />}
          Run OWASP Scan
        </button>
        {owaspResult && (
          <div
            className={cn(
              'rounded-md px-3 py-1.5 text-sm font-medium',
              owaspResult.safe_for_production
                ? 'bg-green-500/20 text-green-400'
                : 'bg-red-500/20 text-red-400'
            )}
          >
            {owaspResult.safe_for_production ? 'Safe for Production' : 'Issues Found'}
          </div>
        )}
      </div>

      {owaspResult && (
        <motion.div initial={{ opacity: 0 }} animate={{ opacity: 1 }} className="space-y-4">
          {/* Summary */}
          <div className="grid grid-cols-3 gap-3">
            <StatCard label="High Risk" value={owaspResult.high_risk_count} color="red" />
            <StatCard label="Medium Risk" value={owaspResult.medium_risk_count} color="yellow" />
            <StatCard
              label="Safe"
              value={owaspResult.findings.filter((f) => f.risk === 'Pass').length}
              color="green"
            />
          </div>

          {/* Finding list */}
          <div className="space-y-2">
            {owaspResult.findings.map((f) => (
              <OwaspFindingCard key={f.id} finding={f} />
            ))}
          </div>
        </motion.div>
      )}
    </div>
  );
}

// ─── Drift Monitor Tab ───────────────────────────────────────
function DriftMonitorTab({
  manifest,
  driftResults,
  onDriftDone,
  onError,
}: {
  manifest: SkillManifest | null;
  driftResults: DriftResult[];
  onDriftDone: (results: DriftResult[]) => void;
  onError: (msg: string) => void;
}) {
  const [loading, setLoading] = useState(false);
  const [window, setWindow] = useState('24h');

  const handleCheck = useCallback(async () => {
    if (!manifest) return;
    setLoading(true);
    try {
      const res = await getSkillDrift(manifest.name, window);
      onDriftDone(res);
    } catch (e: any) {
      onError(e.message || 'Drift check failed');
    } finally {
      setLoading(false);
    }
  }, [manifest, window, onDriftDone, onError]);

  if (!manifest) {
    return <EmptyState message="Load a skill first to monitor drift" icon={TrendingUp} />;
  }

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <button
          onClick={handleCheck}
          disabled={loading}
          className="flex items-center gap-2 rounded-md bg-amber-600 px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-amber-500 disabled:opacity-50"
        >
          {loading ? <Loader2 className="h-4 w-4 animate-spin" /> : <RefreshCw className="h-4 w-4" />}
          Check Drift
        </button>
        <select
          value={window}
          onChange={(e) => setWindow(e.target.value)}
          className="rounded-md border border-border bg-background px-3 py-2 text-sm focus:outline-none"
        >
          <option value="1h">Last 1 hour</option>
          <option value="6h">Last 6 hours</option>
          <option value="24h">Last 24 hours</option>
          <option value="7d">Last 7 days</option>
        </select>
      </div>

      {driftResults.length > 0 && (
        <motion.div initial={{ opacity: 0 }} animate={{ opacity: 1 }} className="space-y-3">
          {driftResults.map((d, i) => (
            <DriftCard key={i} drift={d} />
          ))}
        </motion.div>
      )}
    </div>
  );
}

// ─── Shared Sub-components ───────────────────────────────────

function InfoRow({ label, value, mono, span2 }: { label: string; value: string; mono?: boolean; span2?: boolean }) {
  return (
    <div className={span2 ? 'col-span-2' : ''}>
      <span className="text-muted-foreground">{label}:</span>{' '}
      <span className={cn('font-medium', mono && 'font-mono text-xs')}>{value}</span>
    </div>
  );
}

function Badge({ text, color }: { text: string; color: string }) {
  const colorMap: Record<string, string> = {
    purple: 'bg-purple-500/20 text-purple-400',
    orange: 'bg-orange-500/20 text-orange-400',
    green: 'bg-green-500/20 text-green-400',
    blue: 'bg-blue-500/20 text-blue-400',
    red: 'bg-red-500/20 text-red-400',
  };
  return (
    <span className={cn('rounded-md px-2 py-0.5 text-xs font-medium', colorMap[color] || colorMap.blue)}>
      {text}
    </span>
  );
}

function FindingRow({ finding }: { finding: ValidationFinding }) {
  const icon =
    finding.severity === 'Pass' ? <CheckCircle2 className="h-4 w-4 text-green-400" /> :
    finding.severity === 'Warning' ? <AlertTriangle className="h-4 w-4 text-yellow-400" /> :
    <XCircle className="h-4 w-4 text-red-400" />;

  return (
    <div className="flex items-start gap-3 rounded-md border border-border bg-background px-3 py-2 text-sm">
      {icon}
      <div className="flex-1">
        <span className="font-medium">{finding.check}</span>
        <span className="ml-2 text-muted-foreground">{finding.message}</span>
        {finding.detail && <p className="mt-1 text-xs text-muted-foreground">{finding.detail}</p>}
      </div>
    </div>
  );
}

function StatCard({ label, value, color }: { label: string; value: string | number; color?: string }) {
  const colorMap: Record<string, string> = {
    green: 'text-green-400',
    red: 'text-red-400',
    yellow: 'text-yellow-400',
    blue: 'text-blue-400',
  };
  return (
    <div className="rounded-lg border border-border bg-card p-3 text-center">
      <div className={cn('text-2xl font-bold', color ? colorMap[color] : 'text-foreground')}>
        {value}
      </div>
      <div className="text-xs text-muted-foreground">{label}</div>
    </div>
  );
}

function TestResultCard({
  result,
  expanded,
  onToggle,
}: {
  result: TestResult;
  expanded: boolean;
  onToggle: () => void;
}) {
  const statusIcon =
    result.status === 'Passed' ? <CheckCircle2 className="h-4 w-4 text-green-400" /> :
    result.status === 'Failed' ? <XCircle className="h-4 w-4 text-red-400" /> :
    <AlertTriangle className="h-4 w-4 text-yellow-400" />;

  return (
    <div className="rounded-lg border border-border bg-card">
      <button
        onClick={onToggle}
        className="flex w-full items-center gap-3 px-4 py-3 text-left text-sm"
      >
        {statusIcon}
        <span className="flex-1 font-medium">{result.test_id}</span>
        <span className="text-xs text-muted-foreground">{result.duration_ms}ms</span>
        <span className="text-xs text-muted-foreground">
          {result.assertions_passed}/{result.assertions_passed + result.assertions_failed}
        </span>
        {expanded ? <ChevronDown className="h-4 w-4" /> : <ChevronRight className="h-4 w-4" />}
      </button>
      <AnimatePresence>
        {expanded && (
          <motion.div
            initial={{ height: 0, opacity: 0 }}
            animate={{ height: 'auto', opacity: 1 }}
            exit={{ height: 0, opacity: 0 }}
            className="overflow-hidden border-t border-border"
          >
            <div className="space-y-2 px-4 py-3">
              {result.assertion_results.map((a, i) => (
                <div key={i} className="flex items-start gap-2 text-xs">
                  {a.passed ? (
                    <CheckCircle2 className="mt-0.5 h-3 w-3 text-green-400" />
                  ) : (
                    <XCircle className="mt-0.5 h-3 w-3 text-red-400" />
                  )}
                  <div>
                    <span className="font-medium">{a.assertion_type}</span>
                    <span className="ml-2 text-muted-foreground">{a.message}</span>
                  </div>
                </div>
              ))}
              {result.error && (
                <div className="mt-2 rounded-md bg-red-500/10 p-2 text-xs text-red-400">
                  {result.error}
                </div>
              )}
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}

function OwaspFindingCard({ finding }: { finding: OwaspFinding }) {
  const [expanded, setExpanded] = useState(false);
  const riskColor =
    finding.risk === 'High' ? 'text-red-400 bg-red-500/20' :
    finding.risk === 'Medium' ? 'text-yellow-400 bg-yellow-500/20' :
    finding.risk === 'Low' ? 'text-orange-400 bg-orange-500/20' :
    'text-green-400 bg-green-500/20';

  return (
    <div className="rounded-lg border border-border bg-card">
      <button
        onClick={() => setExpanded(!expanded)}
        className="flex w-full items-center gap-3 px-4 py-3 text-left text-sm"
      >
        <span className={cn('rounded px-2 py-0.5 text-xs font-bold', riskColor)}>{finding.risk}</span>
        <span className="font-mono text-xs text-muted-foreground">{finding.id}</span>
        <span className="flex-1 font-medium">{finding.name}</span>
        {expanded ? <ChevronDown className="h-4 w-4" /> : <ChevronRight className="h-4 w-4" />}
      </button>
      <AnimatePresence>
        {expanded && (
          <motion.div
            initial={{ height: 0, opacity: 0 }}
            animate={{ height: 'auto', opacity: 1 }}
            exit={{ height: 0, opacity: 0 }}
            className="overflow-hidden border-t border-border"
          >
            <div className="space-y-2 px-4 py-3 text-sm">
              <p className="text-muted-foreground">{finding.description}</p>
              {finding.recommendation && (
                <div className="rounded-md bg-blue-500/10 p-2 text-xs text-blue-400">
                  <strong>Recommendation:</strong> {finding.recommendation}
                </div>
              )}
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}

function DriftCard({ drift }: { drift: DriftResult }) {
  const statusColor =
    drift.status === 'Stable' ? 'bg-green-500/20 text-green-400' :
    drift.status === 'Watch' ? 'bg-yellow-500/20 text-yellow-400' :
    drift.status === 'Drift' ? 'bg-orange-500/20 text-orange-400' :
    'bg-red-500/20 text-red-400';

  return (
    <div className="rounded-lg border border-border bg-card p-4">
      <div className="flex items-center gap-3">
        <span className={cn('rounded px-2 py-0.5 text-xs font-bold', statusColor)}>{drift.status}</span>
        <span className="font-medium text-sm">{drift.metric_name}</span>
        <span className="ml-auto text-xs font-mono text-muted-foreground">
          KS={drift.ks_statistic.toFixed(4)}
        </span>
      </div>
      {drift.possible_cause && (
        <p className="mt-2 text-xs text-muted-foreground">{drift.possible_cause}</p>
      )}
      {drift.recommendation && (
        <p className="mt-1 text-xs text-blue-400">{drift.recommendation}</p>
      )}
    </div>
  );
}

function EmptyState({ message, icon: Icon }: { message: string; icon: React.ElementType }) {
  return (
    <div className="flex h-64 flex-col items-center justify-center gap-3 text-muted-foreground">
      <Icon className="h-10 w-10 opacity-30" />
      <p className="text-sm">{message}</p>
    </div>
  );
}

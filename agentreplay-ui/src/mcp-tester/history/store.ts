/**
 * Task 6 — Request History & Session Persistence
 *
 * Indexed request history with full replay capability.
 * Inverted index for O(1) category filtering.
 * Binary search for O(log n) time-range queries.
 * Table virtualization: O(k) viewport rows, O(1) scroll.
 */

// ─── History Entry Types ───────────────────────────────────────────────────────

export interface HistoryEntry {
  id: string;
  timestamp: number;
  method: string;
  category: string;
  request: unknown;
  response: unknown;
  error?: string;
  httpStatus?: number;
  durationMs: number;
  responseSize: number;
  transport: string;
  sessionId: string;
}

export interface HistoryFilter {
  method?: string;
  category?: string;
  status?: 'success' | 'error' | 'all';
  startTime?: number;
  endTime?: number;
  search?: string;
}

export interface HistorySession {
  id: string;
  name: string;
  createdAt: number;
  entryCount: number;
}

// ─── History Store ─────────────────────────────────────────────────────────────

const STORAGE_KEY = 'mcp_tester_history';
const SESSION_KEY = 'mcp_tester_sessions';
const MAX_ENTRIES = 10000;

let _entries: HistoryEntry[] = [];
let _currentSessionId: string = generateSessionId();
let _categoryIndex: Map<string, Set<number>> = new Map();
let _methodIndex: Map<string, Set<number>> = new Map();

function generateSessionId(): string {
  return `session_${Date.now()}_${Math.random().toString(36).slice(2, 8)}`;
}

function generateEntryId(): string {
  return `entry_${Date.now()}_${Math.random().toString(36).slice(2, 10)}`;
}

// ─── Persistence ───────────────────────────────────────────────────────────────

export function loadHistory(): HistoryEntry[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw) {
      _entries = JSON.parse(raw);
      rebuildIndices();
    }
  } catch {
    _entries = [];
  }
  return _entries;
}

export function saveHistory(): void {
  try {
    // Keep only the last MAX_ENTRIES
    if (_entries.length > MAX_ENTRIES) {
      _entries = _entries.slice(-MAX_ENTRIES);
      rebuildIndices();
    }
    localStorage.setItem(STORAGE_KEY, JSON.stringify(_entries));
  } catch {
    // Storage full — drop oldest half
    _entries = _entries.slice(Math.floor(_entries.length / 2));
    rebuildIndices();
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(_entries));
    } catch {
      // Give up
    }
  }
}

function rebuildIndices(): void {
  _categoryIndex.clear();
  _methodIndex.clear();
  _entries.forEach((entry, idx) => {
    // Category index
    if (!_categoryIndex.has(entry.category)) {
      _categoryIndex.set(entry.category, new Set());
    }
    _categoryIndex.get(entry.category)!.add(idx);

    // Method index
    if (!_methodIndex.has(entry.method)) {
      _methodIndex.set(entry.method, new Set());
    }
    _methodIndex.get(entry.method)!.add(idx);
  });
}

// ─── CRUD Operations ───────────────────────────────────────────────────────────

export function addEntry(entry: Omit<HistoryEntry, 'id' | 'sessionId'>): HistoryEntry {
  const fullEntry: HistoryEntry = {
    ...entry,
    id: generateEntryId(),
    sessionId: _currentSessionId,
  };
  _entries.push(fullEntry);

  // Update indices
  const idx = _entries.length - 1;
  if (!_categoryIndex.has(fullEntry.category)) {
    _categoryIndex.set(fullEntry.category, new Set());
  }
  _categoryIndex.get(fullEntry.category)!.add(idx);

  if (!_methodIndex.has(fullEntry.method)) {
    _methodIndex.set(fullEntry.method, new Set());
  }
  _methodIndex.get(fullEntry.method)!.add(idx);

  // Auto-save
  saveHistory();

  return fullEntry;
}

export function getEntries(): HistoryEntry[] {
  return _entries;
}

export function getEntry(id: string): HistoryEntry | undefined {
  return _entries.find((e) => e.id === id);
}

export function clearHistory(): void {
  _entries = [];
  _categoryIndex.clear();
  _methodIndex.clear();
  saveHistory();
}

// ─── Filtering ─────────────────────────────────────────────────────────────────

/**
 * Filter history entries.
 * Category filtering: O(1) via inverted index.
 * Time-range: O(log n) via binary search on sorted timestamps.
 */
export function filterEntries(filter: HistoryFilter): HistoryEntry[] {
  let indices: Set<number> | null = null;

  // Category filter — O(1)
  if (filter.category) {
    indices = _categoryIndex.get(filter.category) || new Set();
  }

  // Method filter — O(1)
  if (filter.method) {
    const methodIndices = _methodIndex.get(filter.method) || new Set();
    if (indices) {
      indices = new Set([...indices].filter((i) => methodIndices.has(i)));
    } else {
      indices = methodIndices;
    }
  }

  // Start from filtered indices or all
  let results: HistoryEntry[];
  if (indices) {
    results = [...indices].sort((a, b) => a - b).map((i) => _entries[i]);
  } else {
    results = [..._entries];
  }

  // Time-range filter — O(log n) for start/end via binary search
  if (filter.startTime !== undefined) {
    const startIdx = binarySearchLower(results, filter.startTime);
    results = results.slice(startIdx);
  }
  if (filter.endTime !== undefined) {
    const endIdx = binarySearchUpper(results, filter.endTime);
    results = results.slice(0, endIdx + 1);
  }

  // Status filter
  if (filter.status && filter.status !== 'all') {
    results = results.filter((e) => {
      if (filter.status === 'error') return !!e.error;
      return !e.error;
    });
  }

  // Text search
  if (filter.search) {
    const query = filter.search.toLowerCase();
    results = results.filter(
      (e) =>
        e.method.toLowerCase().includes(query) ||
        JSON.stringify(e.request).toLowerCase().includes(query) ||
        JSON.stringify(e.response).toLowerCase().includes(query)
    );
  }

  return results;
}

// ─── Binary Search Helpers ─────────────────────────────────────────────────────

function binarySearchLower(entries: HistoryEntry[], timestamp: number): number {
  let lo = 0;
  let hi = entries.length;
  while (lo < hi) {
    const mid = (lo + hi) >>> 1;
    if (entries[mid].timestamp < timestamp) {
      lo = mid + 1;
    } else {
      hi = mid;
    }
  }
  return lo;
}

function binarySearchUpper(entries: HistoryEntry[], timestamp: number): number {
  let lo = 0;
  let hi = entries.length;
  while (lo < hi) {
    const mid = (lo + hi) >>> 1;
    if (entries[mid].timestamp <= timestamp) {
      lo = mid + 1;
    } else {
      hi = mid;
    }
  }
  return lo - 1;
}

// ─── Session Management ────────────────────────────────────────────────────────

export function startNewSession(name?: string): string {
  _currentSessionId = generateSessionId();
  const sessions = getSessions();
  sessions.push({
    id: _currentSessionId,
    name: name || `Session ${sessions.length + 1}`,
    createdAt: Date.now(),
    entryCount: 0,
  });
  localStorage.setItem(SESSION_KEY, JSON.stringify(sessions));
  return _currentSessionId;
}

export function getCurrentSessionId(): string {
  return _currentSessionId;
}

export function getSessions(): HistorySession[] {
  try {
    const raw = localStorage.getItem(SESSION_KEY);
    return raw ? JSON.parse(raw) : [];
  } catch {
    return [];
  }
}

export function getSessionEntries(sessionId: string): HistoryEntry[] {
  return _entries.filter((e) => e.sessionId === sessionId);
}

// ─── Export ────────────────────────────────────────────────────────────────────

export interface HistoryExport {
  version: '1.0';
  exportedAt: string;
  sessionId: string;
  entries: HistoryEntry[];
}

export function exportSession(sessionId?: string): string {
  const entries = sessionId ? getSessionEntries(sessionId) : _entries;
  const data: HistoryExport = {
    version: '1.0',
    exportedAt: new Date().toISOString(),
    sessionId: sessionId || _currentSessionId,
    entries,
  };
  return JSON.stringify(data, null, 2);
}

export function importSession(json: string): number {
  try {
    const data: HistoryExport = JSON.parse(json);
    if (data.version !== '1.0') throw new Error('Unsupported export version');
    data.entries.forEach((entry) => {
      _entries.push(entry);
    });
    rebuildIndices();
    saveHistory();
    return data.entries.length;
  } catch {
    throw new Error('Failed to import session data');
  }
}

// ─── Virtualization Helper ─────────────────────────────────────────────────────

export interface VirtualWindow {
  startIndex: number;
  endIndex: number;
  visibleItems: HistoryEntry[];
  totalItems: number;
  paddingTop: number;
  paddingBottom: number;
}

/**
 * Compute a virtual window for table rendering.
 * Only O(k) DOM nodes where k = viewport height / row height.
 * Scroll performance is O(1) regardless of total n.
 */
export function computeVirtualWindow(
  entries: HistoryEntry[],
  scrollTop: number,
  viewportHeight: number,
  rowHeight: number = 40
): VirtualWindow {
  const totalItems = entries.length;
  const visibleCount = Math.ceil(viewportHeight / rowHeight);
  const overscan = 5; // Buffer rows above/below

  const startIndex = Math.max(0, Math.floor(scrollTop / rowHeight) - overscan);
  const endIndex = Math.min(totalItems - 1, startIndex + visibleCount + 2 * overscan);

  return {
    startIndex,
    endIndex,
    visibleItems: entries.slice(startIndex, endIndex + 1),
    totalItems,
    paddingTop: startIndex * rowHeight,
    paddingBottom: Math.max(0, (totalItems - endIndex - 1) * rowHeight),
  };
}

/**
 * serchX Spotlight Launcher Controller
 * Zero-build native ESM frontend coordinating progressive two-tier search,
 * keyboard navigation, dynamic window height resizing, and native action helpers.
 */

window.addEventListener('contextmenu', (e) => e.preventDefault(), true);

// --- Markdown renderer (uses marked.js loaded via CDN script tag) ---
function renderMarkdown(text) {
  if (!text) return '';
  if (typeof window.marked === 'undefined') {
    // Fallback: return escaped plain text if marked hasn't loaded yet
    const div = document.createElement('div');
    div.textContent = text;
    return div.innerHTML;
  }
  return window.marked.parse(text, { breaks: true, gfm: true });
}

// IPC bridge with standalone browser fallback
const mockState = {
  folders: [],
  pinnedVersions: new Set(),
  crawlActive: false,
};

export const tauri = window.__TAURI__?.core || {
  invoke: async (cmd, args = {}) => {
    console.info(`[Browser Mock IPC] ${cmd}:`, args);

    if (cmd === 'search_exact') {
      const q = (args.query || '').toLowerCase();
      if (!q || q.includes('empty')) return [];
      return [
        {
          chunk_id: 'c1',
          artifact_id: 'art-1',
          version_id: 'v3',
          path: '/Users/hwaiting_dk/Documents/Contracts/MSA-Vendor-2026.pdf',
          score: 0.99,
          snippet: `Master Services Agreement covering terms for "${args.query}", payment schedule, and liability caps under section 4.2.`,
        },
        {
          chunk_id: 'c2',
          artifact_id: 'art-2',
          version_id: 'v2',
          path: '/Users/hwaiting_dk/Repos/serchX/src-tauri/src/search/hybrid.rs',
          score: 0.94,
          snippet: `pub fn hybrid_search(query: &str) -> Vec<GroupedResult> /* matched "${args.query}" in FTS5 index */`,
        }
      ];
    }

    if (cmd === 'search_grouped') {
      const q = (args.query || '').toLowerCase();
      if (!q || q.includes('empty')) return [];
      return [
        {
          primary_result: {
            chunk_id: 'c1',
            artifact_id: 'art-1',
            version_id: 'v3',
            version_number: 3,
            path: '/Users/hwaiting_dk/Documents/Contracts/MSA-Vendor-2026.pdf',
            score: 0.99,
            snippet: `Master Services Agreement covering terms for "${args.query}", payment schedule, and liability caps under section 4.2.`,
            reasons: [{ ExactKeyword: { terms: [args.query] } }, { SemanticMatch: { score: 0.95 } }],
            modified_at: Math.floor(Date.now() / 1000) - 1800,
            is_pinned: mockState.pinnedVersions.has('v3'),
            total_version_count: 3,
          },
          other_versions: [
            { version_id: 'v2', version_number: 2, modified_at: Math.floor(Date.now() / 1000) - 86400 * 2, is_pinned: true },
            { version_id: 'v1', version_number: 1, modified_at: Math.floor(Date.now() / 1000) - 86400 * 14, is_pinned: false },
          ],
          total_version_count: 3,
        },
        {
          primary_result: {
            chunk_id: 'c2',
            artifact_id: 'art-2',
            version_id: 'v2',
            version_number: 2,
            path: '/Users/hwaiting_dk/Repos/serchX/src-tauri/src/search/hybrid.rs',
            score: 0.92,
            snippet: 'Two-tier retrieval pipeline executing SQLite FTS5 exact search then FastEmbed vector reranking.',
            reasons: [{ SemanticMatch: { score: 0.91 } }],
            modified_at: Math.floor(Date.now() / 1000) - 7200,
            is_pinned: mockState.pinnedVersions.has('v2'),
            total_version_count: 2,
          },
          other_versions: [
            { version_id: 'v1_code', version_number: 1, modified_at: Math.floor(Date.now() / 1000) - 86400 * 5, is_pinned: false },
          ],
          total_version_count: 2,
        },
        {
          primary_result: {
            chunk_id: 'c3',
            artifact_id: 'art-3',
            version_id: 'v1_fin',
            version_number: 1,
            path: '/Users/hwaiting_dk/Downloads/Q3_Financial_Forecast.xlsx',
            score: 0.85,
            snippet: 'Revenue forecast and enterprise budget allocations for next fiscal year.',
            reasons: [{ FuzzyKeyword: { distance: 1 } }],
            modified_at: Math.floor(Date.now() / 1000) - 86400 * 4,
            is_pinned: false,
            total_version_count: 1,
          },
          other_versions: [],
          total_version_count: 1,
        }
      ];
    }

    if (cmd === 'get_version_diff') {
      return {
        summary: {
          added_lines: 4,
          removed_lines: 2,
          unchanged_lines: 14,
        },
        hunks: [
          { tag: 'equal', text: '// Section 1: Parties and Purpose' },
          { tag: 'equal', text: '// This agreement defines the legal terms for software services.' },
          { tag: 'removed', text: '- Payment terms: Net 60 days upon invoice receipt.' },
          { tag: 'added', text: '+ Payment terms: Net 30 days upon electronic invoice receipt.' },
          { tag: 'equal', text: '// Section 2: Intellectual Property & Liability' },
          { tag: 'removed', text: '- Liability ceiling limited to $50,000.' },
          { tag: 'added', text: '+ Liability ceiling limited to aggregate trailing 12-month fees.' },
          { tag: 'added', text: '+ Confidentiality covenants extended to 3 years post-termination.' },
          { tag: 'added', text: '+ Mutual indemnification clause included per legal review.' },
          { tag: 'equal', text: '1. Standard definitions and governing law (Delaware).' },
          { tag: 'equal', text: '2. Dispute resolution through AAA arbitration.' },
          { tag: 'equal', text: '3. Severability and entire agreement stipulations.' },
          { tag: 'equal', text: '4. Non-solicitation of key engineering personnel.' },
          { tag: 'equal', text: '5. Force majeure provisions including natural disasters.' },
          { tag: 'equal', text: '6. Counterparts and digital signature validity.' },
          { tag: 'equal', text: '7. Notice delivery via designated secure email addresses.' },
          { tag: 'equal', text: '8. Independent contractor relationship verification.' },
          { tag: 'equal', text: '9. Audit rights once annually during standard business hours.' },
          { tag: 'equal', text: '10. Insurance coverage maintenance requirements.' },
          { tag: 'equal', text: '11. Term renewal notifications 60 days prior to expiry.' },
          { tag: 'equal', text: '12. Signatures and execution date.' },
        ],
      };
    }

    if (cmd === 'get_watched_folders') {
      return [...mockState.folders];
    }

    if (cmd === 'get_default_folders') {
      return [
        { path: '/Users/hwaiting_dk/Documents', display_name: 'Documents', enabled_by_default: true, is_opt_in: false },
        { path: '/Users/hwaiting_dk/Desktop', display_name: 'Desktop', enabled_by_default: true, is_opt_in: false },
        { path: '/Users/hwaiting_dk/Downloads', display_name: 'Downloads', enabled_by_default: false, is_opt_in: true },
      ];
    }

    if (cmd === 'check_folder_permissions') {
      return { status: 'Granted', has_access: true };
    }

    if (cmd === 'open_privacy_settings') {
      console.info('[Browser Mock IPC] Opened macOS Privacy Settings');
      return true;
    }

    if (cmd === 'get_crawl_progress') {
      return {
        active: mockState.crawlActive,
        indexed_files: 1630,
        total_files: 1630,
        pending_embeddings: 0,
        current_file: '/Users/hwaiting_dk/Documents/Contracts/MSA-Vendor-2026.pdf',
        bytes_indexed: 42189012,
        throughput_mb_s: 3.8,
      };
    }

    if (cmd === 'toggle_watched_folder') {
      const folder = mockState.folders.find(f => f.id === args.id);
      if (folder) folder.enabled = args.enabled;
      return true;
    }

    if (cmd === 'add_watched_folder') {
      const newFolder = {
        id: Date.now(),
        path: args.path,
        enabled: true,
        recursive: !!args.recursive,
        file_count: 42,
      };
      mockState.folders.push(newFolder);
      return newFolder;
    }

    if (cmd === 'remove_watched_folder') {
      mockState.folders = mockState.folders.filter(f => f.id !== args.id);
      return true;
    }

    if (cmd === 'toggle_version_pin') {
      const vId = args.versionId || args.version_id;
      if (mockState.pinnedVersions.has(vId)) {
        mockState.pinnedVersions.delete(vId);
        return false;
      } else {
        mockState.pinnedVersions.add(vId);
        return true;
      }
    }

    if (cmd === 'start_crawl') {
      mockState.crawlActive = true;
      return true;
    }

    if (cmd === 'pause_crawl') {
      mockState.crawlActive = false;
      return true;
    }

    if (cmd === 'resume_crawl') {
      mockState.crawlActive = true;
      return true;
    }

    if (cmd === 'get_settings') {
      return mockState.appSettings || {
        preset: 'Balanced',
        semantic_search_enabled: true,
        auto_pause_on_battery: false,
        max_file_size_mb: 50,
        max_versions_per_artifact: 3,
        max_indexed_files: 20000,
        max_vector_chunks: 250000,
        thermal_throttle_delay_ms: 200,
        embedding_model: 'AllMiniLML6V2',
        embedding_dimension: 384,
        fts_weight: 1.0,
        vector_weight: 1.0,
        rrf_k: 60,
      };
    }

    if (cmd === 'update_settings') {
      mockState.appSettings = { ...(mockState.appSettings || {}), ...(args?.settings || {}) };
      return true;
    }

    if (cmd === 'reset_settings') {
      mockState.appSettings = {
        preset: 'Balanced',
        semantic_search_enabled: true,
        auto_pause_on_battery: false,
        max_file_size_mb: 50,
        max_versions_per_artifact: 3,
        max_indexed_files: 20000,
        max_vector_chunks: 250000,
        thermal_throttle_delay_ms: 200,
        embedding_model: 'AllMiniLML6V2',
        embedding_dimension: 384,
        fts_weight: 1.0,
        vector_weight: 1.0,
        rrf_k: 60,
      };
      return mockState.appSettings;
    }

    if (cmd === 'set_dock_visible') {
      return true;
    }

    if (cmd === 'pick_folder') {
      return '/Users/hwaiting_dk/Desktop/Legal_Agreements';
    }

    if (cmd === 'show_window' || cmd === 'set_auto_hide' || cmd === 'start_dragging' || cmd === 'center_window') {
      return true;
    }

    return null;
  }
};

// Global App State
export const state = {
  query: '',
  results: [], // array of result objects
  selectedIndex: -1,
  currentQuerySeq: 0,
  activeView: 'search', // 'search' | 'diff' | 'settings' | 'onboarding'
  activeDiffItem: null,
  isDebouncing: null,
  isCrawlActive: false,
  isSynthesizing: false,
  synthesisSources: [],
  synthesisText: '',
  showThinkingTokens: true,
};

// DOM Element References
const elements = {
  app: document.getElementById('app'),
  searchHeader: document.getElementById('search-header'),
  searchIconWrapper: document.querySelector('.search-icon-wrapper'),
  searchInput: document.getElementById('search-input'),
  searchSpinner: document.getElementById('search-spinner'),
  clearBtn: document.getElementById('clear-btn'),
  settingsToggleBtn: document.getElementById('settings-toggle-btn'),
  searchView: document.getElementById('search-view'),
  resultsList: document.getElementById('results-list'),
  emptyState: document.getElementById('empty-state'),
  appFooter: document.getElementById('app-footer'),
  footerStatusText: document.getElementById('footer-status-text'),
  previewColumn: document.getElementById('preview-column'),
  previewPinBtn: document.getElementById('preview-pin-btn'),
  previewOpenBtn: document.getElementById('preview-open-btn'),
  previewRevealBtn: document.getElementById('preview-reveal-btn'),
  previewCompareBtn: document.getElementById('preview-compare-btn'),
  previewSummarizeBtn: document.getElementById('preview-summarize-btn'),
  synthesisCard: document.getElementById('synthesis-card'),
  synthesisStatusBadge: document.getElementById('synthesis-status-badge'),
  synthesisModelInfo: document.getElementById('synthesis-model-info'),
  infoTooltipProvider: document.getElementById('info-tooltip-provider'),
  infoTooltipModel: document.getElementById('info-tooltip-model'),
  infoTooltipRamRow: document.getElementById('info-tooltip-ram-row'),
  infoTooltipRam: document.getElementById('info-tooltip-ram'),
  infoTooltipEndpointRow: document.getElementById('info-tooltip-endpoint-row'),
  infoTooltipEndpoint: document.getElementById('info-tooltip-endpoint'),
  synthesisCopyBtn: document.getElementById('synthesis-copy-btn'),
  synthesisCloseBtn: document.getElementById('synthesis-close-btn'),
  synthesisSourcesRow: document.getElementById('synthesis-sources-row'),
  synthesisContent: document.getElementById('synthesis-content'),
  aiGlowStack: document.getElementById('ai-glow-stack'),
};

export function formatBytes(bytes) {
  if (!bytes || bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i];
}

// --- Apple Intelligence Glow & Traveling Highlight Engine ---
let travelAnimFrame = null;
let travelAngle = 0;

function tickTravelGlow() {
  if (!state.isSynthesizing) return;
  travelAngle = (travelAngle + 2) % 360;
  document.documentElement.style.setProperty('--ai-travel-angle', `${travelAngle}deg`);
  travelAnimFrame = requestAnimationFrame(tickTravelGlow);
}

export function setGeneratingState(isGenerating) {
  state.isSynthesizing = isGenerating;
  if (isGenerating) {
    if (elements.app) elements.app.classList.add('is-generating');
    if (elements.searchHeader) elements.searchHeader.classList.add('is-generating');
    if (elements.synthesisCard) elements.synthesisCard.classList.add('is-generating');
    if (elements.synthesisStatusBadge) {
      elements.synthesisStatusBadge.classList.add('generating');
      elements.synthesisStatusBadge.classList.remove('ready');
    }
    if (!travelAnimFrame) {
      travelAnimFrame = requestAnimationFrame(tickTravelGlow);
    }
  } else {
    if (elements.app) elements.app.classList.remove('is-generating');
    if (elements.searchHeader) elements.searchHeader.classList.remove('is-generating');
    if (elements.synthesisCard) elements.synthesisCard.classList.remove('is-generating');
    if (elements.synthesisStatusBadge) {
      elements.synthesisStatusBadge.classList.remove('generating');
    }
    if (travelAnimFrame) {
      cancelAnimationFrame(travelAnimFrame);
      travelAnimFrame = null;
    }
  }
}

// --- Window Height Management ---
export function updateWindowHeight() {
  if (!state.query.trim()) {
    elements.app.classList.remove('has-results', 'has-empty');
    const height = 64;
    elements.app.style.height = `${height}px`;
    tauri.invoke('resize_window', { width: 680, height }).catch(() => {});
  } else if (elements.emptyState && elements.emptyState.classList.contains('active')) {
    elements.app.classList.remove('has-results');
    elements.app.classList.add('has-empty');
    const height = 240;
    elements.app.style.height = `${height}px`;
    tauri.invoke('resize_window', { width: 680, height }).catch(() => {});
  } else {
    elements.app.classList.add('has-results');
    elements.app.classList.remove('has-empty');
    const height = 520;
    elements.app.style.height = `${height}px`;
    tauri.invoke('resize_window', { width: 820, height }).catch(() => {});
  }
}

// --- Status Indicator Helper ---
export function setStatusText(text, isTemporary = false) {
  const el = elements.footerStatusText || document.getElementById('footer-status-text');
  if (el) {
    el.textContent = text;
    if (isTemporary) {
      setTimeout(() => {
        el.textContent = state.results.length
          ? `${state.results.length} file${state.results.length === 1 ? '' : 's'} found`
          : 'Ready';
      }, 2200);
    }
  }
}

// --- Relative Time Helper ---
export function formatRelativeTime(ts) {
  if (!ts) return '';
  // Support both millisecond (> 10^10) and second timestamps
  const tsSeconds = ts > 10000000000 ? Math.floor(ts / 1000) : ts;
  const now = Math.floor(Date.now() / 1000);
  const diff = now - tsSeconds;
  if (diff < 60) return 'just now';
  if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
  if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`;
  if (diff < 604800) return `${Math.floor(diff / 86400)}d ago`;
  const date = new Date(tsSeconds * 1000);
  return date.toLocaleDateString(undefined, { month: 'short', day: 'numeric' });
}

// --- Path Truncation Helper ---
export function truncatePath(fullPath, maxLen = 36) {
  if (!fullPath || fullPath.length <= maxLen) return fullPath;
  const parts = fullPath.split('/');
  if (parts.length <= 2) return fullPath;
  const fileName = parts.pop();
  const root = parts.slice(0, 2).join('/');
  return `${root}/.../${fileName}`;
}

// --- File Type Icon SVG Provider ---
export function getFileIconSvg(filePath) {
  const ext = (filePath.split('.').pop() || '').toLowerCase();
  if (['rs', 'js', 'ts', 'jsx', 'tsx', 'py', 'json', 'html', 'css', 'go', 'c', 'cpp'].includes(ext)) {
    return `<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="16 18 22 12 16 6"></polyline><polyline points="8 6 2 12 8 18"></polyline></svg>`;
  }
  if (['pdf'].includes(ext)) {
    return `<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="#ef4444" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"></path><polyline points="14 2 14 8 20 8"></polyline><line x1="16" y1="13" x2="8" y2="13"></line><line x1="16" y1="17" x2="8" y2="17"></line></svg>`;
  }
  if (['png', 'jpg', 'jpeg', 'gif', 'svg', 'webp'].includes(ext)) {
    return `<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="#10b981" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="3" width="18" height="18" rx="2" ry="2"></rect><circle cx="8.5" cy="8.5" r="1.5"></circle><polyline points="21 15 16 10 5 21"></polyline></svg>`;
  }
  return `<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"></path><polyline points="14 2 14 8 20 8"></polyline></svg>`;
}

// --- Safe Query Term Highlighting ---
function highlightSnippet(snippet, query) {
  if (!snippet) return '';
  const div = document.createElement('div');
  div.textContent = snippet;
  let text = div.innerHTML;

  const terms = query
    .trim()
    .split(/\s+/)
    .filter(t => t.length > 1 && !['before:', 'after:', 'from:', 'type:', 'in:'].some(p => t.startsWith(p)));

  terms.forEach(term => {
    try {
      const escaped = term.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
      const regex = new RegExp(`(${escaped})`, 'gi');
      text = text.replace(regex, '<mark>$1</mark>');
    } catch (_) {}
  });

  return text;
}

// --- Render Evidence Text ---
function renderEvidenceText(reasons, query) {
  if (!reasons || !reasons.length) return '';

  const parts = [];
  reasons.forEach(reason => {
    if (reason.ExactKeyword) {
      parts.push(`Contains <mark>${reason.ExactKeyword.terms.slice(0, 2).join(', ')}</mark>`);
    } else if (reason.SemanticSimilarity || reason.SemanticMatch) {
      parts.push(`Similar meaning to <mark>${query}</mark>`);
    } else if (reason.TimeRangeMatch) {
      parts.push(`Time: ${reason.TimeRangeMatch.label}`);
    } else if (reason.VersionRelationship) {
      const v = reason.VersionRelationship;
      parts.push(`v${v.version_number} • ${v.relation_summary}`);
    } else if (reason.PathMatch) {
      parts.push(`In folder ${reason.PathMatch.folder}`);
    } else if (reason.TypeMatch) {
      parts.push(`Type ${reason.TypeMatch.extension}`);
    } else if (reason.FuzzyKeyword) {
      parts.push(`Approximate match`);
    }
  });
  return parts.join(' · ');
}

// --- Render Result List ---
export function renderResults(items, currentQuery) {
  elements.resultsList.innerHTML = '';
  state.results = items;

  if (items.length === 0) {
    if (elements.emptyState) elements.emptyState.classList.add('active');
    elements.app.classList.remove('has-results');
    elements.app.classList.add('has-empty');
    state.selectedIndex = -1;
    setStatusText('0 files found');
    updateWindowHeight();
    return;
  }

  if (elements.emptyState) elements.emptyState.classList.remove('active');
  elements.app.classList.remove('has-empty');
  elements.app.classList.add('has-results');
  setStatusText(`${items.length} file${items.length === 1 ? '' : 's'} found`);

  // Reset or preserve selection index
  if (state.selectedIndex < 0 || state.selectedIndex >= items.length) {
    state.selectedIndex = 0;
  }

  items.forEach((item, index) => {
    const card = document.createElement('div');
    card.className = `result-card${index === state.selectedIndex ? ' selected' : ''}`;
    card.setAttribute('role', 'option');
    card.setAttribute('aria-selected', index === state.selectedIndex ? 'true' : 'false');
    card.dataset.index = index;

    // 1. Icon (32x32) as direct left child
    const iconSpan = document.createElement('div');
    iconSpan.className = 'card-file-icon';
    iconSpan.innerHTML = getFileIconSvg(item.path);
    card.appendChild(iconSpan);

    // 2. Metadata column (middle)
    const fileMeta = document.createElement('div');
    fileMeta.className = 'card-file-meta';

    // Top row: File name + optional version badge + relative time
    const topRow = document.createElement('div');
    topRow.className = 'card-title-row';

    const nameSpan = document.createElement('span');
    nameSpan.className = 'card-file-name';
    const filename = item.path.split('/').pop() || item.path;
    nameSpan.textContent = filename;
    nameSpan.title = item.path;
    topRow.appendChild(nameSpan);

    // Version badge if multi-version
    const versionCount = item.total_version_count || (item.other_versions ? item.other_versions.length + 1 : 1);
    if (versionCount > 1) {
      const vBadge = document.createElement('span');
      vBadge.className = `version-badge${item.is_pinned ? ' pinned' : ''}`;
      vBadge.textContent = `v${item.version_number || versionCount} of ${versionCount}`;
      vBadge.title = 'Press ⌘D to inspect versions';
      topRow.appendChild(vBadge);
    }

    const timeSpan = document.createElement('span');
    timeSpan.className = 'card-time';
    timeSpan.textContent = formatRelativeTime(item.modified_at);
    topRow.appendChild(timeSpan);

    fileMeta.appendChild(topRow);

    // Bottom row: Evidence line / Snippet / Path
    const snippetDiv = document.createElement('div');
    snippetDiv.className = 'card-snippet';
    if (item.reasons && item.reasons.length > 0) {
      snippetDiv.innerHTML = renderEvidenceText(item.reasons, currentQuery);
    } else if (item.snippet) {
      snippetDiv.innerHTML = highlightSnippet(item.snippet, currentQuery);
    } else {
      snippetDiv.textContent = truncatePath(item.path, 40);
    }
    fileMeta.appendChild(snippetDiv);

    card.appendChild(fileMeta);

    // Click handlers: click to select, double click to open
    card.addEventListener('click', () => {
      selectResultIndex(index);
    });
    card.addEventListener('dblclick', () => {
      selectResultIndex(index);
      openSelectedFile();
    });

    elements.resultsList.appendChild(card);
  });

  selectResultIndex(state.selectedIndex);
  updateWindowHeight();
}

// --- Selection Management ---
export function selectResultIndex(index) {
  if (state.results.length === 0) return;
  state.selectedIndex = Math.max(0, Math.min(index, state.results.length - 1));

  const cards = elements.resultsList.querySelectorAll('.result-card');
  cards.forEach((c, idx) => {
    if (idx === state.selectedIndex) {
      c.classList.add('selected');
      c.setAttribute('aria-selected', 'true');
      c.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
    } else {
      c.classList.remove('selected');
      c.setAttribute('aria-selected', 'false');
    }
  });
  updatePreviewPane(state.results[state.selectedIndex]);
}

function updatePreviewPane(item) {
  if (!item) return;
  const iconEl = document.getElementById('preview-icon');
  if (iconEl) iconEl.innerHTML = getFileIconSvg(item.path);
  
  const titleEl = document.getElementById('preview-title');
  if (titleEl) {
    const filename = item.path.split('/').pop() || item.path;
    titleEl.textContent = filename;
    titleEl.title = filename;
  }
  
  const pathEl = document.getElementById('preview-path');
  if (pathEl) {
    pathEl.textContent = truncatePath(item.path, 50);
    pathEl.title = item.path;
  }
  
  const evidenceEl = document.getElementById('preview-evidence');
  if (evidenceEl) {
    evidenceEl.innerHTML = '';
    const reasons = item.reasons || [];
    if (reasons.length > 0) {
      reasons.forEach(r => {
        const row = document.createElement('div');
        row.className = 'preview-evidence-row';
        row.innerHTML = `<svg class="preview-evidence-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"></circle><path d="M12 16v-4"></path><path d="M12 8h.01"></path></svg><div class="preview-evidence-text">${renderEvidenceText([r], state.query)}</div>`;
        evidenceEl.appendChild(row);
      });
    } else if (item.snippet) {
      const row = document.createElement('div');
      row.className = 'preview-evidence-row';
      row.innerHTML = `<svg class="preview-evidence-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"></circle><path d="M12 16v-4"></path><path d="M12 8h.01"></path></svg><div class="preview-evidence-text">${highlightSnippet(item.snippet, state.query)}</div>`;
      evidenceEl.appendChild(row);
    } else {
      evidenceEl.innerHTML = `<div class="preview-evidence-row"><svg class="preview-evidence-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"></circle><path d="M12 16v-4"></path><path d="M12 8h.01"></path></svg><div class="preview-evidence-text">Matches search query</div></div>`;
    }
  }

  const infoEl = document.getElementById('preview-info');
  if (infoEl) {
    const modMs = item.modified_at ? (item.modified_at > 10000000000 ? item.modified_at : item.modified_at * 1000) : Date.now();
    infoEl.innerHTML = `
      <div class="preview-info-row"><span class="preview-info-key">Modified</span><span class="preview-info-val">${new Date(modMs).toLocaleString()}</span></div>
      <div class="preview-info-row"><span class="preview-info-key">Kind</span><span class="preview-info-val">${(item.path.split('.').pop() || 'File').toUpperCase()}</span></div>
      <div class="preview-info-row"><span class="preview-info-key">Versions</span><span class="preview-info-val">${item.total_version_count || 1} available</span></div>
    `;
  }

  const pinBtn = document.getElementById('preview-pin-btn');
  if (pinBtn) {
    if (item.is_pinned) {
      pinBtn.classList.add('pinned');
      pinBtn.title = 'Unpin Version (⌘P)';
    } else {
      pinBtn.classList.remove('pinned');
      pinBtn.title = 'Pin Version (⌘P)';
    }
  }
}

// --- Native Action Dispatchers ---
export async function openSelectedFile() {
  const item = state.results[state.selectedIndex];
  if (!item || !item.path) return;
  try {
    await tauri.invoke('open_file', { path: item.path });
    await tauri.invoke('hide_window');
  } catch (err) {
    setStatusText(`Error opening file: ${err}`, true);
  }
}

export async function revealSelectedInFinder() {
  const item = state.results[state.selectedIndex];
  if (!item || !item.path) return;
  try {
    await tauri.invoke('reveal_in_finder', { path: item.path });
    setStatusText('Revealed in Finder', true);
  } catch (err) {
    setStatusText(`Error: ${err}`, true);
  }
}

export async function copySelectedPath() {
  const item = state.results[state.selectedIndex];
  if (!item || !item.path) return;
  try {
    await navigator.clipboard.writeText(item.path);
    setStatusText('Copied path to clipboard', true);
  } catch (err) {
    setStatusText('Failed to copy path', true);
  }
}

export async function toggleSelectedPin() {
  const item = state.results[state.selectedIndex];
  if (!item || !item.version_id) return;
  try {
    const isPinned = await tauri.invoke('toggle_version_pin', { versionId: item.version_id });
    item.is_pinned = isPinned;
    renderResults(state.results, state.query);
    setStatusText(isPinned ? 'Version pinned' : 'Version unpinned', true);
  } catch (err) {
    setStatusText(`Pin error: ${err}`, true);
  }
}

// --- Search Dispatcher (Progressive Two-Tier Retrieval) ---
export async function dispatchSearch(queryText) {
  const trimmed = queryText.trim();
  state.query = trimmed;

  if (!trimmed) {
    elements.searchSpinner.classList.remove('active');
    elements.clearBtn.style.display = 'none';
    if (elements.emptyState) elements.emptyState.classList.remove('active');
    elements.resultsList.innerHTML = '';
    elements.app.classList.remove('has-results', 'has-empty');
    state.results = [];
    state.selectedIndex = -1;
    setStatusText('Ready');
    updateWindowHeight();
    return;
  }

  elements.clearBtn.style.display = 'flex';
  const querySeq = ++state.currentQuerySeq;
  elements.searchSpinner.classList.add('active');

  // Tier 1: Fast exact search paint (<50ms)
  try {
    const exactResults = await tauri.invoke('search_exact', {
      query: trimmed,
      limit: 10
    });

    if (querySeq === state.currentQuerySeq && exactResults && exactResults.length > 0) {
      // Map SearchResult to GroupedSearchResult shape for immediate display
      const mapped = exactResults.map(r => ({
        chunk_id: r.chunk_id,
        artifact_id: r.artifact_id,
        version_id: r.version_id,
        path: r.path,
        score: r.score,
        snippet: r.snippet,
        reasons: [{ ExactKeyword: { terms: [trimmed] } }],
        modified_at: Math.floor(Date.now() / 1000),
        is_pinned: false,
        total_version_count: 1,
      }));
      renderResults(mapped, trimmed);
    }
  } catch (_) {
    // If Tier 1 fails, allow Tier 2 to proceed
  }

  // Tier 2: Deep Hybrid Search with diff & version awareness (<180ms)
  try {
    const grouped = await tauri.invoke('search_grouped', {
      query: trimmed,
      limit: 20
    });

    if (querySeq === state.currentQuerySeq) {
      elements.searchSpinner.classList.remove('active');
      if (grouped && Array.isArray(grouped) && grouped.length > 0) {
        const fullResults = grouped.map(g => ({
          ...g.primary_result,
          other_versions: g.other_versions,
          total_version_count: g.total_version_count,
        }));
        renderResults(fullResults, trimmed);
      } else if (state.results.length === 0) {
        // Only show empty state if Tier 1 also found nothing
        renderResults([], trimmed);
      }
    }
  } catch (err) {
    if (querySeq === state.currentQuerySeq) {
      elements.searchSpinner.classList.remove('active');
      setStatusText('Search index unavailable', true);
    }
  }
}

// --- View Navigation Management ---
export async function switchView(targetView, contextItem = null) {
  if (targetView === 'search') {
    state.activeView = 'search';
    updateWindowHeight();
    return;
  }

  if (targetView === 'settings') {
    try {
      await tauri.invoke('open_settings_window');
    } catch (e) {
      console.error('Failed to open settings window', e);
    }
    return;
  }

  if (targetView === 'diff') {
    try {
      const path = contextItem?.path || (state.results && state.selectedIndex >= 0 ? state.results[state.selectedIndex]?.path : null);
      await tauri.invoke('open_compare_window', { path: path || null });
    } catch (e) {
      console.error('Failed to open compare window', e);
    }
    return;
  }

  if (targetView === 'onboarding') {
    try {
      await tauri.invoke('open_onboarding_window');
    } catch (e) {
      console.error('Failed to open onboarding window', e);
    }
    return;
  }
}

// --- Synthesis Model Indicator Helpers ---

export function updateSynthesisModel(provider, model, endpoint, ramBytes, modelSizeBytes) {
  if (!model) {
    if (elements.synthesisModelInfo) {
      elements.synthesisModelInfo.style.display = 'none';
    }
    return;
  }

  let cleanModel = model;
  if (cleanModel.startsWith('custom-')) {
    cleanModel = cleanModel.replace('custom-', '');
  }
  cleanModel = cleanModel.replace(/\.gguf$/i, '');

  const providerLabels = {
    lm_studio: 'LM Studio',
    lmstudio: 'LM Studio',
    ollama: 'Ollama',
    openai_compat: 'OpenAI Local',
    anthropic_compat: 'Anthropic Local',
    native: 'Native Metal',
    hybrid: 'Native Metal',
    local: 'Local (GGUF)',
  };
  const provLabel = providerLabels[provider] || (provider ? provider.toUpperCase() : 'Local');

  if (elements.synthesisModelInfo) {
    if (elements.infoTooltipProvider) {
      elements.infoTooltipProvider.textContent = provLabel;
    }
    if (elements.infoTooltipModel) {
      elements.infoTooltipModel.textContent = cleanModel;
    }
    if (elements.infoTooltipRam && elements.infoTooltipRamRow) {
      const isNative = provider === 'native' || provider === 'hybrid' || provider === 'local';
      if (ramBytes && ramBytes > 0) {
        elements.infoTooltipRam.textContent = `${formatBytes(ramBytes)} (Metal GPU)`;
        elements.infoTooltipRam.style.color = '#34c759';
        elements.infoTooltipRamRow.style.display = 'flex';
      } else if (modelSizeBytes && modelSizeBytes > 0) {
        elements.infoTooltipRam.textContent = `${formatBytes(modelSizeBytes)} (on demand)`;
        elements.infoTooltipRam.style.color = 'var(--label-2, #86868b)';
        elements.infoTooltipRamRow.style.display = 'flex';
      } else if (isNative) {
        elements.infoTooltipRam.textContent = 'Metal / CPU (on demand)';
        elements.infoTooltipRam.style.color = 'var(--label-2, #86868b)';
        elements.infoTooltipRamRow.style.display = 'flex';
      } else {
        elements.infoTooltipRamRow.style.display = 'none';
      }
    }
    if (elements.infoTooltipEndpoint && elements.infoTooltipEndpointRow) {
      if (endpoint) {
        elements.infoTooltipEndpoint.textContent = endpoint;
        elements.infoTooltipEndpointRow.style.display = 'flex';
      } else {
        elements.infoTooltipEndpointRow.style.display = 'none';
      }
    }
    elements.synthesisModelInfo.style.display = 'inline-flex';
  }
}

// --- Synthesis Streaming & Display Helpers ---
let activeTokenSource = null;

export function handleSynthesisToken(source, token) {
  if (!state.isSynthesizing) return;
  if (!activeTokenSource) {
    activeTokenSource = source;
  }
  if (source !== activeTokenSource) {
    return;
  }
  state.synthesisText += token;
  renderSynthesisStream();
}

export function renderSynthesisStream() {
  if (!elements.synthesisContent) return;

  const raw = state.synthesisText;
  const thinkOpenIndex = raw.indexOf('<think>');
  const thinkCloseIndex = raw.indexOf('</think>');

  if (thinkOpenIndex !== -1) {
    if (thinkCloseIndex === -1) {
      // Currently in reasoning phase
      if (elements.synthesisStatusBadge) {
        elements.synthesisStatusBadge.textContent = 'Thinking...';
        elements.synthesisStatusBadge.className = 'synthesis-badge generating thinking';
      }
      if (state.showThinkingTokens) {
        const thought = raw.substring(thinkOpenIndex + 7).trimStart();
        elements.synthesisContent.textContent = thought ? `💭 Thinking:\n${thought}` : '💭 Thinking...';
      } else {
        elements.synthesisContent.textContent = '💭 Thinking...';
      }
      elements.synthesisContent.scrollTop = elements.synthesisContent.scrollHeight;
      return;
    } else {
      // Reasoning completed, streaming final answer
      const answer = raw.substring(thinkCloseIndex + 8).trimStart();
      if (state.isSynthesizing && elements.synthesisStatusBadge) {
        elements.synthesisStatusBadge.textContent = 'Generating...';
        elements.synthesisStatusBadge.className = 'synthesis-badge generating';
      }
      if (answer) {
        elements.synthesisContent.innerHTML = renderMarkdown(answer);
      } else {
        elements.synthesisContent.textContent = 'Synthesizing response...';
      }
      elements.synthesisContent.scrollTop = elements.synthesisContent.scrollHeight;
      return;
    }
  }

  // Standard token streaming
  if (state.isSynthesizing && elements.synthesisStatusBadge && !elements.synthesisStatusBadge.classList.contains('generating')) {
    elements.synthesisStatusBadge.textContent = 'Generating...';
    elements.synthesisStatusBadge.className = 'synthesis-badge generating';
  }
  elements.synthesisContent.innerHTML = renderMarkdown(raw.trimStart());
  elements.synthesisContent.scrollTop = elements.synthesisContent.scrollHeight;
}

// --- Synthesis Controllers (Phase 6) ---
export async function startSynthesis(query) {
  if (!query || state.results.length === 0) return;
  setGeneratingState(true);
  state.synthesisSources = [];
  state.synthesisText = '';
  activeTokenSource = null;

  if (elements.synthesisCard) {
    elements.synthesisCard.style.display = 'flex';
  }
  if (elements.synthesisStatusBadge) {
    elements.synthesisStatusBadge.textContent = 'Generating...';
    elements.synthesisStatusBadge.className = 'synthesis-badge generating';
  }
  if (elements.synthesisSourcesRow) {
    elements.synthesisSourcesRow.innerHTML = '';
  }
  if (elements.synthesisContent) {
    elements.synthesisContent.textContent = '';
  }

  // Pre-fetch model status and settings so info icon, RAM, and thinking display are populated immediately
  tauri.invoke('get_llm_status').then((status) => {
    if (status && status.model_name) {
      updateSynthesisModel(status.provider, status.model_name, status.endpoint, status.ram_usage_bytes, status.model_size_bytes);
    }
  }).catch(() => {});

  tauri.invoke('get_settings').then((settings) => {
    if (settings && typeof settings.show_thinking_tokens === 'boolean') {
      state.showThinkingTokens = settings.show_thinking_tokens;
    }
  }).catch(() => {});

  try {
    const channel = new window.__TAURI__.core.Channel();
    channel.onmessage = (token) => {
      handleSynthesisToken('channel', token);
    };
    await tauri.invoke('synthesize_search_results', { query, maxSources: 5, onToken: channel });
    setGeneratingState(false);
    if (elements.synthesisStatusBadge) {
      elements.synthesisStatusBadge.textContent = 'Ready';
      elements.synthesisStatusBadge.className = 'synthesis-badge ready';
    }
    renderSynthesisStream();
  } catch (err) {
    console.error('Synthesis error:', err);
    const errStr = String(err);
    if (
      errStr.includes('NO_MODEL_AVAILABLE') ||
      errStr.includes('LM_STUDIO_UNREACHABLE') ||
      errStr.includes('OLLAMA_UNREACHABLE') ||
      errStr.includes('Model file does not exist') ||
      errStr.includes('No such file')
    ) {
      renderNoModelAvailablePrompt(errStr);
    } else {
      if (elements.synthesisStatusBadge) {
        elements.synthesisStatusBadge.textContent = 'Error';
        elements.synthesisStatusBadge.className = 'synthesis-badge error';
      }
      if (elements.synthesisContent) {
        elements.synthesisContent.textContent = `Synthesis unavailable: ${err}`;
      }
    }
    setGeneratingState(false);
  }
}

export function renderNoModelAvailablePrompt(errMessage) {
  if (!elements.synthesisContent) return;

  if (elements.synthesisStatusBadge) {
    elements.synthesisStatusBadge.textContent = 'Setup Required';
    elements.synthesisStatusBadge.className = 'synthesis-badge warning';
  }

  let title = 'No Local Model Ready';
  let desc = 'To synthesize answers and summaries, download an on-device model or connect your local LM Studio / Ollama instance in Settings.';

  if (String(errMessage).includes('LM_STUDIO_UNREACHABLE')) {
    title = 'LM Studio Not Reachable';
    desc = 'Could not connect to LM Studio on localhost. Start the local server in LM Studio or choose another provider in Settings.';
  } else if (String(errMessage).includes('OLLAMA_UNREACHABLE')) {
    title = 'Ollama Not Reachable';
    desc = 'Could not connect to Ollama on localhost. Start Ollama or download an on-device model in Settings.';
  }

  elements.synthesisContent.innerHTML = `
    <div style="padding: 14px 16px; background: rgba(255, 255, 255, 0.04); border: 0.5px solid var(--separator); border-radius: 10px; margin-top: 6px; display: flex; flex-direction: column; gap: 8px;">
      <div style="display: flex; align-items: center; gap: 8px;">
        <span style="font-size: 15px;">🧠</span>
        <span style="font-weight: 600; font-size: 13px; color: var(--label);">${title}</span>
      </div>
      <p style="font-size: 12px; color: var(--label-2); margin: 0; line-height: 1.5;">${desc}</p>
      <div style="display: flex; gap: 8px; flex-wrap: wrap; margin-top: 4px;">
        <button id="btn-prompt-download-model" class="btn btn-primary" style="height: 26px; padding: 0 12px; font-size: 11px; display: inline-flex; align-items: center; gap: 6px; cursor: pointer;">
          <span>⚡</span> Download Model (Neural Engine)
        </button>
        <button id="btn-prompt-configure-lmstudio" class="btn btn-secondary" style="height: 26px; padding: 0 12px; font-size: 11px; display: inline-flex; align-items: center; gap: 6px; cursor: pointer;">
          <span>🔌</span> Configure LM Studio / Ollama
        </button>
      </div>
    </div>
  `;

  const btnDl = document.getElementById('btn-prompt-download-model');
  if (btnDl) {
    btnDl.addEventListener('click', async () => {
      try {
        await tauri.invoke('open_settings_window', { tab: 'neural-engine' });
        if (window.__TAURI__?.event) {
          window.__TAURI__.event.emit('navigate-settings-tab', { tab: 'neural-engine', sub: 'models' });
        }
      } catch (e) {
        console.error('Failed to open settings window:', e);
      }
    });
  }

  const btnCfg = document.getElementById('btn-prompt-configure-lmstudio');
  if (btnCfg) {
    btnCfg.addEventListener('click', async () => {
      try {
        await tauri.invoke('open_settings_window', { tab: 'neural-engine' });
        if (window.__TAURI__?.event) {
          window.__TAURI__.event.emit('navigate-settings-tab', { tab: 'neural-engine', sub: 'providers' });
        }
      } catch (e) {
        console.error('Failed to open settings window:', e);
      }
    });
  }
}

export async function startDocumentSummary(artifactId) {
  if (!artifactId) return;
  setGeneratingState(true);
  state.synthesisSources = [];
  state.synthesisText = '';
  activeTokenSource = null;

  if (elements.synthesisCard) {
    elements.synthesisCard.style.display = 'flex';
  }
  if (elements.synthesisStatusBadge) {
    elements.synthesisStatusBadge.textContent = 'Summarizing...';
    elements.synthesisStatusBadge.className = 'synthesis-badge generating';
  }
  if (elements.synthesisSourcesRow) {
    elements.synthesisSourcesRow.innerHTML = '';
  }
  if (elements.synthesisContent) {
    elements.synthesisContent.textContent = '';
  }

  // Pre-fetch model status and settings so info icon, RAM, and thinking display are populated immediately
  tauri.invoke('get_llm_status').then((status) => {
    if (status && status.model_name) {
      updateSynthesisModel(status.provider, status.model_name, status.endpoint, status.ram_usage_bytes, status.model_size_bytes);
    }
  }).catch(() => {});

  tauri.invoke('get_settings').then((settings) => {
    if (settings && typeof settings.show_thinking_tokens === 'boolean') {
      state.showThinkingTokens = settings.show_thinking_tokens;
    }
  }).catch(() => {});

  try {
    const channel = new window.__TAURI__.core.Channel();
    channel.onmessage = (token) => {
      handleSynthesisToken('channel', token);
    };
    await tauri.invoke('summarize_document', { artifactId, onToken: channel });
    setGeneratingState(false);
    if (elements.synthesisStatusBadge) {
      elements.synthesisStatusBadge.textContent = 'Ready';
      elements.synthesisStatusBadge.className = 'synthesis-badge ready';
    }
    renderSynthesisStream();
  } catch (err) {
    console.error('Summary error:', err);
    const errStr = String(err);
    if (
      errStr.includes('NO_MODEL_AVAILABLE') ||
      errStr.includes('LM_STUDIO_UNREACHABLE') ||
      errStr.includes('OLLAMA_UNREACHABLE') ||
      errStr.includes('Model file does not exist') ||
      errStr.includes('No such file')
    ) {
      renderNoModelAvailablePrompt(errStr);
    } else {
      if (elements.synthesisStatusBadge) {
        elements.synthesisStatusBadge.textContent = 'Error';
        elements.synthesisStatusBadge.className = 'synthesis-badge error';
      }
      if (elements.synthesisContent) {
        elements.synthesisContent.textContent = `Summary unavailable: ${err}`;
      }
    }
    setGeneratingState(false);
  }
}

export async function stopSynthesis() {
  if (state.isSynthesizing) {
    await tauri.invoke('cancel_synthesis').catch(() => {});
    setGeneratingState(false);
  }
  activeTokenSource = null;
  if (elements.synthesisCard) {
    elements.synthesisCard.style.display = 'none';
  }
  if (elements.synthesisModelInfo) {
    elements.synthesisModelInfo.style.display = 'none';
  }
}

// --- Event Listeners Setup ---
function setupEventListeners() {
  // Apple Intelligence Glow Focus & Hover Event Listeners
  if (elements.searchInput) {
    elements.searchInput.addEventListener('focus', () => {
      if (elements.app) elements.app.classList.add('is-focused');
      if (elements.searchHeader) elements.searchHeader.classList.add('is-focused');
    });

    elements.searchInput.addEventListener('blur', () => {
      if (elements.app) elements.app.classList.remove('is-focused');
      if (elements.searchHeader) elements.searchHeader.classList.remove('is-focused');
    });
  }

  // Apple Intelligence Reactive Cursor Tracking
  if (elements.searchHeader) {
    elements.searchHeader.addEventListener('pointermove', (e) => {
      const rect = elements.searchHeader.getBoundingClientRect();
      const x = ((e.clientX - rect.left) / rect.width) * 100;
      const y = ((e.clientY - rect.top) / rect.height) * 100;
      const xPct = `${x.toFixed(1)}%`;
      const yPct = `${y.toFixed(1)}%`;
      elements.searchHeader.style.setProperty('--mouse-x', xPct);
      elements.searchHeader.style.setProperty('--mouse-y', yPct);
      if (elements.app) {
        elements.app.style.setProperty('--mouse-x', xPct);
        elements.app.style.setProperty('--mouse-y', yPct);
      }
    });

    elements.searchHeader.addEventListener('pointerleave', () => {
      elements.searchHeader.style.setProperty('--mouse-x', '50%');
      elements.searchHeader.style.setProperty('--mouse-y', '50%');
      if (elements.app) {
        elements.app.style.setProperty('--mouse-x', '50%');
        elements.app.style.setProperty('--mouse-y', '50%');
      }
    });
  }

  // Input debouncing (100ms)
  elements.searchInput.addEventListener('input', (e) => {
    clearTimeout(state.isDebouncing);
    const text = e.target.value;
    state.isDebouncing = setTimeout(() => {
      dispatchSearch(text);
    }, 100);
  });

  // Clear button
  elements.clearBtn.addEventListener('click', () => {
    elements.searchInput.value = '';
    dispatchSearch('');
    elements.searchInput.focus();
  });

  // Settings toggle button
  elements.settingsToggleBtn.addEventListener('click', () => {
    tauri.invoke('open_settings_window').catch((err) => console.error('Failed to open settings window:', err));
  });

  // Global Keyboard Navigation
  document.addEventListener('keydown', (e) => {
    // Esc handling
    if (e.key === 'Escape') {
      e.preventDefault();
      if (state.isSynthesizing || (elements.synthesisCard && elements.synthesisCard.style.display !== 'none')) {
        stopSynthesis();
        return;
      }
      if (state.activeView !== 'search') {
        if (state.activeView === 'onboarding') {
          localStorage.setItem('serchx_onboarded', 'true');
          tauri.invoke('set_auto_hide', { enabled: true }).catch(() => {});
        }
        switchView('search');
        return;
      }
      if (elements.searchInput.value.trim().length > 0) {
        elements.searchInput.value = '';
        dispatchSearch('');
        return;
      }
      tauri.invoke('hide_window').catch(() => {});
      return;
    }

    // Settings shortcut (⌘,)
    if ((e.metaKey || e.ctrlKey) && e.key === ',') {
      e.preventDefault();
      tauri.invoke('open_settings_window').catch((err) => console.error('Failed to open settings window:', err));
      return;
    }

    // Only process list shortcuts in search view
    if (state.activeView === 'search') {
      // Tab: Expand Synthesis and Trigger Multi-Document Synthesis (D-13)
      if (e.key === 'Tab') {
        e.preventDefault();
        startSynthesis(state.query);
        return;
      }
      if (e.key === 'ArrowDown') {
        e.preventDefault();
        selectResultIndex(state.selectedIndex + 1);
        return;
      }
      if (e.key === 'ArrowUp') {
        e.preventDefault();
        selectResultIndex(state.selectedIndex - 1);
        return;
      }

      // Shift + Space: Quick Preview
      if (e.shiftKey && e.key === ' ') {
        e.preventDefault();
        const item = state.results[state.selectedIndex];
        if (item) {
          tauri.invoke('quick_look_file', { path: item.path }).catch((err) => console.error('Quick Look error:', err));
        }
        return;
      }

      // ⌘+Enter: Reveal in Finder
      if ((e.metaKey || e.ctrlKey) && e.key === 'Enter') {
        e.preventDefault();
        revealSelectedInFinder();
        return;
      }

      // Enter: Open File
      if (e.key === 'Enter') {
        e.preventDefault();
        openSelectedFile();
        return;
      }

      // ⌘+C: Copy Path
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'c') {
        if (!window.getSelection()?.toString()) {
          e.preventDefault();
          copySelectedPath();
        }
        return;
      }

      // ⌘+D: Open Diff View
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'd') {
        e.preventDefault();
        const item = state.results[state.selectedIndex];
        if (item) {
          switchView('diff', item);
        }
        return;
      }

      // ⌘+P: Toggle Version Pin
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'p') {
        e.preventDefault();
        toggleSelectedPin();
        return;
      }
    }
  });

  // Preview action buttons
  document.getElementById('preview-open-btn')?.addEventListener('click', () => {
    openSelectedFile();
  });
  document.getElementById('preview-reveal-btn')?.addEventListener('click', () => {
    revealSelectedInFinder();
  });
  document.getElementById('preview-compare-btn')?.addEventListener('click', () => {
    const item = state.results[state.selectedIndex];
    if (item) switchView('diff', item);
  });
  document.getElementById('preview-pin-btn')?.addEventListener('click', () => {
    toggleSelectedPin();
  });
  document.getElementById('preview-summarize-btn')?.addEventListener('click', () => {
    const item = state.results[state.selectedIndex];
    if (item && item.artifact_id) {
      startDocumentSummary(item.artifact_id);
    }
  });

  elements.synthesisCloseBtn?.addEventListener('click', () => {
    stopSynthesis();
  });

  elements.synthesisCopyBtn?.addEventListener('click', async () => {
    if (!state.synthesisText) return;
    try {
      await navigator.clipboard.writeText(state.synthesisText);
      setStatusText('✓ Answer copied to clipboard', true);
    } catch (_) {}
  });

  const refreshSynthesisInfo = () => {
    tauri.invoke('get_llm_status').then((status) => {
      if (status && status.model_name) {
        updateSynthesisModel(
          status.provider,
          status.model_name,
          status.endpoint,
          status.ram_usage_bytes,
          status.model_size_bytes
        );
      }
    }).catch(() => {});
  };

  elements.synthesisModelInfo?.addEventListener('mouseenter', refreshSynthesisInfo);
  elements.synthesisModelInfo?.addEventListener('focus', refreshSynthesisInfo);

  // Listen for LLM Synthesis events
  if (window.__TAURI__?.event?.listen) {
    window.__TAURI__.event.listen('llm-sources', (event) => {
      const sources = event.payload || [];
      state.synthesisSources = sources;
      if (!elements.synthesisSourcesRow) return;
      elements.synthesisSourcesRow.innerHTML = '';

      sources.forEach((src) => {
        const badge = document.createElement('button');
        badge.className = 'citation-badge';
        badge.title = `${src.file_path} (Click to jump)`;
        badge.innerHTML = `<span class="cite-num">[${src.index}]</span> <span>${src.document_title}</span>`;
        badge.addEventListener('click', () => {
          const idx = state.results.findIndex(
            (r) => r.path === src.file_path || r.artifact_id === src.artifact_id
          );
          if (idx !== -1) {
            selectResultIndex(idx);
          }
        });
        elements.synthesisSourcesRow.appendChild(badge);
      });
    });

    window.__TAURI__.event.listen('llm-start', (event) => {
      const { provider, model, endpoint, ram_usage_bytes, model_size_bytes } = event.payload || {};
      updateSynthesisModel(provider, model, endpoint, ram_usage_bytes, model_size_bytes);
    });

    window.__TAURI__.event.listen('llm-token', (event) => {
      handleSynthesisToken('event', event.payload || '');
    });

    window.__TAURI__.event.listen('llm-error', (event) => {
      setGeneratingState(false);
      if (elements.synthesisStatusBadge) {
        elements.synthesisStatusBadge.textContent = 'Error';
        elements.synthesisStatusBadge.className = 'synthesis-badge error';
      }
      if (elements.synthesisContent && !state.synthesisText) {
        elements.synthesisContent.textContent = `Synthesis error: ${event.payload || 'Failed'}`;
      }
    });

    window.__TAURI__.event.listen('llm-done', () => {
      setGeneratingState(false);
      if (elements.synthesisStatusBadge) {
        elements.synthesisStatusBadge.textContent = 'Ready';
        elements.synthesisStatusBadge.className = 'synthesis-badge ready';
      }
      renderSynthesisStream();
      tauri.invoke('get_llm_status').then((status) => {
        if (status && status.model_name) {
          updateSynthesisModel(status.provider, status.model_name, status.endpoint, status.ram_usage_bytes, status.model_size_bytes);
        }
      }).catch(() => {});
    });

    window.__TAURI__.event.listen('llm-status-changed', (event) => {
      const payload = event.payload || {};
      if (payload && payload.model) {
        updateSynthesisModel(payload.provider, payload.model, null, payload.ram_usage_bytes, payload.model_size_bytes);
      } else {
        tauri.invoke('get_llm_status').then((status) => {
          if (status && status.model_name) {
            updateSynthesisModel(status.provider, status.model_name, status.endpoint, status.ram_usage_bytes, status.model_size_bytes);
          }
        }).catch(() => {});
      }
    });

    window.__TAURI__.event.listen('settings-updated', (event) => {
      const updated = event.payload;
      if (updated && typeof updated.show_thinking_tokens === 'boolean') {
        state.showThinkingTokens = updated.show_thinking_tokens;
        if (state.isSynthesizing) {
          renderSynthesisStream();
        }
      }
    });
  }
}

// --- Free-Hand Window Drag & Centering ---
function setupWindowDragHandlers() {
  const handleDragMouseDown = (e) => {
    // Only primary mouse button (left click)
    if (e.button !== 0) return;

    // Do not initiate drag on interactive form controls, links, or buttons
    const interactive = e.target.closest('input, button, select, textarea, [role="button"], [role="option"], a, .interactive');
    if (interactive) return;

    // Check if target is inside any designated drag region
    const dragRegion = e.target.closest('[data-tauri-drag-region], #search-header, #app-footer, #empty-state, .diff-header, .settings-header, .onboarding-header');
    if (dragRegion) {
      tauri.invoke('start_dragging').catch(() => {});
    }
  };

  elements.app?.addEventListener('mousedown', handleDragMouseDown);

  // Double-click search icon to reset/center window on screen
  elements.searchIconWrapper?.addEventListener('dblclick', (e) => {
    e.preventDefault();
    e.stopPropagation();
    tauri.invoke('center_window').catch(() => {});
  });
}

// --- Diff Inspector State & Controller ---
export const diffState = {
  item: null,
  allVersions: [],
  versionAId: null,
  versionBId: null,
  diffData: null,
};

export async function openDiffInspector(item) {
  if (!item) return;
  diffState.item = item;
  
  const filename = item.path.split('/').pop() || item.path;
  const fileNameEl = document.getElementById('diff-file-name');
  if (fileNameEl) fileNameEl.textContent = filename;

  // Build version list
  const primary = {
    version_id: item.version_id,
    version_number: item.version_number || (item.total_version_count || 1),
    modified_at: item.modified_at,
    is_pinned: item.is_pinned,
  };
  const others = (item.other_versions || []).map(v => ({
    version_id: v.version_id,
    version_number: v.version_number,
    modified_at: v.modified_at,
    is_pinned: v.is_pinned,
  }));

  const all = [primary, ...others];
  // Sort ascending by version number or timestamp
  all.sort((a, b) => (a.version_number || 0) - (b.version_number || 0) || (a.modified_at || 0) - (b.modified_at || 0));
  diffState.allVersions = all;

  updateDiffPinButton(item.is_pinned);

  if (all.length <= 1) {
    const contentEl = document.getElementById('diff-content');
    if (contentEl) {
      contentEl.innerHTML = '<div class="diff-empty-notice">Only 1 version exists for this document. No prior version to compare.</div>';
    }
    const versionPill = document.getElementById('diff-version-pill');
    if (versionPill) versionPill.textContent = 'v1 (Single)';
    renderTimelineScrubber();
    return;
  }

  // Baseline: Previous version (A), Target: Latest version (B)
  const verB = all[all.length - 1];
  const verA = all[all.length - 2];
  diffState.versionAId = verA.version_id;
  diffState.versionBId = verB.version_id;

  renderTimelineScrubber();
  await loadAndRenderDiff(diffState.versionAId, diffState.versionBId);
}

function updateDiffPinButton(isPinned) {
  const pinBtn = document.getElementById('diff-pin-btn');
  const pinText = document.getElementById('diff-pin-btn-text');
  if (!pinBtn || !pinText) return;
  if (isPinned) {
    pinBtn.classList.add('pinned');
    pinText.textContent = '★ Pinned';
  } else {
    pinBtn.classList.remove('pinned');
    pinText.textContent = '☆ Pin';
  }
}

function renderTimelineScrubber() {
  const trackEl = document.getElementById('diff-timeline-tracks');
  if (!trackEl) return;
  trackEl.innerHTML = '';

  diffState.allVersions.forEach((v, index) => {
    const dotWrapper = document.createElement('div');
    dotWrapper.className = 'timeline-version-dot';
    if (v.version_id === diffState.versionAId) dotWrapper.classList.add('active-a');
    if (v.version_id === diffState.versionBId) dotWrapper.classList.add('active-b');

    const dot = document.createElement('div');
    dot.className = 'timeline-dot';

    const label = document.createElement('span');
    label.className = 'timeline-label';
    label.textContent = `v${v.version_number || (index + 1)}${v.is_pinned ? '★' : ''}`;

    dotWrapper.appendChild(dot);
    dotWrapper.appendChild(label);

    dotWrapper.title = `Version ${v.version_number || (index + 1)} (${formatRelativeTime(v.modified_at)})`;

    dotWrapper.addEventListener('click', async () => {
      // Toggle comparison targets
      if (v.version_id === diffState.versionBId) return;
      diffState.versionAId = v.version_id;
      renderTimelineScrubber();
      await loadAndRenderDiff(diffState.versionAId, diffState.versionBId);
    });

    trackEl.appendChild(dotWrapper);
  });
}

export async function loadAndRenderDiff(verAId, verBId) {
  const contentEl = document.getElementById('diff-content');
  const versionPill = document.getElementById('diff-version-pill');
  if (!contentEl) return;

  const verAObj = diffState.allVersions.find(v => v.version_id === verAId);
  const verBObj = diffState.allVersions.find(v => v.version_id === verBId);
  if (versionPill && verAObj && verBObj) {
    versionPill.textContent = `v${verAObj.version_number || 1} → v${verBObj.version_number || 2}`;
  }

  contentEl.innerHTML = '<div class="diff-empty-notice">Computing text difference...</div>';

  try {
    const diff = await tauri.invoke('get_version_diff', {
      versionAId: verAId,
      versionBId: verBId,
    });
    diffState.diffData = diff;

    // Update stats bar
    const statAdd = document.getElementById('diff-stat-added');
    const statRem = document.getElementById('diff-stat-removed');
    const statUnc = document.getElementById('diff-stat-unchanged');
    if (statAdd) statAdd.textContent = `+${diff.summary?.added_lines || 0} lines`;
    if (statRem) statRem.textContent = `-${diff.summary?.removed_lines || 0} lines`;
    if (statUnc) statUnc.textContent = `${diff.summary?.unchanged_lines || 0} unchanged`;

    renderHunks(diff.hunks || [], contentEl);
  } catch (err) {
    contentEl.innerHTML = `<div class="diff-empty-notice">Unable to compute diff: ${err}</div>`;
  }
}

function renderHunks(hunks, container) {
  container.innerHTML = '';
  if (!hunks || hunks.length === 0) {
    container.innerHTML = '<div class="diff-empty-notice">No textual differences between selected versions.</div>';
    return;
  }

  let i = 0;
  while (i < hunks.length) {
    const hunk = hunks[i];

    // Check for collapsible contiguous equal block (> 10 lines)
    if (hunk.tag === 'equal') {
      let equalCount = 0;
      let j = i;
      while (j < hunks.length && hunks[j].tag === 'equal') {
        equalCount++;
        j++;
      }

      if (equalCount > 10) {
        const startIdx = i;
        const endIdx = j;
        const collapsedBlock = document.createElement('div');
        collapsedBlock.className = 'diff-collapsed-block';
        collapsedBlock.textContent = `··· ${equalCount} unchanged lines (click to expand) ···`;
        collapsedBlock.addEventListener('click', () => {
          // Replace collapsed block with actual lines
          const fragment = document.createDocumentFragment();
          for (let k = startIdx; k < endIdx; k++) {
            fragment.appendChild(createDiffLineElement(hunks[k]));
          }
          collapsedBlock.replaceWith(fragment);
        });
        container.appendChild(collapsedBlock);
        i = j;
        continue;
      }
    }

    container.appendChild(createDiffLineElement(hunk));
    i++;
  }
}

function createDiffLineElement(hunk) {
  const line = document.createElement('div');
  line.className = 'diff-line';

  if (hunk.tag === 'added') {
    line.classList.add('diff-line-added');
  } else if (hunk.tag === 'removed') {
    line.classList.add('diff-line-removed');
  }

  const num = document.createElement('span');
  num.className = 'diff-line-num';
  num.textContent = hunk.line_number != null ? hunk.line_number : '';

  const marker = document.createElement('span');
  marker.className = 'diff-line-marker';
  marker.textContent = hunk.tag === 'added' ? '+' : hunk.tag === 'removed' ? '-' : ' ';

  const text = document.createElement('span');
  text.className = 'diff-line-text';
  text.textContent = hunk.content || '';

  line.appendChild(num);
  line.appendChild(marker);
  line.appendChild(text);
  return line;
}

// Wire Diff Action Buttons
function setupDiffActions() {
  document.getElementById('diff-back-btn')?.addEventListener('click', () => {
    switchView('search');
  });

  document.getElementById('diff-open-btn')?.addEventListener('click', () => {
    if (diffState.item?.path) {
      tauri.invoke('open_file', { path: diffState.item.path });
      tauri.invoke('hide_window');
    }
  });

  document.getElementById('diff-reveal-btn')?.addEventListener('click', () => {
    if (diffState.item?.path) {
      tauri.invoke('reveal_in_finder', { path: diffState.item.path });
      setStatusText('Revealed in Finder', true);
    }
  });

  document.getElementById('diff-copy-btn')?.addEventListener('click', async () => {
    if (!diffState.diffData || !diffState.diffData.hunks) return;
    const lines = diffState.diffData.hunks.map(h => {
      const prefix = h.tag === 'added' ? '+' : h.tag === 'removed' ? '-' : ' ';
      return `${prefix} ${h.content}`;
    });
    const header = `--- Version ${diffState.versionAId}\n+++ Version ${diffState.versionBId}\n`;
    try {
      await navigator.clipboard.writeText(header + lines.join('\n'));
      setStatusText('Diff copied to clipboard', true);
    } catch (_) {
      setStatusText('Failed to copy diff', true);
    }
  });

  document.getElementById('diff-pin-btn')?.addEventListener('click', async () => {
    if (!diffState.versionBId) return;
    try {
      const isPinned = await tauri.invoke('toggle_version_pin', { version_id: diffState.versionBId });
      updateDiffPinButton(isPinned);
      // Update local state
      const target = diffState.allVersions.find(v => v.version_id === diffState.versionBId);
      if (target) target.is_pinned = isPinned;
      if (diffState.item && diffState.item.version_id === diffState.versionBId) {
        diffState.item.is_pinned = isPinned;
      }
      renderTimelineScrubber();
      setStatusText(isPinned ? 'Version pinned' : 'Version unpinned', true);
    } catch (err) {
      setStatusText(`Pin error: ${err}`, true);
    }
  });

  // Quick compare buttons
  document.getElementById('diff-quick-prev')?.addEventListener('click', async () => {
    if (diffState.allVersions.length < 2) return;
    const all = diffState.allVersions;
    diffState.versionBId = all[all.length - 1].version_id;
    diffState.versionAId = all[all.length - 2].version_id;
    document.getElementById('diff-quick-prev')?.classList.add('active');
    document.getElementById('diff-quick-oldest')?.classList.remove('active');
    renderTimelineScrubber();
    await loadAndRenderDiff(diffState.versionAId, diffState.versionBId);
  });

  document.getElementById('diff-quick-oldest')?.addEventListener('click', async () => {
    if (diffState.allVersions.length < 2) return;
    const all = diffState.allVersions;
    diffState.versionBId = all[all.length - 1].version_id;
    diffState.versionAId = all[0].version_id;
    document.getElementById('diff-quick-oldest')?.classList.add('active');
    document.getElementById('diff-quick-prev')?.classList.remove('active');
    renderTimelineScrubber();
    await loadAndRenderDiff(diffState.versionAId, diffState.versionBId);
  });
}

// --- Settings & Crawler Dashboard State & Controller ---
export const settingsState = {
  watchedFolders: [],
  crawlProgress: null,
  pollTimer: null,
  appSettings: null,
  saveTimeout: null,
  savePillTimer: null,
};

export async function openSettingsDashboard() {
  await loadAppSettings();
  await loadWatchedFolders();
  await checkSystemPermissions();
  await updateCrawlMetrics();
  startCrawlerPolling();
}

async function loadAppSettings() {
  try {
    const settings = await tauri.invoke('get_settings');
    settingsState.appSettings = settings;
    if (settings && typeof settings.show_thinking_tokens === 'boolean') {
      state.showThinkingTokens = settings.show_thinking_tokens;
    }
    populateSettingsUI(settings);
  } catch (err) {
    console.error('Failed to load app settings:', err);
  }
}

function updatePresetCardsHighlight(presetName) {
  document.querySelectorAll('.preset-card').forEach(card => {
    if (card.dataset.preset === presetName) {
      card.classList.add('active');
    } else {
      card.classList.remove('active');
    }
  });
}

function populateSettingsUI(settings) {
  if (!settings) return;

  // 1. Preset Cards
  updatePresetCardsHighlight(settings.preset);

  // 2. Smart Toggles
  const toggleSemantic = document.getElementById('toggle-semantic-search');
  if (toggleSemantic) toggleSemantic.checked = !!settings.semantic_search_enabled;

  const toggleBattery = document.getElementById('toggle-auto-pause-battery');
  if (toggleBattery) toggleBattery.checked = !!settings.auto_pause_on_battery;

  // 3. Limits & Sliders
  const sliderFileSize = document.getElementById('slider-file-size');
  const valFileSize = document.getElementById('val-file-size');
  if (sliderFileSize && valFileSize) {
    sliderFileSize.value = settings.max_file_size_mb || 50;
    valFileSize.textContent = `${sliderFileSize.value} MB`;
  }

  const sliderVersions = document.getElementById('slider-versions');
  const valVersions = document.getElementById('val-versions');
  if (sliderVersions && valVersions) {
    sliderVersions.value = settings.max_versions_per_artifact || 3;
    valVersions.textContent = `${sliderVersions.value} versions`;
  }

  // 4. Ranking & Weights
  const sliderBalance = document.getElementById('slider-weight-balance');
  const valBalance = document.getElementById('val-weight-balance');
  if (sliderBalance && valBalance) {
    const total = (settings.fts_weight || 1.0) + (settings.vector_weight || 1.0);
    const balance = total > 0 ? Math.round(((settings.vector_weight || 1.0) / total) * 200) : 100;
    sliderBalance.value = balance;
    valBalance.textContent = `${(settings.fts_weight || 1.0).toFixed(1)} : ${(settings.vector_weight || 1.0).toFixed(1)}`;
  }

  const inputRrfK = document.getElementById('input-rrf-k');
  if (inputRrfK) inputRrfK.value = settings.rrf_k || 60;

  const inputThermal = document.getElementById('input-thermal-delay');
  if (inputThermal) inputThermal.value = settings.thermal_throttle_delay_ms || 200;
}

function showSettingsSavedIndicator(text = '✓ Saved') {
  const pill = document.getElementById('settings-save-pill');
  if (pill) {
    pill.textContent = text;
    pill.classList.add('visible');
    clearTimeout(settingsState.savePillTimer);
    settingsState.savePillTimer = setTimeout(() => {
      pill.classList.remove('visible');
    }, 2000);
  }
}

async function saveAppSettings(immediate = false) {
  clearTimeout(settingsState.saveTimeout);

  const doSave = async () => {
    const sliderBalance = document.getElementById('slider-weight-balance');
    const balanceVal = parseInt(sliderBalance?.value || '100', 10);
    // 0 -> fts 2.0, vector 0.0; 100 -> fts 1.0, vector 1.0; 200 -> fts 0.0, vector 2.0
    const ftsWeight = parseFloat(((200 - balanceVal) / 100).toFixed(2));
    const vectorWeight = parseFloat((balanceVal / 100).toFixed(2));

    const updated = {
      ...(settingsState.appSettings || {}),
      preset: settingsState.appSettings?.preset || 'Custom',
      semantic_search_enabled: !!document.getElementById('toggle-semantic-search')?.checked,
      auto_pause_on_battery: !!document.getElementById('toggle-auto-pause-battery')?.checked,
      max_file_size_mb: Math.max(1, parseInt(document.getElementById('slider-file-size')?.value || '50', 10)),
      max_versions_per_artifact: Math.max(1, parseInt(document.getElementById('slider-versions')?.value || '3', 10)),
      max_indexed_files: settingsState.appSettings?.max_indexed_files || 20000,
      max_vector_chunks: settingsState.appSettings?.max_vector_chunks || 250000,
      thermal_throttle_delay_ms: Math.max(0, parseInt(document.getElementById('input-thermal-delay')?.value || '200', 10)),
      embedding_model: settingsState.appSettings?.embedding_model || 'AllMiniLML6V2',
      embedding_dimension: settingsState.appSettings?.embedding_dimension || 384,
      fts_weight: ftsWeight,
      vector_weight: vectorWeight,
      rrf_k: Math.max(1, parseInt(document.getElementById('input-rrf-k')?.value || '60', 10)),
    };

    try {
      await tauri.invoke('update_settings', { settings: updated });
      settingsState.appSettings = updated;
      showSettingsSavedIndicator('✓ Saved');
    } catch (err) {
      console.error('Failed to update settings:', err);
    }
  };

  if (immediate) {
    await doSave();
  } else {
    settingsState.saveTimeout = setTimeout(doSave, 250);
  }
}

function applyPreset(presetName) {
  let newSettings = { ...(settingsState.appSettings || {}) };

  if (presetName === 'Balanced') {
    newSettings = {
      ...newSettings,
      preset: 'Balanced',
      semantic_search_enabled: true,
      auto_pause_on_battery: false,
      max_file_size_mb: 50,
      max_versions_per_artifact: 3,
      thermal_throttle_delay_ms: 200,
      fts_weight: 1.0,
      vector_weight: 1.0,
      rrf_k: 60,
    };
  } else if (presetName === 'Light') {
    newSettings = {
      ...newSettings,
      preset: 'Light',
      semantic_search_enabled: true,
      auto_pause_on_battery: true,
      max_file_size_mb: 20,
      max_versions_per_artifact: 1,
      thermal_throttle_delay_ms: 500,
      fts_weight: 1.2,
      vector_weight: 0.8,
      rrf_k: 60,
    };
  } else if (presetName === 'Power') {
    newSettings = {
      ...newSettings,
      preset: 'Power',
      semantic_search_enabled: true,
      auto_pause_on_battery: false,
      max_file_size_mb: 100,
      max_versions_per_artifact: 5,
      thermal_throttle_delay_ms: 50,
      fts_weight: 1.0,
      vector_weight: 1.0,
      rrf_k: 60,
    };
  } else if (presetName === 'Custom') {
    newSettings.preset = 'Custom';
  }

  settingsState.appSettings = newSettings;
  populateSettingsUI(newSettings);
  saveAppSettings(true);
}

function switchSettingsTab(tabName) {
  document.querySelectorAll('.settings-tab-btn').forEach(b => {
    const isTarget = b.dataset.tab === tabName;
    b.classList.toggle('active', isTarget);
    b.setAttribute('aria-selected', isTarget ? 'true' : 'false');
  });

  document.querySelectorAll('.settings-tab-panel').forEach(p => {
    p.classList.toggle('active', p.id === `tab-panel-${tabName}`);
  });
}

async function loadWatchedFolders() {
  const gridEl = document.getElementById('folders-grid');
  if (!gridEl) return;
  gridEl.innerHTML = '';

  try {
    const folders = await tauri.invoke('get_watched_folders');
    settingsState.watchedFolders = folders || [];
    renderFolderCards(settingsState.watchedFolders, gridEl);
  } catch (err) {
    gridEl.innerHTML = `<div class="folder-path">Error loading folders: ${err}</div>`;
  }
}

function renderFolderCards(folders, container) {
  container.innerHTML = '';
  if (!folders || folders.length === 0) {
    container.innerHTML = '<div class="folder-path" style="padding: 12px 0;">No watched folders configured. Click "+ Add Folder" to begin.</div>';
    return;
  }

  folders.forEach(folder => {
    const card = document.createElement('div');
    card.className = 'folder-card';

    const info = document.createElement('div');
    info.className = 'folder-info';

    const icon = document.createElement('div');
    icon.className = 'folder-icon';
    icon.innerHTML = `<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"></path></svg>`;

    const texts = document.createElement('div');
    texts.className = 'folder-texts';

    const name = document.createElement('span');
    name.className = 'folder-name';
    const folderName = folder.path.split('/').pop() || folder.path;
    name.textContent = folderName;

    const path = document.createElement('span');
    path.className = 'folder-path';
    path.textContent = truncatePath(folder.path, 42);

    texts.appendChild(name);
    texts.appendChild(path);
    info.appendChild(icon);
    info.appendChild(texts);

    const actions = document.createElement('div');
    actions.className = 'folder-actions';

    // Re-scan button
    const rescanBtn = document.createElement('button');
    rescanBtn.className = 'folder-rescan-btn';
    rescanBtn.title = 'Re-scan this folder';
    rescanBtn.innerHTML = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="23 4 23 10 17 10"></polyline><polyline points="1 20 1 14 7 14"></polyline><path d="M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15"></path></svg>`;
    rescanBtn.addEventListener('click', async () => {
      await tauri.invoke('start_crawl', { paths: [folder.path] });
      startCrawlerPolling();
      setStatusText(`Re-scanning ${folderName}...`, true);
    });

    // Remove button
    const removeBtn = document.createElement('button');
    removeBtn.className = 'folder-remove-btn';
    removeBtn.title = 'Remove folder';
    removeBtn.innerHTML = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="18" y1="6" x2="6" y2="18"></line><line x1="6" y1="6" x2="18" y2="18"></line></svg>`;
    removeBtn.addEventListener('click', async () => {
      await tauri.invoke('remove_watched_folder', { id: folder.id });
      await loadWatchedFolders();
    });

    // Toggle switch
    const switchLabel = document.createElement('label');
    switchLabel.className = 'switch';
    switchLabel.setAttribute('role', 'switch');
    switchLabel.setAttribute('aria-checked', folder.enabled ? 'true' : 'false');

    const switchInput = document.createElement('input');
    switchInput.type = 'checkbox';
    switchInput.checked = !!folder.enabled;
    switchInput.addEventListener('change', async () => {
      await tauri.invoke('toggle_watched_folder', {
        id: folder.id,
        enabled: switchInput.checked,
      });
      folder.enabled = switchInput.checked;
      switchLabel.setAttribute('aria-checked', folder.enabled ? 'true' : 'false');
    });

    const slider = document.createElement('span');
    slider.className = 'slider';

    switchLabel.appendChild(switchInput);
    switchLabel.appendChild(slider);

    actions.appendChild(rescanBtn);
    actions.appendChild(removeBtn);
    actions.appendChild(switchLabel);

    card.appendChild(info);
    card.appendChild(actions);
    container.appendChild(card);
  });
}

async function checkSystemPermissions() {
  const banner = document.getElementById('permission-banner');
  const bannerText = document.getElementById('permission-text');
  const bannerBtn = document.getElementById('permission-settings-btn');
  if (!banner || !bannerText) return;

  try {
    const testPath = settingsState.watchedFolders[0]?.path || '/Users';
    const perm = await tauri.invoke('check_folder_permissions', { path: testPath });

    banner.classList.remove('granted', 'warning');
    if (perm === 'Granted' || perm?.status === 'Granted') {
      banner.classList.add('granted');
      bannerText.textContent = 'Full Disk Access active: System permits fast local indexing.';
      if (bannerBtn) bannerBtn.style.display = 'none';
    } else {
      banner.classList.add('warning');
      bannerText.textContent = 'Full Disk Access recommended to search protected documents and desktop.';
      if (bannerBtn) bannerBtn.style.display = 'block';
    }
  } catch (_) {
    banner.classList.add('warning');
    bannerText.textContent = 'Grant Full Disk Access in macOS Settings for complete coverage.';
    if (bannerBtn) bannerBtn.style.display = 'block';
  }
}

async function updateCrawlMetrics() {
  try {
    const progress = await tauri.invoke('get_crawl_progress');
    settingsState.crawlProgress = progress;

    const statusTextEl = document.getElementById('crawler-status-text');
    const dotEl = document.getElementById('crawler-dot');
    const fillEl = document.getElementById('progress-bar-fill');
    const fileEl = document.getElementById('crawler-current-file');
    const pauseBtn = document.getElementById('crawler-pause-btn');

    const statIdx = document.getElementById('stat-indexed-count');
    const statDisc = document.getElementById('stat-discovered-count');
    const statSkip = document.getElementById('stat-skipped-count');
    const statFail = document.getElementById('stat-failed-count');
    const diskPill = document.getElementById('disk-usage-pill');

    if (statIdx) statIdx.textContent = progress.indexed_count || 0;
    if (statDisc) statDisc.textContent = progress.total_discovered || 0;
    if (statSkip) statSkip.textContent = progress.skipped_count || 0;
    if (statFail) statFail.textContent = progress.failed_count || 0;

    const mb = ((progress.disk_usage_bytes || 0) / (1024 * 1024)).toFixed(1);
    if (diskPill) diskPill.textContent = `${mb} MB on disk`;

    const footerPauseBtn = document.getElementById('footer-pause-crawl-btn');
    const footerResumeBtn = document.getElementById('footer-resume-crawl-btn');
    const footerDot = document.getElementById('footer-status-dot');

    if (progress.is_running) {
      if (statusTextEl) statusTextEl.textContent = progress.is_paused ? 'Paused' : 'Indexing...';
      if (dotEl) dotEl.className = 'footer-status-dot active';
      if (pauseBtn) pauseBtn.textContent = progress.is_paused ? 'Resume' : 'Pause';

      const total = progress.total_discovered || 1;
      const pct = Math.min(100, Math.round(((progress.indexed_count + progress.skipped_count) / total) * 100));
      if (fillEl) fillEl.style.width = `${pct}%`;

      if (fileEl) fileEl.textContent = progress.current_file ? truncatePath(progress.current_file, 50) : 'Indexing files...';

      // Mirror to footer pill and pause/resume button
      if (footerDot) footerDot.style.display = 'inline-block';
      if (progress.is_paused) {
        setStatusText(`Indexing paused (${progress.indexed_count} / ${progress.total_discovered || progress.indexed_count})`);
        if (footerPauseBtn) footerPauseBtn.style.display = 'none';
        if (footerResumeBtn) footerResumeBtn.style.display = 'inline-block';
      } else {
        setStatusText(`Indexing ${progress.indexed_count} / ${progress.total_discovered || progress.indexed_count} files`);
        if (footerPauseBtn) footerPauseBtn.style.display = 'inline-block';
        if (footerResumeBtn) footerResumeBtn.style.display = 'none';
      }
      if (elements.footerStatusDot) elements.footerStatusDot.className = 'footer-status-dot active';
    } else {
      if (statusTextEl) statusTextEl.textContent = 'Idle';
      if (dotEl) dotEl.className = 'footer-status-dot';
      if (fillEl) fillEl.style.width = '0%';
      if (fileEl) fileEl.textContent = 'No active crawl';
      if (pauseBtn) pauseBtn.textContent = 'Pause';
      if (footerDot) footerDot.style.display = 'none';
      if (footerPauseBtn) footerPauseBtn.style.display = 'none';
      if (footerResumeBtn) footerResumeBtn.style.display = 'none';
      if (elements.footerStatusDot) elements.footerStatusDot.className = 'footer-status-dot';
    }
  } catch (_) {}
}

function startCrawlerPolling() {
  clearInterval(settingsState.pollTimer);
  settingsState.pollTimer = setInterval(async () => {
    await updateCrawlMetrics();
    if (!settingsState.crawlProgress?.is_running && state.activeView !== 'settings') {
      clearInterval(settingsState.pollTimer);
    }
  }, 250);
}

function setupSettingsActions() {
  document.getElementById('settings-back-btn')?.addEventListener('click', () => {
    switchView('search');
  });

  document.getElementById('permission-settings-btn')?.addEventListener('click', () => {
    tauri.invoke('open_privacy_settings');
  });

  // Tab navigation
  document.querySelectorAll('.settings-tab-btn').forEach(btn => {
    btn.addEventListener('click', () => {
      switchSettingsTab(btn.dataset.tab);
    });
  });

  // Preset selection
  document.querySelectorAll('.preset-card').forEach(card => {
    card.addEventListener('click', () => {
      applyPreset(card.dataset.preset);
    });
  });

  // Smart Toggles
  document.getElementById('toggle-semantic-search')?.addEventListener('change', () => {
    if (settingsState.appSettings) settingsState.appSettings.preset = 'Custom';
    updatePresetCardsHighlight('Custom');
    saveAppSettings();
  });

  document.getElementById('toggle-auto-pause-battery')?.addEventListener('change', () => {
    if (settingsState.appSettings) settingsState.appSettings.preset = 'Custom';
    updatePresetCardsHighlight('Custom');
    saveAppSettings();
  });

  // Advanced sliders & inputs
  const sliderFileSize = document.getElementById('slider-file-size');
  const valFileSize = document.getElementById('val-file-size');
  sliderFileSize?.addEventListener('input', () => {
    if (valFileSize) valFileSize.textContent = `${sliderFileSize.value} MB`;
    if (settingsState.appSettings) settingsState.appSettings.preset = 'Custom';
    updatePresetCardsHighlight('Custom');
    saveAppSettings();
  });

  const sliderVersions = document.getElementById('slider-versions');
  const valVersions = document.getElementById('val-versions');
  sliderVersions?.addEventListener('input', () => {
    if (valVersions) valVersions.textContent = `${sliderVersions.value} versions`;
    if (settingsState.appSettings) settingsState.appSettings.preset = 'Custom';
    updatePresetCardsHighlight('Custom');
    saveAppSettings();
  });

  const sliderBalance = document.getElementById('slider-weight-balance');
  const valBalance = document.getElementById('val-weight-balance');
  sliderBalance?.addEventListener('input', () => {
    const b = parseInt(sliderBalance.value, 10);
    const f = ((200 - b) / 100).toFixed(1);
    const v = (b / 100).toFixed(1);
    if (valBalance) valBalance.textContent = `${f} : ${v}`;
    if (settingsState.appSettings) settingsState.appSettings.preset = 'Custom';
    updatePresetCardsHighlight('Custom');
    saveAppSettings();
  });

  document.getElementById('input-rrf-k')?.addEventListener('input', () => {
    if (settingsState.appSettings) settingsState.appSettings.preset = 'Custom';
    updatePresetCardsHighlight('Custom');
    saveAppSettings();
  });

  document.getElementById('input-thermal-delay')?.addEventListener('input', () => {
    if (settingsState.appSettings) settingsState.appSettings.preset = 'Custom';
    updatePresetCardsHighlight('Custom');
    saveAppSettings();
  });

  // Window center button
  document.getElementById('btn-center-window')?.addEventListener('click', async () => {
    await tauri.invoke('center_window');
    showSettingsSavedIndicator('✓ Window Centered');
  });

  // Reset defaults button
  document.getElementById('btn-reset-defaults')?.addEventListener('click', async () => {
    try {
      const defaults = await tauri.invoke('reset_settings');
      settingsState.appSettings = defaults;
      populateSettingsUI(defaults);
      showSettingsSavedIndicator('✓ Defaults Restored');
    } catch (err) {
      console.error('Failed to reset settings:', err);
    }
  });

  // Folders & Crawler Actions
  document.getElementById('add-folder-btn')?.addEventListener('click', async () => {
    try {
      const selected = await tauri.invoke('pick_folder');
      if (selected) {
        await tauri.invoke('add_watched_folder', { path: selected, recursive: true });
        await tauri.invoke('start_crawl', { paths: [selected] });
        await loadWatchedFolders();
        startCrawlerPolling();
        setStatusText(`Added and indexing ${selected.split('/').pop()}`, true);
      }
    } catch (err) {
      setStatusText(`Picker error: ${err}`, true);
    }
  });

  document.getElementById('crawler-pause-btn')?.addEventListener('click', async () => {
    if (settingsState.crawlProgress?.is_paused) {
      await tauri.invoke('resume_crawl');
    } else {
      await tauri.invoke('pause_crawl');
    }
    await updateCrawlMetrics();
  });

  document.getElementById('footer-pause-crawl-btn')?.addEventListener('click', async (e) => {
    e.stopPropagation();
    try {
      await tauri.invoke('pause_crawl');
      await updateCrawlMetrics();
    } catch (err) {
      console.error('Failed to pause crawl:', err);
    }
  });

  document.getElementById('footer-resume-crawl-btn')?.addEventListener('click', async (e) => {
    e.stopPropagation();
    try {
      await tauri.invoke('resume_crawl');
      await updateCrawlMetrics();
    } catch (err) {
      console.error('Failed to resume crawl:', err);
    }
  });

  document.getElementById('crawler-rescan-all-btn')?.addEventListener('click', async () => {
    const paths = settingsState.watchedFolders.filter(f => f.enabled).map(f => f.path);
    if (paths.length > 0) {
      await tauri.invoke('start_crawl', { paths });
      startCrawlerPolling();
      setStatusText('Re-scanning all watched folders...', true);
    }
  });

  // Danger Zone: Reset Index Modal
  const modal = document.getElementById('reset-modal');
  document.getElementById('reset-index-btn')?.addEventListener('click', () => {
    if (modal) modal.classList.add('active');
  });

  document.getElementById('modal-cancel-btn')?.addEventListener('click', () => {
    if (modal) modal.classList.remove('active');
  });

  document.getElementById('modal-confirm-btn')?.addEventListener('click', async () => {
    if (modal) modal.classList.remove('active');
    try {
      await tauri.invoke('reset_settings');
      localStorage.removeItem('serchx_onboarded');
      await updateCrawlMetrics();
      await loadWatchedFolders();
      showSettingsSavedIndicator('✓ Index Reset');
      setStatusText('Search index reset successfully', true);
    } catch (err) {
      setStatusText(`Reset error: ${err}`, true);
    }
  });

  // Listen for backend system tray open-settings event
  if (window.__TAURI__?.event?.listen) {
    window.__TAURI__.event.listen('open-settings', () => {
      tauri.invoke('open_settings_window').catch(() => {});
    });
  }
}

// Global modal escape handler
document.addEventListener('keydown', (e) => {
  if (e.key === 'Escape') {
    const modal = document.getElementById('reset-modal');
    if (modal && modal.classList.contains('active')) {
      e.stopPropagation();
      modal.classList.remove('active');
    }
  }
});

// --- Standalone Onboarding Window Integration ---
export async function openOnboarding() {
  try {
    await tauri.invoke('open_onboarding_window');
  } catch (err) {
    console.error('Failed to open onboarding window:', err);
  }
}

// Attach global hooks for switchView
window.renderDiffInspector = openDiffInspector;
window.renderSettingsDashboard = openSettingsDashboard;
window.openOnboarding = openOnboarding;

// Initial application boot
document.addEventListener('DOMContentLoaded', () => {
  if (!window.__TAURI__) {
    document.body.classList.add('in-browser');
  }

  // Active window tracking
  document.documentElement.setAttribute('data-window-active', 'true');
  window.addEventListener('focus', () => {
    document.documentElement.setAttribute('data-window-active', 'true');
    if (state.activeView === 'search' && elements.searchInput) {
      elements.searchInput.focus();
    }
  });
  window.addEventListener('blur', () => {
    document.documentElement.removeAttribute('data-window-active');
  });

  setupEventListeners();
  setupWindowDragHandlers();
  setStatusText('Ready');
  startCrawlerPolling();

  tauri.invoke('get_llm_status').then((status) => {
    if (status && status.model_name) {
      updateSynthesisModel(status.provider, status.model_name, status.endpoint, status.ram_usage_bytes, status.model_size_bytes);
    }
  }).catch(() => {});

  function handleHashRoute() {
    const hash = window.location.hash || '';
    if (hash === '#onboarding') {
      switchView('onboarding');
    } else if (hash.startsWith('#search=')) {
      const query = decodeURIComponent(hash.substring(8));
      elements.searchInput.value = query;
      elements.searchInput.dispatchEvent(new Event('input', { bubbles: true }));
    } else if (hash === '#diff') {
      switchView('diff');
    } else if (hash === '#settings') {
      switchView('settings');
    } else {
      switchView('search');
    }
  }

  window.addEventListener('hashchange', handleHashRoute);

  async function checkFirstRun() {
    const hash = window.location.hash || '';
    if (hash === '#onboarding') {
      await tauri.invoke('set_auto_hide', { enabled: false }).catch(() => {});
      await tauri.invoke('hide_window').catch(() => {});
      switchView('onboarding');
      return;
    }
    if (hash && hash !== '#search') {
      handleHashRoute();
      await tauri.invoke('show_window').catch(() => {});
      return;
    }

    const hasOnboarded = localStorage.getItem('serchx_onboarded');
    let isFirstRun = !hasOnboarded;

    if (isFirstRun) {
      try {
        const folders = await tauri.invoke('get_watched_folders');
        if (folders && folders.length > 0) {
          isFirstRun = false;
          localStorage.setItem('serchx_onboarded', 'true');
        }
      } catch (_) {}
    }

    if (isFirstRun) {
      // Keep main window hidden; open the standalone onboarding window
      await tauri.invoke('hide_window').catch(() => {});
      await tauri.invoke('set_auto_hide', { enabled: false }).catch(() => {});
      await tauri.invoke('open_onboarding_window').catch(() => {});
      return;
    }

    switchView('search');
    await tauri.invoke('resize_window', { width: 680, height: 64 }).catch(() => {});
    await tauri.invoke('show_window').catch(() => {});
    await tauri.invoke('set_auto_hide', { enabled: true }).catch(() => {});
  }

  checkFirstRun();
});

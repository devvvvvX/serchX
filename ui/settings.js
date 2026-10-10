/**
 * serchX Standalone Settings Window Controller
 * Coordinates multi-tab macOS settings, neural engine telemetry,
 * watched folders CRUD, crawl control, live indexing queue,
 * filetype analytics accordion, blacklist management, and system resource telemetry.
 */

window.addEventListener('contextmenu', (e) => e.preventDefault(), true);

// Tauri Core IPC Bridge with standalone browser fallback
export const tauri = window.__TAURI__?.core || {
  invoke: async (cmd, args = {}) => {
    console.info(`[Settings Mock IPC] ${cmd}:`, args);
    if (cmd === 'get_settings') {
      return {
        keyword_weight: 0.5,
        vector_weight: 0.5,
        rrf_k: 60,
        max_file_size_bytes: 10485760,
        retention_days: 7,
        auto_hide: true,
      };
    }
    if (cmd === 'get_watched_folders') {
      return [
        {
          id: 'mock-docs',
          path: '/Users/demo/Documents',
          enabled: true,
          recursive: true,
          added_at: Math.floor(Date.now() / 1000) - 86400,
          last_scanned_at: Math.floor(Date.now() / 1000) - 300,
        },
        {
          id: 'mock-docs-c',
          path: '/Users/demo/Documents/Private/Reports',
          enabled: true,
          recursive: true,
          added_at: Math.floor(Date.now() / 1000) - 86400,
          last_scanned_at: Math.floor(Date.now() / 1000) - 600,
        }
      ];
    }
    if (cmd === 'get_blacklist_folders') {
      return [
        {
          id: 'mock-black-1',
          path: '/Users/demo/Documents/Private',
          created_at: Math.floor(Date.now() / 1000) - 86400,
        }
      ];
    }
    if (cmd === 'get_indexing_queue') {
      return {
        is_active: false,
        is_paused: false,
        active_folder: null,
        pending_queue: [],
        current_file: null,
        total_queue_depth: 0,
        recent_events: [],
        elapsed_millis: 0,
      };
    }
    if (cmd === 'get_corpus_analytics') {
      return {
        total_files: 2840,
        total_bytes: 429496729,
        total_chunks: 14200,
        categories: [
          {
            category: 'Documents',
            file_count: 1420,
            total_bytes: 280000000,
            extensions: [
              { extension: 'pdf', file_count: 850, chunk_count: 8500, total_bytes: 190000000, percentage: 67.8 },
              { extension: 'docx', file_count: 320, chunk_count: 3200, total_bytes: 60000000, percentage: 21.4 },
              { extension: 'md', file_count: 250, chunk_count: 1500, total_bytes: 30000000, percentage: 10.7 },
            ]
          },
          {
            category: 'Code',
            file_count: 980,
            total_bytes: 85000000,
            extensions: [
              { extension: 'rs', file_count: 420, chunk_count: 2100, total_bytes: 45000000, percentage: 52.9 },
              { extension: 'js', file_count: 350, chunk_count: 1400, total_bytes: 25000000, percentage: 29.4 },
              { extension: 'py', file_count: 210, chunk_count: 840, total_bytes: 15000000, percentage: 17.6 },
            ]
          },
          {
            category: 'Images & OCR',
            file_count: 440,
            total_bytes: 64496729,
            extensions: [
              { extension: 'png', file_count: 300, chunk_count: 300, total_bytes: 45000000, percentage: 69.8 },
              { extension: 'jpg', file_count: 140, chunk_count: 140, total_bytes: 19496729, percentage: 30.2 },
            ]
          }
        ]
      };
    }
    if (cmd === 'get_storage_breakdown') {
      return {
        original_files_bytes: 429496729,
        sqlite_db_bytes: 48200000,
        text_chunks_bytes: 24100000,
        vector_index_bytes: 18400000,
      };
    }
    if (cmd === 'get_system_telemetry') {
      return {
        memory_rss_mb: 48.6,
        cpu_percent: 1.4,
        db_size_mb: 46.0,
        uptime_seconds: 340,
      };
    }
    if (cmd === 'pick_folder') {
      return '/Users/demo/Downloads';
    }
    if (cmd === 'get_crawl_progress') {
      return {
        is_running: false,
        is_paused: false,
        indexed_count: 1420,
        skipped_count: 48,
        failed_count: 0,
        total_discovered: 1468,
        elapsed_millis: 1240,
        disk_usage_bytes: 48200000,
      };
    }
    if (cmd === 'get_app_logs') {
      return [
        '2026-10-08T07:15:20.124Z  INFO serchx::logging: Rolling logger initialized at ~/.serchx/logs/serchx.log',
        '2026-10-08T07:15:21.050Z  INFO serchx::crawler: CrawlerEngine worker pool started with 8 worker threads',
        '2026-10-08T07:15:22.180Z  INFO serchx::ingest: Fast-path check hit for /Users/demo/Documents/notes.txt (unchanged)',
        '2026-10-08T07:15:23.410Z  INFO serchx::db: Persisted batch of 35 items into SQLite in 2.4ms',
        '2026-10-08T07:15:24.005Z  WARN serchx::safety: Ignored file /Users/demo/Documents/.DS_Store by blacklist filter',
        '2026-10-08T07:15:25.890Z  INFO serchx::vector: Background vector embedder processed 32 chunks',
      ];
    }
    if (cmd === 'export_app_logs') {
      return '/Users/demo/Downloads/serchx_logs_20261008_120000.log';
    }
    if (cmd === 'remove_watched_folder') {
      return {
        files_removed: 0,
        chunks_removed: 0,
        path: args.path || '/mock/path',
      };
    }
    if (cmd === 'purge_folder_index') {
      return 'mock-delete-job-id-123';
    }
    if (cmd === 'reset_database_and_reindex') {
      return {
        files_purged: 2840,
        chunks_purged: 14200,
        reindexing_started: args.reindex !== false && !args.factory_reset,
        factory_reset_applied: !!(args.factory_reset || args.factoryReset),
        message: args.factory_reset || args.factoryReset
          ? 'Factory reset complete (2840 files purged).'
          : 'Database cleaned (2840 files purged). Fresh re-indexing started.',
      };
    }
    return null;
  }
};

// Application State
let currentSettings = {
  keyword_weight: 0.5,
  vector_weight: 0.5,
  rrf_k: 60,
  max_file_size_bytes: 10485760,
  retention_days: 7,
  auto_hide: true,
};

let watchedFolders = [];
let blacklistFolders = [];
let autoSaveTimeout = null;
let crawlPollInterval = null;
let telemetryPollInterval = null;
let isQueueDrawerOpen = false;
let expandedCategories = new Set(['Documents']);

// DOM Elements Cache
const elements = {
  navItems: document.querySelectorAll('.nav-item'),
  contentSections: document.querySelectorAll('.content-section'),
  sectionTitle: document.getElementById('section-title'),
  sectionSubtitle: document.getElementById('section-subtitle'),
  savePill: document.getElementById('save-status-pill'),

  // Header Telemetry Meters
  meterRam: document.getElementById('meter-ram'),
  telemetryItemRam: document.getElementById('telemetry-item-ram'),
  meterMetal: document.getElementById('meter-metal'),
  telemetryItemMetal: document.getElementById('telemetry-item-metal'),
  meterCpu: document.getElementById('meter-cpu'),
  meterStorage: document.getElementById('meter-storage'),

  // Neural Presets
  presetCards: document.querySelectorAll('.preset-card'),

  // Retrieval Sliders
  sliderVectorWeight: document.getElementById('slider-vector-weight'),
  sliderKeywordWeight: document.getElementById('slider-keyword-weight'),
  sliderRrfK: document.getElementById('slider-rrf-k'),
  sliderMaxSize: document.getElementById('slider-max-size'),
  sliderRetention: document.getElementById('slider-retention'),
  valVectorWeight: document.getElementById('val-vector-weight'),
  valKeywordWeight: document.getElementById('val-keyword-weight'),
  valRrfK: document.getElementById('val-rrf-k'),
  valMaxSize: document.getElementById('val-max-size'),
  valRetention: document.getElementById('val-retention'),
  btnResetTuning: document.getElementById('btn-reset-tuning'),

  // LLM Server Configuration
  selectLlmProvider: document.getElementById('select-llm-provider'),
  inputLlmEndpoint: document.getElementById('input-llm-endpoint'),
  btnDetectEndpoints: document.getElementById('btn-detect-endpoints'),
  endpointStatusHint: document.getElementById('llm-endpoint-status-hint'),
  toggleShowThinking: document.getElementById('toggle-show-thinking'),
  selectContextLength: document.getElementById('select-context-length'),

  inputGlobalHotkey: document.getElementById('input-global-hotkey'),
  sliderIndexingThreads: document.getElementById('slider-indexing-threads'),
  valIndexingThreads: document.getElementById('val-indexing-threads'),

  // Watched Folders
  watchedFoldersList: document.getElementById('watched-folders-list'),
  folderCountBadge: document.getElementById('folder-count-badge'),
  btnAddFolder: document.getElementById('btn-add-folder'),
  btnRemoveFolder: document.getElementById('btn-remove-folder'),
  btnRescanAll: document.getElementById('btn-rescan-all'),
  btnPauseCrawl: document.getElementById('btn-pause-crawl'),
  btnResumeCrawl: document.getElementById('btn-resume-crawl'),
  crawlStatusText: document.getElementById('crawl-status-text'),
  crawlStatusDot: document.getElementById('crawl-status-dot'),
  crawlProgressBar: document.getElementById('crawl-progress-bar'),
  crawlProgressBarContainer: document.getElementById('crawl-progress-bar-container'),

  // Indexing Queue Table & Ticker
  btnToggleQueueDrawer: document.getElementById('btn-toggle-queue-drawer'),
  queueDrawerContent: document.getElementById('queue-drawer-content'),
  queueDrawerIcon: document.getElementById('queue-drawer-icon'),
  queueDrawerToggleLabel: document.getElementById('queue-drawer-toggle-label'),
  activeFileTicker: document.getElementById('active-file-ticker'),
  activeFileTickerRow: document.getElementById('active-file-ticker-row'),
  queuePulseDot: document.getElementById('queue-pulse-dot'),
  queueStatusLabel: document.getElementById('queue-status-label'),
  queueProgressBadge: document.getElementById('queue-progress-badge'),
  indexingFoldersTbody: document.getElementById('indexing-folders-tbody'),
  recentStreamTbody: document.getElementById('recent-stream-tbody'),
  queueProgressBarContainer: document.getElementById('queue-progress-bar-container'),
  queueProgressStats: document.getElementById('queue-progress-stats'),
  queueProgressPercent: document.getElementById('queue-progress-percent'),
  queueProgressBarFill: document.getElementById('queue-progress-bar-fill'),

  // Blacklist Management
  blacklistFoldersList: document.getElementById('blacklist-folders-list'),
  blacklistCountBadge: document.getElementById('blacklist-count-badge'),
  btnAddBlacklist: document.getElementById('btn-add-blacklist'),
  btnRemoveBlacklist: document.getElementById('btn-remove-blacklist'),

  // Dead Letter Queue
  dlqCountBadge: document.getElementById('dlq-count-badge'),
  btnRefreshDlq: document.getElementById('btn-refresh-dlq'),
  btnClearDlq: document.getElementById('btn-clear-dlq'),
  dlqItemsList: document.getElementById('dlq-items-list'),

  // Folder Action & Purge Modal
  folderActionModal: document.getElementById('folder-action-modal'),
  modalFolderTitle: document.getElementById('modal-folder-title'),
  modalFolderMessage: document.getElementById('modal-folder-message'),
  modalFolderPath: document.getElementById('modal-folder-path'),
  modalFolderSubtext: document.getElementById('modal-folder-subtext'),
  modalBtnCancel: document.getElementById('modal-btn-cancel'),
  modalBtnKeepIndex: document.getElementById('modal-btn-keep-index'),
  modalBtnPurgeData: document.getElementById('modal-btn-purge-data'),
  modalBtnClose: document.getElementById('modal-btn-close'),

  // Analytics & Storage
  analyticsTotalFiles: document.getElementById('analytics-total-files'),
  analyticsTotalBytes: document.getElementById('analytics-total-bytes'),
  analyticsTotalChunks: document.getElementById('analytics-total-chunks'),
  analyticsTopCategory: document.getElementById('analytics-top-category'),
  analyticsCategoriesAccordion: document.getElementById('analytics-categories-accordion'),
  barStorageFiles: document.getElementById('bar-storage-files'),
  barStorageSqlite: document.getElementById('bar-storage-sqlite'),
  barStorageChunks: document.getElementById('bar-storage-chunks'),
  barStorageVectors: document.getElementById('bar-storage-vectors'),
  legendValFiles: document.getElementById('legend-val-files'),
  legendValSqlite: document.getElementById('legend-val-sqlite'),
  legendValChunks: document.getElementById('legend-val-chunks'),
  legendValVectors: document.getElementById('legend-val-vectors'),

  // System & Hotkeys
  toggleAutoHide: document.getElementById('toggle-auto-hide'),
  btnCenterWindow: document.getElementById('btn-center-window'),
  btnPruneVersions: document.getElementById('btn-prune-versions'),
  btnReindexAll: document.getElementById('btn-reindex-all'),
  btnFactoryReset: document.getElementById('btn-factory-reset'),
  btnCleanDbReindexSources: document.getElementById('btn-clean-db-reindex-sources'),

  // Database Reset Modal
  resetDatabaseModal: document.getElementById('reset-database-modal'),
  resetModalTitle: document.getElementById('reset-modal-title'),
  resetModalDesc: document.getElementById('reset-modal-desc'),
  resetModalSubtext: document.getElementById('reset-modal-subtext'),
  resetModalBtnClose: document.getElementById('reset-modal-btn-close'),
  resetModalBtnCancel: document.getElementById('reset-modal-btn-cancel'),
  resetModalBtnFactory: document.getElementById('reset-modal-btn-factory'),
  resetModalBtnCleanReindex: document.getElementById('reset-modal-btn-clean-reindex'),

  // Diagnostics & Logs
  btnRefreshLogs: document.getElementById('btn-refresh-logs'),
  btnCopyLogs: document.getElementById('btn-copy-logs'),
  btnExportLogs: document.getElementById('btn-export-logs'),
  btnRevealLogs: document.getElementById('btn-reveal-logs'),
  btnClearLogs: document.getElementById('btn-clear-logs'),
  logFilterLevel: document.getElementById('log-filter-level'),
  logSearchInput: document.getElementById('log-search-input'),
  logViewerTerminal: document.getElementById('log-viewer-terminal'),
  logCountTag: document.getElementById('log-count-tag'),
  toggleLogAutoscroll: document.getElementById('toggle-log-autoscroll'),
  logStatusIndicator: document.getElementById('log-status-indicator'),

  // LLM Synthesis Models & Telemetry
  llmHardwareBadge: document.getElementById('llm-hardware-badge'),
  llmTotalDownloadedSize: document.getElementById('llm-total-downloaded-size'),
  llmDownloadedCountTag: document.getElementById('llm-downloaded-count-tag'),
  llmRamConsumptionContainer: document.getElementById('llm-ram-consumption-container'),
  llmRamStatusTag: document.getElementById('llm-ram-status-tag'),
  llmRamConsumptionValue: document.getElementById('llm-ram-consumption-value'),
  btnUnloadModelRam: document.getElementById('btn-unload-model-ram'),
  llmMemoryInfoNote: document.getElementById('llm-memory-info-note'),
  llmMemoryInfoText: document.getElementById('llm-memory-info-text'),
  llmContextLengthTokens: document.getElementById('llm-context-length-tokens'),
  llmContextCacheSize: document.getElementById('llm-context-cache-size'),
  llmRamInfoIcon: document.getElementById('llm-ram-info-icon'),
  llmDownloadContainer: document.getElementById('llm-download-container'),
  llmDownloadTitle: document.getElementById('llm-download-title'),
  llmDownloadStats: document.getElementById('llm-download-stats'),
  llmDownloadBar: document.getElementById('llm-download-bar'),
  llmDownloadEta: document.getElementById('llm-download-eta'),
  btnCancelLlmDownload: document.getElementById('btn-cancel-llm-download'),
  llmModelsList: document.getElementById('llm-models-list'),
  btnRevealModelsDir: document.getElementById('btn-reveal-models-dir'),
  btnImportGguf: document.getElementById('btn-import-gguf'),
  inputImportPath: document.getElementById('input-import-path'),
  btnImportPathSubmit: document.getElementById('btn-import-path-submit'),

  // Model Deletion Modal
  modelDeleteModal: document.getElementById('model-delete-modal'),
  modalModelTitle: document.getElementById('modal-model-title'),
  modalModelMessage: document.getElementById('modal-model-message'),
  modalModelName: document.getElementById('modal-model-name'),
  modalModelSize: document.getElementById('modal-model-size'),
  modalModelBtnClose: document.getElementById('modal-model-btn-close'),
  modalModelBtnCancel: document.getElementById('modal-model-btn-cancel'),
  modalModelBtnConfirm: document.getElementById('modal-model-btn-confirm'),
};

const sectionMetadata = {
  'neural-engine': {
    title: 'Neural Engine & AI',
    subtitle: 'Local neural embeddings and on-device machine intelligence',
  },
  'watched-sources': {
    title: 'Watched Sources & Memory',
    subtitle: 'Real-time filesystem monitoring, queue drawer, and blacklist inheritance',
  },
  'analytics': {
    title: 'Analytics & Storage Footprint',
    subtitle: 'Corpus categorization, extension drilldown, and section storage breakdown',
  },
  'retrieval-tuning': {
    title: 'Retrieval Tuning',
    subtitle: 'Fine-tune hybrid rank fusion, keyword weights, and retention policies',
  },
  'system-hotkeys': {
    title: 'System & Hotkeys',
    subtitle: 'Window behaviors, shortcuts, and index maintenance',
  },
  'diagnostics': {
    title: 'App Logs & Diagnostics',
    subtitle: 'Rolling application logs, system diagnostics, and log bundle export',
  },
};

// Initialize Settings Controller
export async function initSettings() {
  const steps = [
    ['setupNavigation', setupNavigation],
    ['setupSensitivityPresets', setupSensitivityPresets],
    ['setupLlmHandlers', setupLlmHandlers],
    ['setupSliders', setupSliders],
    ['setupWatchedFoldersHandlers', setupWatchedFoldersHandlers],
    ['setupDeadLetterQueueHandlers', setupDeadLetterQueueHandlers],
    ['setupFolderActionModal', setupFolderActionModal],
    ['setupResetDatabaseModal', setupResetDatabaseModal],
    ['setupModelDeleteModal', setupModelDeleteModal],
    ['setupFolderAnalysisModal', setupFolderAnalysisModal],
    ['setupIndexingQueue', setupIndexingQueue],
    ['setupBlacklistHandlers', setupBlacklistHandlers],
    ['setupSystemActions', setupSystemActions],
    ['setupDiagnosticsHandlers', setupDiagnosticsHandlers],
    ['setupKeyboardShortcuts', setupKeyboardShortcuts],
    ['setupTelemetryPolling', setupTelemetryPolling],
    ['initDeleteQueueModal', initDeleteQueueModal],
  ];

  for (const [name, fn] of steps) {
    try {
      fn();
    } catch (err) {
      console.error(`[Settings Init] ${name} failed:`, err);
    }
  }

  const asyncLoaders = [
    ['loadBlacklistFolders', loadBlacklistFolders],
    ['loadWatchedFolders', loadWatchedFolders],
    ['loadDeadLetterQueue', loadDeadLetterQueue],
    ['loadSettings', loadSettings],
    ['loadLlmModels', loadLlmModels],
    ['loadIndexingQueue', loadIndexingQueue],
    ['loadDeleteQueue', loadDeleteQueue],
  ];

  for (const [name, fn] of asyncLoaders) {
    try {
      await fn();
    } catch (err) {
      console.error(`[Settings Init] ${name} failed:`, err);
    }
  }

  try {
    startCrawlProgressPolling();
  } catch (err) {
    console.error('[Settings Init] startCrawlProgressPolling failed:', err);
  }

  // Window drag support for macOS Overlay mode
  document.addEventListener('mousedown', (e) => {
    // Only primary mouse button (left click)
    if (e.button !== 0) return;

    // Do not initiate drag on interactive form controls, links, or buttons
    if (e.target.closest('button, input, select, textarea, a, .nav-item, .toggle-switch, .action-btn, .file-tag-remove, .folder-action-btn, [role="button"]')) {
      return;
    }

    // Check if target is inside any designated drag region
    if (e.target.closest('[data-tauri-drag-region], .main-header, .settings-sidebar, .sidebar-header, .sidebar-footer, .header-titles, .header-right, .brand-row')) {
      if (window.__TAURI__?.window?.getCurrentWebviewWindow) {
        try {
          window.__TAURI__.window.getCurrentWebviewWindow().startDragging();
          return;
        } catch (_) { }
      }
      tauri.invoke('start_dragging', { window_label: 'settings', windowLabel: 'settings' }).catch(() => { });
    }
  });
}

// Section Tab Navigation
function setupNavigation() {
  if (!elements.navItems || elements.navItems.length === 0) {
    elements.navItems = document.querySelectorAll('.nav-item');
  }
  if (!elements.contentSections || elements.contentSections.length === 0) {
    elements.contentSections = document.querySelectorAll('.content-section');
  }

  elements.navItems.forEach((btn) => {
    btn.addEventListener('click', () => {
      const tabId = btn.getAttribute('data-tab');
      switchTab(tabId);
    });
  });
}

function switchTab(tabId) {
  if (!elements.navItems || elements.navItems.length === 0) {
    elements.navItems = document.querySelectorAll('.nav-item');
  }
  if (!elements.contentSections || elements.contentSections.length === 0) {
    elements.contentSections = document.querySelectorAll('.content-section');
  }

  elements.navItems.forEach((item) => {
    item.classList.toggle('active', item.getAttribute('data-tab') === tabId);
  });

  elements.contentSections.forEach((section) => {
    const isTarget = section.id === `view-${tabId}`;
    section.classList.toggle('active', isTarget);
  });

  const meta = sectionMetadata[tabId] || { title: 'Settings', subtitle: '' };
  if (elements.sectionTitle) elements.sectionTitle.textContent = meta.title;
  if (elements.sectionSubtitle) elements.sectionSubtitle.textContent = meta.subtitle;

  // Toggle overflow mode for diagnostics tab (log viewer needs constrained height to flex-grow)
  const mainContent = document.querySelector('.main-content');
  if (mainContent) {
    mainContent.classList.toggle('diagnostics-active', tabId === 'diagnostics');
  }

  if (tabId === 'neural-engine') {
    loadLlmModels().catch(() => { });
  }
  if (tabId === 'watched-sources') {
    loadWatchedFolders();
    loadDeadLetterQueue();
    loadDeleteQueue();
  }
  if (tabId === 'analytics') {
    loadAnalyticsData();
  }
  if (tabId === 'diagnostics') {
    loadAppLogs();
  }
}

// Load Settings from SQLite
async function loadSettings() {
  try {
    const s = await tauri.invoke('get_settings');
    if (s) {
      currentSettings = { ...currentSettings, ...s };
      applySettingsToUI();
    }
  } catch (err) {
    console.error('Failed to load settings:', err);
  }
}

function applySettingsToUI() {
  if (elements.sliderVectorWeight) {
    elements.sliderVectorWeight.value = Math.round(currentSettings.vector_weight * 100);
    elements.valVectorWeight.textContent = `${elements.sliderVectorWeight.value}%`;
  }
  if (elements.sliderKeywordWeight) {
    elements.sliderKeywordWeight.value = Math.round(currentSettings.keyword_weight * 100);
    elements.valKeywordWeight.textContent = `${elements.sliderKeywordWeight.value}%`;
  }
  if (elements.sliderRrfK) {
    elements.sliderRrfK.value = currentSettings.rrf_k;
    elements.valRrfK.textContent = `${currentSettings.rrf_k}`;
  }
  if (elements.sliderMaxSize) {
    const mb = Math.round(currentSettings.max_file_size_bytes / (1024 * 1024));
    elements.sliderMaxSize.value = mb;
    elements.valMaxSize.textContent = `${mb} MB`;
  }
  if (elements.sliderRetention) {
    elements.sliderRetention.value = currentSettings.retention_days;
    elements.valRetention.textContent = `${currentSettings.retention_days} days`;
  }
  if (elements.toggleAutoHide) {
    elements.toggleAutoHide.checked = currentSettings.auto_hide;
  }
  if (elements.inputGlobalHotkey) {
    elements.inputGlobalHotkey.value = currentSettings.global_hotkey || 'Option+Space';
  }
  if (elements.selectLlmProvider) {
    let prov = currentSettings.llm_provider || 'native';
    if (prov === 'hybrid' || prov === 'local') prov = 'native';
    elements.selectLlmProvider.value = prov;
  }
  if (elements.inputLlmEndpoint) {
    elements.inputLlmEndpoint.value = currentSettings.custom_llm_endpoint || 'http://127.0.0.1:1234';
  }
  if (elements.sliderIndexingThreads) {
    elements.sliderIndexingThreads.value = currentSettings.indexing_threads || 4;
    elements.valIndexingThreads.textContent = `${currentSettings.indexing_threads || 4} threads`;
  }
  if (elements.toggleShowThinking) {
    elements.toggleShowThinking.checked = currentSettings.show_thinking_tokens !== false;
  }
  if (elements.selectContextLength) {
    elements.selectContextLength.value = String(currentSettings.context_length || 4096);
  }
  updateMemoryArchitectureNote();
  detectAndHighlightPreset();
  updateEndpointStatus();
}

function getContextWindowMetrics() {
  const tokens = parseInt(currentSettings.context_length || (elements.selectContextLength ? elements.selectContextLength.value : 4096), 10) || 4096;
  const kvCacheBytes = tokens * 128 * 1024;
  return {
    tokens,
    tokensFormatted: formatNumber(tokens),
    cacheBytes: kvCacheBytes,
    cacheFormatted: formatBytes(kvCacheBytes),
  };
}

function updateMemoryArchitectureNote() {
  const { tokensFormatted, cacheFormatted } = getContextWindowMetrics();
  if (elements.llmContextLengthTokens) {
    elements.llmContextLengthTokens.textContent = tokensFormatted;
  }
  if (elements.llmContextCacheSize) {
    elements.llmContextCacheSize.textContent = cacheFormatted;
  }
  if (elements.llmRamInfoIcon) {
    elements.llmRamInfoIcon.title = `Metal Unified Memory reserves the model weights plus ~${cacheFormatted} KV cache capacity (${tokensFormatted} tokens). Live Process RAM only maps physical pages into memory on demand during token generation.`;
  }
}

async function updateEndpointStatus() {
  const hint = elements.endpointStatusHint || document.getElementById('llm-endpoint-status-hint');
  if (!hint) return;
  const prov = currentSettings.llm_provider || 'native';
  if (prov === 'hybrid' || prov === 'native' || prov === 'local') {
    hint.textContent = 'Native Metal on-device inference runner.';
    return;
  }
  try {
    const status = await tauri.invoke('get_llm_status');
    if (status && status.model_name) {
      hint.textContent = `Connected: ${status.provider} (${status.model_name})`;
    } else {
      hint.textContent = `Target endpoint: ${currentSettings.custom_llm_endpoint || 'localhost'}`;
    }
  } catch (_) {
    hint.textContent = 'The local address for the selected provider (localhost only)';
  }
}

// Sliders Event Handlers
function setupSliders() {
  const onSliderChange = (e) => {
    const id = e.target.id;
    const val = parseInt(e.target.value, 10);

    if (id === 'slider-vector-weight') {
      currentSettings.vector_weight = val / 100;
      elements.valVectorWeight.textContent = `${val}%`;
    } else if (id === 'slider-keyword-weight') {
      currentSettings.keyword_weight = val / 100;
      elements.valKeywordWeight.textContent = `${val}%`;
    } else if (id === 'slider-rrf-k') {
      currentSettings.rrf_k = val;
      elements.valRrfK.textContent = `${val}`;
    } else if (id === 'slider-max-size') {
      currentSettings.max_file_size_bytes = val * 1024 * 1024;
      elements.valMaxSize.textContent = `${val} MB`;
    } else if (id === 'slider-retention') {
      currentSettings.retention_days = val;
      elements.valRetention.textContent = `${val} days`;
    }

    detectAndHighlightPreset();
    queueAutoSave();
  };

  [
    elements.sliderVectorWeight,
    elements.sliderKeywordWeight,
    elements.sliderRrfK,
    elements.sliderMaxSize,
    elements.sliderRetention,
  ].forEach((slider) => {
    if (slider) {
      slider.addEventListener('input', onSliderChange);
    }
  });

  if (elements.inputGlobalHotkey) {
    elements.inputGlobalHotkey.addEventListener('change', (e) => {
      currentSettings.global_hotkey = e.target.value;
      queueAutoSave();
    });
  }

  if (elements.selectContextLength) {
    elements.selectContextLength.addEventListener('change', (e) => {
      const val = parseInt(e.target.value, 10);
      currentSettings.context_length = val;
      updateMemoryArchitectureNote();
      loadLlmModels().catch(() => { });
      refreshTelemetry().catch(() => { });
      queueAutoSave();
    });
  }

  if (elements.selectLlmProvider) {
    elements.selectLlmProvider.addEventListener('change', (e) => {
      const prov = e.target.value;
      currentSettings.llm_provider = prov;
      const defaultEndpoints = {
        lm_studio: 'http://127.0.0.1:1234',
        ollama: 'http://127.0.0.1:11434',
        openai_compat: 'http://127.0.0.1:8080',
        anthropic_compat: 'http://127.0.0.1:8080',
      };
      if (defaultEndpoints[prov]) {
        const cur = (currentSettings.custom_llm_endpoint || '').trim();
        const knownDefaults = Object.values(defaultEndpoints);
        if (!cur || knownDefaults.includes(cur)) {
          currentSettings.custom_llm_endpoint = defaultEndpoints[prov];
          if (elements.inputLlmEndpoint) {
            elements.inputLlmEndpoint.value = defaultEndpoints[prov];
          }
        }
      }
      queueAutoSave();
      updateEndpointStatus();
      loadLlmModels().catch(() => { });
    });
  }

  if (elements.inputLlmEndpoint) {
    elements.inputLlmEndpoint.addEventListener('input', (e) => {
      currentSettings.custom_llm_endpoint = e.target.value;
      queueAutoSave();
      updateEndpointStatus();
    });
  }

  if (elements.toggleShowThinking) {
    elements.toggleShowThinking.addEventListener('change', (e) => {
      currentSettings.show_thinking_tokens = e.target.checked;
      queueAutoSave();
    });
  }

  if (elements.btnDetectEndpoints) {
    elements.btnDetectEndpoints.addEventListener('click', async () => {
      const btn = elements.btnDetectEndpoints;
      const originalText = btn.textContent;
      btn.textContent = 'Detecting...';
      try {
        const endpoints = await tauri.invoke('probe_local_endpoints');
        if (endpoints && endpoints.length > 0) {
          const ep = endpoints[0]; // Auto-select the first one found
          elements.selectLlmProvider.value = ep.provider;
          elements.inputLlmEndpoint.value = ep.endpoint;
          currentSettings.llm_provider = ep.provider;
          currentSettings.custom_llm_endpoint = ep.endpoint;
          queueAutoSave();
          updateEndpointStatus();
          showToast(`Found ${ep.provider} at ${ep.endpoint}`);
        } else {
          showToast('No local LLM services detected on common ports.', true);
        }
      } catch (err) {
        showToast('Failed to probe endpoints.', true);
      }
      btn.textContent = originalText;
    });
  }

  if (elements.sliderIndexingThreads) {
    elements.sliderIndexingThreads.addEventListener('input', (e) => {
      elements.valIndexingThreads.textContent = `${e.target.value} threads`;
      currentSettings.indexing_threads = parseInt(e.target.value, 10);
      queueAutoSave();
    });
  }

  if (elements.btnResetTuning) {
    elements.btnResetTuning.addEventListener('click', async () => {
      try {
        await tauri.invoke('reset_settings');
        await loadSettings();
        triggerSaveStatus('✓ Reset to Defaults');
      } catch (err) {
        console.error('Failed to reset settings:', err);
      }
    });
  }
}

// Preset Cards Integration
function setupSensitivityPresets() {
  elements.presetCards.forEach((card) => {
    card.addEventListener('click', () => {
      const preset = card.getAttribute('data-preset');
      applyPreset(preset);
    });
  });
}

function applyPreset(preset) {
  if (preset === 'balanced') {
    currentSettings.vector_weight = 0.5;
    currentSettings.keyword_weight = 0.5;
    currentSettings.rrf_k = 60;
  } else if (preset === 'light') {
    currentSettings.vector_weight = 0.2;
    currentSettings.keyword_weight = 0.8;
    currentSettings.rrf_k = 40;
  } else if (preset === 'power') {
    currentSettings.vector_weight = 0.8;
    currentSettings.keyword_weight = 0.2;
    currentSettings.rrf_k = 80;
  }

  applySettingsToUI();
  queueAutoSave();
}

function detectAndHighlightPreset() {
  const vec = Math.round(currentSettings.vector_weight * 10) / 10;
  const kw = Math.round(currentSettings.keyword_weight * 10) / 10;
  const k = currentSettings.rrf_k;

  let activePreset = 'custom';
  if (vec === 0.5 && kw === 0.5 && k === 60) {
    activePreset = 'balanced';
  } else if (vec === 0.2 && kw === 0.8 && k === 40) {
    activePreset = 'light';
  } else if (vec === 0.8 && kw === 0.2 && k === 80) {
    activePreset = 'power';
  }

  elements.presetCards.forEach((card) => {
    card.classList.toggle('active', card.getAttribute('data-preset') === activePreset);
  });
}

// Local LLM Models & Downloader Handlers
let currentDownloadingModelId = null;
let modelDownloadPollInterval = null;

function renderDownloadProgress(p) {
  if (!p) return;
  const btnPause = document.getElementById('btn-pause-llm-download');
  const btnResume = document.getElementById('btn-resume-llm-download');
  const downloadBar = elements.llmDownloadBar;

  if (p.status === 'connecting') {
    currentDownloadingModelId = p.model_id;
    if (elements.llmDownloadContainer) elements.llmDownloadContainer.style.display = 'block';
    if (elements.llmDownloadTitle) elements.llmDownloadTitle.textContent = `Connecting to ${p.model_id}...`;
    if (elements.llmDownloadStats) elements.llmDownloadStats.textContent = 'Connecting to Hugging Face...';
    if (downloadBar) {
      downloadBar.style.width = p.percentage > 0 ? `${p.percentage}%` : '5%';
      downloadBar.style.background = 'var(--accent)';
    }
    if (elements.llmDownloadEta) elements.llmDownloadEta.textContent = 'Waiting for server response...';
    if (btnPause) btnPause.style.display = 'none';
    if (btnResume) btnResume.style.display = 'none';
    if (elements.btnCancelLlmDownload) elements.btnCancelLlmDownload.style.display = 'inline-flex';
  } else if (p.status === 'downloading' || p.status === 'verifying') {
    currentDownloadingModelId = p.model_id;
    if (elements.llmDownloadContainer) elements.llmDownloadContainer.style.display = 'block';
    if (elements.llmDownloadTitle) elements.llmDownloadTitle.textContent = p.status === 'verifying' ? `Verifying ${p.model_id}...` : `Downloading ${p.model_id}...`;
    if (elements.llmDownloadStats) elements.llmDownloadStats.textContent = `${p.percentage.toFixed(1)}% • ${p.speed_mbps.toFixed(1)} MB/s`;
    if (downloadBar) {
      downloadBar.style.width = `${Math.max(p.percentage, 2)}%`;
      downloadBar.style.background = 'var(--accent)';
    }
    if (elements.llmDownloadEta) {
      if (p.status === 'verifying') {
        elements.llmDownloadEta.textContent = 'Verifying SHA-256 integrity...';
      } else {
        const eta = p.eta_seconds;
        const etaStr = eta > 60
          ? `${Math.floor(eta / 60)}m ${eta % 60}s`
          : `${eta}s`;
        const dlMb = (p.bytes_downloaded / (1024 * 1024)).toFixed(1);
        const totalMb = p.total_bytes > 1e9
          ? `${(p.total_bytes / 1e9).toFixed(2)} GB`
          : `${(p.total_bytes / (1024 * 1024)).toFixed(0)} MB`;
        elements.llmDownloadEta.textContent = `${dlMb} MB / ${totalMb} — ETA: ${etaStr}`;
      }
    }
    if (btnPause) btnPause.style.display = p.status === 'verifying' ? 'none' : 'inline-flex';
    if (btnResume) btnResume.style.display = 'none';
    if (elements.btnCancelLlmDownload) elements.btnCancelLlmDownload.style.display = 'inline-flex';
  } else if (p.status === 'paused') {
    stopModelDownloadPolling();
    currentDownloadingModelId = p.model_id;
    if (elements.llmDownloadContainer) elements.llmDownloadContainer.style.display = 'block';
    if (elements.llmDownloadTitle) elements.llmDownloadTitle.textContent = `Paused — ${p.model_id}`;
    if (elements.llmDownloadStats) elements.llmDownloadStats.textContent = `${p.percentage.toFixed(1)}% • Paused`;
    if (downloadBar) {
      downloadBar.style.width = `${p.percentage}%`;
      downloadBar.style.background = '#ff9f0a';
    }
    if (elements.llmDownloadEta) elements.llmDownloadEta.textContent = 'Download paused';
    if (btnPause) btnPause.style.display = 'none';
    if (btnResume) btnResume.style.display = 'inline-flex';
    if (elements.btnCancelLlmDownload) elements.btnCancelLlmDownload.style.display = 'inline-flex';
  } else if (p.status === 'complete') {
    stopModelDownloadPolling();
    currentDownloadingModelId = null;
    if (downloadBar) {
      downloadBar.style.width = '100%';
      downloadBar.style.background = '#34c759';
    }
    if (elements.llmDownloadStats) elements.llmDownloadStats.textContent = '100% • Complete';
    if (elements.llmDownloadEta) elements.llmDownloadEta.textContent = 'Verification passed ✓';
    if (btnPause) btnPause.style.display = 'none';
    if (btnResume) btnResume.style.display = 'none';
    if (elements.btnCancelLlmDownload) elements.btnCancelLlmDownload.style.display = 'none';
    showToast('Model download complete ✓');
    setTimeout(() => {
      if (elements.llmDownloadContainer) elements.llmDownloadContainer.style.display = 'none';
      loadLlmModels();
    }, 1200);
  } else if (p.status === 'error' || p.status === 'cancelled') {
    stopModelDownloadPolling();
    currentDownloadingModelId = null;
    if (elements.llmDownloadEta) elements.llmDownloadEta.textContent = p.error_message || 'Download stopped';
    if (btnPause) btnPause.style.display = 'none';
    if (btnResume) btnResume.style.display = 'none';
    if (elements.btnCancelLlmDownload) elements.btnCancelLlmDownload.style.display = 'none';
    if (p.status === 'error' && p.error_message) {
      showToast(`Download error: ${p.error_message}`, true);
    }
    setTimeout(() => {
      if (elements.llmDownloadContainer) elements.llmDownloadContainer.style.display = 'none';
      loadLlmModels();
    }, 2500);
  }
}

function startModelDownloadPolling(modelId) {
  if (modelDownloadPollInterval) clearInterval(modelDownloadPollInterval);
  modelDownloadPollInterval = setInterval(async () => {
    try {
      const progress = await tauri.invoke('get_model_download_progress', { modelId });
      if (progress) {
        renderDownloadProgress(progress);
      }
    } catch (_) { }
  }, 200);
}

function stopModelDownloadPolling() {
  if (modelDownloadPollInterval) {
    clearInterval(modelDownloadPollInterval);
    modelDownloadPollInterval = null;
  }
}

function setupLlmHandlers() {
  if (elements.btnCancelLlmDownload) {
    elements.btnCancelLlmDownload.addEventListener('click', async () => {
      if (currentDownloadingModelId) {
        stopModelDownloadPolling();
        try {
          await tauri.invoke('cancel_model_download', { modelId: currentDownloadingModelId });
          showToast('Model download cancelled');
        } catch (e) {
          console.error('Failed to cancel download:', e);
        }
      }
    });
  }

  // Pause button handler
  const btnPause = document.getElementById('btn-pause-llm-download');
  if (btnPause) {
    btnPause.addEventListener('click', async () => {
      if (currentDownloadingModelId) {
        // Optimistically toggle UI immediately so user gets instant feedback
        btnPause.style.display = 'none';
        const btnResumeEl = document.getElementById('btn-resume-llm-download');
        if (btnResumeEl) btnResumeEl.style.display = 'inline-flex';
        if (elements.llmDownloadTitle) elements.llmDownloadTitle.textContent = `Paused — ${currentDownloadingModelId}`;
        if (elements.llmDownloadStats) {
          const bar = elements.llmDownloadBar;
          const pct = bar ? parseFloat(bar.style.width) || 0 : 0;
          elements.llmDownloadStats.textContent = `${pct.toFixed(1)}% • Paused`;
        }
        if (elements.llmDownloadEta) elements.llmDownloadEta.textContent = 'Download paused';
        if (elements.llmDownloadBar) elements.llmDownloadBar.style.background = '#ff9f0a';
        stopModelDownloadPolling();
        try {
          await tauri.invoke('pause_model_download', { modelId: currentDownloadingModelId });
        } catch (e) {
          console.error('Failed to pause download:', e);
          // Revert UI on failure
          btnPause.style.display = 'inline-flex';
          if (btnResumeEl) btnResumeEl.style.display = 'none';
          startModelDownloadPolling(currentDownloadingModelId);
        }
      }
    });
  }

  // Resume button handler
  const btnResume = document.getElementById('btn-resume-llm-download');
  if (btnResume) {
    btnResume.addEventListener('click', async () => {
      if (currentDownloadingModelId) {
        // Optimistically toggle UI immediately
        btnResume.style.display = 'none';
        const btnPauseEl = document.getElementById('btn-pause-llm-download');
        if (btnPauseEl) btnPauseEl.style.display = 'inline-flex';
        if (elements.llmDownloadTitle) elements.llmDownloadTitle.textContent = `Resuming ${currentDownloadingModelId}...`;
        if (elements.llmDownloadEta) elements.llmDownloadEta.textContent = 'Reconnecting...';
        if (elements.llmDownloadBar) elements.llmDownloadBar.style.background = 'var(--accent)';
        startModelDownloadPolling(currentDownloadingModelId);
        try {
          await tauri.invoke('resume_model_download', { modelId: currentDownloadingModelId });
        } catch (e) {
          console.error('Failed to resume download:', e);
          showToast(`Resume failed: ${e}`, true);
          // Revert UI on failure
          btnResume.style.display = 'inline-flex';
          if (btnPauseEl) btnPauseEl.style.display = 'none';
          stopModelDownloadPolling();
        }
      }
    });
  }

  if (elements.btnRevealModelsDir) {
    elements.btnRevealModelsDir.addEventListener('click', async () => {
      try {
        await tauri.invoke('reveal_models_dir_in_finder');
      } catch (e) {
        console.error('Failed to reveal models dir:', e);
      }
    });
  }

  if (elements.btnImportGguf) {
    elements.btnImportGguf.addEventListener('click', async () => {
      try {
        const pickedPath = await tauri.invoke('pick_model_file_or_folder');
        if (pickedPath) {
          showToast('Importing model...');
          const item = await tauri.invoke('import_custom_model', { sourcePath: pickedPath });
          showToast(`✓ Imported ${item.display_name}`);
          await loadLlmModels();
        }
      } catch (err) {
        showToast(`Import failed: ${err}`, true);
      }
    });
  }

  const handleManualImport = async () => {
    const raw = elements.inputImportPath?.value?.trim();
    if (!raw) {
      showToast('Please enter a valid .gguf file or folder path', true);
      return;
    }
    try {
      showToast('Importing model...');
      const item = await tauri.invoke('import_custom_model', { sourcePath: raw });
      showToast(`✓ Imported ${item.display_name}`);
      if (elements.inputImportPath) elements.inputImportPath.value = '';
      await loadLlmModels();
    } catch (err) {
      showToast(`Import failed: ${err}`, true);
    }
  };

  elements.btnImportPathSubmit?.addEventListener('click', handleManualImport);
  elements.inputImportPath?.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') {
      e.preventDefault();
      handleManualImport();
    }
  });

  if (window.__TAURI__?.event?.listen) {
    window.__TAURI__.event.listen('llm-download-progress', (event) => {
      renderDownloadProgress(event.payload);
    });
    window.__TAURI__.event.listen('llm-status-changed', () => {
      refreshTelemetry().catch(() => { });
      loadLlmModels().catch(() => { });
    });
    window.__TAURI__.event.listen('llm-done', () => {
      refreshTelemetry().catch(() => { });
      loadLlmModels().catch(() => { });
    });
    window.__TAURI__.event.listen('llm-start', () => {
      refreshTelemetry().catch(() => { });
      loadLlmModels().catch(() => { });
    });
    window.__TAURI__.event.listen('navigate-settings-tab', (event) => {
      const payload = event.payload || {};
      const targetTab = typeof payload === 'string' ? payload : (payload.tab || 'neural-engine');
      switchTab(targetTab);
      setTimeout(() => {
        if (payload.sub === 'providers') {
          const el = document.getElementById('select-llm-provider');
          if (el) el.scrollIntoView({ behavior: 'smooth', block: 'center' });
        } else if (payload.sub === 'models') {
          const el = document.getElementById('llm-models-list');
          if (el) el.scrollIntoView({ behavior: 'smooth', block: 'center' });
        }
      }, 50);
    });
  }
}

async function loadLlmModels() {
  try {
    const profile = await tauri.invoke('get_hardware_profile');
    if (profile && elements.llmHardwareBadge) {
      elements.llmHardwareBadge.textContent = `${profile.chip_name} • ${profile.total_memory_gb} GB — Rec: ${profile.recommended_tier}`;
    }

    const models = await tauri.invoke('get_model_catalog');
    const status = await tauri.invoke('get_llm_status').catch(() => null);
    const provider = currentSettings.llm_provider || 'native';
    const isLocalRunner = (provider === 'native' || provider === 'hybrid' || provider === 'local');

    // Total models downloaded sizes at top
    const downloadedModels = models ? models.filter((m) => m.is_downloaded) : [];
    const totalDownloadedBytes = downloadedModels.reduce((acc, m) => acc + (m.download_size_bytes || 0), 0);
    if (elements.llmTotalDownloadedSize) {
      elements.llmTotalDownloadedSize.textContent = formatBytes(totalDownloadedBytes);
    }
    if (elements.llmDownloadedCountTag) {
      elements.llmDownloadedCountTag.textContent = `${downloadedModels.length} downloaded`;
    }

    // RAM consumption by locally loaded model
    // Display by default for local runner (native or hybrid), even if model is not yet loaded
    if (elements.llmRamConsumptionContainer) {
      if (isLocalRunner) {
        elements.llmRamConsumptionContainer.style.display = 'flex';
        const isLoaded = Boolean(status && status.is_loaded);
        const ramBytes = (isLoaded && status && status.ram_usage_bytes) ? status.ram_usage_bytes : null;
        const footprintBytes = (status && status.model_size_bytes) || null;

        if (elements.llmRamStatusTag) {
          if (isLoaded) {
            elements.llmRamStatusTag.textContent = 'In Memory';
            elements.llmRamStatusTag.style.background = 'rgba(52, 199, 89, 0.15)';
            elements.llmRamStatusTag.style.color = '#34c759';
          } else {
            elements.llmRamStatusTag.textContent = 'On Demand';
            elements.llmRamStatusTag.style.background = 'rgba(142, 142, 147, 0.15)';
            elements.llmRamStatusTag.style.color = 'var(--label-2)';
          }
        }

        const { tokensFormatted, cacheFormatted } = getContextWindowMetrics();
        if (elements.llmRamConsumptionValue) {
          if (isLoaded && ramBytes) {
            elements.llmRamConsumptionValue.textContent = `${formatBytes(ramBytes)} (Metal GPU)`;
            elements.llmRamConsumptionValue.style.color = '#34c759';
            elements.llmRamConsumptionValue.title = `Model weights + ${tokensFormatted} KV cache buffer (${formatBytes(ramBytes)}) allocated on Apple Silicon Unified Memory. Empty cache pages are mapped to RAM on demand during synthesis.`;
          } else if (footprintBytes) {
            elements.llmRamConsumptionValue.textContent = `${formatBytes(footprintBytes)} (on demand)`;
            elements.llmRamConsumptionValue.style.color = 'var(--label)';
            elements.llmRamConsumptionValue.title = `Estimated ${formatBytes(footprintBytes)} (weights + ~${cacheFormatted} KV cache for ${tokensFormatted} tokens) will be allocated in Unified Memory upon load or inference.`;
          } else {
            elements.llmRamConsumptionValue.textContent = '0 B (on demand)';
            elements.llmRamConsumptionValue.style.color = 'var(--label)';
            elements.llmRamConsumptionValue.title = 'No active local model selected.';
          }
        }

        if (elements.btnUnloadModelRam) {
          elements.btnUnloadModelRam.disabled = false;
          if (isLoaded) {
            elements.btnUnloadModelRam.textContent = 'Unload';
            elements.btnUnloadModelRam.title = 'Unload active model from memory';
          } else {
            elements.btnUnloadModelRam.textContent = 'Load';
            elements.btnUnloadModelRam.title = 'Load active model into memory';
          }
        }
      } else {
        elements.llmRamConsumptionContainer.style.display = 'none';
      }
    }

    updateMemoryArchitectureNote();
    if (elements.llmMemoryInfoNote) {
      elements.llmMemoryInfoNote.style.display = isLocalRunner ? 'flex' : 'none';
    }

    if (models && elements.llmModelsList) {
      elements.llmModelsList.innerHTML = '';
      models.forEach((m) => {
        const item = document.createElement('div');
        item.style.cssText = 'display: flex; align-items: center; justify-content: space-between; padding: 10px 12px; border-bottom: 0.5px solid var(--separator);';

        const isLmStudio = m.id.startsWith('lmstudio') || m.display_name.startsWith('LM Studio');
        const isOllama = m.id.startsWith('ollama') || m.display_name.startsWith('Ollama');
        const isApiOnly = m.quantization === 'API' || m.download_size_bytes === 0;

        let sourceBadge = '';
        if (isLmStudio) {
          sourceBadge = '<span class="telemetry-tag" style="background: rgba(175, 82, 222, 0.15); color: #bf5af2; font-size: 10px; padding: 1px 6px;">LM Studio</span>';
        } else if (isOllama) {
          sourceBadge = '<span class="telemetry-tag" style="background: rgba(255, 149, 0, 0.15); color: #ff9500; font-size: 10px; padding: 1px 6px;">Ollama</span>';
        } else if (m.is_custom) {
          sourceBadge = '<span class="telemetry-tag" style="background: rgba(255, 149, 0, 0.15); color: #ff9500; font-size: 10px;">Custom</span>';
        }

        const sizeStr = isApiOnly
          ? 'Connected via API'
          : (m.download_size_bytes > 1000000000
            ? `${(m.download_size_bytes / 1000000000).toFixed(2)} GB`
            : `${(m.download_size_bytes / (1024 * 1024)).toFixed(0)} MB`);

        const isInRam = m.is_active && isLocalRunner && status && status.is_loaded;
        const ramText = isInRam && status.ram_usage_bytes ? `In RAM (${formatBytes(status.ram_usage_bytes)})` : 'In RAM';
        const onDemandText = (m.is_active && isLocalRunner && !isInRam && status && status.model_size_bytes)
          ? `On Demand (${formatBytes(status.model_size_bytes)})`
          : null;

        item.innerHTML = `
          <div style="display: flex; flex-direction: column; gap: 2px;">
            <div style="display: flex; align-items: center; gap: 8px;">
              <span style="font-size: 13px; font-weight: 500; color: var(--label);">${escapeHtml(m.display_name)}</span>
              <span class="telemetry-tag" style="font-size: 10px; padding: 1px 6px;">${escapeHtml(m.quantization)}</span>
              ${sourceBadge}
              ${isInRam ? `<span class="telemetry-tag" style="background: rgba(52, 199, 89, 0.15); color: #34c759; font-size: 10px; padding: 1px 6px;">${ramText}</span>` : (onDemandText ? `<span class="telemetry-tag" style="background: rgba(142, 142, 147, 0.15); color: var(--label-2); font-size: 10px; padding: 1px 6px;">${onDemandText}</span>` : '')}
            </div>
            <span style="font-size: 11px; color: var(--label-2);">${escapeHtml(m.parameter_size)} • ${sizeStr}</span>
          </div>
          <div style="display: flex; align-items: center; gap: 8px;">
            ${m.is_downloaded
            ? (m.is_active
              ? '<span class="brand-badge" style="background: rgba(52, 199, 89, 0.2); color: #34c759; margin: 0;">Active</span>'
              : `<button class="btn btn-secondary btn-set-active" data-id="${m.id}" style="height: 24px; padding: 0 8px; font-size: 11px;">Set Active</button>`)
            : `<button class="btn btn-secondary btn-download-model" data-id="${m.id}" style="height: 24px; padding: 0 8px; font-size: 11px;">Download</button>`
          }
            ${(m.is_downloaded && !isApiOnly)
            ? `<button class="btn btn-danger btn-delete-model" data-id="${m.id}" style="height: 24px; padding: 0 8px; font-size: 11px;" title="Delete model to reclaim disk space">Delete</button>`
            : ''
          }
          </div>
        `;

        const btnDownload = item.querySelector('.btn-download-model');
        if (btnDownload) {
          btnDownload.addEventListener('click', async () => {
            btnDownload.disabled = true;
            btnDownload.textContent = 'Starting...';

            currentDownloadingModelId = m.id;
            startModelDownloadPolling(m.id);
            if (elements.llmDownloadContainer) elements.llmDownloadContainer.style.display = 'block';
            if (elements.llmDownloadTitle) elements.llmDownloadTitle.textContent = `Connecting to download ${escapeHtml(m.display_name)}...`;
            if (elements.llmDownloadStats) elements.llmDownloadStats.textContent = 'Establishing connection...';
            if (elements.llmDownloadBar) {
              elements.llmDownloadBar.style.width = '4%';
              elements.llmDownloadBar.style.background = 'var(--accent)';
            }
            if (elements.llmDownloadEta) elements.llmDownloadEta.textContent = 'Waiting for server response...';
            const btnPauseEl = document.getElementById('btn-pause-llm-download');
            const btnResumeEl = document.getElementById('btn-resume-llm-download');
            if (btnPauseEl) btnPauseEl.style.display = 'none';
            if (btnResumeEl) btnResumeEl.style.display = 'none';
            if (elements.btnCancelLlmDownload) elements.btnCancelLlmDownload.style.display = 'inline-flex';

            try {
              await tauri.invoke('start_model_download', { modelId: m.id });
            } catch (err) {
              showToast(`Download failed: ${err}`, true);
              stopModelDownloadPolling();
              currentDownloadingModelId = null;
              if (elements.llmDownloadContainer) elements.llmDownloadContainer.style.display = 'none';
              btnDownload.disabled = false;
              btnDownload.textContent = 'Download';
            }
          });
        }

        const btnSetActive = item.querySelector('.btn-set-active');
        if (btnSetActive) {
          btnSetActive.addEventListener('click', async () => {
            try {
              await tauri.invoke('set_active_model', { modelId: m.id });
              currentSettings.active_llm_model = m.id;
              showToast(`Activated ${m.display_name}`);
              await loadLlmModels();
            } catch (err) {
              showToast(`Activation failed: ${err}`, true);
            }
          });
        }

        const btnDelete = item.querySelector('.btn-delete-model');
        if (btnDelete) {
          btnDelete.addEventListener('click', () => {
            openModelDeleteModal(m, sizeStr);
          });
        }

        elements.llmModelsList.appendChild(item);
      });

      // Check if any non-downloaded model currently has active download progress in background
      for (const m of models) {
        if (!m.is_downloaded) {
          try {
            const prog = await tauri.invoke('get_model_download_progress', { modelId: m.id });
            if (prog && (prog.status === 'downloading' || prog.status === 'connecting' || prog.status === 'verifying' || prog.status === 'paused')) {
              currentDownloadingModelId = m.id;
              renderDownloadProgress(prog);
              if (prog.status !== 'paused') {
                startModelDownloadPolling(m.id);
              }
              break;
            }
          } catch (_) { }
        }
      }
    }
  } catch (err) {
    console.error('Failed to load LLM models:', err);
  }
}

// Debounced SQLite Auto-Save
function queueAutoSave() {
  if (autoSaveTimeout) {
    clearTimeout(autoSaveTimeout);
  }

  autoSaveTimeout = setTimeout(async () => {
    try {
      await tauri.invoke('update_settings', { settings: currentSettings });
      triggerSaveStatus('✓ Saved to SQLite');
    } catch (err) {
      console.error('Failed to auto-save settings:', err);
    }
  }, 400);
}

function triggerSaveStatus(text) {
  if (!elements.savePill) return;
  elements.savePill.textContent = text;
  elements.savePill.classList.add('visible');
  setTimeout(() => {
    elements.savePill.classList.remove('visible');
  }, 2200);
}

function showToast(text, isError = false) {
  triggerSaveStatus(isError ? '❌ ' + text : text);
}

// Watched Folders Management
async function loadWatchedFolders() {
  try {
    const list = await tauri.invoke('get_watched_folders');
    watchedFolders = list || [];
    renderWatchedFolders();
  } catch (err) {
    console.error('Failed to load watched folders:', err);
  }
}

function isPathInsideAncestor(childPath, ancestorPath) {
  if (childPath === ancestorPath) return false;
  const cleanChild = childPath.endsWith('/') ? childPath : childPath + '/';
  const cleanAncestor = ancestorPath.endsWith('/') ? ancestorPath : ancestorPath + '/';
  return cleanChild.startsWith(cleanAncestor);
}

function isSamePath(a, b) {
  if (!a || !b) return false;
  return a.replace(/\/+$/, '') === b.replace(/\/+$/, '');
}

function renderWatchedFolders() {
  if (!elements.watchedFoldersList) return;
  elements.watchedFoldersList.innerHTML = '';
  elements.folderCountBadge.textContent = `${watchedFolders.length} folder${watchedFolders.length === 1 ? '' : 's'}`;

  if (watchedFolders.length === 0) {
    elements.watchedFoldersList.innerHTML = `
      <div style="padding: 24px; text-align: center; color: var(--text-muted); font-size: 13px;">
        No folders configured. Click <strong>+ Add Folder</strong> to start monitoring your files.
      </div>
    `;
    return;
  }

  watchedFolders.forEach((folder) => {
    const card = document.createElement('div');
    card.className = 'folder-card';

    const lastScannedText = folder.last_scanned_at
      ? `Scanned ${formatRelativeTime(folder.last_scanned_at)}`
      : 'Queued for scan';

    // Blacklist override check (D-08)
    const overridingBlacklist = blacklistFolders.find((b) => isPathInsideAncestor(folder.path, b.path));
    const overrideBadgeHtml = overridingBlacklist
      ? `<span class="badge-override" title="Parent directory '${escapeHtml(overridingBlacklist.path)}' is blacklisted. This explicit watched folder overrides the parent and is indexed.">⚡ Exception Override</span>`
      : '';

    card.innerHTML = `
      <div class="folder-info">
        <span class="folder-icon">📂</span>
        <div class="folder-details">
          <div style="display: flex; align-items: center; gap: 8px; flex-wrap: wrap;">
            <span class="folder-path" title="${escapeHtml(folder.path)}">${escapeHtml(folder.path)}</span>
            ${overrideBadgeHtml}
          </div>
          <div class="folder-meta">
            <span class="folder-status">
              <span class="status-dot ${folder.enabled ? '' : 'inactive'}" style="width: 5px; height: 5px; background: ${folder.enabled ? 'var(--success)' : 'var(--text-muted)'};"></span>
              ${folder.enabled ? 'Active & Indexed' : 'Monitoring Disabled'}
            </span>
            <span>•</span>
            <span>${lastScannedText}</span>
          </div>
        </div>
      </div>
      <div class="folder-actions" style="display: flex; align-items: center; gap: 6px;">
        <button class="mac-btn mac-btn-primary btn-start-card-index" data-path="${escapeHtml(folder.path)}" style="height: 24px; padding: 0 9px;" title="Start indexing this folder">
          <svg width="8" height="9" viewBox="0 0 8 9" fill="currentColor" aria-hidden="true"><path d="M1 1.2a.5.5 0 0 1 .76-.43l5.5 3.3a.5.5 0 0 1 0 .86l-5.5 3.3A.5.5 0 0 1 1 7.8V1.2z"/></svg>
          <span>Index</span>
        </button>
        <button class="mac-btn mac-btn-secondary btn-inspect-folder-breakdown" data-path="${escapeHtml(folder.path)}" style="height: 24px; padding: 0 9px;" title="View folder contents breakdown">
          <svg width="10" height="10" viewBox="0 0 12 12" fill="currentColor" aria-hidden="true"><path d="M1 10.5h10v1H0V0.5h1v10zm2.5-3h1.5v2.5H3.5V7.5zm3-3.5h1.5V10H6.5V4zm3-2.5H11V10H9.5V1.5z"/></svg>
          <span>Breakdown</span>
        </button>
        <button class="mac-btn mac-btn-secondary btn-clear-folder-index" data-id="${folder.id}" style="height: 24px; padding: 0 9px;" title="Clear indexed data for this folder">
          Clear Index
        </button>
        <label class="switch">
          <input type="checkbox" class="toggle-folder-switch" data-id="${folder.id}" ${folder.enabled ? 'checked' : ''}>
          <span class="switch-slider"></span>
        </label>
        <button class="btn-icon-only btn-remove-folder" data-id="${folder.id}" title="Remove folder">
          ✕
        </button>
      </div>
    `;

    // Start index listener
    const startIndexBtn = card.querySelector('.btn-start-card-index');
    if (startIndexBtn) {
      startIndexBtn.addEventListener('click', async () => {
        try {
          await tauri.invoke('trigger_folder_scan', { path: folder.path });
          triggerSaveStatus('⚡ Indexing started');
          await loadIndexingQueue();
        } catch (err) {
          console.error('Failed to trigger folder scan:', err);
        }
      });
    }

    // Breakdown inspector listener
    const inspectBtn = card.querySelector('.btn-inspect-folder-breakdown');
    if (inspectBtn) {
      inspectBtn.addEventListener('click', () => {
        openFolderAnalysisInspector(folder.path);
      });
    }

    // Toggle listener
    const toggleInput = card.querySelector('.toggle-folder-switch');
    toggleInput.addEventListener('change', async (e) => {
      const enabled = e.target.checked;
      try {
        await tauri.invoke('toggle_watched_folder', { id: folder.id, enabled });
        folder.enabled = enabled;
        renderWatchedFolders();
        triggerSaveStatus('✓ Watched folder updated');
      } catch (err) {
        console.error('Failed to toggle folder:', err);
      }
    });

    // Clear Index listener
    const clearBtn = card.querySelector('.btn-clear-folder-index');
    if (clearBtn) {
      clearBtn.addEventListener('click', () => {
        openPurgeFolderModal(folder);
      });
    }

    // Remove listener
    const removeBtn = card.querySelector('.btn-remove-folder');
    if (removeBtn) {
      removeBtn.addEventListener('click', () => {
        openRemoveFolderModal(folder);
      });
    }

    elements.watchedFoldersList.appendChild(card);
  });
}

// Dead Letter Queue (Failed Files) Management
let deadLetterItems = [];

async function loadDeadLetterQueue() {
  try {
    const list = await tauri.invoke('get_dead_letter_queue');
    deadLetterItems = list || [];
    renderDeadLetterQueue();
  } catch (err) {
    console.error('Failed to load dead letter queue:', err);
  }
}

function renderDeadLetterQueue() {
  if (!elements.dlqItemsList) return;
  elements.dlqItemsList.innerHTML = '';
  const count = deadLetterItems.length;

  if (elements.dlqCountBadge) {
    elements.dlqCountBadge.textContent = `${count} failed`;
    elements.dlqCountBadge.style.color = count > 0 ? '#ff453a' : 'var(--label-2)';
    elements.dlqCountBadge.style.background = count > 0 ? 'rgba(255, 69, 58, 0.15)' : 'var(--fill-tertiary)';
  }

  if (count === 0) {
    elements.dlqItemsList.innerHTML = `
      <div class="queue-empty-text" style="padding: 16px; text-align: center; color: var(--label-2); font-size: 12px;">
        ✓ No failed files. All indexed sources synchronized.
      </div>
    `;
    return;
  }

  deadLetterItems.forEach((item) => {
    const card = document.createElement('div');
    card.className = 'dlq-item-card';

    const isTimeout = item.error_type === 'Timeout';
    const pillClass = isTimeout ? 'dlq-pill-timeout' : 'dlq-pill-error';
    const relativeTime = item.failed_at ? formatRelativeTime(item.failed_at * 1000) : 'Recently';

    card.innerHTML = `
      <div class="dlq-info">
        <span style="font-size: 16px; flex-shrink: 0;">${isTimeout ? '⏱️' : '⚠️'}</span>
        <div class="dlq-details">
          <span class="dlq-path" title="${escapeHtml(item.path)}">${escapeHtml(item.file_name || item.path)}</span>
          <div class="dlq-meta">
            <span class="${pillClass}">${escapeHtml(item.error_type)}</span>
            <span title="${escapeHtml(item.error_message)}" style="max-width: 280px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;">
              ${escapeHtml(item.error_message)}
            </span>
            <span>•</span>
            <span>${relativeTime}</span>
            ${item.retry_count > 1 ? `<span>• (attempted ${item.retry_count}x)</span>` : ''}
          </div>
        </div>
      </div>
      <div class="dlq-actions">
        <button class="dlq-btn-action btn-dlq-retry" title="Retry indexing this file">↻ Retry</button>
        <button class="dlq-btn-action btn-dlq-reveal" title="Reveal in Finder">↗ Finder</button>
        <button class="dlq-btn-action btn-dlq-dismiss" style="color: var(--red);" title="Dismiss from failed queue">✕</button>
      </div>
    `;

    const retryBtn = card.querySelector('.btn-dlq-retry');
    retryBtn.addEventListener('click', async () => {
      try {
        retryBtn.disabled = true;
        retryBtn.textContent = '...';
        await tauri.invoke('retry_dead_letter_item', { path: item.path });
        triggerSaveStatus('⚡ Retrying file indexing');
        await loadDeadLetterQueue();
      } catch (err) {
        console.error('Failed to retry dead letter file:', err);
      }
    });

    const revealBtn = card.querySelector('.btn-dlq-reveal');
    revealBtn.addEventListener('click', async () => {
      try {
        await tauri.invoke('reveal_in_finder', { path: item.path });
      } catch (err) {
        console.error('Failed to reveal file:', err);
      }
    });

    const dismissBtn = card.querySelector('.btn-dlq-dismiss');
    dismissBtn.addEventListener('click', async () => {
      try {
        await tauri.invoke('remove_dead_letter_item', { id: item.id });
        await loadDeadLetterQueue();
      } catch (err) {
        console.error('Failed to dismiss dead letter item:', err);
      }
    });

    elements.dlqItemsList.appendChild(card);
  });
}

// Modal State & Handlers for Folder Removal and Index Purging
let activeFolderModalTarget = null;
let activeFolderModalAction = 'remove'; // 'remove' | 'purge_only'

function openRemoveFolderModal(folder) {
  if (!folder || !elements.folderActionModal) return;
  activeFolderModalTarget = folder;
  activeFolderModalAction = 'remove';

  if (elements.modalFolderTitle) {
    elements.modalFolderTitle.textContent = 'Remove Watched Folder';
  }
  if (elements.modalFolderMessage) {
    elements.modalFolderMessage.textContent = 'Do you also want to remove already indexed data for this folder?';
  }
  if (elements.modalFolderPath) {
    elements.modalFolderPath.textContent = folder.path;
  }
  if (elements.modalFolderSubtext) {
    elements.modalFolderSubtext.textContent = 'Removing indexed data will permanently delete all search chunks, text tokens, and vector embeddings created from files in this folder.';
  }
  if (elements.modalBtnKeepIndex) {
    elements.modalBtnKeepIndex.style.display = 'inline-flex';
    elements.modalBtnKeepIndex.textContent = 'Remove Watchlist Only';
  }
  if (elements.modalBtnPurgeData) {
    elements.modalBtnPurgeData.textContent = 'Remove & Delete Index Data';
  }

  elements.folderActionModal.style.display = 'flex';
}

function openPurgeFolderModal(folder) {
  if (!folder || !elements.folderActionModal) return;
  activeFolderModalTarget = folder;
  activeFolderModalAction = 'purge_only';

  if (elements.modalFolderTitle) {
    elements.modalFolderTitle.textContent = 'Clear Indexed Data';
  }
  if (elements.modalFolderMessage) {
    elements.modalFolderMessage.textContent = 'Are you sure you want to delete all indexed data for this folder?';
  }
  if (elements.modalFolderPath) {
    elements.modalFolderPath.textContent = folder.path;
  }
  if (elements.modalFolderSubtext) {
    elements.modalFolderSubtext.textContent = 'The folder will remain in your watchlist, but all search chunks, text tokens, and vector embeddings will be purged. You can re-scan it anytime.';
  }
  if (elements.modalBtnKeepIndex) {
    elements.modalBtnKeepIndex.style.display = 'none';
  }
  if (elements.modalBtnPurgeData) {
    elements.modalBtnPurgeData.textContent = 'Delete Index Data';
  }

  elements.folderActionModal.style.display = 'flex';
}

function closeFolderActionModal() {
  if (elements.folderActionModal) {
    elements.folderActionModal.style.display = 'none';
  }
  activeFolderModalTarget = null;
}

function setupFolderActionModal() {
  if (elements.modalBtnClose) {
    elements.modalBtnClose.addEventListener('click', closeFolderActionModal);
  }
  if (elements.modalBtnCancel) {
    elements.modalBtnCancel.addEventListener('click', closeFolderActionModal);
  }
  if (elements.folderActionModal) {
    elements.folderActionModal.addEventListener('click', (e) => {
      if (e.target === elements.folderActionModal) {
        closeFolderActionModal();
      }
    });
  }

  window.addEventListener('keydown', (e) => {
    if (e.key === 'Escape' && elements.folderActionModal && elements.folderActionModal.style.display === 'flex') {
      closeFolderActionModal();
    }
  });

  if (elements.modalBtnKeepIndex) {
    elements.modalBtnKeepIndex.addEventListener('click', async () => {
      if (!activeFolderModalTarget) {
        closeFolderActionModal();
        return;
      }
      const folder = activeFolderModalTarget;
      closeFolderActionModal();
      watchedFolders = watchedFolders.filter((f) => f.id !== folder.id);
      renderWatchedFolders();
      triggerSaveStatus('✓ Folder unwatched (indexed data kept)');
      try {
        await tauri.invoke('remove_watched_folder', {
          id: folder.id,
          path: folder.path,
          purgeData: false,
        });
      } catch (err) {
        console.error('Failed to remove watched folder:', err);
        triggerSaveStatus('❌ Failed to remove folder');
      }
    });
  }

  if (elements.modalBtnPurgeData) {
    elements.modalBtnPurgeData.addEventListener('click', (e) => {
      e?.preventDefault?.();
      e?.stopPropagation?.();

      if (!activeFolderModalTarget) {
        closeFolderActionModal();
        return;
      }
      const folder = activeFolderModalTarget;
      const isRemoval = activeFolderModalAction === 'remove';

      // Close modal immediately and reset target — zero UI delay
      closeFolderActionModal();

      if (isRemoval) {
        watchedFolders = watchedFolders.filter((f) => f.id !== folder.id);
        renderWatchedFolders();
        triggerSaveStatus('🗑️ Removing folder & queueing index purge...');
      } else {
        folder.last_scanned_at = null;
        renderWatchedFolders();
        triggerSaveStatus('🗑️ Delete job queued — running in background...');
      }

      // Non-blocking invocation so UI event loop is never suspended
      const purgePromise = isRemoval
        ? tauri.invoke('remove_watched_folder', {
          id: folder.id,
          path: folder.path,
          purgeData: true,
        })
        : tauri.invoke('purge_folder_index', {
          path: folder.path,
        });

      purgePromise
        .then(() => {
          triggerSaveStatus(
            isRemoval
              ? '✓ Folder removed & delete job running in background'
              : '✓ Delete job queued — track progress in Delete Queue'
          );
          // Refresh delete queue UI immediately without blocking
          loadDeleteQueue().catch(() => { });
        })
        .catch((err) => {
          console.error('Failed to purge/remove folder data:', err);
          triggerSaveStatus('❌ Failed to queue folder deletion');
        });
    });
  }
}

// Folder Analysis Inspector (Non-blocking Breakdown Viewer)
let pendingAnalysisPath = null;

function formatPreIndexEta(seconds) {
  if (!seconds || seconds <= 0) return 'Immediate (<1s)';
  if (seconds < 60) return `~${seconds}s`;
  const mins = Math.floor(seconds / 60);
  const remSecs = seconds % 60;
  if (remSecs === 0) return `~${mins}m`;
  return `~${mins}m ${remSecs}s`;
}

function populateAnalysisReportUI(report) {
  const durationPill = document.getElementById('analysis-duration-pill');
  const loadingState = document.getElementById('analysis-loading-state');
  const contentState = document.getElementById('analysis-content-state');
  const confirmBtn = document.getElementById('analysis-modal-btn-confirm');

  if (durationPill) durationPill.textContent = report.scan_duration_ms ? `Scanned in ${report.scan_duration_ms}ms` : 'Analyzed';

  const totalFilesEl = document.getElementById('analysis-total-files');
  const totalSizeEl = document.getElementById('analysis-total-size');
  const etaEl = document.getElementById('analysis-estimated-time');
  const rowDocs = document.getElementById('analysis-row-docs');
  const rowImages = document.getElementById('analysis-row-images');
  const rowCode = document.getElementById('analysis-row-code');
  const rowData = document.getElementById('analysis-row-data');
  const rowSkipped = document.getElementById('analysis-row-skipped');

  if (totalFilesEl) totalFilesEl.textContent = `${(report.total_indexable_files || 0).toLocaleString()} files`;
  if (totalSizeEl) totalSizeEl.textContent = formatBytes(report.total_indexable_bytes || 0);
  if (etaEl) etaEl.textContent = formatPreIndexEta(report.estimated_time_seconds || 0);

  if (rowDocs) rowDocs.textContent = `${(report.documents_count || 0).toLocaleString()} files (${formatBytes(report.documents_bytes || 0)})`;
  if (rowImages) rowImages.textContent = `${(report.images_count || 0).toLocaleString()} files (${formatBytes(report.images_bytes || 0)})`;
  if (rowCode) rowCode.textContent = `${(report.code_count || 0).toLocaleString()} files (${formatBytes(report.code_bytes || 0)})`;
  if (rowData) rowData.textContent = `${(report.data_count || 0).toLocaleString()} files (${formatBytes(report.data_bytes || 0)})`;
  if (rowSkipped) rowSkipped.textContent = `${(report.ignored_files_count || 0).toLocaleString()} files`;

  if (loadingState) loadingState.style.display = 'none';
  if (contentState) contentState.style.display = 'block';
  if (confirmBtn) confirmBtn.disabled = false;
}

function openFolderAnalysisInspector(folderPath, precomputedReport = null) {
  const modal = document.getElementById('folder-analysis-modal');
  if (!modal) return;
  pendingAnalysisPath = folderPath;

  const pathEl = document.getElementById('analysis-modal-folder-path');
  const durationPill = document.getElementById('analysis-duration-pill');
  const loadingState = document.getElementById('analysis-loading-state');
  const contentState = document.getElementById('analysis-content-state');
  const confirmBtn = document.getElementById('analysis-modal-btn-confirm');

  if (pathEl) pathEl.textContent = folderPath;
  modal.style.display = 'flex';

  if (precomputedReport) {
    populateAnalysisReportUI(precomputedReport);
    return;
  }

  if (durationPill) durationPill.textContent = 'Scanning...';
  if (loadingState) {
    loadingState.style.display = 'flex';
    loadingState.innerHTML = `
      <div class="search-spinner" style="display: block; width: 24px; height: 24px; border-width: 2.5px;"></div>
      <span style="font-size: 12px; color: var(--label-2);">Analyzing directory structure &amp; file types...</span>
    `;
  }
  if (contentState) contentState.style.display = 'none';
  if (confirmBtn) confirmBtn.disabled = true;

  tauri.invoke('analyze_folder', { path: folderPath, recursive: true })
    .then((report) => {
      if (!report || pendingAnalysisPath !== folderPath) return;
      populateAnalysisReportUI(report);
      loadIndexingQueue();
    })
    .catch((err) => {
      console.error('Folder analysis failed:', err);
      if (durationPill) durationPill.textContent = 'Scan error';
      if (loadingState) {
        loadingState.innerHTML = `<span style="color: var(--red); font-size: 12px; padding: 12px; text-align: center;">Failed to scan folder: ${escapeHtml(String(err))}</span>`;
      }
    });
}

const openFolderAnalysisModal = openFolderAnalysisInspector;

function closeFolderAnalysisModal() {
  const modal = document.getElementById('folder-analysis-modal');
  if (modal) modal.style.display = 'none';
  pendingAnalysisPath = null;
  loadIndexingQueue();
}

function setupFolderAnalysisModal() {
  const modal = document.getElementById('folder-analysis-modal');
  const btnClose = document.getElementById('analysis-modal-btn-close');
  const btnCancel = document.getElementById('analysis-modal-btn-cancel');
  const btnConfirm = document.getElementById('analysis-modal-btn-confirm');

  if (btnClose) btnClose.addEventListener('click', closeFolderAnalysisModal);
  if (btnCancel) btnCancel.addEventListener('click', closeFolderAnalysisModal);

  if (btnConfirm) {
    btnConfirm.addEventListener('click', async () => {
      const pathToScan = pendingAnalysisPath;
      closeFolderAnalysisModal();
      if (pathToScan) {
        try {
          await tauri.invoke('trigger_folder_scan', { path: pathToScan });
          triggerSaveStatus('⚡ Indexing started');
          await loadIndexingQueue();
        } catch (err) {
          console.error('Failed to trigger folder scan:', err);
        }
      }
    });
  }

  if (modal) {
    modal.addEventListener('click', (e) => {
      if (e.target === modal) closeFolderAnalysisModal();
    });
  }

  window.addEventListener('keydown', (e) => {
    if (e.key === 'Escape' && modal && modal.style.display === 'flex') {
      closeFolderAnalysisModal();
    }
  });
}

// Model Deletion Modal & RAM Unload Handlers
let activeModelDeleteTarget = null;

function openModelDeleteModal(model, sizeStr) {
  if (!model || !elements.modelDeleteModal) return;
  activeModelDeleteTarget = model;

  if (elements.modalModelName) {
    elements.modalModelName.textContent = model.display_name;
  }
  if (elements.modalModelSize) {
    elements.modalModelSize.textContent = sizeStr;
  }
  if (elements.modalModelBtnConfirm) {
    elements.modalModelBtnConfirm.disabled = false;
    elements.modalModelBtnConfirm.textContent = 'Delete Model';
  }

  elements.modelDeleteModal.style.display = 'flex';
}

function closeModelDeleteModal() {
  if (elements.modelDeleteModal) {
    elements.modelDeleteModal.style.display = 'none';
  }
  activeModelDeleteTarget = null;
}

function setupModelDeleteModal() {
  if (elements.modalModelBtnClose) {
    elements.modalModelBtnClose.addEventListener('click', closeModelDeleteModal);
  }
  if (elements.modalModelBtnCancel) {
    elements.modalModelBtnCancel.addEventListener('click', closeModelDeleteModal);
  }
  if (elements.modelDeleteModal) {
    elements.modelDeleteModal.addEventListener('click', (e) => {
      if (e.target === elements.modelDeleteModal) {
        closeModelDeleteModal();
      }
    });
  }

  window.addEventListener('keydown', (e) => {
    if (e.key === 'Escape' && elements.modelDeleteModal && elements.modelDeleteModal.style.display === 'flex') {
      closeModelDeleteModal();
    }
  });

  if (elements.modalModelBtnConfirm) {
    elements.modalModelBtnConfirm.addEventListener('click', async () => {
      if (!activeModelDeleteTarget) {
        closeModelDeleteModal();
        return;
      }
      const m = activeModelDeleteTarget;
      elements.modalModelBtnConfirm.disabled = true;
      elements.modalModelBtnConfirm.textContent = 'Deleting...';

      try {
        await tauri.invoke('delete_model', { modelId: m.id });
        showToast(`Deleted ${m.display_name}`);
        closeModelDeleteModal();
        await loadLlmModels();
      } catch (err) {
        showToast(`Delete failed: ${err}`, true);
        elements.modalModelBtnConfirm.disabled = false;
        elements.modalModelBtnConfirm.textContent = 'Delete Model';
      }
    });
  }

  if (elements.btnUnloadModelRam) {
    elements.btnUnloadModelRam.addEventListener('click', async () => {
      const isUnloadAction = elements.btnUnloadModelRam.textContent.trim().toLowerCase() === 'unload';
      if (isUnloadAction) {
        try {
          elements.btnUnloadModelRam.disabled = true;
          elements.btnUnloadModelRam.textContent = 'Unloading...';
          await tauri.invoke('unload_llm');
          showToast('Model unloaded from memory');
          await refreshTelemetry();
          await loadLlmModels();
        } catch (err) {
          showToast(`Unload failed: ${err}`, true);
          await loadLlmModels();
        }
      } else {
        try {
          elements.btnUnloadModelRam.disabled = true;
          elements.btnUnloadModelRam.textContent = 'Loading...';
          await tauri.invoke('load_active_model');
          showToast('Model loaded into memory');
          await refreshTelemetry();
          await loadLlmModels();
        } catch (err) {
          showToast(`Load failed: ${err}`, true);
          await loadLlmModels();
        }
      }
    });
  }
}

function setupWatchedFoldersHandlers() {
  if (elements.btnAddFolder) {
    let isPicking = false;
    elements.btnAddFolder.addEventListener('click', async () => {
      if (isPicking) return;
      isPicking = true;
      try {
        const pickedPath = await tauri.invoke('pick_folder');
        if (pickedPath) {
          if (watchedFolders.some((f) => f.path === pickedPath)) {
            triggerSaveStatus('⚠️ Folder is already monitored');
            return;
          }
          const newFolder = await tauri.invoke('add_watched_folder', {
            path: pickedPath,
            recursive: true,
          });
          if (newFolder) {
            watchedFolders.push(newFolder);
            renderWatchedFolders();
            triggerSaveStatus('✓ Folder added — analyzing in background');
            await loadIndexingQueue();
            startIqPoller();
          }
        }
      } catch (err) {
        console.error('Failed to pick/add folder:', err);
      } finally {
        isPicking = false;
      }
    });
  }

  if (elements.btnRescanAll) {
    elements.btnRescanAll.addEventListener('click', async () => {
      try {
        const enabledPaths = watchedFolders.filter((f) => f.enabled).map((f) => f.path);
        if (enabledPaths.length > 0) {
          await tauri.invoke('start_crawl', { paths: enabledPaths });
          triggerSaveStatus('⚡ Crawl initiated');
          await loadIndexingQueue();
        }
      } catch (err) {
        console.error('Failed to rescan all:', err);
      }
    });
  }

  if (elements.btnPauseCrawl) {
    elements.btnPauseCrawl.addEventListener('click', async () => {
      try {
        await tauri.invoke('pause_crawl');
        elements.btnPauseCrawl.style.display = 'none';
        if (elements.btnResumeCrawl) elements.btnResumeCrawl.style.display = 'inline-flex';
        if (elements.queueStatusLabel) elements.queueStatusLabel.textContent = 'Indexing Paused (⏸)';
        triggerSaveStatus('⏸ Indexing paused');
      } catch (err) {
        console.error('Failed to pause crawl:', err);
      }
    });
  }

  if (elements.btnResumeCrawl) {
    elements.btnResumeCrawl.addEventListener('click', async () => {
      try {
        await tauri.invoke('resume_crawl');
        elements.btnResumeCrawl.style.display = 'none';
        if (elements.btnPauseCrawl) elements.btnPauseCrawl.style.display = 'inline-flex';
        if (elements.queueStatusLabel) elements.queueStatusLabel.textContent = 'Indexing Active';
        triggerSaveStatus('▶ Indexing resumed');
      } catch (err) {
        console.error('Failed to resume crawl:', err);
      }
    });
  }

  if (elements.btnRemoveFolder) {
    elements.btnRemoveFolder.addEventListener('click', () => {
      if (watchedFolders.length > 0) {
        openRemoveFolderModal(watchedFolders[watchedFolders.length - 1]);
      }
    });
  }
}

function setupDeadLetterQueueHandlers() {
  if (elements.btnRefreshDlq) {
    elements.btnRefreshDlq.addEventListener('click', async () => {
      await loadDeadLetterQueue();
      triggerSaveStatus('✓ Failed queue refreshed');
    });
  }

  if (elements.btnClearDlq) {
    elements.btnClearDlq.addEventListener('click', async () => {
      try {
        await tauri.invoke('clear_dead_letter_queue');
        await loadDeadLetterQueue();
        triggerSaveStatus('✓ Dead letter queue cleared');
      } catch (err) {
        console.error('Failed to clear dead letter queue:', err);
      }
    });
  }
}

// Indexing Queue & Drawer (D-01, D-02)
function setupIndexingQueue() {
  if (elements.btnToggleQueueDrawer) {
    elements.btnToggleQueueDrawer.addEventListener('click', () => {
      isQueueDrawerOpen = !isQueueDrawerOpen;
      elements.queueDrawerContent.style.display = isQueueDrawerOpen ? 'block' : 'none';
      if (elements.queueDrawerIcon) {
        elements.queueDrawerIcon.classList.toggle('expanded', isQueueDrawerOpen);
      }
    });
  }

  // Tauri Event Streaming listener — indexing queue
  if (window.__TAURI__?.event?.listen) {
    window.__TAURI__.event.listen('indexing-queue-update', (event) => {
      if (event.payload) {
        renderQueueSnapshot(event.payload);
        const snap = event.payload;
        const isFolderAnalyzing = snap.active_folder?.status === 'Analyzing'
          || (snap.pending_queue || []).some((q) => q.status === 'Analyzing');
        const isFolderScanning = snap.active_folder?.status === 'Scanning'
          || (snap.pending_queue || []).some((q) => q.status === 'Scanning');
        if (isFolderAnalyzing || isFolderScanning || snap.is_active) {
          startIqPoller();
        } else {
          stopIqPoller();
        }
      }
    });

    // Delete queue live updates — Tauri broadcasts on every state change
    window.__TAURI__.event.listen('delete-queue-update', (event) => {
      if (event.payload) {
        renderDeleteQueueSnapshot(event.payload);
        const jobs = event.payload.jobs || [];
        const anyRunning = jobs.some((j) => j.status === 'running' || j.status === 'paused');
        const anyDone = jobs.some((j) => j.status === 'done');

        // Start / stop fallback poller depending on whether a job is active
        if (anyRunning) {
          startDqPoller();
        } else {
          stopDqPoller();
        }

        // On completion: refresh folder list, analytics, AND telemetry (DB size in header)
        if (anyDone || jobs.length === 0) {
          loadWatchedFolders().catch(() => { });
          loadAnalyticsData().catch(() => { });
          // Refresh header metrics so DB size shrinks immediately after VACUUM
          refreshTelemetry().catch(() => { });
        }
      }
    });
  }
}

// ─── Delete Queue UI ──────────────────────────────────────────────────────────

let dqPendingStoppedJob = null; // held for requeue modal
const dqDoneTimers = new Map(); // auto-dismiss timers for finished jobs

// Fallback poller — fires every 500 ms while a delete job is running.
// This guarantees progress bar updates even if Tauri WebKit event delivery hiccups.
let _dqPollTimer = null;
function startDqPoller() {
  if (_dqPollTimer) return; // already polling
  _dqPollTimer = setInterval(async () => {
    try {
      const snap = await tauri.invoke('get_delete_queue');
      if (!snap) return;
      renderDeleteQueueSnapshot(snap);
      const anyActive = (snap.jobs || []).some((j) => j.status === 'running' || j.status === 'paused');
      if (!anyActive) stopDqPoller();
    } catch (_) { /* non-fatal */ }
  }, 500);
}
function stopDqPoller() {
  if (_dqPollTimer) {
    clearInterval(_dqPollTimer);
    _dqPollTimer = null;
  }
}

async function loadDeleteQueue() {
  try {
    const snap = await tauri.invoke('get_delete_queue');
    if (snap) {
      renderDeleteQueueSnapshot(snap);
      const anyActive = (snap.jobs || []).some((j) => j.status === 'running' || j.status === 'paused');
      if (anyActive) startDqPoller();
    }
  } catch (err) {
    console.error('Failed to load delete queue:', err);
  }
}

let dqRenderPending = false;
let dqQueuedSnapshot = null;

function renderDeleteQueueSnapshot(snap) {
  if (!snap) return;
  dqQueuedSnapshot = snap;
  if (!dqRenderPending) {
    dqRenderPending = true;
    const scheduleFrame = typeof window !== 'undefined' && window.requestAnimationFrame ? window.requestAnimationFrame : setTimeout;
    scheduleFrame(() => {
      dqRenderPending = false;
      if (dqQueuedSnapshot) {
        doRenderDeleteQueueSnapshot(dqQueuedSnapshot);
      }
    }, 0);
  }
}

function doRenderDeleteQueueSnapshot(snap) {
  const card = document.getElementById('delete-queue-card');
  const tbody = document.getElementById('delete-queue-tbody');
  const statusLabel = document.getElementById('dq-status-label');
  const pulseDot = document.getElementById('dq-pulse-dot');
  if (!card || !tbody) return;

  const jobs = snap.jobs || [];
  // Keep done jobs visible briefly (auto-dismissed after 4s) so user sees completion feedback
  const visibleJobs = jobs.filter((j) => {
    if (j.status === 'done') {
      if (!dqDoneTimers.has(j.id)) {
        const timer = setTimeout(() => {
          tauri.invoke('dismiss_delete_job', { jobId: j.id }).catch(() => { });
          dqDoneTimers.delete(j.id);
        }, 4000);
        dqDoneTimers.set(j.id, timer);
      }
      return true;
    }
    return true;
  });

  // Show/hide card
  card.style.display = visibleJobs.length > 0 ? 'block' : 'none';
  if (visibleJobs.length === 0) return;

  // Header state
  const anyRunning = visibleJobs.some((j) => j.status === 'running');
  const anyPaused = visibleJobs.some((j) => j.status === 'paused');
  const allDone = visibleJobs.length > 0 && visibleJobs.every((j) => j.status === 'done');
  if (statusLabel) {
    statusLabel.textContent = anyRunning ? 'Deleting…' : anyPaused ? 'Paused' : allDone ? 'Completed' : 'Stopped';
  }
  if (pulseDot) {
    pulseDot.style.animation = anyRunning ? 'queuePulse 1.2s ease-in-out infinite' : 'none';
    pulseDot.style.background = anyRunning ? 'var(--red)' : anyPaused ? '#f59e0b' : allDone ? 'var(--green)' : 'var(--label-3)';
  }

  // Render rows
  const rows = visibleJobs.map((job) => {
    const isDone = job.status === 'done';
    const pct = isDone ? 100 : job.total > 0 ? Math.min(100, Math.round((job.deleted / job.total) * 100)) : 0;
    const countText = isDone
      ? `${job.deleted} / ${job.total || job.deleted} files`
      : job.total > 0 ? `${job.deleted} / ${job.total} files` : `${job.deleted} files`;

    // Status badge
    let statusHtml;
    switch (job.status) {
      case 'running':
        statusHtml = `<span style="display:inline-flex;align-items:center;gap:4px;font-size:10px;font-weight:700;color:var(--red);background:rgba(255,59,48,0.10);border:0.5px solid rgba(255,59,48,0.25);border-radius:4px;padding:2px 6px;"><span style="width:6px;height:6px;border-radius:50%;background:var(--red);animation:queuePulse 1.2s ease-in-out infinite;display:inline-block;"></span>DELETING</span>`;
        break;
      case 'paused':
        statusHtml = `<span style="display:inline-flex;align-items:center;gap:4px;font-size:10px;font-weight:700;color:#f59e0b;background:rgba(245,158,11,0.12);border:0.5px solid rgba(245,158,11,0.3);border-radius:4px;padding:2px 6px;">⏸ PAUSED</span>`;
        break;
      case 'stopped':
        statusHtml = `<span style="display:inline-flex;align-items:center;gap:4px;font-size:10px;font-weight:600;color:var(--label-2);background:var(--fill-tertiary);border:0.5px solid var(--separator);border-radius:4px;padding:2px 6px;">⏹ STOPPED</span>`;
        break;
      case 'done':
        statusHtml = `<span style="display:inline-flex;align-items:center;gap:4px;font-size:10px;font-weight:700;color:var(--green);background:rgba(52,199,89,0.12);border:0.5px solid rgba(52,199,89,0.3);border-radius:4px;padding:2px 6px;">✓ DONE</span>`;
        break;
      case 'failed':
        statusHtml = `<span style="display:inline-flex;align-items:center;gap:4px;font-size:10px;font-weight:700;color:var(--red);background:rgba(255,59,48,0.10);border:0.5px solid rgba(255,59,48,0.25);border-radius:4px;padding:2px 6px;">✕ FAILED</span>`;
        break;
      default:
        statusHtml = `<span style="display:inline-flex;align-items:center;gap:4px;font-size:10px;font-weight:600;color:var(--label-2);background:var(--fill-tertiary);border:0.5px solid var(--separator);border-radius:4px;padding:2px 6px;">PENDING</span>`;
    }

    // Progress cell
    const barBg = isDone ? 'var(--green)' : 'var(--red)';
    const progressHtml = `
      <div style="display:flex;flex-direction:column;gap:3px;min-width:120px;">
        <div style="display:flex;justify-content:space-between;font-size:10px;color:var(--label-2);font-variant-numeric:tabular-nums;">
          <span>${escapeHtml(countText)}</span>
          ${job.total > 0 || isDone ? `<span style="color:${isDone ? 'var(--green)' : 'var(--label-3)'};">${pct}%</span>` : ''}
        </div>
        ${job.total > 0 || isDone ? `<div style="width:100%;height:3px;background:var(--fill-tertiary);border-radius:2px;overflow:hidden;"><div style="width:${pct}%;height:100%;background:${barBg};transition:width 300ms ease;"></div></div>` : ''}
        ${job.error ? `<span style="font-size:10px;color:var(--red);">${escapeHtml(job.error)}</span>` : ''}
      </div>`;

    // Action buttons
    let actionsHtml = '';
    if (job.status === 'running') {
      actionsHtml = `
        <div style="display:inline-flex;align-items:center;gap:4px;">
          <button class="mac-btn mac-btn-secondary mac-btn-table dq-btn-pause" data-id="${escapeHtml(job.id)}"><span>⏸ Pause</span></button>
          <button class="mac-btn mac-btn-secondary mac-btn-table dq-btn-stop" data-id="${escapeHtml(job.id)}" style="color:var(--red);">⏹ Stop</button>
        </div>`;
    } else if (job.status === 'paused') {
      actionsHtml = `
        <div style="display:inline-flex;align-items:center;gap:4px;">
          <button class="mac-btn mac-btn-secondary mac-btn-table dq-btn-resume" data-id="${escapeHtml(job.id)}" style="color:var(--green);">▶ Resume</button>
          <button class="mac-btn mac-btn-secondary mac-btn-table dq-btn-stop" data-id="${escapeHtml(job.id)}" style="color:var(--red);">⏹ Stop</button>
        </div>`;
    } else if (job.status === 'done') {
      actionsHtml = `
        <div style="display:inline-flex;align-items:center;gap:4px;">
          <button class="mac-btn mac-btn-secondary mac-btn-table dq-btn-dismiss" data-id="${escapeHtml(job.id)}">Dismiss</button>
        </div>`;
    } else if (job.status === 'stopped' || job.status === 'failed') {
      const requeueBtn = job.can_requeue
        ? `<button class="mac-btn mac-btn-primary mac-btn-table dq-btn-requeue" data-id="${escapeHtml(job.id)}" data-label="${escapeHtml(job.label)}">↺ Re-monitor</button>`
        : '';
      actionsHtml = `
        <div style="display:inline-flex;align-items:center;gap:4px;">
          ${requeueBtn}
          <button class="mac-btn mac-btn-secondary mac-btn-table dq-btn-dismiss" data-id="${escapeHtml(job.id)}">Dismiss</button>
        </div>`;
    }

    return `
      <tr style="border-top: 0.5px solid var(--separator);" data-dq-job-id="${escapeHtml(job.id)}">
        <td style="padding: 8px 14px;">
          <div style="display:flex;flex-direction:column;gap:1px;overflow:hidden;">
            <span style="font-weight:500;color:var(--label);white-space:nowrap;overflow:hidden;text-overflow:ellipsis;">${escapeHtml(job.label)}</span>
          </div>
        </td>
        <td style="padding: 8px; text-align: center;">${statusHtml}</td>
        <td style="padding: 8px;">${progressHtml}</td>
        <td style="padding: 8px 14px; text-align: right; white-space: nowrap;">${actionsHtml}</td>
      </tr>`;
  });

  tbody.innerHTML = rows.join('');

  // Wire pause
  tbody.querySelectorAll('.dq-btn-pause').forEach((btn) => {
    btn.addEventListener('click', async () => {
      console.log('[DeleteQueue] User clicked Pause for job:', btn.dataset.id);
      try { await tauri.invoke('pause_delete_job', { jobId: btn.dataset.id }); } catch (e) { console.error(e); }
    });
  });
  // Wire resume
  tbody.querySelectorAll('.dq-btn-resume').forEach((btn) => {
    btn.addEventListener('click', async () => {
      console.log('[DeleteQueue] User clicked Resume for job:', btn.dataset.id);
      try { await tauri.invoke('resume_delete_job', { jobId: btn.dataset.id }); } catch (e) { console.error(e); }
    });
  });
  // Wire stop → show requeue modal for folder jobs
  tbody.querySelectorAll('.dq-btn-stop').forEach((btn) => {
    btn.addEventListener('click', async () => {
      console.log('[DeleteQueue] User clicked Stop for job:', btn.dataset.id);
      try {
        await tauri.invoke('stop_delete_job', { jobId: btn.dataset.id });
        // snap will come back via event with status=stopped; modal shown by re-render
      } catch (e) { console.error(e); }
    });
  });
  // Wire requeue
  tbody.querySelectorAll('.dq-btn-requeue').forEach((btn) => {
    btn.addEventListener('click', () => {
      console.log('[DeleteQueue] User opened requeue modal for job:', btn.dataset.id);
      dqPendingStoppedJob = { id: btn.dataset.id, label: btn.dataset.label };
      const labelEl = document.getElementById('dq-requeue-modal-label');
      if (labelEl) labelEl.textContent = btn.dataset.label;
      const modal = document.getElementById('dq-requeue-modal');
      if (modal) modal.style.display = 'flex';
    });
  });
  // Wire dismiss
  tbody.querySelectorAll('.dq-btn-dismiss').forEach((btn) => {
    btn.addEventListener('click', async () => {
      console.log('[DeleteQueue] User clicked Dismiss for job:', btn.dataset.id);
      try {
        if (dqDoneTimers.has(btn.dataset.id)) {
          clearTimeout(dqDoneTimers.get(btn.dataset.id));
          dqDoneTimers.delete(btn.dataset.id);
        }
        await tauri.invoke('dismiss_delete_job', { jobId: btn.dataset.id });
        await loadWatchedFolders();
        await loadAnalyticsData().catch(() => { });
      } catch (e) { console.error(e); }
    });
  });

  // Check if any stopped jobs with can_requeue appeared — auto-prompt if first time
  const newStopped = visibleJobs.filter((j) => j.status === 'stopped' && j.can_requeue);
  if (newStopped.length > 0 && !dqPendingStoppedJob) {
    // auto-open modal for the first one
    const job = newStopped[0];
    dqPendingStoppedJob = { id: job.id, label: job.label };
    const labelEl = document.getElementById('dq-requeue-modal-label');
    if (labelEl) labelEl.textContent = job.label;
    const modal = document.getElementById('dq-requeue-modal');
    if (modal) modal.style.display = 'flex';
  }
}

function initDeleteQueueModal() {
  const modal = document.getElementById('dq-requeue-modal');
  const closeBtn = document.getElementById('dq-requeue-modal-close');
  const dismissBtn = document.getElementById('dq-requeue-modal-dismiss');
  const requeueBtn = document.getElementById('dq-requeue-modal-requeue');

  const closeModal = () => {
    if (modal) modal.style.display = 'none';
    dqPendingStoppedJob = null;
  };

  if (closeBtn) closeBtn.addEventListener('click', closeModal);
  if (dismissBtn) {
    dismissBtn.addEventListener('click', async () => {
      if (dqPendingStoppedJob) {
        try { await tauri.invoke('dismiss_delete_job', { jobId: dqPendingStoppedJob.id }); } catch (e) { console.error(e); }
      }
      closeModal();
      await loadWatchedFolders();
      await loadAnalyticsData().catch(() => { });
    });
  }
  if (requeueBtn) {
    requeueBtn.addEventListener('click', async () => {
      if (dqPendingStoppedJob) {
        try {
          await tauri.invoke('requeue_folder_after_stop', { jobId: dqPendingStoppedJob.id });
          triggerSaveStatus('✓ Re-monitoring & re-indexing started');
        } catch (e) {
          console.error('requeue_folder_after_stop failed:', e);
          triggerSaveStatus('❌ Failed to re-monitor folder');
        }
      }
      closeModal();
      await loadWatchedFolders();
      await loadIndexingQueue();
    });
  }
  if (modal) {
    modal.addEventListener('click', (e) => { if (e.target === modal) closeModal(); });
  }
  window.addEventListener('keydown', (e) => {
    if (e.key === 'Escape' && modal && modal.style.display === 'flex') closeModal();
  });
}


// Fallback poller for Indexing Queue — polls while any folder is analyzing or scanning
let _iqPollTimer = null;
function startIqPoller() {
  if (_iqPollTimer) return;
  _iqPollTimer = setInterval(async () => {
    try {
      const snap = await tauri.invoke('get_indexing_queue');
      if (!snap) return;
      renderQueueSnapshot(snap);
      const isFolderAnalyzing = snap.active_folder?.status === 'Analyzing'
        || (snap.pending_queue || []).some((q) => q.status === 'Analyzing');
      const isFolderScanning = snap.active_folder?.status === 'Scanning'
        || (snap.pending_queue || []).some((q) => q.status === 'Scanning');
      if (!isFolderAnalyzing && !isFolderScanning && !snap.is_active) {
        stopIqPoller();
      }
    } catch (_) { /* non-fatal */ }
  }, 400);
}

function stopIqPoller() {
  if (_iqPollTimer) {
    clearInterval(_iqPollTimer);
    _iqPollTimer = null;
  }
}

async function loadIndexingQueue() {
  try {
    const snapshot = await tauri.invoke('get_indexing_queue');
    if (snapshot) {
      renderQueueSnapshot(snapshot);
      const isFolderAnalyzing = snapshot.active_folder?.status === 'Analyzing'
        || (snapshot.pending_queue || []).some((q) => q.status === 'Analyzing');
      const isFolderScanning = snapshot.active_folder?.status === 'Scanning'
        || (snapshot.pending_queue || []).some((q) => q.status === 'Scanning');
      if (isFolderAnalyzing || isFolderScanning || snapshot.is_active) {
        startIqPoller();
      } else {
        stopIqPoller();
      }
    }
  } catch (err) {
    console.error('Failed to load indexing queue:', err);
  }
}

let queueRenderPending = false;
let queuedSnapshot = null;
let lastPendingQueueSignature = '';
let lastRecentEventsSignature = '';

function renderQueueSnapshot(snapshot) {
  if (!snapshot) return;
  queuedSnapshot = snapshot;
  if (!queueRenderPending) {
    queueRenderPending = true;
    const scheduleFrame = typeof window !== 'undefined' && window.requestAnimationFrame ? window.requestAnimationFrame : setTimeout;
    scheduleFrame(() => {
      queueRenderPending = false;
      if (queuedSnapshot) {
        doRenderQueueSnapshot(queuedSnapshot);
      }
    }, 0);
  }
}

let lastQueueActiveState = false;

function formatEta(secs) {
  if (!secs || secs <= 0) return '—';
  if (secs < 60) return `~${secs}s`;
  const m = Math.floor(secs / 60);
  const s = secs % 60;
  if (m < 60) return s > 0 ? `~${m}m ${s}s` : `~${m}m`;
  const h = Math.floor(m / 60);
  const rm = m % 60;
  return `~${h}h ${rm}m`;
}

function doRenderQueueSnapshot(snapshot) {
  if (!snapshot) return;

  const isActive = !!snapshot.is_active;
  if (lastQueueActiveState && !isActive) {
    loadWatchedFolders();
  }
  lastQueueActiveState = isActive;

  // Pulse dot
  if (elements.queuePulseDot) {
    const isAnalyzing = snapshot.active_folder?.status === 'Analyzing';
    elements.queuePulseDot.style.background = isActive
      ? (snapshot.is_paused ? '#f59e0b' : (isAnalyzing ? '#a855f7' : 'var(--accent)'))
      : 'var(--fill-secondary)';
  }

  // Status label
  if (elements.queueStatusLabel) {
    const isAnalyzing = snapshot.active_folder?.status === 'Analyzing';
    const nextStatus = isActive
      ? (snapshot.is_paused ? 'Paused' : (isAnalyzing ? 'Analyzing' : 'Indexing'))
      : 'Idle';
    if (elements.queueStatusLabel.textContent !== nextStatus) {
      elements.queueStatusLabel.textContent = nextStatus;
    }
  }

  // Global Pause All / Resume All buttons
  if (elements.btnPauseCrawl && elements.btnResumeCrawl) {
    if (isActive) {
      elements.btnPauseCrawl.style.display = snapshot.is_paused ? 'none' : 'inline-flex';
      elements.btnResumeCrawl.style.display = snapshot.is_paused ? 'inline-flex' : 'none';
    } else {
      elements.btnPauseCrawl.style.display = 'none';
      elements.btnResumeCrawl.style.display = 'none';
    }
  }

  // Active file ticker
  if (elements.activeFileTicker && elements.activeFileTickerRow) {
    if (snapshot.current_file && isActive) {
      const sizeText = snapshot.current_file_size > 0 ? ` (${formatBytes(snapshot.current_file_size)})` : '';
      const nextTicker = `${snapshot.current_file}${sizeText}`;
      if (elements.activeFileTicker.textContent !== nextTicker) {
        elements.activeFileTicker.textContent = nextTicker;
        elements.activeFileTicker.title = nextTicker;
      }
      elements.activeFileTickerRow.style.display = 'block';
    } else {
      elements.activeFileTickerRow.style.display = 'none';
    }
  }

  // Overall session progress bar (shown when active)
  if (snapshot.active_folder && (snapshot.active_folder.status === 'Scanning' || snapshot.active_folder.status === 'Analyzing')) {
    const f = snapshot.active_folder;
    const isFolderAnalyzing = f.status === 'Analyzing';
    const totalExpected = f.total_files_expected || f.total_files_discovered || 0;
    const filesProcessed = f.files_indexed + f.files_skipped + f.files_failed;
    const pct = totalExpected > 0 ? Math.min(100, Math.round((filesProcessed / totalExpected) * 100)) : 0;

    if (elements.queueProgressBarContainer) {
      elements.queueProgressBarContainer.style.display = 'block';
      if (elements.queueProgressStats) {
        if (isFolderAnalyzing) {
          elements.queueProgressStats.textContent = 'Analyzing folder structure & file types in background...';
        } else {
          elements.queueProgressStats.textContent = `${filesProcessed} / ${totalExpected || filesProcessed} files (${f.files_indexed} indexed, ${f.files_skipped} up-to-date)`;
        }
      }
      if (elements.queueProgressPercent) {
        elements.queueProgressPercent.textContent = isFolderAnalyzing ? 'Scanning' : `${pct}%`;
      }
      if (elements.queueProgressBarFill) {
        elements.queueProgressBarFill.style.width = isFolderAnalyzing ? '100%' : `${pct}%`;
        elements.queueProgressBarFill.style.opacity = isFolderAnalyzing ? '0.35' : '1';
      }
    }
  } else if (elements.queueProgressBarContainer) {
    elements.queueProgressBarContainer.style.display = 'none';
  }

  // Progress badge (header)
  if (elements.queueProgressBadge) {
    const totalQueue = snapshot.total_queue_depth || 0;
    const sessionProcessed = snapshot.session_files_processed || 0;
    let badge = '';
    if (isActive) {
      badge = `${totalQueue} folder${totalQueue !== 1 ? 's' : ''} in queue`;
      if (sessionProcessed > 0) badge += ` • ${sessionProcessed} files processed`;
    }
    if (elements.queueProgressBadge.textContent !== badge) {
      elements.queueProgressBadge.textContent = badge;
    }
  }

  // Recent events drawer label
  const recentCount = (snapshot.recent_events || []).length;
  if (elements.queueDrawerToggleLabel) {
    const nextLabel = `Recent Files (${recentCount})`;
    if (elements.queueDrawerToggleLabel.textContent !== nextLabel) {
      elements.queueDrawerToggleLabel.textContent = nextLabel;
    }
  }

  // ── Folders table ─────────────────────────────────────────────────────────
  if (elements.indexingFoldersTbody) {
    // Build signature for surgical re-render
    const activeKey = snapshot.active_folder
      ? `${snapshot.active_folder.path}:${snapshot.active_folder.status}:${snapshot.active_folder.files_indexed}:${snapshot.active_folder.files_skipped}:${snapshot.active_folder.files_failed}:${snapshot.active_folder.is_paused ? 1 : 0}:${snapshot.active_folder.eta_seconds ?? 'x'}`
      : 'none';
    const pendingKey = (snapshot.pending_queue || [])
      .map((q) => `${q.id}:${q.status}:${q.is_paused ? 1 : 0}`)
      .join(',');
    const currentSig = `${activeKey}|${pendingKey}`;

    if (currentSig !== lastPendingQueueSignature) {
      lastPendingQueueSignature = currentSig;

      const allFolders = [];
      if (snapshot.active_folder) allFolders.push({ ...snapshot.active_folder, _slot: 'active' });
      (snapshot.pending_queue || []).forEach((q) => allFolders.push({ ...q, _slot: 'pending' }));

      if (allFolders.length === 0) {
        elements.indexingFoldersTbody.innerHTML =
          '<tr><td colspan="5" id="indexing-table-empty-row" style="padding: 18px 14px; text-align: center; color: var(--label-2); font-size: 12px;">No folders indexing — queue is idle</td></tr>';
      } else {
        const rows = allFolders.map((f) => {
          const isActiveFolderRow = f._slot === 'active';
          const isFolderAnalyzing = f.status === 'Analyzing';
          const isFolderReady = f.status === 'Ready';
          const totalExpected = f.total_files_expected || f.total_files_discovered || 0;
          const filesProcessed = f.files_indexed + f.files_skipped + f.files_failed;
          const pct = totalExpected > 0 ? Math.min(100, Math.round((filesProcessed / totalExpected) * 100)) : 0;

          // Status badge
          let statusHtml;
          if (isFolderAnalyzing) {
            statusHtml = `<span style="display:inline-flex;align-items:center;gap:4px;font-size:10px;font-weight:700;color:#a855f7;background:rgba(168,85,247,0.12);border:0.5px solid rgba(168,85,247,0.3);border-radius:4px;padding:2px 6px;"><span style="width:6px;height:6px;border-radius:50%;background:#a855f7;animation:queuePulse 1s ease-in-out infinite;display:inline-block;"></span>ANALYZING</span>`;
          } else if (isFolderReady) {
            statusHtml = `<span style="display:inline-flex;align-items:center;gap:4px;font-size:10px;font-weight:700;color:#10b981;background:rgba(16,185,129,0.12);border:0.5px solid rgba(16,185,129,0.3);border-radius:4px;padding:2px 6px;"><span style="width:6px;height:6px;border-radius:50%;background:#10b981;display:inline-block;"></span>READY</span>`;
          } else if (isActiveFolderRow && f.is_paused) {
            statusHtml = `<span style="display:inline-flex;align-items:center;gap:4px;font-size:10px;font-weight:700;color:#f59e0b;background:rgba(245,158,11,0.12);border:0.5px solid rgba(245,158,11,0.3);border-radius:4px;padding:2px 6px;">⏸ PAUSED</span>`;
          } else if (isActiveFolderRow && f.status === 'Scanning') {
            statusHtml = `<span style="display:inline-flex;align-items:center;gap:4px;font-size:10px;font-weight:700;color:var(--accent);background:rgba(10,132,255,0.1);border:0.5px solid rgba(10,132,255,0.25);border-radius:4px;padding:2px 6px;"><span style="width:6px;height:6px;border-radius:50%;background:var(--accent);animation:queuePulse 1.2s ease-in-out infinite;display:inline-block;"></span>INDEXING</span>`;
          } else if (f.is_paused) {
            statusHtml = `<span style="display:inline-flex;align-items:center;gap:4px;font-size:10px;font-weight:700;color:#f59e0b;background:rgba(245,158,11,0.08);border:0.5px solid rgba(245,158,11,0.2);border-radius:4px;padding:2px 6px;">⏸ PAUSED</span>`;
          } else {
            statusHtml = `<span style="display:inline-flex;align-items:center;gap:4px;font-size:10px;font-weight:600;color:var(--label-2);background:var(--fill-tertiary);border:0.5px solid var(--separator);border-radius:4px;padding:2px 6px;">QUEUED</span>`;
          }

          // Progress cell
          let progressHtml;
          if (isFolderAnalyzing) {
            progressHtml = `
              <div style="display:flex;align-items:center;gap:6px;font-size:11px;color:var(--label-2);">
                <span class="search-spinner" style="display:inline-block;width:11px;height:11px;border-width:1.5px;border-top-color:#a855f7;"></span>
                <span>Analyzing folder &amp; types...</span>
              </div>`;
          } else if (isFolderReady) {
            progressHtml = `
              <span style="font-size:11px;color:var(--label-2);font-variant-numeric:tabular-nums;">
                ${totalExpected.toLocaleString()} files analyzed
              </span>`;
          } else if (isActiveFolderRow && f.status === 'Scanning') {
            const countText = totalExpected > 0
              ? `${filesProcessed}/${totalExpected} (${pct}%)`
              : `${filesProcessed} files`;
            progressHtml = `
              <div style="display:flex;flex-direction:column;gap:3px;min-width:120px;">
                <div style="display:flex;justify-content:space-between;font-size:10px;color:var(--label-2);font-variant-numeric:tabular-nums;">
                  <span>${countText}</span>
                  <span style="color:var(--label-3);">${f.files_indexed}↑ ${f.files_skipped}✓</span>
                </div>
                ${totalExpected > 0 ? `<div style="width:100%;height:3px;background:var(--fill-tertiary);border-radius:2px;overflow:hidden;"><div style="width:${pct}%;height:100%;background:var(--accent);transition:width 300ms ease;"></div></div>` : ''}
              </div>`;
          } else {
            progressHtml = `<span style="font-size:11px;color:var(--label-3);">—</span>`;
          }

          // ETA cell
          let etaText = '—';
          if (isFolderAnalyzing) {
            etaText = 'Scanning...';
          } else if (isFolderReady) {
            etaText = f.eta_seconds != null ? `~${formatEta(f.eta_seconds)} est.` : '—';
          } else if (isActiveFolderRow && !f.is_paused && f.eta_seconds != null) {
            etaText = formatEta(f.eta_seconds);
          }
          const etaHtml = `<span style="font-size:11px;color:${(etaText !== '—' && etaText !== 'Scanning...') ? 'var(--label)' : 'var(--label-3)'};font-variant-numeric:tabular-nums;white-space:nowrap;">${escapeHtml(etaText)}</span>`;

          // Actions cell
          const breakdownBtn = `<button class="mac-btn mac-btn-secondary mac-btn-table btn-inspect-queue-breakdown" data-path="${escapeHtml(f.path)}" title="View folder breakdown"><svg width="9" height="9" viewBox="0 0 12 12" fill="currentColor" aria-hidden="true"><path d="M1 10.5h10v1H0V0.5h1v10zm2.5-3h1.5v2.5H3.5V7.5zm3-3.5h1.5V10H6.5V4zm3-2.5H11V10H9.5V1.5z"/></svg><span>Breakdown</span></button>`;
          const startIndexBtn = `<button class="mac-btn mac-btn-primary mac-btn-table btn-start-folder-index" data-path="${escapeHtml(f.path)}" title="Start indexing this folder"><svg width="7" height="8" viewBox="0 0 8 9" fill="currentColor" aria-hidden="true"><path d="M1 1.2a.5.5 0 0 1 .76-.43l5.5 3.3a.5.5 0 0 1 0 .86l-5.5 3.3A.5.5 0 0 1 1 7.8V1.2z"/></svg><span>Index</span></button>`;

          let actionsHtml = '';
          if (isFolderReady) {
            actionsHtml = `<div style="display:inline-flex;align-items:center;gap:4px;">${startIndexBtn}${breakdownBtn}<button class="mac-btn mac-btn-secondary mac-btn-table btn-cancel-queue" data-id="${escapeHtml(f.id)}" style="color:var(--red);" title="Cancel from queue">✕</button></div>`;
          } else if (isActiveFolderRow && f.status === 'Scanning') {
            if (f.is_paused) {
              actionsHtml = `<div style="display:inline-flex;align-items:center;gap:4px;">${breakdownBtn}<button class="mac-btn mac-btn-secondary mac-btn-table btn-folder-resume" data-id="${escapeHtml(f.id)}" style="color:var(--green);"><svg width="7" height="8" viewBox="0 0 8 9" fill="currentColor" aria-hidden="true"><path d="M1 1.2a.5.5 0 0 1 .76-.43l5.5 3.3a.5.5 0 0 1 0 .86l-5.5 3.3A.5.5 0 0 1 1 7.8V1.2z"/></svg><span>Resume</span></button></div>`;
            } else {
              actionsHtml = `<div style="display:inline-flex;align-items:center;gap:4px;">${breakdownBtn}<button class="mac-btn mac-btn-secondary mac-btn-table btn-folder-pause" data-id="${escapeHtml(f.id)}"><span>⏸ Pause</span></button></div>`;
            }
          } else {
            if (f.is_paused) {
              actionsHtml = `
                <div style="display:inline-flex;align-items:center;gap:4px;">
                  ${breakdownBtn}
                  <button class="mac-btn mac-btn-secondary mac-btn-table btn-folder-queue-resume" data-id="${escapeHtml(f.id)}" style="color:var(--green);" title="Resume queued folder"><svg width="7" height="8" viewBox="0 0 8 9" fill="currentColor" aria-hidden="true"><path d="M1 1.2a.5.5 0 0 1 .76-.43l5.5 3.3a.5.5 0 0 1 0 .86l-5.5 3.3A.5.5 0 0 1 1 7.8V1.2z"/></svg><span>Resume</span></button>
                  <button class="mac-btn mac-btn-secondary mac-btn-table btn-cancel-queue" data-id="${escapeHtml(f.id)}" style="color:var(--red);" title="Cancel from queue">✕</button>
                </div>`;
            } else {
              actionsHtml = `
                <div style="display:inline-flex;align-items:center;gap:4px;">
                  ${startIndexBtn}
                  ${breakdownBtn}
                  <button class="mac-btn mac-btn-secondary mac-btn-table btn-folder-queue-pause" data-id="${escapeHtml(f.id)}" title="Pause queued folder"><span>⏸ Pause</span></button>
                  <button class="mac-btn mac-btn-secondary mac-btn-table btn-cancel-queue" data-id="${escapeHtml(f.id)}" style="color:var(--red);" title="Cancel from queue">✕</button>
                </div>`;
            }
          }

          // Folder name display (basename + dimmed path prefix)
          const pathParts = f.path.replace(/\\/g, '/').split('/');
          const folderName = pathParts.pop() || f.path;
          const folderPrefix = pathParts.join('/');
          const folderHtml = `
            <div style="display:flex;flex-direction:column;gap:1px;overflow:hidden;">
              <span style="font-weight:500;color:var(--label);white-space:nowrap;overflow:hidden;text-overflow:ellipsis;" title="${escapeHtml(f.path)}">${escapeHtml(folderName)}</span>
              <span style="font-size:10px;color:var(--label-3);white-space:nowrap;overflow:hidden;text-overflow:ellipsis;" title="${escapeHtml(f.path)}">${escapeHtml(folderPrefix)}</span>
            </div>`;

          return `
            <tr style="border-top: 0.5px solid var(--separator);" data-folder-id="${escapeHtml(f.id)}">
              <td style="padding: 8px 14px;">${folderHtml}</td>
              <td style="padding: 8px; text-align: center;">${statusHtml}</td>
              <td style="padding: 8px;">${progressHtml}</td>
              <td style="padding: 8px; text-align: right;">${etaHtml}</td>
              <td style="padding: 8px 14px; text-align: right; white-space: nowrap;">${actionsHtml}</td>
            </tr>`;
        });

        elements.indexingFoldersTbody.innerHTML = rows.join('');

        // Wire up start index button
        elements.indexingFoldersTbody.querySelectorAll('.btn-start-folder-index').forEach((btn) => {
          btn.addEventListener('click', async () => {
            const folderPath = btn.dataset.path;
            try {
              await tauri.invoke('trigger_folder_scan', { path: folderPath });
              triggerSaveStatus('⚡ Indexing started');
              await loadIndexingQueue();
            } catch (err) {
              console.error('trigger_folder_scan failed:', err);
            }
          });
        });

        // Wire up breakdown inspector button
        elements.indexingFoldersTbody.querySelectorAll('.btn-inspect-queue-breakdown').forEach((btn) => {
          btn.addEventListener('click', () => {
            const folderPath = btn.dataset.path;
            const folderItem = allFolders.find((x) => isSamePath(x.path, folderPath));
            openFolderAnalysisInspector(folderPath, folderItem?.analysis_report);
          });
        });

        // Wire up action buttons
        elements.indexingFoldersTbody.querySelectorAll('.btn-folder-pause').forEach((btn) => {
          btn.addEventListener('click', async () => {
            const folderId = btn.dataset.id;
            try { await tauri.invoke('pause_folder_indexing', { folderId }); } catch (err) { console.error('pause_folder_indexing failed:', err); }
          });
        });
        elements.indexingFoldersTbody.querySelectorAll('.btn-folder-resume').forEach((btn) => {
          btn.addEventListener('click', async () => {
            const folderId = btn.dataset.id;
            try { await tauri.invoke('resume_folder_indexing', { folderId }); } catch (err) { console.error('resume_folder_indexing failed:', err); }
          });
        });
        elements.indexingFoldersTbody.querySelectorAll('.btn-folder-queue-pause').forEach((btn) => {
          btn.addEventListener('click', async () => {
            const folderId = btn.dataset.id;
            try { await tauri.invoke('pause_folder_indexing', { folderId }); } catch (err) { console.error('pause_folder_indexing failed:', err); }
          });
        });
        elements.indexingFoldersTbody.querySelectorAll('.btn-folder-queue-resume').forEach((btn) => {
          btn.addEventListener('click', async () => {
            const folderId = btn.dataset.id;
            try { await tauri.invoke('resume_folder_indexing', { folderId }); } catch (err) { console.error('resume_folder_indexing failed:', err); }
          });
        });
        elements.indexingFoldersTbody.querySelectorAll('.btn-cancel-queue').forEach((btn) => {
          btn.addEventListener('click', async () => {
            const folderId = btn.dataset.id;
            try {
              await tauri.invoke('cancel_queued_folder', { folderId });
              await loadIndexingQueue();
            } catch (err) { console.error('cancel_queued_folder failed:', err); }
          });
        });
      }
    }
  }

  // Surgical recent processing stream rendering
  if (elements.recentStreamTbody) {
    if (!snapshot.recent_events || snapshot.recent_events.length === 0) {
      if (lastRecentEventsSignature !== 'empty') {
        lastRecentEventsSignature = 'empty';
        elements.recentStreamTbody.innerHTML = '<tr><td colspan="4" class="queue-empty-text">Waiting for indexing activity...</td></tr>';
      }
    } else {
      const reversedEvents = [...snapshot.recent_events].reverse().slice(0, 30);
      const recentSig = reversedEvents.map((ev) => `${ev.path}:${ev.status}`).join(';');
      if (recentSig !== lastRecentEventsSignature) {
        lastRecentEventsSignature = recentSig;
        elements.recentStreamTbody.innerHTML = reversedEvents
          .map((ev) => {
            const statusClass =
              ev.status === 'Indexed'
                ? 'status-pill-indexed'
                : ev.status === 'Skipped'
                  ? 'status-pill-skipped'
                  : 'status-pill-error';
            const ext = ev.extension ? `.${ev.extension}` : '—';
            return `
              <tr>
                <td><span class="ext-badge">${escapeHtml(ext)}</span></td>
                <td style="max-width: 320px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;" title="${escapeHtml(ev.path)}">
                  ${escapeHtml(ev.path)}
                </td>
                <td>${formatBytes(ev.size_bytes)}</td>
                <td><span class="${statusClass}">${escapeHtml(ev.status)}</span></td>
              </tr>
            `;
          })
          .join('');
      }
    }
  }
}

// Blacklist Management (D-06, D-07, D-08)
async function loadBlacklistFolders() {

  try {
    const list = await tauri.invoke('get_blacklist_folders');
    blacklistFolders = list || [];
    renderBlacklistFolders();
  } catch (err) {
    console.error('Failed to load blacklist folders:', err);
  }
}

function renderBlacklistFolders() {
  if (!elements.blacklistFoldersList) return;
  elements.blacklistFoldersList.innerHTML = '';
  if (elements.blacklistCountBadge) {
    elements.blacklistCountBadge.textContent = `${blacklistFolders.length} blacklisted`;
  }

  if (blacklistFolders.length === 0) {
    elements.blacklistFoldersList.innerHTML = `
      <div style="padding: 16px; text-align: center; color: var(--text-muted); font-size: 12px;">
        No folders blacklisted. Directories added here will be avoided during search crawls.
      </div>
    `;
    return;
  }

  blacklistFolders.forEach((folder) => {
    const card = document.createElement('div');
    card.className = 'blacklist-item-card';

    card.innerHTML = `
      <div style="display: flex; align-items: center; gap: 10px;">
        <span style="font-size: 16px;">🚫</span>
        <div style="display: flex; flex-direction: column; gap: 2px;">
          <span class="folder-path" title="${escapeHtml(folder.path)}" style="color: #fca5a5;">${escapeHtml(folder.path)}</span>
          <span style="font-size: 11px; color: var(--text-muted);">Avoids this directory and all nested subfolders</span>
        </div>
      </div>
      <button class="btn-icon-only btn-remove-blacklist" data-id="${folder.id}" title="Remove from blacklist" style="color: #f87171;">
        ✕
      </button>
    `;

    const removeBtn = card.querySelector('.btn-remove-blacklist');
    removeBtn.addEventListener('click', async () => {
      try {
        await tauri.invoke('remove_blacklist_folder', { id: folder.id });
        blacklistFolders = blacklistFolders.filter((b) => b.id !== folder.id);
        renderBlacklistFolders();
        renderWatchedFolders(); // Refresh override badges
        triggerSaveStatus('✓ Blacklist folder removed');
      } catch (err) {
        console.error('Failed to remove blacklist folder:', err);
      }
    });

    elements.blacklistFoldersList.appendChild(card);
  });
}

function setupBlacklistHandlers() {
  if (elements.btnAddBlacklist) {
    let isPicking = false;
    elements.btnAddBlacklist.addEventListener('click', async () => {
      if (isPicking) return;
      isPicking = true;
      try {
        const pickedPath = await tauri.invoke('pick_folder');
        if (pickedPath) {
          const record = await tauri.invoke('add_blacklist_folder', { path: pickedPath });
          if (record) {
            blacklistFolders.push(record);
            renderBlacklistFolders();
            renderWatchedFolders(); // Refresh override badges
            loadDeadLetterQueue();
            triggerSaveStatus('✓ Added to blacklist & purged from index');
          }
        }
      } catch (err) {
        console.error('Failed to add blacklist folder:', err);
      } finally {
        isPicking = false;
      }
    });
  }

  if (elements.btnRemoveBlacklist) {
    elements.btnRemoveBlacklist.addEventListener('click', async () => {
      if (blacklistFolders.length > 0) {
        const item = blacklistFolders[blacklistFolders.length - 1];
        try {
          await tauri.invoke('remove_blacklist_folder', { id: item.id });
          blacklistFolders = blacklistFolders.filter((b) => b.id !== item.id);
          renderBlacklistFolders();
          renderWatchedFolders();
          triggerSaveStatus('✓ Blacklist folder removed');
        } catch (err) {
          console.error('Failed to remove blacklist folder:', err);
        }
      }
    });
  }
}

// Filetype Analytics & Storage Breakdown (D-03, D-04, D-05)
async function loadAnalyticsData() {
  try {
    const [analytics, storage] = await Promise.all([
      tauri.invoke('get_corpus_analytics'),
      tauri.invoke('get_storage_breakdown'),
    ]);

    if (analytics) {
      renderCorpusAnalytics(analytics);
    }
    if (storage) {
      renderStorageBreakdown(storage);
    }
  } catch (err) {
    console.error('Failed to load analytics data:', err);
  }
}

function renderCorpusAnalytics(analytics) {
  if (elements.analyticsTotalFiles) elements.analyticsTotalFiles.textContent = formatNumber(analytics.total_files);
  if (elements.analyticsTotalBytes) elements.analyticsTotalBytes.textContent = formatBytes(analytics.total_bytes);
  if (elements.analyticsTotalChunks) elements.analyticsTotalChunks.textContent = formatNumber(analytics.total_chunks);
  if (elements.analyticsTopCategory) {
    elements.analyticsTopCategory.textContent = analytics.categories[0]?.category || '—';
  }

  if (!elements.analyticsCategoriesAccordion) return;
  elements.analyticsCategoriesAccordion.innerHTML = '';

  if (analytics.categories.length === 0) {
    elements.analyticsCategoriesAccordion.innerHTML = `
      <div style="padding: 24px; text-align: center; color: var(--text-muted);">
        No indexed files found. Monitored documents will appear here once scanned.
      </div>
    `;
    return;
  }

  const catIcons = {
    'Documents': '📄',
    'Code': '💻',
    'Images & OCR': '🖼️',
    'Data & Sheets': '📊',
    'Others': '📦',
  };

  analytics.categories.forEach((cat) => {
    const isExpanded = expandedCategories.has(cat.category);
    const item = document.createElement('div');
    item.className = `category-accordion-item ${isExpanded ? 'expanded' : ''}`;

    const icon = catIcons[cat.category] || '📁';

    item.innerHTML = `
      <div class="category-header-row" data-cat="${escapeHtml(cat.category)}">
        <div class="category-title-left">
          <span style="font-size: 18px;">${icon}</span>
          <span class="category-name">${escapeHtml(cat.category)}</span>
          <span class="category-file-badge">${cat.file_count} ${cat.file_count === 1 ? 'file' : 'files'}</span>
        </div>
        <div class="category-meta-right">
          <span class="category-stat-text">${formatBytes(cat.total_bytes)}</span>
          <span class="accordion-chevron ${isExpanded ? 'expanded' : ''}">▼</span>
        </div>
      </div>
      <div class="category-details-panel" style="${isExpanded ? '' : 'display: none;'}">
        ${renderExtensionRows(cat.extensions, cat.total_bytes)}
      </div>
    `;

    const header = item.querySelector('.category-header-row');
    header.addEventListener('click', () => {
      const catName = cat.category;
      if (expandedCategories.has(catName)) {
        expandedCategories.delete(catName);
      } else {
        expandedCategories.add(catName);
      }
      renderCorpusAnalytics(analytics);
    });

    elements.analyticsCategoriesAccordion.appendChild(item);
  });
}

function renderExtensionRows(extensions, catTotalBytes) {
  if (!extensions || extensions.length === 0) {
    return '<div class="extension-empty">No extension statistics available</div>';
  }

  const sorted = [...extensions].sort((a, b) => (b.total_bytes - a.total_bytes) || (b.file_count - a.file_count));

  const rows = sorted
    .map((ext) => {
      const pct = catTotalBytes > 0 ? Math.min(100, Math.round((ext.total_bytes / catTotalBytes) * 100)) : 0;
      const extLabel = ext.extension ? `.${ext.extension}` : '(no ext)';
      return `
        <div class="extension-stat-row">
          <div class="extension-col-ext">
            <span class="ext-badge" title="${escapeHtml(extLabel)}">${escapeHtml(extLabel)}</span>
          </div>
          <div class="extension-col-files">
            ${ext.file_count} ${ext.file_count === 1 ? 'file' : 'files'}
          </div>
          <div class="extension-col-bar">
            <div class="extension-progress-bar-container">
              <div class="extension-progress-bar-fill" style="width: ${pct}%;"></div>
            </div>
          </div>
          <div class="extension-col-chunks">
            ${formatNumber(ext.chunk_count)} chunks
          </div>
          <div class="extension-col-bytes">
            ${formatBytes(ext.total_bytes)}
          </div>
        </div>
      `;
    })
    .join('');

  return `
    <div class="extension-details-header">
      <div class="extension-col-ext">Extension</div>
      <div class="extension-col-files">Files</div>
      <div class="extension-col-bar">Share</div>
      <div class="extension-col-chunks">Chunks</div>
      <div class="extension-col-bytes">Size</div>
    </div>
    ${rows}
  `;
}

function renderStorageBreakdown(storage) {
  if (!storage) return;

  const total = (storage.sqlite_db_bytes + storage.text_chunks_bytes + storage.vector_index_bytes) || 1;

  const pctSqlite = Math.max(2, Math.round((storage.sqlite_db_bytes / total) * 100));
  const pctChunks = Math.max(2, Math.round((storage.text_chunks_bytes / total) * 100));
  const pctVectors = Math.max(2, Math.round((storage.vector_index_bytes / total) * 100));

  if (elements.barStorageSqlite) elements.barStorageSqlite.style.width = `${pctSqlite}%`;
  if (elements.barStorageChunks) elements.barStorageChunks.style.width = `${pctChunks}%`;
  if (elements.barStorageVectors) elements.barStorageVectors.style.width = `${pctVectors}%`;

  if (elements.legendValSqlite) elements.legendValSqlite.textContent = formatBytes(storage.sqlite_db_bytes);
  if (elements.legendValChunks) elements.legendValChunks.textContent = formatBytes(storage.text_chunks_bytes);
  if (elements.legendValVectors) elements.legendValVectors.textContent = formatBytes(storage.vector_index_bytes);
}

// System Hardware Telemetry Meters (D-09)
async function refreshTelemetry() {
  if (document.visibilityState !== 'visible') return;
  try {
    const telemetry = await tauri.invoke('get_system_telemetry');
    if (telemetry) {
      if (elements.meterRam) {
        elements.meterRam.textContent = `${telemetry.memory_rss_mb.toFixed(1)} MB`;
        if (elements.telemetryItemRam) {
          elements.telemetryItemRam.title = `Process Resident RAM (RSS): ${telemetry.memory_rss_mb.toFixed(1)} MB (Physical memory currently resident. Empty KV cache pages and untouched weights are faulted into RAM on demand).`;
        }
      }
      if (elements.meterMetal) {
        const metalMb = typeof telemetry.metal_memory_mb === 'number' ? telemetry.metal_memory_mb : 0;
        elements.meterMetal.textContent = metalMb > 0 ? `${metalMb.toFixed(1)} MB` : '0 MB';
        if (elements.telemetryItemMetal) {
          const { tokensFormatted, cacheFormatted } = getContextWindowMetrics();
          if (metalMb > 0) {
            elements.telemetryItemMetal.title = `Metal Unified Memory: ${metalMb.toFixed(1)} MB (Model weights + ${tokensFormatted} KV cache buffer [~${cacheFormatted}] reserved on Apple Silicon Unified Memory. May exceed idle Process RAM because cache pages are mapped on demand).`;
            elements.meterMetal.style.color = '#34c759';
          } else if (telemetry.metal_model_footprint_mb) {
            elements.telemetryItemMetal.title = `Metal Unified Memory: 0 MB (${telemetry.metal_model_footprint_mb.toFixed(1)} MB reserved on demand for weights + ${tokensFormatted} KV cache [~${cacheFormatted}]).`;
            elements.meterMetal.style.color = '';
          } else {
            elements.telemetryItemMetal.title = 'Metal Unified Memory: 0 MB (Apple Silicon Metal / CPU on demand)';
            elements.meterMetal.style.color = '';
          }
        }
      }
      if (elements.meterCpu) elements.meterCpu.textContent = `${telemetry.cpu_percent.toFixed(1)}%`;
      if (elements.meterStorage) elements.meterStorage.textContent = `${telemetry.db_size_mb.toFixed(1)} MB`;
    }
  } catch (_) { }
}

function setupTelemetryPolling() {
  refreshTelemetry();
  telemetryPollInterval = setInterval(refreshTelemetry, 2500);

  document.addEventListener('visibilitychange', () => {
    if (document.visibilityState === 'visible') {
      refreshTelemetry();
    }
  });
}

// Crawl Progress Polling Loop (D-09)
let wasCrawlRunning = false;
let isCrawlPollBusy = false;

function startCrawlProgressPolling() {
  if (crawlPollInterval) clearInterval(crawlPollInterval);

  const pollIntervalMs = 250;

  crawlPollInterval = setInterval(async () => {
    if (isCrawlPollBusy) return;
    isCrawlPollBusy = true;
    try {
      const progress = await tauri.invoke('get_crawl_progress');
      if (!progress) return;

      if (progress.is_running) {
        wasCrawlRunning = true;
        if (elements.crawlStatusDot) {
          elements.crawlStatusDot.style.background = 'var(--accent)';
          elements.crawlStatusDot.classList.add('pulsing');
        }
        const processed = progress.indexed_count + progress.skipped_count;
        if (elements.crawlStatusText) {
          elements.crawlStatusText.textContent = `Indexing... (${processed} / ${progress.total_discovered || processed} files • ${progress.indexed_count} new, ${progress.skipped_count} up-to-date)`;
        }

        if (elements.btnPauseCrawl) elements.btnPauseCrawl.style.display = progress.is_paused ? 'none' : 'inline-flex';
        if (elements.btnResumeCrawl) elements.btnResumeCrawl.style.display = progress.is_paused ? 'inline-flex' : 'none';

        if (elements.crawlProgressBarContainer) elements.crawlProgressBarContainer.style.display = 'block';
        const pct = progress.total_discovered > 0
          ? Math.min(100, Math.round((processed / progress.total_discovered) * 100))
          : 0;
        if (elements.crawlProgressBar) elements.crawlProgressBar.style.width = `${pct}%`;

        // Poll queue drawer to ensure progress bar updates reliably
        loadIndexingQueue();
      } else {
        if (elements.crawlStatusDot) {
          elements.crawlStatusDot.style.background = 'var(--success)';
          elements.crawlStatusDot.classList.remove('pulsing');
        }
        if (elements.crawlStatusText) {
          elements.crawlStatusText.textContent = `Live Indexing Engine: Idle • All sources synchronized`;
        }

        if (elements.btnPauseCrawl) elements.btnPauseCrawl.style.display = 'none';
        if (elements.btnResumeCrawl) elements.btnResumeCrawl.style.display = 'none';
        if (elements.crawlProgressBarContainer) elements.crawlProgressBarContainer.style.display = 'none';

        // Only sync queue & dead letter queue when transitioning from running to idle,
        // avoiding constant table queries that tie up SQLite
        if (wasCrawlRunning) {
          wasCrawlRunning = false;
          loadIndexingQueue();
          loadDeadLetterQueue();
        }
      }
    } catch (_) {
    } finally {
      isCrawlPollBusy = false;
    }
  }, pollIntervalMs);
}

// System Actions
function setupSystemActions() {
  if (elements.toggleAutoHide) {
    elements.toggleAutoHide.addEventListener('change', async (e) => {
      const enabled = e.target.checked;
      try {
        await tauri.invoke('set_auto_hide', { enabled });
        currentSettings.auto_hide = enabled;
        queueAutoSave();
      } catch (err) {
        console.error('Failed to toggle auto hide:', err);
      }
    });
  }

  if (elements.btnCenterWindow) {
    elements.btnCenterWindow.addEventListener('click', async () => {
      try {
        await tauri.invoke('center_window');
        triggerSaveStatus('✓ Search bar centered');
      } catch (err) {
        console.error('Failed to center window:', err);
      }
    });
  }

  if (elements.btnPruneVersions) {
    elements.btnPruneVersions.addEventListener('click', async () => {
      try {
        await tauri.invoke('prune_versions', { retentionDays: currentSettings.retention_days });
        triggerSaveStatus('✓ Revisions pruned');
      } catch (err) {
        console.error('Failed to prune versions:', err);
      }
    });
  }

  if (elements.btnReindexAll) {
    elements.btnReindexAll.addEventListener('click', () => {
      openResetDatabaseModal(false);
    });
  }

  if (elements.btnCleanDbReindexSources) {
    elements.btnCleanDbReindexSources.addEventListener('click', () => {
      openResetDatabaseModal(false);
    });
  }

  if (elements.btnFactoryReset) {
    elements.btnFactoryReset.addEventListener('click', () => {
      openResetDatabaseModal(true);
    });
  }
}

// Database Reset & Full Re-index Modal Controllers
function openResetDatabaseModal(isFactoryDefault = false) {
  if (!elements.resetDatabaseModal) return;

  if (isFactoryDefault) {
    if (elements.resetModalTitle) {
      elements.resetModalTitle.textContent = 'Factory Reset Everything';
    }
    if (elements.resetModalDesc) {
      elements.resetModalDesc.textContent =
        'This will erase all indexed search data, delete all watched folders and blacklist filters, and reset settings back to default values.';
    }
  } else {
    if (elements.resetModalTitle) {
      elements.resetModalTitle.textContent = 'Reset Database & Re-index All';
    }
    if (elements.resetModalDesc) {
      elements.resetModalDesc.textContent =
        'Are you sure you want to clean the entire SQLite search database and re-index all monitored folders from scratch?';
    }
  }

  elements.resetDatabaseModal.style.display = 'flex';
}

function closeResetDatabaseModal() {
  if (elements.resetDatabaseModal) {
    elements.resetDatabaseModal.style.display = 'none';
  }
}

function setupResetDatabaseModal() {
  if (elements.resetModalBtnClose) {
    elements.resetModalBtnClose.addEventListener('click', closeResetDatabaseModal);
  }
  if (elements.resetModalBtnCancel) {
    elements.resetModalBtnCancel.addEventListener('click', closeResetDatabaseModal);
  }
  if (elements.resetDatabaseModal) {
    elements.resetDatabaseModal.addEventListener('click', (e) => {
      if (e.target === elements.resetDatabaseModal) {
        closeResetDatabaseModal();
      }
    });
  }

  window.addEventListener('keydown', (e) => {
    if (e.key === 'Escape' && elements.resetDatabaseModal && elements.resetDatabaseModal.style.display === 'flex') {
      closeResetDatabaseModal();
    }
  });

  // Action 1: Clean DB & Re-index All
  if (elements.resetModalBtnCleanReindex) {
    elements.resetModalBtnCleanReindex.addEventListener('click', async () => {
      closeResetDatabaseModal();
      triggerSaveStatus('⚡ Cleaning database & starting re-indexing...');

      // Navigate to Watched Sources immediately so the Delete Queue card is visible
      switchTab('watched-sources');

      try {
        const res = await tauri.invoke('reset_database_and_reindex', {
          reindex: true,
          factoryReset: false,
        });

        triggerSaveStatus(res?.message || '✓ Database wiped & re-indexing started');
        // Reload delete queue to pick up the new job and start polling
        loadDeleteQueue().catch(() => { });
        await Promise.all([
          loadWatchedFolders(),
          loadAnalyticsData(),
          loadIndexingQueue(),
        ]);
      } catch (err) {
        console.error('Failed to reset database and reindex:', err);
        triggerSaveStatus('❌ Failed to reset database');
      }
    });
  }

  // Action 2: Factory Reset Everything
  if (elements.resetModalBtnFactory) {
    elements.resetModalBtnFactory.addEventListener('click', async () => {
      closeResetDatabaseModal();
      triggerSaveStatus('⚠️ Performing complete factory reset...');

      // Navigate to Watched Sources immediately so the Delete Queue card is visible
      switchTab('watched-sources');

      try {
        const res = await tauri.invoke('reset_database_and_reindex', {
          reindex: false,
          factoryReset: true,
        });

        triggerSaveStatus(res?.message || '✓ Factory reset complete');
        // Reload delete queue to pick up the new job and start polling
        loadDeleteQueue().catch(() => { });
        await Promise.all([
          loadSettings(),
          loadWatchedFolders(),
          loadBlacklistFolders(),
          loadAnalyticsData(),
          loadIndexingQueue(),
        ]);
      } catch (err) {
        console.error('Failed to factory reset:', err);
        triggerSaveStatus('❌ Failed to perform factory reset');
      }
    });
  }
}

// Diagnostics & App Logs Handlers (D-02, D-03)
let rawLogLines = [];

function setupDiagnosticsHandlers() {
  if (elements.btnRefreshLogs) {
    elements.btnRefreshLogs.addEventListener('click', () => loadAppLogs());
  }

  if (elements.btnCopyLogs) {
    elements.btnCopyLogs.addEventListener('click', async () => {
      try {
        const textToCopy = getFilteredLogText();
        await navigator.clipboard.writeText(textToCopy);
        triggerSaveStatus('✓ Copied logs to clipboard');
      } catch (err) {
        console.error('Failed to copy logs:', err);
      }
    });
  }

  if (elements.btnExportLogs) {
    elements.btnExportLogs.addEventListener('click', async () => {
      try {
        const dest = await tauri.invoke('export_app_logs', {});
        triggerSaveStatus(`✓ Exported to ${dest}`);
      } catch (err) {
        console.error('Failed to export logs:', err);
        triggerSaveStatus('Failed to export logs');
      }
    });
  }

  if (elements.btnRevealLogs) {
    elements.btnRevealLogs.addEventListener('click', async () => {
      try {
        await tauri.invoke('reveal_app_logs_in_finder');
      } catch (err) {
        console.error('Failed to reveal logs:', err);
      }
    });
  }

  if (elements.btnClearLogs) {
    elements.btnClearLogs.addEventListener('click', () => {
      rawLogLines = [];
      renderLogs();
    });
  }

  if (elements.logFilterLevel) {
    elements.logFilterLevel.addEventListener('change', () => renderLogs());
  }

  if (elements.logSearchInput) {
    elements.logSearchInput.addEventListener('input', () => renderLogs());
  }
}

async function loadAppLogs(tailLines = 250) {
  try {
    if (elements.logStatusIndicator) {
      elements.logStatusIndicator.textContent = 'Fetching...';
    }
    const lines = await tauri.invoke('get_app_logs', { tailLines, tail_lines: tailLines });
    if (Array.isArray(lines)) {
      rawLogLines = lines;
      renderLogs();
    }
    if (elements.logStatusIndicator) {
      elements.logStatusIndicator.textContent = 'Connected';
    }
  } catch (err) {
    console.error('Failed to load app logs:', err);
    if (elements.logStatusIndicator) {
      elements.logStatusIndicator.textContent = 'Error';
    }
  }
}

function parseLogLine(rawLine) {
  if (!rawLine || typeof rawLine !== 'string') {
    return { time: '', level: 'INFO', target: '', message: '', raw: '', isRaw: false };
  }

  const str = rawLine.trim();

  // Pattern 1: ISO timestamp, level, target with Rust colons (::), message
  // e.g. 2026-10-08T11:45:30.123456Z  INFO serchx::crawler: Started crawler
  const matchTracing = str.match(
    /^\[?(\d{4}-\d{2}-\d{2}[T\s](\d{2}:\d{2}:\d{2})(?:\.\d+)?Z?)\]?\s+([A-Z]+)\s+([\w\d_.:\-]+):\s*(.*)$/
  );
  if (matchTracing) {
    return {
      time: matchTracing[2],
      level: matchTracing[3].toUpperCase(),
      target: matchTracing[4].trim(),
      message: matchTracing[5].trim(),
      raw: str,
      isRaw: false,
    };
  }

  // Pattern 2: timestamp, level, message (without target colon)
  const matchTimeLevel = str.match(
    /^\[?(\d{4}-\d{2}-\d{2}[T\s](\d{2}:\d{2}:\d{2})(?:\.\d+)?Z?)\]?\s+([A-Z]+)\s+(.*)$/
  );
  if (matchTimeLevel) {
    return {
      time: matchTimeLevel[2],
      level: matchTimeLevel[3].toUpperCase(),
      target: 'app',
      message: matchTimeLevel[4].trim(),
      raw: str,
      isRaw: false,
    };
  }

  // Pattern 3: level only prefix, e.g. [ERROR] Something failed
  const matchLevelOnly = str.match(/^\[?(ERROR|WARN|WARNING|INFO|DEBUG|TRACE)\]?:?\s*(.*)$/i);
  if (matchLevelOnly) {
    let lvl = matchLevelOnly[1].toUpperCase();
    if (lvl === 'WARNING') lvl = 'WARN';
    return {
      time: '',
      level: lvl,
      target: 'app',
      message: matchLevelOnly[2].trim(),
      raw: str,
      isRaw: false,
    };
  }

  // Fallback / Continuation lines
  let detectedLevel = 'INFO';
  if (/error/i.test(str)) detectedLevel = 'ERROR';
  else if (/warn/i.test(str)) detectedLevel = 'WARN';
  else if (/debug/i.test(str)) detectedLevel = 'DEBUG';

  return {
    time: '',
    level: detectedLevel,
    target: '',
    message: str,
    raw: str,
    isRaw: true,
  };
}

function getFilteredLogText() {
  const selectedLevel = elements.logFilterLevel ? elements.logFilterLevel.value : 'ALL';
  const searchTerm = elements.logSearchInput ? elements.logSearchInput.value.toLowerCase().trim() : '';

  return rawLogLines
    .filter((line) => {
      const parsed = parseLogLine(line);
      if (selectedLevel !== 'ALL' && parsed.level !== selectedLevel && !line.includes(selectedLevel)) return false;
      if (searchTerm && !line.toLowerCase().includes(searchTerm)) return false;
      return true;
    })
    .join('\n');
}

function renderLogs() {
  if (!elements.logViewerTerminal) return;

  const selectedLevel = elements.logFilterLevel ? elements.logFilterLevel.value : 'ALL';
  const searchTerm = elements.logSearchInput ? elements.logSearchInput.value.toLowerCase().trim() : '';

  const parsedList = rawLogLines.map((line, idx) => ({
    lineNum: idx + 1,
    raw: line,
    parsed: parseLogLine(line),
  }));

  const filtered = parsedList.filter(({ raw, parsed }) => {
    if (selectedLevel !== 'ALL') {
      if (parsed.level !== selectedLevel && !raw.includes(selectedLevel)) return false;
    }
    if (searchTerm) {
      if (!raw.toLowerCase().includes(searchTerm) && !parsed.message.toLowerCase().includes(searchTerm) && !parsed.target.toLowerCase().includes(searchTerm)) {
        return false;
      }
    }
    return true;
  });

  if (elements.logCountTag) {
    elements.logCountTag.textContent = `${filtered.length} entries`;
  }

  if (filtered.length === 0) {
    elements.logViewerTerminal.innerHTML = '<div class="log-empty-state">No matching log entries found</div>';
    return;
  }

  const html = filtered
    .map(({ lineNum, parsed }) => {
      const pillClass =
        parsed.level === 'ERROR'
          ? 'log-pill-error'
          : parsed.level === 'WARN'
            ? 'log-pill-warn'
            : parsed.level === 'DEBUG'
              ? 'log-pill-debug'
              : parsed.level === 'TRACE'
                ? 'log-pill-trace'
                : 'log-pill-info';

      const timeDisplay = parsed.time ? escapeHtml(parsed.time) : '<span class="log-empty-val">—</span>';
      const targetDisplay = parsed.target ? escapeHtml(parsed.target) : '<span class="log-empty-val">—</span>';

      return `<div class="log-entry-line ${parsed.isRaw ? 'is-raw' : ''}">
        <span class="log-line-num">${lineNum}</span>
        <span class="log-line-time">${timeDisplay}</span>
        <span class="log-line-pill ${pillClass}">${escapeHtml(parsed.level)}</span>
        <span class="log-line-target" title="${parsed.target ? escapeHtml(parsed.target) : ''}">${targetDisplay}</span>
        <span class="log-line-msg">${escapeHtml(parsed.message)}</span>
      </div>`;
    })
    .join('');

  elements.logViewerTerminal.innerHTML = html;

  if (elements.toggleLogAutoscroll && elements.toggleLogAutoscroll.checked) {
    elements.logViewerTerminal.scrollTop = elements.logViewerTerminal.scrollHeight;
  }
}

// Keyboard Navigation & Shortcuts
function setupKeyboardShortcuts() {
  window.addEventListener('keydown', (e) => {
    // ⌘ W or Escape closes/hides the settings window
    if ((e.metaKey && e.key.toLowerCase() === 'w') || e.key === 'Escape') {
      e.preventDefault();
      try {
        window.close();
      } catch (_) { }
    }
    // ⌘ 1..6 tab navigation
    if (e.metaKey && ['1', '2', '3', '4', '5', '6'].includes(e.key)) {
      e.preventDefault();
      const tabs = ['neural-engine', 'watched-sources', 'analytics', 'retrieval-tuning', 'system-hotkeys', 'diagnostics'];
      const index = parseInt(e.key, 10) - 1;
      if (tabs[index]) {
        switchTab(tabs[index]);
      }
    }
  });
}

// Format Utilities
function formatBytes(bytes) {
  if (!bytes || bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i];
}

function formatNumber(num) {
  if (num === null || num === undefined) return '0';
  return Number(num).toLocaleString();
}

function formatRelativeTime(epochSec) {
  const elapsed = Math.floor(Date.now() / 1000) - epochSec;
  if (elapsed < 60) return 'just now';
  if (elapsed < 3600) return `${Math.floor(elapsed / 60)}m ago`;
  if (elapsed < 86400) return `${Math.floor(elapsed / 3600)}h ago`;
  return `${Math.floor(elapsed / 86400)}d ago`;
}

function escapeHtml(str) {
  if (!str) return '';
  return str
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#039;');
}

// Auto-run on document ready
if (document.readyState === 'loading') {
  document.addEventListener('DOMContentLoaded', initSettings);
} else {
  initSettings();
}

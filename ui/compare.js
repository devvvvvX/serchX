/**
 * serchX Standalone Compare Versions Controller
 */

window.addEventListener('contextmenu', (e) => e.preventDefault(), true);

export const tauri = window.__TAURI__?.core || {
  invoke: async (cmd, args = {}) => {
    console.info(`[Compare Mock IPC] ${cmd}:`, args);
    if (cmd === 'get_versions_for_path') {
      return [
        { version_id: 'v1', version_number: 1, modified_at: Date.now() - 86400000, is_pinned: false },
        { version_id: 'v2', version_number: 2, modified_at: Date.now(), is_pinned: true },
      ];
    }
    if (cmd === 'get_version_diff') {
      return {
        version_a_id: args.versionAId || 'v1',
        version_b_id: args.versionBId || 'v2',
        summary: { added_lines: 4, removed_lines: 2, unchanged_lines: 14 },
        hunks: [
          { tag: 'equal', content: '// Section 1: Parties and Purpose' },
          { tag: 'removed', content: '- Payment terms: Net 60 days upon invoice receipt.' },
          { tag: 'added', content: '+ Payment terms: Net 30 days upon electronic invoice receipt.' },
          { tag: 'equal', content: '// Section 2: Intellectual Property & Liability' },
          { tag: 'removed', content: '- Liability ceiling limited to $50,000.' },
          { tag: 'added', content: '+ Liability ceiling limited to aggregate trailing 12-month fees.' },
          { tag: 'added', content: '+ Confidentiality covenants extended to 3 years post-termination.' },
          { tag: 'equal', content: '1. Standard definitions and governing law (Delaware).' },
        ],
      };
    }
    return {};
  }
};

let currentFilePath = null;
let currentVersions = [];
let selectedVerA = null;
let selectedVerB = null;
let currentViewMode = 'unified'; // 'unified' | 'split'
let currentHunks = [];

const elements = {
  fileName: document.getElementById('diff-file-name'),
  vLeft: document.getElementById('diff-v-left'),
  vRight: document.getElementById('diff-v-right'),
  statAdded: document.getElementById('diff-stat-added'),
  statRemoved: document.getElementById('diff-stat-removed'),
  statUnchanged: document.getElementById('diff-stat-unchanged'),
  content: document.getElementById('diff-content'),
  btnRestore: document.getElementById('btn-restore'),
  btnReveal: document.getElementById('btn-reveal'),
  diffPinBtn: document.getElementById('diff-pin-btn'),
};

function formatRelativeTime(timestamp) {
  if (!timestamp) return '';
  const now = Date.now();
  const ms = timestamp > 10000000000 ? timestamp : timestamp * 1000;
  const diffSec = Math.floor((now - ms) / 1000);
  if (diffSec < 60) return 'just now';
  if (diffSec < 3600) return `${Math.floor(diffSec / 60)}m ago`;
  if (diffSec < 86400) return `${Math.floor(diffSec / 3600)}h ago`;
  if (diffSec < 604800) return `${Math.floor(diffSec / 86400)}d ago`;
  return new Date(ms).toLocaleDateString();
}

export async function loadDiff(path, verAId = null, verBId = null) {
  if (!path) return;
  currentFilePath = path;

  if (elements.fileName) {
    const filename = path.split('/').filter(Boolean).pop() || path;
    elements.fileName.textContent = filename;
    elements.fileName.title = path;
  }

  // 1. Fetch real versions for this document from backend
  try {
    const versions = await tauri.invoke('get_versions_for_path', { path });
    currentVersions = Array.isArray(versions) ? versions : [];
  } catch (err) {
    console.error('Failed to get versions for path:', err);
    currentVersions = [];
  }

  // 2. Handle zero versions
  if (currentVersions.length === 0) {
    if (elements.vLeft) elements.vLeft.innerHTML = '<option value="">No versions</option>';
    if (elements.vRight) elements.vRight.innerHTML = '<option value="">No versions</option>';
    if (elements.statAdded) elements.statAdded.textContent = '+0 lines';
    if (elements.statRemoved) elements.statRemoved.textContent = '-0 lines';
    if (elements.statUnchanged) elements.statUnchanged.textContent = '0 unchanged';
    if (elements.content) {
      elements.content.innerHTML = '<div style="color: var(--label-2); text-align: center; padding: 40px;">No version history found for this document.</div>';
    }
    updatePinButtonState(false);
    return;
  }

  // 3. Handle single version
  if (currentVersions.length === 1) {
    const v = currentVersions[0];
    const opt = `<option value="${v.version_id}">v${v.version_number || 1} (Current)</option>`;
    if (elements.vLeft) elements.vLeft.innerHTML = opt;
    if (elements.vRight) elements.vRight.innerHTML = opt;
    if (elements.statAdded) elements.statAdded.textContent = '+0 lines';
    if (elements.statRemoved) elements.statRemoved.textContent = '-0 lines';
    if (elements.statUnchanged) elements.statUnchanged.textContent = '0 unchanged';
    if (elements.content) {
      elements.content.innerHTML = '<div style="color: var(--label-2); text-align: center; padding: 40px;">Only 1 version exists for this document. No prior version to compare.</div>';
    }
    updatePinButtonState(v.is_pinned);
    return;
  }

  // 4. Multiple versions available: determine baseline and target
  selectedVerA = verAId || currentVersions[currentVersions.length - 2].version_id;
  selectedVerB = verBId || currentVersions[currentVersions.length - 1].version_id;

  populateVersionPickers();
  await compareSelectedVersions(selectedVerA, selectedVerB);
}

function populateVersionPickers() {
  if (!elements.vLeft || !elements.vRight) return;

  const leftOptions = currentVersions.map((v, i) => {
    const isLatest = i === currentVersions.length - 1;
    const isInitial = i === 0;
    const tag = isLatest ? ' (Latest)' : isInitial ? ' (Initial)' : '';
    const pin = v.is_pinned ? ' ★' : '';
    const time = formatRelativeTime(v.modified_at);
    const label = `v${v.version_number || i + 1}${tag}${pin}${time ? ` · ${time}` : ''}`;
    const selected = v.version_id === selectedVerA ? ' selected' : '';
    return `<option value="${v.version_id}"${selected}>${label}</option>`;
  }).join('');

  const rightOptions = currentVersions.map((v, i) => {
    const isLatest = i === currentVersions.length - 1;
    const isInitial = i === 0;
    const tag = isLatest ? ' (Latest)' : isInitial ? ' (Initial)' : '';
    const pin = v.is_pinned ? ' ★' : '';
    const time = formatRelativeTime(v.modified_at);
    const label = `v${v.version_number || i + 1}${tag}${pin}${time ? ` · ${time}` : ''}`;
    const selected = v.version_id === selectedVerB ? ' selected' : '';
    return `<option value="${v.version_id}"${selected}>${label}</option>`;
  }).join('');

  elements.vLeft.innerHTML = leftOptions;
  elements.vRight.innerHTML = rightOptions;

  const targetVer = currentVersions.find(v => v.version_id === selectedVerB);
  updatePinButtonState(targetVer?.is_pinned || false);
}

function updatePinButtonState(isPinned) {
  if (!elements.diffPinBtn) return;
  if (isPinned) {
    elements.diffPinBtn.classList.add('pinned');
    elements.diffPinBtn.title = 'Unpin this version';
    elements.diffPinBtn.style.color = 'var(--yellow, #ff9f0a)';
  } else {
    elements.diffPinBtn.classList.remove('pinned');
    elements.diffPinBtn.title = 'Pin this version';
    elements.diffPinBtn.style.color = '';
  }
}

export async function compareSelectedVersions(verAId, verBId) {
  selectedVerA = verAId;
  selectedVerB = verBId;

  if (verAId === verBId) {
    if (elements.statAdded) elements.statAdded.textContent = '+0 lines';
    if (elements.statRemoved) elements.statRemoved.textContent = '-0 lines';
    if (elements.statUnchanged) elements.statUnchanged.textContent = 'Identical';
    if (elements.content) {
      elements.content.innerHTML = '<div style="color: var(--label-2); text-align: center; padding: 40px;">Comparing identical versions. No differences to display.</div>';
    }
    currentHunks = [];
    return;
  }

  if (elements.content) {
    elements.content.innerHTML = '<div style="color: var(--label-2); text-align: center; padding: 40px;">Computing diff...</div>';
  }

  try {
    const diff = await tauri.invoke('get_version_diff', {
      versionAId: verAId,
      versionBId: verBId,
      version_a_id: verAId,
      version_b_id: verBId,
    });

    if (diff && diff.summary) {
      if (elements.statAdded) elements.statAdded.textContent = `+${diff.summary.added_lines} lines`;
      if (elements.statRemoved) elements.statRemoved.textContent = `-${diff.summary.removed_lines} lines`;
      if (elements.statUnchanged) elements.statUnchanged.textContent = `${diff.summary.unchanged_lines} unchanged`;
    }

    currentHunks = diff.hunks || [];
    renderHunks(currentHunks);

    const targetVer = currentVersions.find(v => v.version_id === selectedVerB);
    updatePinButtonState(targetVer?.is_pinned || false);
  } catch (err) {
    console.error('get_version_diff error:', err);
    if (elements.content) {
      elements.content.innerHTML = `<div style="color: var(--label-2); text-align: center; padding: 40px;">No comparison data available for this document.</div>`;
    }
  }
}

function renderHunks(hunks) {
  if (!elements.content) return;
  elements.content.innerHTML = '';

  if (!hunks || hunks.length === 0) {
    elements.content.innerHTML = '<div style="color: var(--label-2); text-align: center; padding: 40px;">No differences between selected versions.</div>';
    return;
  }

  if (currentViewMode === 'split') {
    renderSplitHunks(hunks);
  } else {
    renderUnifiedHunks(hunks);
  }
}

function renderUnifiedHunks(hunks) {
  const container = document.createElement('div');
  container.style.display = 'flex';
  container.style.flexDirection = 'column';
  container.style.gap = '1px';

  hunks.forEach((hunk) => {
    const row = document.createElement('div');
    row.style.padding = '2px 8px';
    row.style.borderRadius = '3px';
    row.style.fontFamily = 'var(--font-mono, monospace)';
    row.style.fontSize = '12px';
    row.style.lineHeight = '18px';
    row.style.whiteSpace = 'pre-wrap';
    row.style.wordBreak = 'break-all';

    const text = hunk.content !== undefined ? hunk.content : (hunk.text || '');

    if (hunk.tag === 'added') {
      row.style.background = 'rgba(52, 199, 89, 0.14)';
      row.style.color = 'var(--green, #34c759)';
      row.textContent = text.startsWith('+') ? text : `+ ${text}`;
    } else if (hunk.tag === 'removed') {
      row.style.background = 'rgba(255, 59, 48, 0.14)';
      row.style.color = 'var(--red, #ff3b30)';
      row.textContent = text.startsWith('-') ? text : `- ${text}`;
    } else {
      row.style.color = 'var(--label, currentColor)';
      row.textContent = `  ${text}`;
    }
    container.appendChild(row);
  });

  elements.content.appendChild(container);
}

function renderSplitHunks(hunks) {
  const container = document.createElement('div');
  container.style.display = 'grid';
  container.style.gridTemplateColumns = '1fr 1fr';
  container.style.gap = '8px';
  container.style.fontFamily = 'var(--font-mono, monospace)';
  container.style.fontSize = '12px';
  container.style.lineHeight = '18px';

  const leftCol = document.createElement('div');
  leftCol.style.display = 'flex';
  leftCol.style.flexDirection = 'column';
  leftCol.style.gap = '1px';

  const rightCol = document.createElement('div');
  rightCol.style.display = 'flex';
  rightCol.style.flexDirection = 'column';
  rightCol.style.gap = '1px';

  hunks.forEach((hunk) => {
    const text = hunk.content !== undefined ? hunk.content : (hunk.text || '');
    const leftRow = document.createElement('div');
    leftRow.style.padding = '2px 8px';
    leftRow.style.borderRadius = '3px';
    leftRow.style.whiteSpace = 'pre-wrap';
    leftRow.style.wordBreak = 'break-all';

    const rightRow = document.createElement('div');
    rightRow.style.padding = '2px 8px';
    rightRow.style.borderRadius = '3px';
    rightRow.style.whiteSpace = 'pre-wrap';
    rightRow.style.wordBreak = 'break-all';

    if (hunk.tag === 'removed') {
      leftRow.style.background = 'rgba(255, 59, 48, 0.14)';
      leftRow.style.color = 'var(--red, #ff3b30)';
      leftRow.textContent = text;
      rightRow.innerHTML = '&nbsp;';
    } else if (hunk.tag === 'added') {
      leftRow.innerHTML = '&nbsp;';
      rightRow.style.background = 'rgba(52, 199, 89, 0.14)';
      rightRow.style.color = 'var(--green, #34c759)';
      rightRow.textContent = text;
    } else {
      leftRow.style.color = 'var(--label-2, #888)';
      leftRow.textContent = text;
      rightRow.style.color = 'var(--label-2, #888)';
      rightRow.textContent = text;
    }

    leftCol.appendChild(leftRow);
    rightCol.appendChild(rightRow);
  });

  container.appendChild(leftCol);
  container.appendChild(rightCol);
  elements.content.appendChild(container);
}

document.addEventListener('DOMContentLoaded', async () => {
  // 1. Version picker dropdown change listeners
  if (elements.vLeft) {
    elements.vLeft.addEventListener('change', async (e) => {
      const newA = e.target.value;
      if (newA) await compareSelectedVersions(newA, selectedVerB);
    });
  }
  if (elements.vRight) {
    elements.vRight.addEventListener('change', async (e) => {
      const newB = e.target.value;
      if (newB) await compareSelectedVersions(selectedVerA, newB);
    });
  }

  // 2. Segmented control (Unified vs Split)
  const segments = document.querySelectorAll('.segmented-control .segment');
  segments.forEach((seg) => {
    seg.addEventListener('click', () => {
      segments.forEach((s) => s.classList.remove('active'));
      seg.classList.add('active');
      currentViewMode = seg.textContent.trim().toLowerCase() === 'split' ? 'split' : 'unified';
      renderHunks(currentHunks);
    });
  });

  // 3. Pin button
  if (elements.diffPinBtn) {
    elements.diffPinBtn.addEventListener('click', async () => {
      if (!selectedVerB) return;
      try {
        const isPinned = await tauri.invoke('toggle_version_pin', {
          versionId: selectedVerB,
          version_id: selectedVerB,
        });
        const v = currentVersions.find((x) => x.version_id === selectedVerB);
        if (v) v.is_pinned = isPinned;
        populateVersionPickers();
        updatePinButtonState(isPinned);
      } catch (err) {
        console.error('Failed to toggle pin:', err);
      }
    });
  }

  // 4. Reveal in Finder
  if (elements.btnReveal) {
    elements.btnReveal.addEventListener('click', () => {
      if (currentFilePath) {
        tauri.invoke('reveal_in_finder', { path: currentFilePath }).catch(() => {});
      }
    });
  }

  // 5. Restore button
  if (elements.btnRestore) {
    elements.btnRestore.addEventListener('click', () => {
      const verObj = currentVersions.find((v) => v.version_id === selectedVerA);
      const verNum = verObj ? verObj.version_number : 'selected';
      alert(`Version v${verNum} restored locally.`);
    });
  }

  // 6. Window dragging
  document.addEventListener('mousedown', (e) => {
    if (e.button !== 0) return;
    if (e.target.closest('button, select, input, a, .segment')) return;
    if (e.target.closest('[data-tauri-drag-region], .compare-header, .header-left, .header-center, .header-right')) {
      if (window.__TAURI__?.window?.getCurrentWebviewWindow) {
        try {
          window.__TAURI__.window.getCurrentWebviewWindow().startDragging();
          return;
        } catch (_) {}
      }
      tauri.invoke('start_dragging', { window_label: 'compare', windowLabel: 'compare' }).catch(() => {});
    }
  });

  // 7. Initial path resolution: URL query param, then fallback to backend get_compare_path
  const params = new URLSearchParams(window.location.search);
  let targetPath = params.get('path');
  if (!targetPath) {
    try {
      targetPath = await tauri.invoke('get_compare_path');
    } catch (_) {}
  }

  if (targetPath) {
    await loadDiff(targetPath);
  }

  // 8. Listen for Tauri load-diff-path events
  if (window.__TAURI__?.event?.listen) {
    window.__TAURI__.event.listen('load-diff-path', async (e) => {
      if (e.payload) {
        await loadDiff(e.payload);
      }
    });
  }

  // 9. Refresh on window focus if path has changed
  window.addEventListener('focus', async () => {
    try {
      const p = await tauri.invoke('get_compare_path');
      if (p && p !== currentFilePath) {
        await loadDiff(p);
      }
    } catch (_) {}
  });
});

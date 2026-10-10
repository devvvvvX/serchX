/**
 * serchX Modern Onboarding Wizard Controller
 * 3-Step Guided Setup: Welcome -> Folders to Index -> Neural Engine Setup
 */

window.addEventListener('contextmenu', (e) => e.preventDefault(), true);

export const tauri = window.__TAURI__?.core || {
  invoke: async (cmd, args = {}) => {
    console.info(`[Onboarding Mock IPC] ${cmd}:`, args);
    return {};
  }
};

document.addEventListener('DOMContentLoaded', async () => {
  // Navigation elements
  const btnBack = document.getElementById('btn-back');
  const btnNext = document.getElementById('btn-next');
  const btnSkip = document.getElementById('btn-skip-setup');

  const dotStep1 = document.getElementById('dot-step-1');
  const dotStep2 = document.getElementById('dot-step-2');
  const dotStep3 = document.getElementById('dot-step-3');

  const paneStep1 = document.getElementById('step-1');
  const paneStep2 = document.getElementById('step-2');
  const paneStep3 = document.getElementById('step-3');

  // Step 2 elements
  const checkDocs = document.getElementById('check-folder-documents');
  const checkDesktop = document.getElementById('check-folder-desktop');
  const checkDownloads = document.getElementById('check-folder-downloads');
  const btnPickCustomFolder = document.getElementById('btn-pick-custom-folder');
  const customFoldersContainer = document.getElementById('custom-folders-container');

  // Step 3 elements
  const cardNative = document.getElementById('card-ai-native');
  const cardServer = document.getElementById('card-ai-server');
  const cardNone = document.getElementById('card-ai-none');
  const hardwareBadge = document.getElementById('ai-hardware-badge');
  const nativeModelDesc = document.getElementById('ai-native-model-desc');
  const checkDownloadNative = document.getElementById('check-download-native-model');
  const downloadNativeLabel = document.getElementById('download-native-model-label');
  const selectProvider = document.getElementById('select-onboarding-provider');
  const inputEndpoint = document.getElementById('input-onboarding-endpoint');
  const btnTestServer = document.getElementById('btn-onboarding-test-server');
  const serverStatus = document.getElementById('onboarding-server-status');

  // Wizard state
  let currentStep = 1;
  let selectedAiMode = 'native'; // 'native' | 'server' | 'none'
  let recommendedModelId = 'occ-rag-0.6b-q4_k_m';
  let recommendedModelName = 'OCC-RAG 0.6B';
  let isRecommendedModelDownloaded = false;
  const customFolders = [];

  // ==========================================
  // 1. Hardware Detection & Model Intelligence
  // ==========================================
  try {
    const profile = await tauri.invoke('get_hardware_profile');
    if (profile) {
      const chip = profile.chip_name || 'Apple Silicon';
      const ram = profile.total_memory_gb || 8;

      if (ram <= 8) {
        recommendedModelId = 'occ-rag-0.6b-q4_k_m';
        recommendedModelName = 'OCC-RAG 0.6B';
        if (hardwareBadge) hardwareBadge.textContent = 'Optimized for 8GB RAM';
        if (nativeModelDesc) {
          nativeModelDesc.textContent = `Lightweight OCC-RAG 0.6B model tailored for your ${chip} (${ram} GB RAM). Instant on-device answers with minimal RAM footprint.`;
        }
        if (downloadNativeLabel) {
          downloadNativeLabel.textContent = `Download OCC-RAG 0.6B in background (~396 MB)`;
        }
      } else if (ram <= 18) {
        recommendedModelId = 'qwen3.5-2b-q4_k_m';
        recommendedModelName = 'Qwen 3.5 2B';
        if (hardwareBadge) hardwareBadge.textContent = 'Recommended for your Mac';
        if (nativeModelDesc) {
          nativeModelDesc.textContent = `Qwen 3.5 2B model optimized for your ${chip} (${ram} GB RAM). High quality reasoning, search synthesis, and summaries with Apple Metal acceleration.`;
        }
        if (downloadNativeLabel) {
          downloadNativeLabel.textContent = `Download Qwen 3.5 2B in background (~1.3 GB)`;
        }
      } else {
        recommendedModelId = 'qwen3.8-4b-distill-q4_k_m';
        recommendedModelName = 'Qwen 3.8 4B Distill';
        if (hardwareBadge) hardwareBadge.textContent = 'High Capacity Model';
        if (nativeModelDesc) {
          nativeModelDesc.textContent = `Qwen 3.8 4B Distill model optimized for your ${chip} (${ram} GB RAM). Deep document comprehension and high-capacity local synthesis.`;
        }
        if (downloadNativeLabel) {
          downloadNativeLabel.textContent = `Download Qwen 3.8 4B Distill in background (~2.8 GB)`;
        }
      }
    }
  } catch (err) {
    console.warn('Could not read hardware profile:', err);
  }

  // Check if recommended model is already downloaded locally or from LM Studio
  try {
    const catalog = await tauri.invoke('get_model_catalog');
    if (catalog && Array.isArray(catalog)) {
      const rec = catalog.find((m) => m.id === recommendedModelId);
      if (rec && rec.is_downloaded) {
        isRecommendedModelDownloaded = true;
        if (downloadNativeLabel) {
          downloadNativeLabel.textContent = `${recommendedModelName} is already installed on your Mac`;
        }
        if (checkDownloadNative) {
          checkDownloadNative.checked = true;
          checkDownloadNative.disabled = true;
        }
      }
    }
  } catch (err) {
    console.warn('Could not check model catalog:', err);
  }

  // Pre-probe for local servers in background (LM Studio / Ollama)
  try {
    const endpoints = await tauri.invoke('probe_local_endpoints');
    if (endpoints && endpoints.length > 0) {
      const lmStudio = endpoints.find((e) => e.provider === 'lm_studio');
      const ollama = endpoints.find((e) => e.provider === 'ollama');
      if (lmStudio && selectProvider && inputEndpoint) {
        selectProvider.value = 'lm_studio';
        inputEndpoint.value = lmStudio.endpoint;
        if (serverStatus) {
          serverStatus.textContent = '✓ Detected running LM Studio server';
          serverStatus.style.color = 'var(--accent, #0a84ff)';
        }
      } else if (ollama && selectProvider && inputEndpoint) {
        selectProvider.value = 'ollama';
        inputEndpoint.value = ollama.endpoint;
        if (serverStatus) {
          serverStatus.textContent = '✓ Detected running Ollama server';
          serverStatus.style.color = 'var(--accent, #0a84ff)';
        }
      }
    }
  } catch (_) {}

  // ==========================================
  // 2. Step Navigation
  // ==========================================
  function goToStep(step) {
    currentStep = step;

    // Panes
    paneStep1.classList.toggle('active', step === 1);
    paneStep2.classList.toggle('active', step === 2);
    paneStep3.classList.toggle('active', step === 3);

    // Indicator Dots
    dotStep1.classList.toggle('active', step === 1);
    dotStep1.classList.toggle('completed', step > 1);

    dotStep2.classList.toggle('active', step === 2);
    dotStep2.classList.toggle('completed', step > 2);

    dotStep3.classList.toggle('active', step === 3);

    // Navigation buttons
    if (step === 1) {
      btnBack.style.display = 'none';
      btnNext.textContent = 'Get Started →';
    } else if (step === 2) {
      btnBack.style.display = 'inline-flex';
      btnNext.textContent = 'Continue →';
    } else if (step === 3) {
      btnBack.style.display = 'inline-flex';
      btnNext.textContent = 'Finish Setup';
    }
  }

  btnNext.addEventListener('click', async () => {
    if (currentStep === 1) {
      goToStep(2);
    } else if (currentStep === 2) {
      goToStep(3);
    } else if (currentStep === 3) {
      await finishOnboarding();
    }
  });

  btnBack.addEventListener('click', () => {
    if (currentStep > 1) {
      goToStep(currentStep - 1);
    }
  });

  btnSkip.addEventListener('click', async () => {
    await completeWithoutChanges();
  });

  // ==========================================
  // 3. Step 2: Custom Folder Selection
  // ==========================================
  function renderCustomFolders() {
    if (!customFoldersContainer) return;
    customFoldersContainer.innerHTML = '';

    customFolders.forEach((folderPath, index) => {
      const itemEl = document.createElement('div');
      itemEl.className = 'folder-item';

      const folderName = folderPath.split('/').filter(Boolean).pop() || folderPath;

      itemEl.innerHTML = `
        <div class="folder-info">
          <div class="folder-icon">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/>
            </svg>
          </div>
          <div class="folder-texts">
            <div class="folder-name-row">
              <span class="folder-name">${escapeHtml(folderName)}</span>
              <span class="folder-tag custom">Custom</span>
            </div>
            <span class="folder-subpath">${escapeHtml(folderPath)}</span>
          </div>
        </div>
        <div class="folder-actions">
          <button type="button" class="btn-remove-folder" title="Remove folder" data-idx="${index}">
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <line x1="18" y1="6" x2="6" y2="18"></line>
              <line x1="6" y1="6" x2="18" y2="18"></line>
            </svg>
          </button>
        </div>
      `;

      const removeBtn = itemEl.querySelector('.btn-remove-folder');
      removeBtn.addEventListener('click', (e) => {
        e.stopPropagation();
        customFolders.splice(index, 1);
        renderCustomFolders();
      });

      customFoldersContainer.appendChild(itemEl);
    });
  }

  if (btnPickCustomFolder) {
    btnPickCustomFolder.addEventListener('click', async () => {
      btnPickCustomFolder.disabled = true;
      try {
        const picked = await tauri.invoke('pick_folder');
        if (picked && !customFolders.includes(picked)) {
          customFolders.push(picked);
          renderCustomFolders();
        }
      } catch (err) {
        console.error('Failed to pick folder:', err);
      } finally {
        btnPickCustomFolder.disabled = false;
      }
    });
  }

  // ==========================================
  // 4. Step 3: Neural Engine Setup
  // ==========================================
  function selectAiOption(mode) {
    selectedAiMode = mode;
    cardNative.classList.toggle('selected', mode === 'native');
    cardServer.classList.toggle('selected', mode === 'server');
    cardNone.classList.toggle('selected', mode === 'none');

    // Show/hide subconfigs
    const nativeSub = cardNative.querySelector('.ai-subconfig');
    const serverSub = cardServer.querySelector('.ai-subconfig');
    if (nativeSub) nativeSub.style.display = mode === 'native' ? 'flex' : 'none';
    if (serverSub) serverSub.style.display = mode === 'server' ? 'flex' : 'none';
  }

  cardNative.addEventListener('click', (e) => {
    if (e.target.closest('input, label')) return;
    selectAiOption('native');
  });

  cardServer.addEventListener('click', (e) => {
    if (e.target.closest('input, select, button')) return;
    selectAiOption('server');
  });

  cardNone.addEventListener('click', () => {
    selectAiOption('none');
  });

  if (selectProvider) {
    selectProvider.addEventListener('change', () => {
      if (selectProvider.value === 'lm_studio') {
        inputEndpoint.value = 'http://127.0.0.1:1234';
      } else if (selectProvider.value === 'ollama') {
        inputEndpoint.value = 'http://127.0.0.1:11434';
      }
      if (serverStatus) serverStatus.textContent = '';
    });
  }

  if (btnTestServer) {
    btnTestServer.addEventListener('click', async (e) => {
      e.stopPropagation();
      const prov = selectProvider.value;
      const endpoint = inputEndpoint.value.trim();
      serverStatus.textContent = 'Checking...';
      serverStatus.style.color = 'var(--label-2)';

      try {
        const probeList = await tauri.invoke('probe_local_endpoints');
        const found = probeList?.find((p) => p.provider === prov || p.endpoint === endpoint);
        if (found) {
          serverStatus.textContent = `✓ Connected to ${prov === 'lm_studio' ? 'LM Studio' : 'Ollama'}`;
          serverStatus.style.color = '#34c759';
        } else {
          serverStatus.textContent = `⚠️ No response at ${endpoint}`;
          serverStatus.style.color = '#ff9500';
        }
      } catch (_) {
        serverStatus.textContent = `⚠️ Could not reach server`;
        serverStatus.style.color = '#ff9500';
      }
    });
  }

  // ==========================================
  // 5. Completion Handlers
  // ==========================================
  async function finishOnboarding() {
    btnNext.disabled = true;
    btnNext.textContent = 'Configuring...';

    // 1. Collect selected folders
    const selectedFolders = [];
    if (checkDocs?.checked) selectedFolders.push('~/Documents');
    if (checkDesktop?.checked) selectedFolders.push('~/Desktop');
    if (checkDownloads?.checked) selectedFolders.push('~/Downloads');
    selectedFolders.push(...customFolders);

    // 2. Add watched folders (asynchronously registers them and analyses without blocking)
    for (const folderPath of selectedFolders) {
      try {
        await tauri.invoke('add_watched_folder', { path: folderPath, recursive: true });
      } catch (err) {
        console.warn('Failed to add watched folder:', folderPath, err);
      }
    }

    // 3. Configure AI Engine in AppSettings
    try {
      const settings = await tauri.invoke('get_settings');
      if (settings) {
        if (selectedAiMode === 'native') {
          settings.llm_provider = 'native';
          settings.active_llm_model = recommendedModelId;
          settings.llm_enabled = true;

          // Start background model download if opted in and not already downloaded
          if (checkDownloadNative?.checked && !isRecommendedModelDownloaded) {
            try {
              await tauri.invoke('set_active_model', { modelId: recommendedModelId });
              await tauri.invoke('start_model_download', { modelId: recommendedModelId });
            } catch (dlErr) {
              console.warn('Failed to trigger background model download:', dlErr);
            }
          }
        } else if (selectedAiMode === 'server') {
          const provider = selectProvider?.value || 'lm_studio';
          const endpoint = inputEndpoint?.value?.trim() || 'http://127.0.0.1:1234';
          settings.llm_provider = provider;
          settings.custom_llm_endpoint = endpoint;
          settings.llm_enabled = true;
        } else if (selectedAiMode === 'none') {
          settings.llm_enabled = false;
        }

        await tauri.invoke('update_settings', { settings });
      }
    } catch (err) {
      console.warn('Failed to save onboarding settings:', err);
    }

    // 4. Mark onboarding complete & open search
    localStorage.setItem('serchx_onboarded', 'true');
    await finalizeAndOpenMain();
  }

  async function completeWithoutChanges() {
    localStorage.setItem('serchx_onboarded', 'true');
    await finalizeAndOpenMain();
  }

  async function finalizeAndOpenMain() {
    try {
      await tauri.invoke('show_window');
    } catch (_) {}

    if (window.__TAURI__?.window?.getCurrentWebviewWindow) {
      try {
        const win = window.__TAURI__.window.getCurrentWebviewWindow();
        await win.close();
        return;
      } catch (_) {}
    }
    window.close();
  }

  function escapeHtml(str) {
    return String(str)
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;')
      .replace(/"/g, '&quot;')
      .replace(/'/g, '&#39;');
  }

  // Window drag support
  document.addEventListener('mousedown', (e) => {
    if (e.button !== 0) return;
    if (e.target.closest('button, input, select, label, a, .folder-item, .ai-option-card')) return;
    if (e.target.closest('[data-tauri-drag-region], .onboarding-header')) {
      if (window.__TAURI__?.window?.getCurrentWebviewWindow) {
        try {
          window.__TAURI__.window.getCurrentWebviewWindow().startDragging();
          return;
        } catch (_) {}
      }
      tauri.invoke('start_dragging', { window_label: 'onboarding', windowLabel: 'onboarding' }).catch(() => {});
    }
  });
});

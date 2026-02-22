/**
 * Chord - Main Application Entry Point
 * Architecture: Backend owns all state. Frontend is a pure view layer.
 */

import UI from '@/js/ui.ts';
import Downloader from '@/js/downloader.ts';
import { getConfig, updateConfig, applyConfigToUI } from '@/js/config.ts';
import {
  setupEventListeners,
  setupTauriListeners,
  setupWindowControls
} from '@/js/events.ts';
import {
  cancelDownload,
  openDownloadFolder,
  pauseDownload,
  resumeDownload
} from '@/js/actions.ts';
import { ChordWindow } from '@/js/types.ts';

// Component Imports
import setupHtml from '@/components/setup.html?raw';
import mainContentHtml from '@/components/main_content.html?raw';
import settingsHtml from '@/components/settings.html?raw';
import confirmHtml from '@/components/confirm.html?raw';


let isReconciling = false;


/**
 * Inject HTML components into the DOM
 */
function injectComponents() {
  const container = document.getElementById('dynamic-content');
  if (container) {
    container.innerHTML = `
      ${setupHtml}
      ${mainContentHtml}
      ${settingsHtml}
      ${confirmHtml}
    `;
  }
}

/**
 * Initialize the application
 */
async function init() {
  // Setup Tauri event listeners FIRST (before backend emits events)
  await setupTauriListeners(checkDependencies);

  const appWindow = window as ChordWindow;

  if (appWindow.__TAURI__) {
    try {
      await appWindow.__TAURI__.window.getCurrentWindow().show();
    } catch (err) {
      console.error('Failed to show window:', err);
    }
  }

  UI.init();

  try {
    const resumableDownloads = await Downloader.initializeApp();
    resumableDownloads.forEach(task => {
      UI.renderTask(task, true);
    });
  } catch (e) {
    console.error("Failed to initialize app", e);
    UI.showToast('Failed to load previous downloads', 'error');
  }

  try {
    await checkDependencies();
  } catch (e) {
    console.error("Failed to check dependencies", e);
    UI.showToast('Failed to check dependencies', 'error');
  }

  const config = await getConfig();
  await applyConfigToUI(config);

  // Download button state is safe by default
  UI.setDownloadButtonState('safe');

  setupEventListeners(checkDependencies);

  try {
    const { Terminal } = await import('./js/ui/terminal.ts');
    await Terminal.init();

    if (UI.elements.tabDownloads && UI.elements.tabTerminal) {
      UI.elements.tabDownloads.addEventListener('click', () => {
        UI.switchQueueTab('downloads');
        Terminal.setVisible(false);
      });
      UI.elements.tabTerminal.addEventListener('click', () => {
        UI.switchQueueTab('terminal');
        Terminal.setVisible(true);
      });
    }
  } catch (e) {
    console.error("Terminal initialization failed:", e);
    UI.showToast('Terminal failed to initialize', 'warning');
  }

  setupWindowControls();

  window.addEventListener('mediaTypeChange', async () => {
    const config = await getConfig();
    applyConfigToUI(config);
  });

  if (UI.elements.debugLoggingToggle) {
    UI.elements.debugLoggingToggle.checked = config.debug_logging === true;
    UI.elements.debugLoggingToggle.addEventListener('change', async (e: Event) => {
      const target = e.target as HTMLInputElement;
      await updateConfig({ debug_logging: target.checked });
    });
  }

  if (UI.elements.openLogsBtn) {
    UI.elements.openLogsBtn.addEventListener('click', async () => {
      try {
        await Downloader.openAppLogsDir();
      } catch (err) {
        console.error('Failed to open logs folder:', err);
        UI.showToast('Failed to open logs folder');
      }
    });
  }

  document.addEventListener('visibilitychange', () => {
    if (document.visibilityState === 'visible') {
      reconcileState();
    }
  });
}

/**
 * Reconcile frontend state with backend ground truth
 */
async function reconcileState() {
  if (isReconciling) return;
  isReconciling = true;

  try {
    try {
      const downloads = await Downloader.getAllDownloads();
      downloads.forEach(task => {
        const existingItem = document.getElementById(`download-${task.id}`);
        if (!existingItem) {
          UI.renderTask(task, true);
          return;
        }
        if (task.status) {
          UI.updateTaskStatus(task.id, task.status);
        }
      });
    } catch (e) {
      console.error('Reconcile downloads failed:', e);
    }

    try {
      const states = await Downloader.getDependencyInstallationState();
      await checkDependencies();
      Object.values(states).forEach(state => {
        UI.updateInstallProgress(state.target, state.progress, state.status);
      });
    } catch (e) {
      console.error('Reconcile dependencies failed:', e);
    }
  } finally {
    isReconciling = false;
  }
}

/**
 * Check and handle dependency status (now mainly reactive via events)
 */
async function checkDependencies() {
  // Trigger a check on the backend, which will emit app:deps-status-changed
  await Downloader.checkDepsStatus();
}



document.addEventListener('DOMContentLoaded', () => {
  injectComponents();
  init();
});


/**
 * Global Event Delegation for Queue Actions
 */
document.addEventListener('click', (e: MouseEvent) => {
  const target = e.target as HTMLElement;
  const button = target.closest('[data-action]') as HTMLElement;
  if (!button) return;

  const action = button.dataset.action;
  const id = button.dataset.taskId;

  if (!id) return;

  switch (action) {
    case 'pause': pauseDownload(id); break;
    case 'cancel': cancelDownload(id); break;
    case 'resume': resumeDownload(id); break;
    case 'open-folder': openDownloadFolder(id); break;
  }
});

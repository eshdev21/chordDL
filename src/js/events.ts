import UI from './ui.ts';
import Downloader from './downloader.ts';
import {
    handleInstall,
    handleUpdate,
    handleCheckUpdates,
    handleDenoUpdate,
    handleReinstall,
    handleDownload,
    handleMainPathPicker,
    handleAudioPathPicker,
    handleVideoPathPicker,
    checkCookieStatus
} from './actions.ts';
import { updateConfig, applyConfigToUI } from './config.ts';
import {
    DependencyStatus,
    StateChangedPayload,
    DependencyInstallationPayload,
    ConfigChangedPayload,
    TaskCreatedEvent,
    ChordWindow
} from './types.ts';
import { listen, type Event as TauriEvent } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';

/**
 * Setup DOM event listeners
 */
/**
 * Bind DOM events to action handlers (clicks, keypresses, etc.).
 */
export function setupEventListeners(checkDependencies: () => Promise<void>) {
    // Disable context menu (except for inputs)
    document.addEventListener('contextmenu', (e: MouseEvent) => {
        const target = e.target as HTMLElement;
        const isInput = target.tagName === 'INPUT' ||
            target.tagName === 'TEXTAREA' ||
            target.isContentEditable;
        if (!isInput) e.preventDefault();
    });

    // Install button
    UI.elements.installBtn.addEventListener('click', handleInstall);

    // Manual components checkbox
    if (UI.elements.manualDepsCheckbox) {
        UI.elements.manualDepsCheckbox.addEventListener('change', (e: Event) => {
            const target = e.target as HTMLInputElement;
            if (target.checked) {
                UI.elements.manualInstructions.classList.remove('hidden');
                (UI.elements.installBtn.querySelector('span') as HTMLElement).textContent = 'I\'ve placed the files';
            } else {
                UI.elements.manualInstructions.classList.add('hidden');
                (UI.elements.installBtn.querySelector('span') as HTMLElement).textContent = 'Install Components';
            }
        });
    }

    // Update buttons
    UI.elements.updateBannerBtn.addEventListener('click', () => {
        const isWarning = UI.elements.updateBanner.classList.contains('warning');
        if (!isWarning) {
            handleUpdate().then(checkDependencies);
        }
    });
    UI.elements.settingsUpdateBtn.addEventListener('click', () => handleUpdate().then(checkDependencies));
    UI.elements.checkUpdatesBtn.addEventListener('click', handleCheckUpdates);

    // Reinstall buttons
    UI.elements.reinstallYtdlpBtn.addEventListener('click', () => handleReinstall('yt-dlp'));
    UI.elements.reinstallFfmpegBtn.addEventListener('click', () => handleReinstall('ffmpeg'));
    UI.elements.reinstallDenoBtn.addEventListener('click', () => handleReinstall('deno'));
    UI.elements.settingsDenoUpdateBtn.addEventListener('click', () => handleDenoUpdate(checkDependencies));

    // Download button
    UI.elements.downloadBtn.addEventListener('click', () => {
        handleDownload();
    });

    // Path picker
    UI.elements.pathDisplay.addEventListener('click', handleMainPathPicker);
    UI.elements.settingsPathDisplay.addEventListener('click', handleAudioPathPicker);
    UI.elements.settingsVideoPathDisplay.addEventListener('click', handleVideoPathPicker);

    // Clear completed
    UI.elements.clearBtn.addEventListener('click', () => UI.clearCompleted());

    // Enter key to download
    UI.elements.urlInput.addEventListener('keypress', (e: KeyboardEvent) => {
        if (e.key === 'Enter') handleDownload();
    });

    // Save format/quality preference on change
    UI.elements.formatSelect.addEventListener('change', async () => {
        const mediaType = UI.getMediaType();
        if (mediaType === 'video') {
            await updateConfig({ video_format: UI.elements.formatSelect.value });
        } else {
            await updateConfig({ default_format: UI.elements.formatSelect.value });
        }
    });

    UI.elements.qualitySelect.addEventListener('change', async () => {
        await updateConfig({ video_quality: UI.elements.qualitySelect.value });
    });

    // Concurrent limit slider
    UI.elements.concurrentLimitSlider.addEventListener('input', () => {
        UI.elements.concurrentLimitValue.textContent = UI.elements.concurrentLimitSlider.value;
    });

    UI.elements.concurrentLimitSlider.addEventListener('change', async () => {
        const val = parseInt(UI.elements.concurrentLimitSlider.value);
        await updateConfig({ max_concurrent_downloads: val });
    });

    if (UI.elements.settingsCheckAuthBtn) {
        UI.elements.settingsCheckAuthBtn.addEventListener('click', checkCookieStatus);
    }

    if (UI.elements.authEnabledToggle) {
        UI.elements.authEnabledToggle.addEventListener('change', async (e: Event) => {
            const val = (e.target as HTMLInputElement).checked;
            await updateConfig({ cookies_enabled: val });
        });
    }
}

/**
 * Setup Tauri backend event listeners (event-driven architecture)
 */
/**
 * Bind Tauri backend events to UI updates (reactive state sync).
 */
export async function setupTauriListeners(checkDependencies: () => Promise<void>) {
    // New download created
    await listen('download:created', (event: TauriEvent<TaskCreatedEvent>) => {
        const { id, title, media_type, is_playlist, output_path } = event.payload;
        UI.renderTask({
            id, title, media_type, is_playlist, output_path,
            url: '', format: '', quality: '', timestamp: Date.now(),
            args: [], temp_path: '', status: { type: 'Queued' }, children: []
        });
    });

    // State change — update existing queue item
    await listen('download:state-changed', (event: TauriEvent<StateChangedPayload>) => {
        const { id, status, title, progress: flattenedProgress, item_title: flattenedItemTitle } = event.payload;
        const statusType = status.type;
        const data = status.data || {};

        const handlers: Record<string, () => void> = {
            'Queued': () => UI.updateTaskStatus(id, 'Queued...'),
            'Starting': () => UI.updateTaskStatus(id, 'Starting...'),
            'FetchingMetadata': () => UI.updateTaskStatus(id, 'Fetching metadata...'),
            'Downloading': () => {
                let displayItemTitle = flattenedItemTitle;
                if (data.playlist) {
                    const p = data.playlist;
                    const itemProgress = Math.round(data.progress || 0);
                    displayItemTitle = `[${p.current_index} of ${p.total_items}] (${itemProgress}%)`;
                }
                UI.updateProgress(id, flattenedProgress, data.speed || '0KB/s', data.eta || 'Unknown', title, displayItemTitle);
            },
            'Merging': () => {
                let displayItemTitle = flattenedItemTitle;
                if (data.playlist) {
                    const p = data.playlist;
                    displayItemTitle = `[${p.current_index} of ${p.total_items}] (100%)`;
                }
                UI.updateProgress(id, flattenedProgress, 'Merging...', 'Finalizing', title, displayItemTitle);
            },
            'Finalizing': () => {
                let displayItemTitle = flattenedItemTitle;
                if (data.playlist) {
                    const p = data.playlist;
                    displayItemTitle = `[${p.current_index} of ${p.total_items}] (100%)`;
                }
                UI.updateProgress(id, flattenedProgress, 'Saving to folder...', 'Completing', title, displayItemTitle);
            },
            'Completed': () => UI.markComplete(id, data.filename || 'unknown'),
            'Failed': () => UI.markError(id, data.reason || 'Unknown error', flattenedProgress),
            'Cancelled': () => UI.removeTask(id),
            'Paused': () => UI.markPaused(id, flattenedProgress),
            'Interrupted': () => UI.markError(id, 'Interrupted', flattenedProgress)
        };

        if (handlers[statusType]) handlers[statusType]();

        // Show toast on failure
        if (statusType === 'Failed') {
            const category = data.category || 'generic';

            if (category === 'auth_required' || category === 'rate_limited') {
                UI.showToast('Restricted content — Enable cookies', 'warning', {
                    actions: [{
                        label: 'Settings',
                        onClick: () => (document.getElementById('settings-btn') as HTMLElement).click()
                    }]
                });
            } else if (category === 'browser_lock') {
                UI.showToast('Close Firefox and retry', 'warning');
            } else if (category === 'dependency_missing') {
                const depName = data.reason?.split(': ')[1] || 'a dependency';
                UI.showToast(`Missing: ${depName} — install it in Settings`, 'warning', {
                    actions: [{
                        label: 'Settings',
                        onClick: () => {
                            (document.getElementById('settings-btn') as HTMLElement).click();
                            (document.getElementById('settings-tab-system-btn') as HTMLElement).click();
                        }
                    }]
                });
                (window as ChordWindow).__TAURI__?.core.invoke('check_deps_status').catch(() => { });
            } else if (category === 'unavailable') {
                UI.showToast('Content unavailable or private', 'error');
            } else {
                UI.showToast(`Failed: ${data.reason?.substring(0, 60)}`, 'error');
            }
        }
    });

    // Dependency events (unified)
    await Downloader.onDependencyStatus(async (data: DependencyInstallationPayload) => {
        if (data.status === 'complete' && data.target === 'all') {
            // Clean up all progress bars
            UI.hideAllGlobalProgress();

            // Only manipulate Setup UI if it's currently visible
            const isSetupVisible = !UI.elements.setupScreen.classList.contains('hidden');

            if (isSetupVisible) {
                UI.showInstallProgress(true, true); // Finalizing...
            }

            if (checkDependencies) {
                await checkDependencies();
                if (isSetupVisible) UI.showInstallProgress(false);
            }
        } else {
            UI.updateInstallProgress(data.target, data.progress, data.status);
        }
    });

    // Dependency status changed
    await listen('app:deps-status-changed', (event: TauriEvent<DependencyStatus>) => {
        const status = event.payload;
        UI.updateDepsStatus(status);
    });

    // Config changed
    await listen('app:config-changed', async (event: TauriEvent<ConfigChangedPayload>) => {
        const config = event.payload;
        await applyConfigToUI(config);
        UI.updateConfigUI(config);
    });
}

/**
 * Setup Window Control buttons
 */
export function setupWindowControls() {
    const appWindow = getCurrentWindow();

    UI.elements.windowClose.addEventListener('click', () => appWindow.close());
    UI.elements.windowMinimize.addEventListener('click', () => appWindow.minimize());
}

import UI from './ui.ts';
import Downloader from './downloader.ts';
import { getConfig, updateConfig, applyConfigToUI } from './config.ts';
import { showNotification } from './utils.ts';
import { open } from '@tauri-apps/plugin-dialog';
import { invoke } from '@tauri-apps/api/core';
import { AppErrorPayload } from './types.ts';

/**
 * Handle install button click
 */
export async function handleInstall() {
    const isManual = UI.elements.manualDepsCheckbox?.checked;

    if (isManual) {
        UI.showInstallProgress(true, true); // Finalizing...
        try {
            await Downloader.completeSetup(true);
            const status = await Downloader.checkDependencies();

            if (status.binaries_missing) {
                showNotification('Some dependencies are still missing or invalid in custombin/');
                UI.showInstallProgress(false);
            } else {
                UI.showMainContent();
            }
        } catch (err) {
            console.error('Failed to complete setup:', err);
            showNotification(`Setup failed: ${err}`);
            UI.showInstallProgress(false);
        }
    } else {
        UI.showInstallProgress(true);
        const result = await Downloader.installDependencies();
        if (result && !result.success) {
            UI.showInstallProgress(false); // Reset only on hard failure
            showNotification(`Installation failed: ${result.error}`);
        }
        // On success, we wait for the dependency-status events to finalize the UI
    }
}

/**
 * Handle update button click
 */
export async function handleUpdate() {
    UI.elements.updateBannerBtn.disabled = true;
    UI.elements.updateBannerBtn.textContent = 'Updating...';
    UI.elements.settingsUpdateBtn.disabled = true;
    UI.elements.settingsUpdateBtn.textContent = 'Updating...';

    const result = await Downloader.updateYtdlp();

    if (result.success) {
        showNotification('yt-dlp updated successfully!');
        await Downloader.checkDepsStatus();
    } else {
        showNotification(`Update failed: ${result.error}`);
    }

    UI.elements.updateBannerBtn.disabled = false;
    UI.elements.updateBannerBtn.textContent = 'Update Now';
}

/**
 * Handle manual check for updates
 */
export async function handleCheckUpdates() {
    UI.setCheckingUpdates(true);

    try {
        const status = await Downloader.checkDepsStatus();

        // UI.updateDepsStatus(status) is called via event listener in events.js
        // We just need to show the results notification here
        if (status.custom_deps) {
            // Custom mode: Only check presence, don't check updates
            if (status.binaries_missing) {
                const missing = [];
                if (!status.yt_dlp_installed) missing.push('yt-dlp');
                if (!status.ffmpeg_installed) missing.push('ffmpeg');
                if (!status.ffprobe_installed) missing.push('ffprobe');
                if (!status.deno_installed) missing.push('deno');
                showNotification(`Missing: ${missing.join(', ')}`);
            } else {
                showNotification('All dependencies found');
            }
        } else {
            // Normal mode: Check both presence AND updates
            if (status.binaries_missing) {
                showNotification('Dependencies missing — Install in Settings');
            } else if (status.yt_dlp_update_available) {
                showNotification(`yt-dlp update available: ${status.yt_dlp_latest_version}`);
            } else if (status.deno_update_available) {
                showNotification(`Deno update available: ${status.deno_latest_version}`);
            } else {
                showNotification('All components up to date');
            }
        }
    } catch (err) {
        console.error('Check updates failed:', err);
        showNotification('Failed to check for updates');
    }

    UI.setCheckingUpdates(false);
}

/**
 * Handle Deno update
 */
export async function handleDenoUpdate(checkDependenciesCallback?: () => Promise<void>) {
    UI.elements.settingsDenoUpdateBtn.disabled = true;
    UI.elements.settingsDenoUpdateBtn.textContent = 'Updating...';
    UI.elements.settingsModal.classList.add('hidden');

    const result = await Downloader.updateDeno();

    if (result.success) {
        UI.showToast('Deno updated successfully!');
        if (checkDependenciesCallback) await checkDependenciesCallback();
    } else {
        UI.showToast(`Deno update failed: ${result.error}`);
    }
}

/**
 * Handle force reinstall
 */
export async function handleReinstall(component: string) {
    UI.elements.settingsModal.classList.add('hidden');

    const result = await Downloader.installDependencies(component);

    if (result.success) {
        UI.showToast(`${component} reinstalled successfully!`);
    } else {
        UI.showToast(`Reinstallation failed: ${result.error}`);
    }
}

// Session state REMOVED.

/**
 * Handle download button click (Supports Single and Playlist modes)
 */
export async function handleDownload() {
    const urls = UI.getUrls();
    const format = UI.elements.formatSelect.value;
    const quality = UI.elements.qualitySelect.value;
    const mediaType = UI.getMediaType();
    const mode = UI.getCurrentMode();

    const isPlaylist = !!(urls[0]?.includes('list=') || urls[0]?.includes('playlist'));
    let result;

    if (mode === 'batch') {
        // Batch not supported for auth retry logic yet in this simple plan, just run loop
        for (const url of urls) {
            await Downloader.downloadSingle(url, mediaType, format, quality, "", false, null);
        }
        UI.clearInput();
        return;
    }

    // Single / Playlist
    try {
        result = await Downloader.downloadSingle(urls[0], mediaType, format, quality, "", isPlaylist, null);
    } catch (err: unknown) {
        const appError = err as AppErrorPayload;
        if (appError?.category === 'already_active' || appError?.message?.includes('already')) {
            UI.showToast(appError.message || 'Already downloading', 'warning');
            UI.clearInput();
            return;
        }

        import('./utils/errorHandler.ts').then(module => module.handleAppError(appError));
        return;
    }

    // Handle Returned Errors
    if (result && !result.success) {
        const { handleAppError } = await import('./utils/errorHandler.ts');
        handleAppError(result.error);
    } else {
        UI.clearInput();
    }
}

/**
 * Generic folder picker utilizing the Tauri Dialog plugin
 */
async function pickDirectory(type: string) {
    try {
        const config = await getConfig();
        const configKey = (type === 'video' ? 'video_path' : 'download_path') as 'video_path' | 'download_path';
        const currentPath = config[configKey] || (type === 'video' ? await Downloader.getVideoPath() : await Downloader.getDownloadPath()) || '';

        const selected = await open({
            directory: true,
            multiple: false,
            defaultPath: currentPath,
            title: `Select ${type.charAt(0).toUpperCase() + type.slice(1)} Download Folder`
        });

        if (selected) {
            await updateConfig({ [configKey]: selected });
            applyConfigToUI(await getConfig());
        }
    } catch (err) {
        console.error('Folder picker error:', err);
        showNotification('Failed to open folder picker');
    }
}

/**
 * Handle folder picker for the main UI (context-sensitive)
 */
export const handleMainPathPicker = () => {
    const mediaType = UI.getMediaType();
    pickDirectory(mediaType);
};

/**
 * Handle audio folder picker specifically
 */
export const handleAudioPathPicker = () => pickDirectory('audio');

/**
 * Handle video folder picker specifically
 */
export const handleVideoPathPicker = () => pickDirectory('video');

/**
 * Global pause
 */
export async function pauseDownload(id: string) {
    const result = await Downloader.pauseDownload(id);
    if (!result.success) {
        showNotification(`Failed to pause: ${result.error}`);
    }
}

/**
 * Global cancel
 */
export async function cancelDownload(id: string) {
    const result = await Downloader.cancelDownload(id);
    if (!result.success) {
        showNotification(`Failed to cancel: ${result.error}`);
    }
}

/**
 * Global open folder function
 */
export async function openDownloadFolder(id: string) {
    const item = document.getElementById(`download-${id}`);

    if (!item) {
        showNotification('Download not found');
        return;
    }

    const outputPath = item.dataset.outputPath;

    if (!outputPath) {
        showNotification('Folder location unavailable');
        return;
    }

    try {
        await invoke('open_folder', {
            path: outputPath
        });
    } catch (err) {
        console.error('Failed to open folder:', err);
        showNotification(`Cannot open folder: ${err}`);
    }
}

/**
 * Global resume function
 */
export async function resumeDownload(id: string) {
    try {
        const result = await Downloader.resumeDownload(id);
        if (!result.success) {
            const { handleAppError } = await import('./utils/errorHandler.ts');
            handleAppError(result.error);
        }
    } catch (err) {
        const { handleAppError } = await import('./utils/errorHandler.ts');
        handleAppError(err as unknown);
    }
}

/**
 * Handle cookie status check
 */
export async function checkCookieStatus() {
    const btn = UI.elements.settingsCheckAuthBtn;
    const originalText = btn.textContent;
    btn.disabled = true;
    btn.textContent = 'Checking...';

    try {
        const result = await Downloader.checkFirefoxAuth();
        if (result.success) {
            const msg = result.message || 'Unknown status';
            const isSuccess = msg.includes('found ✓');
            UI.showToast(msg, isSuccess ? 'success' : 'warning');
        } else {
            UI.showToast(String(result.error || 'Failed to check cookies'), 'error');
        }
    } catch (err) {
        UI.showToast(`Cookie check failed: ${err}`, 'error');
    } finally {
        btn.disabled = false;
        btn.textContent = originalText;
    }
}

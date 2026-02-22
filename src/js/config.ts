import Downloader from './downloader.ts';
import UI from './ui.ts';
import { AppConfig } from './types.ts';

/**
 * Frontend Configuration Interface - Bridges UI state with Backend persistence.
 */

// Config cache (loaded once from backend, updated on changes)
let cachedConfig: AppConfig | null = null;

/**
 * Get the current configuration
 */
export async function getConfig(): Promise<AppConfig> {
    if (!cachedConfig) {
        cachedConfig = await Downloader.loadConfig();
    }
    return cachedConfig!;
}

/**
 * Update the configuration with new values
 */
export async function updateConfig(updates: Partial<AppConfig>) {
    const config = await getConfig();
    Object.assign(config, updates);
    await Downloader.saveConfig(config);
    cachedConfig = config;
}

/**
 * Apply config to UI (paths and format preferences)
 */
export async function applyConfigToUI(config: AppConfig) {
    const mediaType = UI.getMediaType();

    // Update path display — show actual default paths when config has no explicit path
    if (mediaType === 'video') {
        const path = config.video_path || '';
        if (path) {
            UI.updatePath(path, mediaType);
        } else {
            const defaultPath = await Downloader.getVideoPath();
            UI.updatePath(defaultPath || 'Select folder...', mediaType);
        }
    } else {
        const path = config.download_path || '';
        if (path) {
            UI.updatePath(path, mediaType);
        } else {
            const defaultPath = await Downloader.getDownloadPath();
            UI.updatePath(defaultPath || 'Select folder...', mediaType);
        }
    }

    // Update settings paths — resolve defaults for unset paths
    const audioSettingsPath = config.download_path || await Downloader.getDownloadPath() || 'Default';
    const videoSettingsPath = config.video_path || await Downloader.getVideoPath() || 'Default';
    UI.updateSettingsPaths(audioSettingsPath, videoSettingsPath);

    // Apply format preference
    if (mediaType === 'video') {
        if (config.video_format) UI.setFormat(config.video_format);
        if (config.video_quality) UI.setQuality(config.video_quality);
    } else {
        if (config.default_format) UI.setFormat(config.default_format);
    }

    // Apply concurrent limit
    if (config.max_concurrent_downloads) {
        UI.elements.concurrentLimitSlider.value = config.max_concurrent_downloads.toString();
        UI.elements.concurrentLimitValue.textContent = config.max_concurrent_downloads.toString();
    }

    // Sync Cookie Toggle
    if (UI.elements.authEnabledToggle) {
        UI.elements.authEnabledToggle.checked = config.cookies_enabled;
    }

    if (UI.elements.customDepsToggle) {
        UI.elements.customDepsToggle.checked = config.custom_deps === true;
    }

    // Download button state is always safe now, backend handles cookies
    UI.setDownloadButtonState('safe');
}

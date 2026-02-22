/**
 * Downloader Module - Handles Tauri backend communication
 *
 * This is a thin wrapper around Tauri's invoke API.
 * No state management — just command dispatch.
 */

import {
    AppConfig,
    DownloadTask,
    DependencyStatus,
    DependencyInstallationState,
    AppErrorPayload
} from './types.ts';

import { invoke } from '@tauri-apps/api/core';
import { listen, type Event as TauriEvent } from '@tauri-apps/api/event';

const Downloader = {
    /**
     * Initialize app — returns verified resumable downloads
     */
    async initializeApp(): Promise<DownloadTask[]> {
        try {
            return await invoke('initialize_app');
        } catch (error) {
            console.error('Initialize app failed:', error);
            return [];
        }
    },

    /**
     * Check if dependencies are installed
     */
    async checkDependencies(): Promise<DependencyStatus> {
        try {
            return await invoke('check_dependencies');
        } catch (error) {
            console.error('Check dependencies failed:', error);
            return {
                yt_dlp_installed: false,
                yt_dlp_version: null,
                yt_dlp_update_available: false,
                yt_dlp_latest_version: null,
                ffmpeg_installed: false,
                ffprobe_installed: false,
                deno_installed: false,
                deno_version: null,
                deno_update_available: false,
                deno_latest_version: null,
                binaries_missing: true,
                installation_in_progress: false,
                custom_deps: false,
                setup_shown: false
            };
        }
    },

    /**
     * Install dependencies (yt-dlp and ffmpeg)
     */
    async installDependencies(target: string | null = null): Promise<{ success: boolean; error?: AppErrorPayload | string }> {
        try {
            await invoke('install_dependencies', { target });
            return { success: true };
        } catch (error) {
            console.error('Install failed:', error);
            return { success: false, error: error as AppErrorPayload | string };
        }
    },

    /**
     * Check dependencies and emit event
     */
    async checkDepsStatus(): Promise<DependencyStatus> {
        return await invoke('check_deps_status');
    },

    /**
     * Update yt-dlp
     */
    async updateYtdlp(): Promise<{ success: boolean; error?: AppErrorPayload | string }> {
        try {
            await invoke('update_ytdlp');
            return { success: true };
        } catch (error) {
            console.error('Update failed:', error);
            return { success: false, error: error as AppErrorPayload | string };
        }
    },

    /**
     * Update Deno
     */
    async updateDeno(): Promise<{ success: boolean; error?: AppErrorPayload | string }> {
        try {
            await invoke('update_deno');
            return { success: true };
        } catch (error) {
            console.error('Deno update failed:', error);
            return { success: false, error: error as AppErrorPayload | string };
        }
    },

    /**
     * Download a single track or playlist
     */
    async downloadSingle(
        url: string,
        mediaType: string,
        format: string,
        quality: string,
        outputPath: string,
        isPlaylist = false,
        existingId: string | null = null
    ): Promise<{ success: boolean; taskId?: string; error?: AppErrorPayload | string }> {
        try {
            const taskId = await invoke<string>('download_single', {
                request: {
                    url,
                    media_type: mediaType,
                    format,
                    quality,
                    output_path: outputPath,
                    is_playlist: isPlaylist,
                    existing_id: existingId
                }
            });
            return { success: true, taskId };
        } catch (error) {
            console.error('Download failed:', error);
            return { success: false, error: error as AppErrorPayload | string };
        }
    },

    /**
     * Cancel an active download
     */
    async cancelDownload(id: string): Promise<{ success: boolean; error?: AppErrorPayload | string }> {
        try {
            await invoke('cancel_download', { id });
            return { success: true };
        } catch (error) {
            console.error('Cancel failed:', error);
            return { success: false, error: error as AppErrorPayload | string };
        }
    },

    /**
     * Pause an active download
     */
    async pauseDownload(id: string): Promise<{ success: boolean; error?: AppErrorPayload | string }> {
        try {
            await invoke('pause_download', { id });
            return { success: true };
        } catch (error) {
            console.error('Pause failed:', error);
            return { success: false, error: error as AppErrorPayload | string };
        }
    },

    /**
     * Get active downloads
     */
    async getActiveDownloads(): Promise<DownloadTask[]> {
        try {
            return await invoke('get_active_downloads');
        } catch (error) {
            console.error('Failed to get active downloads:', error);
            return [];
        }
    },

    /**
     * Get default download path
     */
    async getDownloadPath(): Promise<string | null> {
        try {
            return await invoke('get_default_download_path');
        } catch (error) {
            console.error('Failed to get path:', error);
            return null;
        }
    },

    /**
     * Get default video path
     */
    async getVideoPath(): Promise<string | null> {
        try {
            return await invoke('get_default_video_path');
        } catch (error) {
            console.error('Failed to get video path:', error);
            return null;
        }
    },

    /**
     * Load saved config (from backend cache, no disk I/O)
     */
    async loadConfig(): Promise<AppConfig> {
        try {
            return await invoke('load_config');
        } catch (error) {
            console.error('Failed to load config:', error);
            // Return a default config object to satisfy the interface
            return {
                download_path: null,
                video_path: null,
                video_format: 'mp4',
                video_quality: '1080',
                default_format: 'm4a',
                max_concurrent_downloads: 3,
                cookies_enabled: false,
                custom_deps: false,
                debug_logging: false,
                setup_shown: true
            };
        }
    },

    /**
     * Save config
     */
    async saveConfig(config: AppConfig): Promise<{ success: boolean; error?: AppErrorPayload | string }> {
        try {
            await invoke('save_config', { config });
            return { success: true };
        } catch (error) {
            console.error('Failed to save config:', error);
            return { success: false, error: error as AppErrorPayload | string };
        }
    },

    /**
     * Listen for dependency status events (progress, extracting, complete)
     */
    async onDependencyStatus(callback: (payload: DependencyInstallationState) => void): Promise<() => void> {
        return await listen('dependency:status', (event: TauriEvent<DependencyInstallationState>) => {
            callback(event.payload);
        });
    },

    /**
     * Check if user is signed into video providers in Firefox (cookies)
     */
    async checkFirefoxAuth(): Promise<{ success: boolean; message?: string; error?: AppErrorPayload | string }> {
        try {
            const message = await invoke('check_firefox_auth') as string;
            return { success: true, message };
        } catch (error) {
            console.error('Check auth failed:', error);
            return { success: false, error: error as AppErrorPayload | string };
        }
    },

    /**
     * Get all downloads (including queued/paused) - used for state reconciliation
     */
    async getAllDownloads(): Promise<DownloadTask[]> {
        try {
            return await invoke('get_all_downloads');
        } catch (error) {
            console.error('Failed to get all downloads:', error);
            return [];
        }
    },

    /**
     * Get current dependency installation state
     */
    async getDependencyInstallationState(): Promise<Record<string, DependencyInstallationState>> {
        try {
            return await invoke('get_dependency_installation_state');
        } catch (error) {
            console.error('Failed to get dependency state:', error);
            return {};
        }
    },

    async resumeDownload(id: string): Promise<{ success: boolean; error?: AppErrorPayload | string }> {
        try {
            await invoke('resume_download', { id });
            return { success: true };
        } catch (error) {
            console.error('Resume failed:', error);
            return { success: false, error: error as AppErrorPayload | string };
        }
    },

    /**
     * Complete setup and mark in config
     */
    async completeSetup(custom: boolean): Promise<void> {
        return await invoke('complete_setup', { custom });
    },

    /**
     * Toggle custom dependencies mode
     */
    async toggleCustomMode(enabled: boolean): Promise<void> {
        return await invoke('toggle_custom_mode', { enabled });
    },

    /**
     * Open app logs directory
     */
    async openAppLogsDir(): Promise<void> {
        return await invoke('open_app_logs_dir');
    }
};

export default Downloader;

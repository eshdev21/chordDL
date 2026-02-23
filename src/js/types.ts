/**
 * Core Type Definitions for Chord
 */

export interface PlaylistInfo {
    total_items: number;
    current_index: number;
    item_title: string;
}

export interface TaskStatusData {
    progress?: number;
    reason?: string;
    filename?: string;
    playlist?: PlaylistInfo;
    speed?: string;
    eta?: string;
    category?: 'auth_required' | 'rate_limited' | 'browser_lock' | 'dependency_missing' | 'unavailable' | 'generic';
}

export interface TaskStatus {
    type: 'Queued' | 'Starting' | 'FetchingMetadata' | 'Downloading' | 'Merging' | 'Finalizing' | 'Completed' | 'Failed' | 'Paused' | 'Interrupted' | 'Cancelled';
    data?: TaskStatusData;
}

export interface SubTask {
    id: string;
    title: string;
    status: TaskStatus;
}

export interface DownloadTask {
    id: string;
    url: string;
    title: string;
    media_type: string;
    format: string;
    quality: string;
    output_path: string;
    is_playlist: boolean;
    timestamp: number;
    args: string[];
    temp_path: string;
    status: TaskStatus;
    children: SubTask[];

    // UI-specific properties that get flattened on events
    progress?: number;
}

export interface TaskCreatedEvent {
    id: string;
    title: string;
    media_type: 'audio' | 'video';
    is_playlist: boolean;
    output_path: string;
}

export interface AppConfig {
    download_path: string | null;
    video_path: string | null;
    video_format: string;
    video_quality: string;
    default_format: string;
    max_concurrent_downloads: number;
    concurrent_fragments: number;
    cookies_enabled: boolean;
    write_subs: boolean;
    custom_deps: boolean;
    debug_logging: boolean;
    setup_shown: boolean;
}

export interface DependencyInstallationState {
    target: string;
    progress: number;
    status: 'starting' | 'downloading' | 'extracting' | 'complete';
    total_size?: number;
    downloaded?: number;
}

export interface DependencyStatus {
    yt_dlp_installed: boolean;
    yt_dlp_version: string | null;
    yt_dlp_update_available: boolean;
    yt_dlp_latest_version: string | null;
    ffmpeg_installed: boolean;
    ffprobe_installed: boolean;
    deno_installed: boolean;
    deno_version: string | null;
    deno_update_available: boolean;
    deno_latest_version: string | null;
    binaries_missing: boolean;
    installation_in_progress: boolean;
    custom_deps: boolean;
    setup_shown: boolean;
}
export interface StateChangedPayload {
    id: string;
    status: TaskStatus;
    title: string;
    progress: number;
    item_title?: string;
}

export type DependencyInstallationPayload = DependencyInstallationState;

export interface ConfigChangedPayload extends AppConfig { }

export interface TauriWindowApi {
    core: {
        invoke<T = unknown>(command: string, args?: Record<string, unknown>): Promise<T>;
    };
    window: {
        getCurrentWindow(): {
            show(): Promise<void>;
        };
    };
}

export interface ChordWindow extends Window {
    __TAURI__?: TauriWindowApi;
}

export interface AppErrorPayload {
    message?: string;
    category?: string;
    severity?: 'info' | 'warning' | 'error' | 'success' | 'critical';
}

export interface ToastAction {
    label: string;
    onClick: () => void;
}

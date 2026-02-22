/**
 * UI Queue - Pure DOM rendering for download tasks
 *
 * No state tracking — the backend owns all state.
 * These functions simply create/update/remove DOM elements.
 */
import elements from './elements.ts';
import { DownloadTask, TaskStatus, TaskStatusData } from '../types.ts';


const queue = {
    uiStateCache: new Map<string, string>(), // id -> last rendered status

    /**
     * Render a task from backend data.
     * Used for both new downloads and resumed tasks on startup.
     */
    renderTask(task: DownloadTask, skipScroll = false) {
        // Don't duplicate
        if (document.getElementById(`download-${task.id}`)) return;

        const uiData = this.getTaskUIData(task.status || 'Starting', task.title, task.id);
        if (typeof task.progress === 'number') uiData.progress = task.progress;

        const icon = task.media_type === 'audio'
            ? 'assets/icons/folder-music-symbolic.svg'
            : 'assets/icons/folder-videos-symbolic.svg';

        const item = document.createElement('div');
        item.className = 'queue-item';
        item.id = `download-${task.id}`;
        item.dataset.status = uiData.statusClass;


        // Store metadata as data attributes
        if (task.output_path) item.dataset.outputPath = task.output_path;
        if (task.media_type) item.dataset.mediaType = task.media_type;

        item.innerHTML = `
            <div class="queue-item-icon">
                <img src="${icon}" alt="">
            </div>
            <div class="queue-item-info">
                <div class="queue-item-title ${uiData.isLoading ? 'loading' : ''}">${escapeHtml(cleanTitle(task.title || 'Starting download...'))}</div>
                <div class="progress-bar">
                    <div class="progress-bar-fill ${uiData.statusClass}" style="width: ${uiData.progress}%"></div>
                </div>
                <div class="queue-item-meta ${uiData.textClass}">
                    ${uiData.statusText}
                </div>
                <div class="queue-item-item-title"></div>
            </div>
            <div class="queue-item-actions">
                ${uiData.actionsHTML}
            </div>
        `;

        elements.queueContainer.appendChild(item);
        elements.queueEmpty.classList.add('hidden');

        if (!skipScroll) {
            item.scrollIntoView({ behavior: 'smooth', block: 'end' });
        }
    },

    /**
     * Update download progress for a task
     */
    updateProgress(id: string, progress: number, speed: string, eta: string, title?: string, itemTitle?: string) {
        const item = document.getElementById(`download-${id}`);
        if (!item) return;

        // Construct a pseudo-status object for the helper or update directly
        // Better: extract the meta text logic specifically
        const meta = item.querySelector('.queue-item-meta');
        const fill = item.querySelector('.progress-bar-fill');
        const titleEl = item.querySelector('.queue-item-title');
        const itemTitleEl = item.querySelector('.queue-item-item-title');

        if (fill) {
            (fill as HTMLElement).style.width = `${progress}%`;
            fill.className = 'progress-bar-fill active';
        }
        if (meta) {
            meta.textContent = `${progress.toFixed(1)}% • ${speed} • ${eta}`;
            meta.className = 'queue-item-meta';
        }
        if (titleEl && title) {
            titleEl.textContent = cleanTitle(title);
            titleEl.classList.remove('loading');
        }

        this.markActive(id);
        if (itemTitleEl && itemTitle && itemTitle !== title) {
            itemTitleEl.textContent = itemTitle;
            itemTitleEl.classList.add('visible');
        } else if (itemTitleEl) {
            itemTitleEl.classList.remove('visible');
        }
    },

    /**
     * Unified helper to get UI representation for any task state
     */
    getTaskUIData(status: TaskStatus | string | undefined, title: string | null | undefined, id: string) {
        const cleanStatus = status || 'Starting';
        const type = (typeof cleanStatus === 'string') ? cleanStatus : (cleanStatus.type || 'Starting');
        const data = (typeof cleanStatus === 'string' ? {} : (cleanStatus.data || {})) as TaskStatusData;

        const isInterrupted = type === 'Interrupted';
        const isPaused = type === 'Paused';
        const isError = type === 'Failed';
        const isComplete = type === 'Completed';

        // Safe access to data properties
        let progress = typeof data.progress === 'number' ? data.progress : 0;
        let reason = data.reason || 'Unknown error';
        // Truncate long errors
        if (reason.length > 80) {
            reason = reason.substring(0, 80) + '...';
        }

        // Hierarchical Progress Calculation
        if (data.playlist) {
            const p = data.playlist;
            if (p.total_items > 0) {
                if (['Merging', 'Finalizing'].includes(type)) {
                    progress = (p.current_index * 100.0) / p.total_items;
                } else if (type === 'Downloading') {
                    progress = ((p.current_index - 1) * 100.0 + (data.progress || 0)) / p.total_items;
                }
            }
        } else if (['Merging', 'Finalizing', 'Completed'].includes(type)) {
            progress = 100;
        }

        let statusText = 'Starting...';
        if (isInterrupted) statusText = 'Interrupted';
        else if (isPaused) statusText = 'Paused';
        else if (isError) statusText = `Error: ${reason}`;
        else if (type === 'Queued') statusText = 'Queued...';
        else if (type === 'FetchingMetadata') statusText = 'Fetching metadata...';
        else if (type === 'Merging') statusText = 'Merging...';
        else if (type === 'Finalizing') statusText = 'Saving to folder...';
        else if (isComplete) statusText = 'Completed';

        const actionsHTML = (isComplete) ? `
            <button class="btn btn-flat btn-icon-xs" data-action="open-folder" data-task-id="${id}" title="Open Folder">
                <img src="assets/icons/folder-open-colored.svg" class="icon-xs" alt="Open">
            </button>
        ` : (isInterrupted || isPaused || isError) ? `
            <button class="btn btn-flat btn-icon-xs" data-action="resume" data-task-id="${id}" title="Resume">
                <img src="assets/icons/view-refresh-symbolic.svg" class="icon-xs" alt="Resume">
            </button>
            <button class="btn btn-flat btn-icon-xs" data-action="cancel" data-task-id="${id}" title="Cancel">
                <img src="assets/icons/process-stop-symbolic.svg" class="icon-xs" alt="Cancel">
            </button>
        ` : `
            <button class="btn btn-flat btn-icon-xs" data-action="pause" data-task-id="${id}" title="Pause">
                <img src="assets/icons/media-playback-pause-symbolic.svg" class="icon-xs" alt="Pause">
            </button>
            <button class="btn btn-flat btn-icon-xs" data-action="cancel" data-task-id="${id}" title="Cancel">
                <img src="assets/icons/process-stop-symbolic.svg" class="icon-xs" alt="Cancel">
            </button>
        `;

        return {
            statusText,
            progress: progress,
            statusClass: isError || isInterrupted ? 'error' : (isPaused ? 'warning' : (isComplete ? 'success' : 'active')),
            textClass: isError || isInterrupted ? 'status-error' : (isPaused ? 'status-warning' : (isComplete ? 'status-complete' : '')),
            isLoading: title === 'Fetching metadata...',
            actionsHTML
        };
    },

    /**
     * Update task status text (for non-progress states)
     */
    updateTaskStatus(id: string, statusType: TaskStatus | string) {
        const item = document.getElementById(`download-${id}`);
        if (!item) return;

        // Note: we might be passed a string or an object here depending on source
        const status: TaskStatus = typeof statusType === 'string' ? { type: statusType as TaskStatus['type'] } : statusType;

        // Check cache - skip if same status
        const cacheKey = `${id}-${status.type}`;
        if (this.uiStateCache.get(id) === cacheKey) {
            return; // Already rendered this exact state
        }
        this.uiStateCache.set(id, cacheKey);

        const titleEl = item.querySelector('.queue-item-title');
        const uiData = this.getTaskUIData(status, titleEl?.textContent, id);

        const fillEl = item.querySelector('.progress-bar-fill') as HTMLElement | null;
        const currentStatusClass = fillEl?.className || '';
        const currentStatusText = item.querySelector('.queue-item-meta')?.textContent || '';

        // Change detection - skip if nothing changed
        const newStatusClass = `progress-bar-fill ${uiData.statusClass}`;
        if (currentStatusClass === newStatusClass && currentStatusText === uiData.statusText) {
            return; // No changes needed
        }

        const meta = item.querySelector('.queue-item-meta');
        const fill = item.querySelector('.progress-bar-fill');
        const actions = item.querySelector('.queue-item-actions');

        if (meta) {
            meta.textContent = uiData.statusText;
            meta.className = `queue-item-meta ${uiData.textClass}`;
        }
        if (fill) {
            (fill as HTMLElement).style.width = `${uiData.progress}%`;
            fill.className = `progress-bar-fill ${uiData.statusClass}`;
        }
        if (actions && (actions as HTMLElement).dataset.lastStatus !== status.type) {
            actions.innerHTML = uiData.actionsHTML;
            (actions as HTMLElement).dataset.lastStatus = status.type;
        }

        item.dataset.status = uiData.statusClass;
        this.markActive(id);

    },

    /**
     * Ensure a task shows the Pause button (active state)
     */
    markActive(_id: string) {
        // No-op if already managed by updateTaskStatus/updateProgress reliably
    },

    /**
     * Mark a task as complete
     */
    markComplete(id: string, filename: string) {
        this.updateTaskStatus(id, { type: 'Completed', data: { filename } });
    },

    /**
     * Mark a task as paused
     */
    markPaused(id: string, progress: number) {
        this.updateTaskStatus(id, { type: 'Paused', data: { progress } });
    },

    /**
     * Mark a task as errored/interrupted
     */
    markError(id: string, message: string, progress: number) {
        const type = (message === 'Interrupted' ? 'Interrupted' : 'Failed') as TaskStatus['type'];
        this.updateTaskStatus(id, { type, data: { reason: message, progress } });
    },

    /**
     * Remove a task from the DOM entirely
     */
    removeTask(id: string) {
        const item = document.getElementById(`download-${id}`);
        if (item) item.remove();
        this.uiStateCache.delete(id);

        // Show empty state if no items left
        if (elements.queueContainer.children.length === 0) {
            elements.queueEmpty.classList.remove('hidden');
        }
    },

    /**
     * Clear all completed/errored tasks from the DOM
     */
    clearCompleted() {
        const items = elements.queueContainer.querySelectorAll('.queue-item');
        items.forEach(item => {
            const htmlItem = item as HTMLElement;
            if (htmlItem.dataset.status === 'success' || htmlItem.dataset.status === 'error') {
                const taskId = htmlItem.id.replace('download-', '');
                this.uiStateCache.delete(taskId);
                htmlItem.remove();
            }
        });


        if (elements.queueContainer.children.length === 0) {
            elements.queueEmpty.classList.remove('hidden');
        }
    }
};

function escapeHtml(text: string) {
    const div = document.createElement('div');
    div.textContent = text;
    return div.innerHTML;
}

/**
 * Clean a title by removing common media extensions for a cleaner UI
 */
function cleanTitle(title: string): string {
    if (!title) return title;
    return title
        .replace(/\.(part|ytdl|temp)$/i, '') // Remove temp extensions
        .replace(/\.(mp3|mp4|webm|m4a|opus|flac|wav|mkv|avi)$/i, '') // Remove media extension
        .replace(/\.f\d+$/i, ''); // Remove format ID like .f399
}

export default queue;

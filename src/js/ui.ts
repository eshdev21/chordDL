/**
 * UI Module Facade - Consolidates all UI sub-modules
 */
import elements from './ui/elements.ts';
import views from './ui/views.ts';
import modals from './ui/modals.ts';
import queue from './ui/queue.ts';
import controls from './ui/controls.ts';
import { DownloadTask, TaskStatus } from './types.ts';

interface UIType {
    elements: typeof elements;
    init(): void;
    showSetupScreen: typeof views.showSetupScreen;
    showMainContent: typeof views.showMainContent;
    hideSplashScreen: typeof views.hideSplashScreen;
    updateSetupStatus: typeof views.updateSetupStatus;
    showInstallProgress: typeof views.showInstallProgress;
    updateInstallProgress: typeof views.updateInstallProgress;
    updateSettingsInfo: typeof views.updateSettingsInfo;
    updateDepsStatus: typeof views.updateDepsStatus;
    updateConfigUI: typeof views.updateConfigUI;
    setCheckingUpdates: typeof views.setCheckingUpdates;
    hideAllGlobalProgress: typeof views.hideAllGlobalProgress;
    showToast: (message: string, type?: 'info' | 'warning' | 'error' | 'success' | 'critical', options?: { duration?: number; actions?: { label: string; onClick: () => void }[] }) => void;
    showConfirm: typeof modals.showConfirm;
    renderTask: (task: DownloadTask, skipScroll?: boolean) => void;
    updateProgress: (id: string, progress: number, speed: string, eta: string, title?: string, itemTitle?: string) => void;
    updateTaskStatus: (id: string, statusType: TaskStatus | string) => void;
    markComplete: (id: string, filename: string) => void;
    markPaused: (id: string, progress: number) => void;
    markError: (id: string, message: string, progress: number) => void;
    removeTask: (id: string) => void;
    clearCompleted: () => void;
    getMediaType: () => 'audio' | 'video';
    switchMode: (mode: 'single' | 'batch' | 'playlist') => void;
    getCurrentMode: () => 'single' | 'batch' | 'playlist';
    getUrls: () => string[];
    clearInput: () => void;
    updatePath: (path: string, type: 'audio' | 'video') => void;
    updateSettingsPaths: (audioPath: string, videoPath: string) => void;
    setFormat: (format: string) => void;
    setQuality: (quality: string) => void;
    setDownloadButtonState: (state: 'safe' | 'loading' | 'disabled') => void;
    switchQueueTab: (tab: 'downloads' | 'terminal') => void;
}

/**
 * Global UI facade. Orchestrates components from elements, views, and controls.
 */
const UI: UIType = {
    // Properties
    elements,

    // Initialization
    /**
     * Bootstraps UI components and establishes initial view states.
     */
    init() {
        controls.setupMediaTypeSwitcher();
        controls.setupModeSwitcher();
        controls.setupPasteButton();
        modals.setupSettingsModal();
        modals.setupConfirmModal();

        // Initial media type check
        const type = controls.getMediaType();
        controls.updateMediaOptions(type);
        controls.updateUrlModeTag();
    },

    // Methods from views
    showSetupScreen: views.showSetupScreen,
    showMainContent: views.showMainContent,
    hideSplashScreen: views.hideSplashScreen,
    updateSetupStatus: views.updateSetupStatus,
    showInstallProgress: views.showInstallProgress,
    updateInstallProgress: views.updateInstallProgress,
    updateSettingsInfo: views.updateSettingsInfo,
    updateDepsStatus: views.updateDepsStatus,
    updateConfigUI: views.updateConfigUI,
    setCheckingUpdates: views.setCheckingUpdates,
    hideAllGlobalProgress: views.hideAllGlobalProgress,

    // Methods from modals
    showToast: modals.showToast,
    showConfirm: modals.showConfirm,

    // Methods from queue (pure DOM operations)
    renderTask: queue.renderTask.bind(queue),
    updateProgress: queue.updateProgress.bind(queue),
    updateTaskStatus: queue.updateTaskStatus.bind(queue),
    markComplete: queue.markComplete.bind(queue),
    markPaused: queue.markPaused.bind(queue),
    markError: queue.markError.bind(queue),
    removeTask: queue.removeTask.bind(queue),
    clearCompleted: queue.clearCompleted.bind(queue),

    // Methods from controls
    getMediaType: controls.getMediaType.bind(controls),
    switchMode: controls.switchMode.bind(controls),
    getCurrentMode: controls.getCurrentMode.bind(controls),
    getUrls: controls.getUrls.bind(controls),
    clearInput: controls.clearInput.bind(controls),
    updatePath: controls.updatePath.bind(controls),
    updateSettingsPaths: controls.updateSettingsPaths.bind(controls),
    setFormat: controls.setFormat.bind(controls),
    setQuality: controls.setQuality.bind(controls),
    setDownloadButtonState: controls.setDownloadButtonState.bind(controls),

    // Terminal
    switchQueueTab: views.switchQueueTab
};

export default UI;

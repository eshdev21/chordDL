/**
 * UI Elements Cache
 */

const elements = {
    get appContainer() { return document.querySelector('.app-container') as HTMLElement; },
    get setupScreen() { return document.getElementById('setup-screen') as HTMLElement; },
    get mainContent() { return document.getElementById('main-content') as HTMLElement; },
    get splashScreen() { return document.getElementById('splash-screen') as HTMLElement; },

    // Setup elements
    get installBtn() { return document.getElementById('install-btn') as HTMLButtonElement; },
    get ytdlpStatus() { return document.getElementById('ytdlp-status') as HTMLElement; },
    get ffmpegStatus() { return document.getElementById('ffmpeg-status') as HTMLElement; },
    get denoStatus() { return document.getElementById('deno-status') as HTMLElement; },
    get setupProgress() { return document.getElementById('setup-progress') as HTMLElement; },
    get setupProgressBar() { return document.getElementById('setup-progress-bar') as HTMLElement; },
    get setupProgressText() { return document.getElementById('setup-progress-text') as HTMLElement; },
    get manualDepsCheckbox() { return document.getElementById('manual-deps-checkbox') as HTMLInputElement; },
    get manualInstructions() { return document.getElementById('manual-instructions') as HTMLElement; },
    get setupOptions() { return document.querySelector('.setup-options') as HTMLElement; },

    // Main Toolbar
    get mediaTypeSwitcher() { return document.getElementById('media-type-switcher') as HTMLElement; },
    get modeSwitcher() { return document.getElementById('mode-switcher') as HTMLElement; },
    get audioTab() { return document.getElementById('audio-tab') as HTMLButtonElement; },
    get videoTab() { return document.getElementById('video-tab') as HTMLButtonElement; },

    // Update Banner
    get updateBanner() { return document.getElementById('update-banner') as HTMLElement; },
    get updateBannerText() { return document.getElementById('update-banner-text') as HTMLElement; },
    get updateBannerBtn() { return document.getElementById('update-banner-btn') as HTMLButtonElement; },

    // Download Form
    get urlEntry() { return document.getElementById('url-entry') as HTMLElement; },
    get urlInput() { return document.getElementById('url-input') as HTMLInputElement; },
    get pasteBtn() { return document.getElementById('paste-btn') as HTMLButtonElement; },
    get batchEntry() { return document.getElementById('batch-entry') as HTMLElement; },
    get batchInput() { return document.getElementById('batch-input') as HTMLTextAreaElement; },
    get formatSelect() { return document.getElementById('format-select') as HTMLSelectElement; },
    get qualitySelect() { return document.getElementById('quality-select') as HTMLSelectElement; },
    get pathDisplay() { return document.getElementById('path-display') as HTMLElement; },
    get pathText() { return document.getElementById('path-text') as HTMLElement; },
    get downloadBtn() { return document.getElementById('download-btn') as HTMLButtonElement; },
    get downloadBtnText() { return document.getElementById('download-btn-text') as HTMLElement; },
    get urlModeTag() { return document.getElementById('url-mode-tag') as HTMLElement; },

    // Queue Section
    get queueContainer() { return document.getElementById('queue-container') as HTMLElement; },
    get queueEmpty() { return document.getElementById('queue-empty') as HTMLElement; },
    get clearBtn() { return document.getElementById('clear-btn') as HTMLButtonElement; },
    get tabDownloads() { return document.getElementById('tab-downloads') as HTMLButtonElement; },
    get tabTerminal() { return document.getElementById('tab-terminal') as HTMLButtonElement; },

    // Global Progress Container (holds dynamic per-binary progress bars)
    get globalProgressContainer() { return document.getElementById('global-progress-container') as HTMLElement; },

    // Settings Modal
    get settingsModal() { return document.getElementById('settings-modal') as HTMLElement; },
    get settingsBtn() { return document.getElementById('settings-btn') as HTMLButtonElement; },
    get closeSettingsBtn() { return document.getElementById('close-settings-btn') as HTMLButtonElement; },
    get settingsYtdlpVersion() { return document.getElementById('settings-ytdlp-version') as HTMLElement; },
    get settingsFfmpegStatus() { return document.getElementById('settings-ffmpeg-status') as HTMLElement; },
    get settingsFfprobeStatus() { return document.getElementById('settings-ffprobe-status') as HTMLElement; },
    get settingsUpdateBtn() { return document.getElementById('settings-update-btn') as HTMLButtonElement; },
    get checkUpdatesBtn() { return document.getElementById('check-updates-btn') as HTMLButtonElement; },
    get checkUpdatesIcon() { return document.getElementById('check-updates-icon') as HTMLElement; },
    get reinstallYtdlpBtn() { return document.getElementById('reinstall-ytdlp-btn') as HTMLButtonElement; },
    get reinstallFfmpegBtn() { return document.getElementById('reinstall-ffmpeg-btn') as HTMLButtonElement; },
    get reinstallDenoBtn() { return document.getElementById('reinstall-deno-btn') as HTMLButtonElement; },
    get settingsDenoStatus() { return document.getElementById('settings-deno-status') as HTMLElement; },
    get settingsDenoVersion() { return document.getElementById('settings-deno-version') as HTMLElement; },
    get settingsDenoUpdateBtn() { return document.getElementById('settings-deno-update-btn') as HTMLButtonElement; },
    get settingsPathDisplay() { return document.getElementById('settings-path-display') as HTMLElement; },
    get settingsPathText() { return document.getElementById('settings-path-text') as HTMLElement; },
    get settingsVideoPathDisplay() { return document.getElementById('settings-video-path-display') as HTMLElement; },
    get settingsVideoPathText() { return document.getElementById('settings-video-path-text') as HTMLElement; },
    get settingsCheckAuthBtn() { return document.getElementById('settings-check-auth-btn') as HTMLButtonElement; },
    get authEnabledToggle() { return document.getElementById('auth-enabled-toggle') as HTMLInputElement; },
    get writeSubsToggle() { return document.getElementById('write-subs-toggle') as HTMLInputElement; },
    get customDepsToggle() { return document.getElementById('custom-deps-toggle') as HTMLInputElement; },
    get debugLoggingToggle() { return document.getElementById('debug-logging-toggle') as HTMLInputElement; },
    get openLogsBtn() { return document.getElementById('open-logs-btn') as HTMLButtonElement; },

    // Settings Tabs
    get settingsTabSystemBtn() { return document.getElementById('settings-tab-system-btn') as HTMLButtonElement; },
    get settingsTabAppBtn() { return document.getElementById('settings-tab-app-btn') as HTMLButtonElement; },
    get settingsTabCookiesBtn() { return document.getElementById('settings-tab-cookies-btn') as HTMLButtonElement; },
    get settingsTabSystemPane() { return document.getElementById('settings-tab-system-pane') as HTMLElement; },
    get settingsTabAppPane() { return document.getElementById('settings-tab-app-pane') as HTMLElement; },
    get settingsTabCookiesPane() { return document.getElementById('settings-tab-cookies-pane') as HTMLElement; },

    get concurrentLimitSlider() { return document.getElementById('concurrent-limit-slider') as HTMLInputElement; },
    get concurrentLimitValue() { return document.getElementById('concurrent-limit-value') as HTMLElement; },
    get fragmentsSlider() { return document.getElementById('fragments-slider') as HTMLInputElement; },
    get fragmentsValue() { return document.getElementById('fragments-value') as HTMLElement; },

    // Confirmation Modal
    get confirmModal() { return document.getElementById('confirm-modal') as HTMLElement; },
    get confirmTitle() { return document.getElementById('confirm-title') as HTMLElement; },
    get confirmMessage() { return document.getElementById('confirm-message') as HTMLElement; },
    get confirmOkBtn() { return document.getElementById('confirm-ok-btn') as HTMLButtonElement; },
    get confirmCancelBtn() { return document.getElementById('confirm-cancel-btn') as HTMLButtonElement; },

    // Containers
    get toastContainer() { return document.getElementById('toast-container') as HTMLElement; },

    // Windows Controls
    get windowClose() { return document.getElementById('window-close') as HTMLButtonElement; },
    get windowMinimize() { return document.getElementById('window-minimize') as HTMLButtonElement; }
};

export default elements;

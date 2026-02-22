/**
 * UI Views - Handles screen transitions
 */
import elements from './elements.ts';
import { DependencyStatus, AppConfig } from '../types.ts';

const views = {
    showSetupScreen() {
        elements.setupScreen.classList.remove('hidden');
        elements.mainContent.classList.add('hidden');
        this.hideSplashScreen();
    },

    showMainContent() {
        elements.setupScreen.classList.add('hidden');
        elements.mainContent.classList.remove('hidden');
        this.hideSplashScreen();
    },

    hideSplashScreen() {
        const splash = elements.splashScreen;
        if (splash) {
            splash.classList.add('fade-out');
            setTimeout(() => {
                splash.remove();
            }, 400);
        }
    },

    updateSetupStatus(status: DependencyStatus) {
        if (status.yt_dlp_installed) {
            elements.ytdlpStatus.textContent = status.yt_dlp_version ? `Installed (${status.yt_dlp_version})` : 'Installed';
            elements.ytdlpStatus.classList.add('installed');
        } else {
            elements.ytdlpStatus.textContent = 'Not installed';
            elements.ytdlpStatus.classList.remove('installed');
        }

        if (status.ffmpeg_installed && status.ffprobe_installed) {
            elements.ffmpegStatus.textContent = 'Installed';
            elements.ffmpegStatus.classList.add('installed');
        } else {
            elements.ffmpegStatus.textContent = 'Not installed';
            elements.ffmpegStatus.classList.remove('installed');
        }

        if (status.deno_installed) {
            elements.denoStatus.textContent = status.deno_version ? `Installed (${status.deno_version})` : 'Installed';
            elements.denoStatus.classList.add('installed');
        } else {
            elements.denoStatus.textContent = 'Not installed';
            elements.denoStatus.classList.remove('installed');
        }
    },

    updateDepsStatus(status: DependencyStatus) {
        // Transition from splash screen to the appropriate initial screen
        const isOnSplash = elements.setupScreen.classList.contains('hidden') &&
            elements.mainContent.classList.contains('hidden');

        if (isOnSplash) {
            // Fresh install (setup never shown) AND deps missing → show setup screen
            if (!status.setup_shown && status.binaries_missing) {
                this.showSetupScreen();
                this.updateSetupStatus(status);
            } else {
                this.showMainContent();
            }
        } else if (status.setup_shown && !status.installation_in_progress && elements.mainContent.classList.contains('hidden')) {
            // Setup is done AND no installs running → transition to main
            this.showMainContent();
        }

        const anyMissing = status.binaries_missing;
        const banner = elements.updateBanner;
        const bannerText = elements.updateBannerText;
        const bannerBtn = elements.updateBannerBtn;

        if (anyMissing) {
            const missing = [];
            if (!status.yt_dlp_installed) missing.push('yt-dlp');
            if (!status.ffmpeg_installed) missing.push('ffmpeg');
            if (!status.ffprobe_installed) missing.push('ffprobe');
            if (!status.deno_installed) missing.push('deno');

            banner.classList.remove('hidden');
            banner.classList.add('warning');
            bannerText.textContent = `⚠️ Dependencies Missing: ${missing.join(', ')}`;
            bannerBtn.textContent = 'Go to Settings';
            bannerBtn.onclick = () => {
                elements.settingsBtn.click();
                elements.settingsTabSystemBtn.click();
            };
        } else if (status.yt_dlp_update_available) {
            banner.classList.remove('hidden', 'warning');
            bannerText.textContent = `yt-dlp update available (${status.yt_dlp_latest_version})`;
            bannerBtn.textContent = 'Update Now';
            // Update logic is handled via events/actions
        } else {
            banner.classList.add('hidden');
        }

        // Update download button state — disabled if deps missing or install in progress
        const downloadBtn = elements.downloadBtn;
        if (anyMissing) {
            downloadBtn.disabled = true;
            downloadBtn.title = 'Install dependencies in Settings to download';
        } else if (status.installation_in_progress) {
            downloadBtn.disabled = true;
            downloadBtn.title = 'Dependency installation in progress...';
        } else {
            downloadBtn.disabled = false;
            downloadBtn.title = '';
        }

        // Update settings info if modal is open (or just always update it)
        this.updateSettingsInfo(status);
    },

    updateConfigUI(config: AppConfig) {
        if (elements.customDepsToggle) {
            elements.customDepsToggle.checked = config.custom_deps;
        }

        // Toggle visibility of version info/update buttons in custom mode
        const updateButtons = [
            elements.settingsUpdateBtn,
            elements.settingsDenoUpdateBtn
        ];

        updateButtons.forEach(btn => {
            if (btn) btn.style.display = config.custom_deps ? 'none' : '';
        });

        // Update refresh button tooltip
        const refreshBtn = elements.checkUpdatesBtn;
        if (refreshBtn) {
            refreshBtn.title = config.custom_deps
                ? 'Refresh dependency status'
                : 'Check for updates';
        }

        // Version info itself might need hiding or replaced with "Custom"
        if (config.custom_deps) {
            if (elements.settingsYtdlpVersion) elements.settingsYtdlpVersion.textContent = 'Custom Mode';
            if (elements.settingsDenoVersion) elements.settingsDenoVersion.textContent = 'Custom Mode';
        }
    },


    showInstallProgress(show: boolean, isFinalizing = false) {
        if (show) {
            elements.setupProgress.classList.remove('hidden');
            if (elements.setupOptions) elements.setupOptions.classList.add('hidden');
            elements.installBtn.disabled = true;
            elements.installBtn.textContent = isFinalizing ? 'Finalizing...' : 'Installing...';
        } else {
            elements.setupProgress.classList.add('hidden');
            if (elements.setupOptions) elements.setupOptions.classList.remove('hidden');
            elements.installBtn.disabled = false;
            elements.installBtn.textContent = 'Install Components';
        }
    },

    updateInstallProgress(name: string, progress: number, status = 'downloading') {
        let text = '';
        if (status === 'downloading') {
            text = `Downloading ${name}: ${Math.round(progress)}%`;
        } else if (status === 'finalizing') {
            text = `Finalizing ${name}...`;
        } else if (status === 'extracting') {
            text = `Extracting ${name}: ${Math.round(progress)}%`;
        } else if (status === 'complete') {
            text = `Finished ${name}`;
        }

        // Update Setup Screen Progress
        if (elements.setupProgress && !elements.setupProgress.classList.contains('hidden')) {
            elements.setupProgressBar.style.width = `${progress}%`;
            elements.setupProgressText.textContent = text;
        }

        // Update per-binary global progress bar (for reinstalls)
        // Skip the "all" meta-target — it's handled separately in events.ts
        if (name === 'all') return;
        const container = elements.globalProgressContainer;
        if (container) {
            let bar = container.querySelector(`[data-target="${name}"]`) as HTMLElement | null;
            if (!bar && status !== 'complete') {
                // Create a new progress bar for this binary
                bar = document.createElement('div');
                bar.className = 'global-progress';
                bar.dataset.target = name;
                bar.innerHTML = `
                    <div class="progress-bar">
                        <div class="progress-bar-fill active" style="width: 0%"></div>
                    </div>
                    <span class="global-progress-text">${text}</span>
                `;
                container.appendChild(bar);
            }
            if (bar) {
                const fill = bar.querySelector('.progress-bar-fill') as HTMLElement;
                const textEl = bar.querySelector('.global-progress-text') as HTMLElement;
                if (fill) fill.style.width = `${progress}%`;
                if (textEl) textEl.textContent = text;

                // Remove bar on completion after a brief delay
                if (status === 'complete') {
                    setTimeout(() => bar?.remove(), 1000);
                }
            }
        }

        // Reactively disable/enable reinstall buttons based on backend state
        const btnMap: Record<string, HTMLButtonElement | null> = {
            'yt-dlp': elements.reinstallYtdlpBtn,
            'ffmpeg': elements.reinstallFfmpegBtn,
            'deno': elements.reinstallDenoBtn,
        };
        const btn = btnMap[name];
        if (btn) {
            const isActive = status !== 'complete';
            btn.disabled = isActive;
            const icon = btn.querySelector('img');
            if (icon) {
                if (isActive) icon.classList.add('spin');
                else icon.classList.remove('spin');
            }
        }
    },

    updateSettingsInfo(status: DependencyStatus) {
        // Shared Status Helper
        const setStatus = (el: HTMLElement | null, installed: boolean, version: string | null = null) => {
            if (!el) return;
            el.className = installed ? 'list-row-subtitle status-complete' : 'list-row-subtitle status-missing';
            if (status.custom_deps) {
                el.textContent = installed ? '✓ Installed' : 'Not installed';
            } else {
                el.textContent = installed ? (version ? `v${version}` : '✓ Installed') : 'Not installed';
            }
        };

        // yt-dlp
        setStatus(elements.settingsYtdlpVersion, status.yt_dlp_installed, status.yt_dlp_version);

        // ffmpeg & ffprobe
        setStatus(elements.settingsFfmpegStatus, status.ffmpeg_installed);
        setStatus(elements.settingsFfprobeStatus, status.ffprobe_installed);

        // deno
        setStatus(elements.settingsDenoVersion, status.deno_installed, status.deno_version);

        // Update buttons (Only relevant in non-custom mode)
        if (!status.custom_deps) {
            if (status.deno_update_available) {
                elements.settingsDenoUpdateBtn.disabled = false;
                elements.settingsDenoUpdateBtn.textContent = 'Update Available';
            } else {
                elements.settingsDenoUpdateBtn.disabled = true;
                elements.settingsDenoUpdateBtn.textContent = 'Up to date';
            }

            if (status.yt_dlp_update_available) {
                elements.settingsUpdateBtn.disabled = false;
                elements.settingsUpdateBtn.textContent = 'Update Available';
            } else {
                elements.settingsUpdateBtn.disabled = true;
                elements.settingsUpdateBtn.textContent = 'Up to date';
            }
        }
    },

    setCheckingUpdates(isChecking: boolean) {
        if (isChecking) {
            elements.checkUpdatesIcon.classList.add('spin');
            elements.checkUpdatesBtn.disabled = true;
        } else {
            elements.checkUpdatesIcon.classList.remove('spin');
            elements.checkUpdatesBtn.disabled = false;
        }
    },

    hideAllGlobalProgress() {
        const container = elements.globalProgressContainer;
        if (container) container.innerHTML = '';
    },


    switchQueueTab(tabName: 'downloads' | 'terminal') {
        const tabDownloads = document.getElementById('tab-downloads') as HTMLElement;
        const tabTerminal = document.getElementById('tab-terminal') as HTMLElement;
        const viewDownloads = document.getElementById('queue-container') as HTMLElement;
        const viewTerminal = document.getElementById('terminal-view') as HTMLElement;
        const terminalControls = document.getElementById('terminal-controls') as HTMLElement;
        const clearBtn = document.getElementById('clear-btn') as HTMLElement;

        if (tabName === 'terminal') {
            tabDownloads.classList.remove('active');
            tabTerminal.classList.add('active');
            viewDownloads.classList.add('hidden');
            viewTerminal.classList.remove('hidden');

            // Show terminal controls, hide normal clear button
            if (terminalControls) terminalControls.classList.remove('hidden');
            if (clearBtn) clearBtn.classList.add('hidden');

            // Notify terminal module to scroll to bottom if needed
            // This is handled by main.js wiring or direct call if we imported Terminal (circular dep avoidance)
        } else {
            tabDownloads.classList.add('active');
            tabTerminal.classList.remove('active');
            viewDownloads.classList.remove('hidden');
            viewTerminal.classList.add('hidden');

            if (terminalControls) terminalControls.classList.add('hidden');
            if (clearBtn) clearBtn.classList.remove('hidden');
        }
    }
};

export default views;

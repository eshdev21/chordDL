/**
 * UI Controls - Handles input, mode switchers, and path displays
 */
import elements from './elements.ts';
import CustomSelect from './CustomSelect.ts';
import { updateConfig } from '../config.ts';

const controls = {
    formatSelectRef: null as CustomSelect | null,
    qualitySelectRef: null as CustomSelect | null,
    lastAudioFormat: null as string | null,
    lastVideoFormat: null as string | null,

    setupModeSwitcher() {
        const buttons = elements.modeSwitcher.querySelectorAll('.mode-btn');
        buttons.forEach(btn => {
            btn.addEventListener('click', () => {
                buttons.forEach(b => (b as HTMLElement).classList.remove('active'));
                (btn as HTMLElement).classList.add('active');
                this.switchMode((btn as HTMLElement).dataset.mode as 'single' | 'batch' | 'playlist');
            });
        });
    },

    setupMediaTypeSwitcher() {
        const audioTab = elements.audioTab;
        const videoTab = elements.videoTab;

        audioTab.addEventListener('click', () => {
            audioTab.classList.add('active');
            videoTab.classList.remove('active');
            this.updateMediaOptions('audio');
            this.setDownloadButtonState((elements.downloadBtn.dataset.lastState as 'safe' | 'loading' | 'disabled') || 'safe');
            window.dispatchEvent(new CustomEvent('mediaTypeChange', { detail: { type: 'audio' } }));
        });

        videoTab.addEventListener('click', () => {
            videoTab.classList.add('active');
            audioTab.classList.remove('active');
            this.updateMediaOptions('video');
            this.setDownloadButtonState((elements.downloadBtn.dataset.lastState as 'safe' | 'loading' | 'disabled') || 'safe');
            window.dispatchEvent(new CustomEvent('mediaTypeChange', { detail: { type: 'video' } }));
        });

        // Initialize Custom Select for Format
        if (elements.formatSelect) {
            this.formatSelectRef = new CustomSelect(elements.formatSelect, {
                onChange: (value: string) => {
                    if (value) {
                        const type = this.getMediaType();
                        if (type === 'audio') {
                            this.lastAudioFormat = value;
                            updateConfig({ default_format: value });
                        } else {
                            this.lastVideoFormat = value;
                            updateConfig({ video_format: value });
                        }
                    }
                }
            });
        }

        // Initialize Custom Select for Quality
        if (elements.qualitySelect) {
            this.qualitySelectRef = new CustomSelect(elements.qualitySelect, {
                onChange: (value: string) => {
                    if (value) {
                        updateConfig({ video_quality: value });
                    }
                }
            });
        }
    },

    updateMediaOptions(type: 'audio' | 'video') {
        const formatSelect = elements.formatSelect;

        // Clear existing options
        if (this.formatSelectRef) {
            this.formatSelectRef.clear(true); // Silent clear of selection
            this.formatSelectRef.clearOptions();
        } else {
            formatSelect.innerHTML = '';
        }

        let formats: { val: string, label: string }[] = [];

        if (type === 'audio') {
            // Hide Quality Dropdown
            if (this.qualitySelectRef) {
                this.qualitySelectRef.wrapper.classList.add('hidden');
            } else {
                elements.qualitySelect.classList.add('hidden');
            }

            formats = [
                { val: 'mp3', label: 'mp3' },
                { val: 'm4a', label: 'm4a' },
                { val: 'wav', label: 'wav' },
                { val: 'flac', label: 'flac' },
                { val: 'opus', label: 'opus' }
            ];
        } else {
            // Show Quality Dropdown
            if (this.qualitySelectRef) {
                this.qualitySelectRef.wrapper.classList.remove('hidden');
            } else {
                elements.qualitySelect.classList.remove('hidden');
            }

            formats = [
                { val: 'mp4', label: 'mp4' },
                { val: 'webm', label: 'webm' },
                { val: 'mkv', label: 'mkv' }
            ];
        }

        // Add options
        formats.forEach(f => {
            if (this.formatSelectRef) {
                this.formatSelectRef.addOption({ value: f.val, text: f.label });
            } else {
                const opt = document.createElement('option');
                opt.value = f.val;
                opt.textContent = f.label;
                formatSelect.appendChild(opt);
            }
        });

        // Sync and select first option
        if (this.formatSelectRef) {
            this.formatSelectRef.refreshOptions(false); // Ensure UI reflects changes

            // Restore last selected format for this type
            let restored = null;
            if (type === 'audio' && this.lastAudioFormat) restored = this.lastAudioFormat;
            if (type === 'video' && this.lastVideoFormat) restored = this.lastVideoFormat;

            if (restored) {
                this.formatSelectRef.setValue(restored, true); // silent update
            } else if (formats.length > 0) {
                this.formatSelectRef.setValue(formats[0].val, true);
                // Set initial defaults
                if (type === 'audio') this.lastAudioFormat = formats[0].val;
                else this.lastVideoFormat = formats[0].val;
            }
        }
    },

    getMediaType(): 'audio' | 'video' {
        return elements.audioTab.classList.contains('active') ? 'audio' : 'video';
    },

    switchMode(mode: 'single' | 'batch' | 'playlist') {
        if (mode === 'batch') {
            elements.urlEntry.classList.add('hidden');
            elements.batchEntry.classList.remove('hidden');
        } else {
            elements.urlEntry.classList.remove('hidden');
            elements.batchEntry.classList.add('hidden');
        }

        this.updateUrlModeTag(mode);
    },

    updateUrlModeTag(mode?: string) {
        if (!elements.urlModeTag) return;

        const labels: Record<string, string> = {
            'single': '(Single Download Mode)',
            'batch': '(Batch Mode)',
            'playlist': '(Playlist Mode)'
        };

        const activeMode = mode || this.getCurrentMode();
        elements.urlModeTag.textContent = labels[activeMode] || '';
    },


    setDownloadButtonState(state: 'loading' | 'safe' | 'disabled') {
        const btn = elements.downloadBtn;
        let text = elements.downloadBtnText;

        const labels: Record<string, string> = {
            'audio': 'Download Audio',
            'video': 'Download Video'
        };

        const type = this.getMediaType();
        const typeLabel = labels[type] || 'Download';
        const downloadIcon = `<img src="assets/icons/action-download.svg" class="icon-md" alt="">`;
        const defaultContent = `${downloadIcon} <span id="download-btn-text">${typeLabel}</span>`;

        // If text element is missing (e.g. wiped by innerHTML), try to re-find it
        if (!text) {
            text = btn.querySelector('#download-btn-text') as HTMLElement;
        }

        // Reset classes
        btn.classList.remove('loading');

        // Restore default structure if returning from verify state or if text is missing
        if (!text) {
            btn.innerHTML = defaultContent;
            text = btn.querySelector('#download-btn-text') as HTMLElement;
        }

        // If we still don't have text and we aren't in a state where we are about to rewrite it completely
        if (!text) {
            btn.innerHTML = defaultContent;
            text = btn.querySelector('#download-btn-text') as HTMLElement;
        }

        switch (state) {
            case 'loading':
                btn.classList.add('loading');
                btn.disabled = true;
                break;
            case 'disabled':
                btn.disabled = true;
                break;
            default: {
                // Always update if the text doesn't match the current label, or if coming from loading
                const currentText = text ? text.textContent : '';
                if (btn.dataset.lastState === 'loading' || !text || currentText !== typeLabel) {
                    btn.innerHTML = defaultContent;
                }
                btn.disabled = false;
                break;
            }
        }
        btn.dataset.lastState = state;
    },

    setupPasteButton() {
        elements.pasteBtn.addEventListener('click', async () => {
            try {
                const text = await navigator.clipboard.readText();
                elements.urlInput.value = text;
                elements.urlInput.focus();
            } catch (err) {
                console.error('Failed to read clipboard:', err);
            }
        });
    },

    getCurrentMode(): 'single' | 'batch' | 'playlist' {
        const active = elements.modeSwitcher.querySelector('.mode-btn.active') as HTMLElement;
        return active ? (active.dataset.mode as 'single' | 'batch' | 'playlist') || 'single' : 'single';
    },

    getUrls(): string[] {
        const mode = this.getCurrentMode();
        if (mode === 'batch') {
            return elements.batchInput.value.split('\n').map(u => u.trim()).filter(u => u);
        } else {
            return [elements.urlInput.value.trim()];
        }
    },

    clearInput() {
        elements.urlInput.value = '';
        elements.batchInput.value = '';
    },

    updatePath(path: string, mediaType: 'audio' | 'video' = 'audio') {
        elements.pathText.textContent = path || 'Select folder...';
        const icon = mediaType === 'video' ? 'assets/icons/folder-videos.svg' : 'assets/icons/folder-music.svg';
        (elements.pathDisplay.querySelector('img') as HTMLImageElement).src = icon;
    },

    updateSettingsPaths(audioPath: string, videoPath: string) {
        elements.settingsPathText.textContent = audioPath || 'Default';
        elements.settingsVideoPathText.textContent = videoPath || 'Default';
    },

    setFormat(value: string) {
        if (this.formatSelectRef) {
            this.formatSelectRef.setValue(value, true); // silent update

            // Also update our local state so we don't revert on tab switch
            const type = this.getMediaType();
            if (type === 'audio') this.lastAudioFormat = value;
            else this.lastVideoFormat = value;
        } else if (elements.formatSelect) {
            elements.formatSelect.value = value;
        }
    },

    setQuality(value: string) {
        if (this.qualitySelectRef) {
            this.qualitySelectRef.setValue(value, true); // silent update
        } else if (elements.qualitySelect) {
            elements.qualitySelect.value = value;
        }
    }
};

export default controls;

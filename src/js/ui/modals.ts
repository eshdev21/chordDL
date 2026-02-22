/**
 * UI Modals - Handles Settings, Toast, and Confirmation dialogs
 */
import elements from './elements.ts';
import { invoke } from '@tauri-apps/api/core';

const modals = {
    setupSettingsModal() {
        elements.settingsBtn?.addEventListener('click', () => {
            elements.settingsModal?.classList.remove('hidden');
            this.updateSliderFill(elements.concurrentLimitSlider);
            invoke('check_deps_status').catch(() => { });
        });

        this.setupSettingsTabs();

        elements.closeSettingsBtn?.addEventListener('click', () => {
            elements.settingsModal?.classList.add('hidden');
        });

        elements.settingsModal?.addEventListener('click', (e: MouseEvent) => {
            if (e.target === elements.settingsModal) {
                elements.settingsModal?.classList.add('hidden');
            }
        });

        if (elements.concurrentLimitSlider) {
            elements.concurrentLimitSlider.addEventListener('input', (e: Event) => {
                const target = e.target as HTMLInputElement;
                this.updateSliderFill(target);
                if (elements.concurrentLimitValue) {
                    elements.concurrentLimitValue.textContent = target.value;
                }
            });
        }

        // Custom Deps Toggle
        if (elements.customDepsToggle) {
            elements.customDepsToggle.addEventListener('change', async (e: Event) => {
                const target = e.target as HTMLInputElement;
                const enabled = target.checked;
                const confirmed = await this.showConfirm(
                    "Switch Custom Dependencies Mode?",
                    enabled
                        ? "Chord will use binaries in 'custombin/'. You must manually manage updates. Switch now?"
                        : "Chord will use its managed binaries in 'bin/'. Switch back to automatic mode?"
                );

                if (confirmed) {
                    try {
                        await invoke('toggle_custom_mode', { enabled });
                    } catch (err) {
                        console.error('Failed to toggle custom mode:', err);
                        this.showToast('Failed to switch mode', 'error');
                        target.checked = !enabled; // Revert
                    }
                } else {
                    target.checked = !enabled; // Revert toggle
                }
            });
        }
    },

    setupSettingsTabs() {
        const tabs = [
            { btn: elements.settingsTabSystemBtn, pane: elements.settingsTabSystemPane },
            { btn: elements.settingsTabAppBtn, pane: elements.settingsTabAppPane },
            { btn: elements.settingsTabCookiesBtn, pane: elements.settingsTabCookiesPane }
        ];

        tabs.forEach(tab => {
            if (!tab.btn) return; // Guard against missing elements
            tab.btn.addEventListener('click', () => {
                // Deactivate all
                tabs.forEach(t => {
                    if (t.btn) t.btn.classList.remove('active');
                    if (t.pane) t.pane.classList.add('hidden');
                });

                // Activate clicked
                tab.btn.classList.add('active');
                if (tab.pane) tab.pane.classList.remove('hidden');
            });
        });
    },

    updateSliderFill(slider: HTMLInputElement | null) {
        if (!slider) return;
        const min = Number(slider.min) || 1;
        const max = Number(slider.max) || 10;
        const val = Number(slider.value);
        const percent = (val - min) * 100 / (max - min);
        slider.style.background = `linear-gradient(to right, var(--accent-color) 0%, var(--accent-color) ${percent}%, rgba(0, 0, 0, 0.1) ${percent}%, rgba(0, 0, 0, 0.1) 100%)`;
    },

    setupConfirmModal() {
        // Shared logic for the generic confirmation modal
        elements.confirmModal?.addEventListener('click', (e: MouseEvent) => {
            if (e.target === elements.confirmModal) {
                elements.confirmModal?.classList.add('hidden');
            }
        });
    },

    showToast(message: string, type: 'info' | 'warning' | 'error' | 'success' | 'critical' = 'info', options: { duration?: number; actions?: { label: string; onClick: () => void }[] } = {}) {
        const toast = document.createElement('div');
        toast.className = `toast ${type}`;

        const messageSpan = document.createElement('span');
        messageSpan.textContent = message;
        toast.appendChild(messageSpan);

        if (options.actions && options.actions.length > 0) {
            toast.classList.add('has-action');
            const actionsContainer = document.createElement('div');
            actionsContainer.className = 'toast-actions';

            options.actions.forEach((action: { label: string; onClick: () => void }) => {
                const btn = document.createElement('button');
                btn.className = 'btn-flat btn-sm';
                btn.textContent = action.label;
                btn.onclick = (e: MouseEvent) => {
                    e.stopPropagation();
                    action.onClick();
                };
                actionsContainer.appendChild(btn);
            });
            toast.appendChild(actionsContainer);
        }

        elements.toastContainer.appendChild(toast);

        // Auto-remove
        const duration = options.duration || 4000;
        setTimeout(() => {
            toast.classList.add('removing');
            setTimeout(() => {
                toast.remove();
            }, 300);
        }, duration);
    },

    showConfirm(title: string, message: string, options: { okText?: string; cancelText?: string } = { okText: 'OK', cancelText: 'Cancel' }): Promise<boolean> {
        return new Promise((resolve) => {
            if (elements.confirmTitle) elements.confirmTitle.textContent = title;
            if (elements.confirmMessage) elements.confirmMessage.textContent = message;
            if (elements.confirmOkBtn) elements.confirmOkBtn.textContent = options.okText || 'OK';
            if (elements.confirmCancelBtn) elements.confirmCancelBtn.textContent = options.cancelText || 'Cancel';

            const handleOk = () => {
                cleanup();
                resolve(true);
            };

            const handleCancel = () => {
                cleanup();
                resolve(false);
            };

            const cleanup = () => {
                elements.confirmOkBtn?.removeEventListener('click', handleOk);
                elements.confirmCancelBtn?.removeEventListener('click', handleCancel);
                elements.confirmModal?.classList.add('hidden');
            };

            elements.confirmOkBtn?.addEventListener('click', handleOk);
            elements.confirmCancelBtn?.addEventListener('click', handleCancel);
            elements.confirmModal?.classList.remove('hidden');
        });
    }
};

export default modals;

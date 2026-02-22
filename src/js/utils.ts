import UI from './ui.ts';

/**
 * Show notification (custom toast)
 */
export function showNotification(message: string) {
    UI.showToast(message);
}

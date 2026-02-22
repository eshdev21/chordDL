import UI from '../ui.ts';
import { AppErrorPayload, ChordWindow, ToastAction } from '../types.ts';

/**
 * Centralized Error Handler for Chord
 * Maps backend errors (typed string or objects) to user-friendly toasts and actions.
 */
export function handleAppError(error: unknown) {
    console.error('handleAppError caught:', error);

    let message = 'An unknown error occurred.';
    let category = 'generic';
    let severity: 'info' | 'warning' | 'error' | 'success' | 'critical' = 'error';

    // Tauri wraps backend errors - unwrap if needed
    const unwrapped = unwrapError(error);

    if (typeof unwrapped === 'object' && unwrapped !== null) {
        const payload = unwrapped as AppErrorPayload;
        message = payload.message || JSON.stringify(payload);
        category = (payload.category || 'generic').toLowerCase();
        severity = normalizeSeverity(payload.severity);
    } else {
        message = String(unwrapped);
    }

    const actions: ToastAction[] = [];

    // Actions based on category (backend decided, frontend just renders)
    if (category === 'auth_required' || category === 'browser_lock') {
        actions.push({
            label: 'Open Settings',
            onClick: () => {
                const settingsBtn = document.getElementById('settings-btn');
                if (settingsBtn) settingsBtn.click();
            }
        });
    } else if (category === 'dependency_missing') {
        severity = 'critical';

        // Trigger deps check to show banner correctly (long term)
        const appWindow = window as ChordWindow;
        appWindow.__TAURI__?.core.invoke('check_deps_status').catch((invokeError: unknown) => {
            console.error('Failed to check deps:', invokeError);
        });

        // Also force banner to show immediately (don't wait for event)
        const banner = document.getElementById('update-banner');
        const bannerText = document.getElementById('update-banner-text');
        if (banner && bannerText) {
            banner.classList.remove('hidden');
            banner.classList.add('warning');
            bannerText.textContent = '⚠️ Dependency missing — Install in Settings';
        }

        actions.push({
            label: 'Open Settings',
            onClick: () => {
                const settingsBtn = document.getElementById('settings-btn');
                const systemTab = document.getElementById('settings-tab-system-btn');
                if (settingsBtn) {
                    settingsBtn.click();
                    if (systemTab) {
                        setTimeout(() => systemTab.click(), 100);
                    }
                }
            }
        });
    }

    // Always add Copy Details
    actions.push({
        label: 'Copy Details',
        onClick: () => copyErrorDetails({ message, category })
    });

    // Clean message
    let cleanMsg = message.replace('yt-dlp error: ', '').replace('internal error: ', '');
    cleanMsg = cleanMsg.split('\n')[0];
    if (cleanMsg.length > 100) cleanMsg = `${cleanMsg.substring(0, 100)}...`;

    UI.showToast(cleanMsg, severity, { actions });
}

function unwrapError(error: unknown): unknown {
    if (typeof error !== 'object' || error === null) return error;

    const payload = error as { message?: unknown };
    if (typeof payload.message === 'object' && payload.message !== null) {
        return payload.message;
    }

    return error;
}

function normalizeSeverity(rawSeverity: AppErrorPayload['severity']): 'info' | 'warning' | 'error' | 'success' | 'critical' {
    const normalized = typeof rawSeverity === 'string' ? rawSeverity.toLowerCase() : 'error';
    if (normalized === 'info' || normalized === 'warning' || normalized === 'error' || normalized === 'success' || normalized === 'critical') {
        return normalized;
    }

    return 'error';
}

/**
 * Format and copy error details to clipboard
 */
function copyErrorDetails(error: { message: string; category: string }) {
    const timestamp = new Date().toLocaleString();
    const details = `Error: ${error.message}\nCategory: ${error.category}\nTime: ${timestamp}\nApp: Chord v2.0`;

    navigator.clipboard.writeText(details).then(() => {
        UI.showToast('Error details copied to clipboard', 'info', { duration: 2000 });
    }).catch(err => {
        console.error('Failed to copy details:', err);
    });
}

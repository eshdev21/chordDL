import { listen } from '@tauri-apps/api/event';

const MAX_LOG_LINES = 1000;
let isTerminalVisible = false;
let autoScroll = true;
let terminalOutput: HTMLElement | null = null;

export const Terminal = {
    init: async () => {
        terminalOutput = document.getElementById('terminal-output');
        const clearBtn = document.getElementById('terminal-clear-btn');
        const toggleAutoScrollBtn = document.getElementById('terminal-autoscroll-btn');

        if (!terminalOutput) return;

        // Auto-scroll toggle logic
        if (toggleAutoScrollBtn) {
            toggleAutoScrollBtn.onclick = () => {
                autoScroll = !autoScroll;
                toggleAutoScrollBtn.classList.toggle('active', autoScroll);
                toggleAutoScrollBtn.title = autoScroll ? "Auto-scroll: ON" : "Auto-scroll: OFF";
            };
        }

        // Clear logs
        if (clearBtn) {
            clearBtn.onclick = () => {
                if (terminalOutput) {
                    terminalOutput.innerHTML = '';
                }
            };
        }

        // Listen for logs from backend
        // Use standard Tauri event listener
        await listen('download:log', (event: { payload: string }) => {
            const line = event.payload; // String: "[stdout] ..." or "[stderr] ..."
            appendLog(line);
        });

        console.log("Terminal initialized");
    },

    setVisible: (visible: boolean) => {
        isTerminalVisible = visible;
        const terminalView = document.getElementById('terminal-view');
        if (terminalView) {
            terminalView.style.display = visible ? 'flex' : 'none';
            if (visible && autoScroll) {
                scrollToBottom();
            }
        }
    }
};

function appendLog(text: string) {
    if (!terminalOutput) return;

    // Clear "Waiting for logs..." placeholder on first real log
    const placeholder = terminalOutput.querySelector('.log-line.system');
    if (placeholder) {
        terminalOutput.removeChild(placeholder);
    }

    const div = document.createElement('div');
    div.classList.add('log-line');

    // Style stderr differently
    if (text.toLowerCase().includes('[stderr]') || text.toLowerCase().includes('error:')) {
        div.classList.add('log-stderr');
    } else if (text.toLowerCase().includes('[system]')) {
        div.classList.add('log-system');
    }

    div.textContent = text;
    terminalOutput.appendChild(div);

    // Buffer limit
    if (terminalOutput.childElementCount > MAX_LOG_LINES) {
        if (terminalOutput.firstChild) {
            terminalOutput.removeChild(terminalOutput.firstChild);
        }
    }

    if (autoScroll && isTerminalVisible) {
        scrollToBottom();
    }
}

function scrollToBottom() {
    if (terminalOutput) {
        requestAnimationFrame(() => {
            if (terminalOutput) {
                terminalOutput.scrollTop = terminalOutput.scrollHeight;
            }
        });
    }
}

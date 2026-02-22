/**
 * CustomSelect - Lightweight Replacement
 * Maintains the consistent API used in controls.js for seamless integration.
 */

interface CustomSelectOption {
    value: string;
    text: string;
}

interface CustomSelectOptions {
    onChange?: (value: string) => void;
}

export default class CustomSelect {
    private originalSelect: HTMLSelectElement;
    private onChange: (value: string) => void;
    private isOpen: boolean;
    private items: CustomSelectOption[];
    private selectedValue: string | null;

    public wrapper!: HTMLElement;
    private trigger!: HTMLElement;
    private triggerContent!: HTMLElement;
    private dropdown!: HTMLElement;
    private dropdownContent!: HTMLElement;

    constructor(originalSelect: HTMLSelectElement, options: CustomSelectOptions = {}) {
        this.originalSelect = originalSelect;
        this.onChange = options.onChange || (() => { });
        this.isOpen = false;
        this.items = []; // Current options {value, text}
        this.selectedValue = null;

        this.init();
    }

    init() {
        // Hide original select
        this.originalSelect.style.display = 'none';

        // Create DOM structure
        this.wrapper = document.createElement('div');
        this.wrapper.className = 'custom-select-wrapper code-select'; // 'code-select' for specific styling hooks if needed

        // Inherit classes from original select (like 'hidden')
        if (this.originalSelect.classList.contains('hidden')) {
            this.wrapper.classList.add('hidden');
        }

        // Trigger element (displayed value)
        this.trigger = document.createElement('div');
        this.trigger.className = 'custom-select-trigger';
        this.trigger.tabIndex = 0; // Make focusable

        this.triggerContent = document.createElement('div');
        this.triggerContent.className = 'custom-select-value';
        this.trigger.appendChild(this.triggerContent);

        // Arrow icon is handled by CSS ::after

        // Dropdown container
        this.dropdown = document.createElement('div');
        this.dropdown.className = 'custom-select-dropdown';
        this.dropdown.style.display = 'none';

        this.dropdownContent = document.createElement('div');
        this.dropdownContent.className = 'custom-select-content';
        this.dropdown.appendChild(this.dropdownContent);

        this.wrapper.appendChild(this.trigger);
        this.wrapper.appendChild(this.dropdown);

        // Insert into DOM
        if (this.originalSelect.parentNode) {
            this.originalSelect.parentNode.insertBefore(this.wrapper, this.originalSelect.nextSibling);
        }

        // Event Listeners
        this.trigger.addEventListener('click', () => {
            if (!this.wrapper.classList.contains('disabled')) {
                this.toggle();
            }
        });

        if (this.originalSelect.disabled) {
            this.wrapper.classList.add('disabled');
        }

        this.trigger.addEventListener('keydown', (e: KeyboardEvent) => {
            switch (e.key) {
                case 'Enter':
                case ' ':
                    e.preventDefault();
                    this.toggle();
                    break;
                case 'Escape':
                    this.close();
                    break;
                case 'ArrowDown':
                    e.preventDefault();
                    if (!this.isOpen) {
                        this.open();
                    } else {
                        this.navigateOptions(1);
                    }
                    break;
                case 'ArrowUp':
                    e.preventDefault();
                    if (!this.isOpen) {
                        this.open();
                    } else {
                        this.navigateOptions(-1);
                    }
                    break;
            }
        });

        // Close on outside click
        document.addEventListener('click', (e: MouseEvent) => {
            if (!this.wrapper.contains(e.target as Node)) {
                this.close();
            }
        });

        // Initialize from existing options in the select tag
        this.syncFromOriginal();
    }

    syncFromOriginal() {
        const originalOptions = Array.from(this.originalSelect.options);
        this.clearOptions();

        let hasSelected = false;
        originalOptions.forEach(opt => {
            this.addOption({
                value: opt.value,
                text: opt.textContent || ''
            });
            if (opt.selected) {
                this.setValue(opt.value, true);
                hasSelected = true;
            }
        });

        // Fallback: If nothing selected, select the first option (standard select behavior)
        if (!hasSelected && this.items.length > 0) {
            this.setValue(this.items[0].value, true);
        }
    }

    addOption(option: CustomSelectOption) {
        // option: { value, text }
        this.items.push(option);

        // Sync with original select
        const nativeOption = document.createElement('option');
        nativeOption.value = option.value;
        nativeOption.textContent = option.text;
        this.originalSelect.appendChild(nativeOption);

        const optionEl = document.createElement('div');
        optionEl.className = 'custom-select-option';
        optionEl.textContent = option.text;
        optionEl.dataset.value = option.value;

        optionEl.addEventListener('click', (e) => {
            e.stopPropagation();
            this.setValue(option.value);
            this.close();
        });

        this.dropdownContent.appendChild(optionEl);
    }

    clear(silent = false) {
        this.selectedValue = null;
        this.triggerContent.textContent = '';
        this.originalSelect.value = '';
        this.updateSelectedOption();
        if (!silent) this.onChange('');
    }

    clearOptions() {
        this.items = [];
        this.dropdownContent.innerHTML = '';
        this.originalSelect.innerHTML = ''; // Critical: Sync native select
        this.clear(true);
    }

    refreshOptions(_silent = false) {
        // No-op for this simple implementation
    }

    setValue(value: string, silent = false) {
        // Prevent re-triggering if same value (unless forced/silent)
        if (!silent && value == this.selectedValue) {
            return;
        }

        const item = this.items.find(i => i.value == value); // loose equality for string/number mix
        if (item) {
            this.selectedValue = item.value;
            this.triggerContent.textContent = item.text;
            this.originalSelect.value = item.value;
            this.updateSelectedOption(); // visual highlight

            if (!silent) {
                this.onChange(value);
            }
        }
    }

    navigateOptions(direction: number) {
        // direction: 1 (down) or -1 (up)
        if (this.items.length === 0) return;

        let index = this.items.findIndex(i => i.value == this.selectedValue);
        if (index === -1) index = 0;

        let newIndex = index + direction;

        // Clamp logic
        if (newIndex < 0) newIndex = 0;
        if (newIndex >= this.items.length) newIndex = this.items.length - 1;

        const newItem = this.items[newIndex];
        this.setValue(newItem.value);

        // Scroll to ensure visible
        const selectedEl = this.dropdownContent.querySelector(`.custom-select-option[data-value="${newItem.value}"]`) as HTMLElement;
        if (selectedEl) {
            selectedEl.scrollIntoView({ block: 'nearest' });
        }
    }

    updateSelectedOption() {
        const options = this.dropdownContent.querySelectorAll('.custom-select-option');
        options.forEach(opt => {
            const htmlOpt = opt as HTMLElement;
            if (htmlOpt.dataset.value == this.selectedValue) {
                htmlOpt.classList.add('selected');
                htmlOpt.classList.add('active'); // for styling compatibility
            } else {
                htmlOpt.classList.remove('selected');
                htmlOpt.classList.remove('active');
            }
        });
    }

    toggle() {
        if (this.isOpen) {
            this.close();
        } else {
            this.open();
        }
    }

    open() {
        this.isOpen = true;
        this.wrapper.classList.add('focus');
        this.wrapper.classList.add('input-active');
        this.dropdown.style.display = 'block';
        this.trigger.setAttribute('aria-expanded', 'true');

        // Scroll to selected
        const selected = this.dropdownContent.querySelector('.selected') as HTMLElement;
        if (selected) {
            selected.scrollIntoView({ block: 'nearest' });
        }
    }

    close() {
        if (!this.isOpen) return;
        this.isOpen = false;
        this.wrapper.classList.remove('focus');
        this.wrapper.classList.remove('input-active');
        this.trigger.setAttribute('aria-expanded', 'false');

        // Animation
        this.dropdown.classList.add('closing');
        this.dropdown.addEventListener('animationend', () => {
            this.dropdown.classList.remove('closing');
            this.dropdown.style.display = 'none';
        }, { once: true });
    }

    blur() {
        this.close();
        this.trigger.blur();
    }
}

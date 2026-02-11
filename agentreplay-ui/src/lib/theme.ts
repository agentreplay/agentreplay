// Theme utility functions for AgentReplay UI

export type ThemePreference = 'light' | 'dark' | 'midnight';
export type ResolvedTheme = 'light' | 'dark' | 'midnight';

/**
 * Resolves a theme preference to an actual theme value.
 */
export function resolveTheme(preference: ThemePreference): ResolvedTheme {
    return preference;
}

/**
 * Applies a resolved theme to the document root by toggling theme classes.
 * - 'light': no special class (default)
 * - 'dark': .dark class  
 * - 'midnight': .dark + .midnight classes (midnight inherits dark utilities)
 */
export function applyTheme(resolvedTheme: ResolvedTheme): void {
    const root = document.documentElement;
    root.classList.remove('dark', 'midnight');

    if (resolvedTheme === 'dark') {
        root.classList.add('dark');
    } else if (resolvedTheme === 'midnight') {
        root.classList.add('dark', 'midnight');
    }
    // 'light' = no classes needed
}

/**
 * Gets the current stored theme preference.
 */
export function getStoredTheme(): ThemePreference {
    const stored = localStorage.getItem('agentreplay-theme') as ThemePreference | null;
    return stored || 'dark';
}

/**
 * Initializes the theme on app startup from localStorage.
 * Returns a cleanup function (kept for API compat).
 */
export async function initTheme(): Promise<() => void> {
    const preference = getStoredTheme();
    applyTheme(resolveTheme(preference));

    // No-op cleanup (system preference listener removed since we no longer support 'system')
    return () => { };
}

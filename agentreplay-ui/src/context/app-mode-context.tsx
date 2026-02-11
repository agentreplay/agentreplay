import React, { createContext, useContext, useState, useEffect, useCallback } from 'react';

export type AppMode = 'basic' | 'pro';

interface AppModeContextValue {
    appMode: AppMode;
    setAppMode: (mode: AppMode) => void;
}

const STORAGE_KEY = 'agentreplay_app_mode';

const AppModeContext = createContext<AppModeContextValue>({
    appMode: 'basic',
    setAppMode: () => { },
});

export function AppModeProvider({ children }: { children: React.ReactNode }) {
    const [appMode, setAppModeState] = useState<AppMode>(() => {
        if (typeof window !== 'undefined') {
            const stored = localStorage.getItem(STORAGE_KEY);
            if (stored === 'pro' || stored === 'basic') return stored;
        }
        return 'basic';
    });

    const setAppMode = useCallback((mode: AppMode) => {
        setAppModeState(mode);
        localStorage.setItem(STORAGE_KEY, mode);
    }, []);

    return (
        <AppModeContext.Provider value={{ appMode, setAppMode }}>
            {children}
        </AppModeContext.Provider>
    );
}

export function useAppMode() {
    return useContext(AppModeContext);
}

import React, { createContext, useContext, useState, useCallback } from 'react';
import { BOTS, BOT_ORDER, type BotKind, type BotInfo } from '../lib/bot-constants';

// Re-export for convenience so consumers can import from one place
export { BOTS, BOT_ORDER };
export type { BotKind, BotInfo };

interface BotContextValue {
  activeBot: BotKind;
  setActiveBot: (bot: BotKind) => void;
  botInfo: BotInfo;
}

const STORAGE_KEY = 'agentreplay_active_bot';

const BotContext = createContext<BotContextValue>({
  activeBot: 'clawdbot',
  setActiveBot: () => {},
  botInfo: BOTS.clawdbot,
});

export function BotProvider({ children }: { children: React.ReactNode }) {
  const [activeBot, setActiveBotState] = useState<BotKind>(() => {
    if (typeof window !== 'undefined') {
      const stored = localStorage.getItem(STORAGE_KEY) as BotKind | null;
      if (stored && stored in BOTS) return stored;
    }
    return 'clawdbot';
  });

  const setActiveBot = useCallback((bot: BotKind) => {
    setActiveBotState(bot);
    localStorage.setItem(STORAGE_KEY, bot);
  }, []);

  return (
    <BotContext.Provider value={{ activeBot, setActiveBot, botInfo: BOTS[activeBot] }}>
      {children}
    </BotContext.Provider>
  );
}

export function useBot() {
  return useContext(BotContext);
}

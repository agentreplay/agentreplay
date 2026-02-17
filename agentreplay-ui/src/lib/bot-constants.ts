// Bot / Agent constants — aligned with OpenClaw's real architecture.
// OpenClaw is a self-hosted AI-assistant gateway that connects
// messaging channels (WhatsApp, Telegram, Discord, Slack, Signal, …)
// to LLM backends and executes tools/skills on behalf of the user.

export type BotKind = 'moltbot' | 'clawdbot' | 'openclaw';

export interface BotInfo {
  kind: BotKind;
  label: string;
  emoji: string;
  color: string;        // tailwind text-* color
  bgColor: string;      // tailwind bg-* for badges/rings
  borderColor: string;  // tailwind border-*
  description: string;
}

export const BOTS: Record<BotKind, BotInfo> = {
  moltbot: {
    kind: 'moltbot',
    label: 'Moltbot',
    emoji: '🔮',
    color: 'text-purple-400',
    bgColor: 'bg-purple-500/10',
    borderColor: 'border-purple-500/30',
    description: 'Multi-model orchestration bot',
  },
  clawdbot: {
    kind: 'clawdbot',
    label: 'Clawdbot',
    emoji: '🦀',
    color: 'text-orange-400',
    bgColor: 'bg-orange-500/10',
    borderColor: 'border-orange-500/30',
    description: 'Claude-powered coding agent',
  },
  openclaw: {
    kind: 'openclaw',
    label: 'OpenClaw',
    emoji: '🦞',
    color: 'text-red-400',
    bgColor: 'bg-red-500/10',
    borderColor: 'border-red-500/30',
    description: 'Self-hosted AI assistant gateway',
  },
};

export const BOT_ORDER: BotKind[] = ['moltbot', 'clawdbot', 'openclaw'];

// ── OpenClaw Channel Types ──────────────────────────────────────────────────
export type ChannelKind =
  | 'whatsapp' | 'telegram' | 'discord' | 'slack' | 'signal'
  | 'imessage' | 'webchat' | 'msteams' | 'matrix' | 'googlechat'
  | 'irc' | 'nostr' | 'line' | 'twitch';

export interface ChannelDef {
  kind: ChannelKind;
  label: string;
  emoji: string;
  color: string;
}

export const CHANNELS: ChannelDef[] = [
  { kind: 'whatsapp',   label: 'WhatsApp',     emoji: '💬', color: 'text-green-500'  },
  { kind: 'telegram',   label: 'Telegram',     emoji: '✈️', color: 'text-blue-400'   },
  { kind: 'discord',    label: 'Discord',      emoji: '🎮', color: 'text-indigo-400' },
  { kind: 'slack',      label: 'Slack',        emoji: '💼', color: 'text-purple-400' },
  { kind: 'signal',     label: 'Signal',       emoji: '🔒', color: 'text-sky-400'    },
  { kind: 'imessage',   label: 'iMessage',     emoji: '🍎', color: 'text-blue-500'   },
  { kind: 'webchat',    label: 'WebChat',      emoji: '🌐', color: 'text-cyan-400'   },
  { kind: 'msteams',    label: 'MS Teams',     emoji: '🟣', color: 'text-violet-400' },
  { kind: 'matrix',     label: 'Matrix',       emoji: '🟢', color: 'text-emerald-400'},
  { kind: 'googlechat', label: 'Google Chat',  emoji: '📧', color: 'text-yellow-500' },
];

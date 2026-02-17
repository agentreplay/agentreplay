// Copyright 2025 Sushanth (https://github.com/sushanthpy)
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

import { useEffect, useState, useMemo } from 'react';

const TIPS = [
  'Connecting to AgentReplay server…',
  'Syncing project metadata…',
  'Loading trace indices…',
  'Preparing evaluation pipelines…',
  'Initializing analytics engine…',
  'Configuring observability hooks…',
  'Warming up the playground…',
  'Almost there…',
];

/** Animated orbital ring SVG used as the loading indicator. */
function OrbitalSpinner() {
  return (
    <div className="relative w-28 h-28">
      {/* Outer rotating ring */}
      <svg
        className="absolute inset-0 w-full h-full animate-[spin_3s_linear_infinite]"
        viewBox="0 0 120 120"
        fill="none"
      >
        <circle
          cx="60"
          cy="60"
          r="54"
          stroke="hsl(var(--primary) / 0.15)"
          strokeWidth="3"
        />
        <circle
          cx="60"
          cy="60"
          r="54"
          stroke="hsl(var(--primary))"
          strokeWidth="3"
          strokeLinecap="round"
          strokeDasharray="85 255"
          className="drop-shadow-[0_0_6px_hsl(var(--primary)/0.5)]"
        />
      </svg>

      {/* Middle counter-rotating ring */}
      <svg
        className="absolute inset-0 w-full h-full animate-[spin_2s_linear_infinite_reverse]"
        viewBox="0 0 120 120"
        fill="none"
      >
        <circle
          cx="60"
          cy="60"
          r="42"
          stroke="hsl(var(--primary) / 0.10)"
          strokeWidth="2"
        />
        <circle
          cx="60"
          cy="60"
          r="42"
          stroke="hsl(var(--primary) / 0.6)"
          strokeWidth="2"
          strokeLinecap="round"
          strokeDasharray="50 214"
        />
      </svg>

      {/* Centre pulsing dot */}
      <div className="absolute inset-0 flex items-center justify-center">
        <div className="w-8 h-8 rounded-full bg-primary/20 animate-[pulse_2s_ease-in-out_infinite] flex items-center justify-center">
          <div className="w-4 h-4 rounded-full bg-primary shadow-[0_0_12px_hsl(var(--primary)/0.6)]" />
        </div>
      </div>
    </div>
  );
}

/** Horizontal shimmer progress bar that loops continuously. */
function ShimmerBar() {
  return (
    <div className="w-64 h-1 rounded-full bg-primary/10 overflow-hidden">
      <div
        className="h-full w-1/3 rounded-full bg-gradient-to-r from-transparent via-primary/60 to-transparent animate-[shimmer-slide_2s_ease-in-out_infinite]"
      />
    </div>
  );
}

/** Step indicator dots showing conceptual progress. */
function StepDots({ active }: { active: number }) {
  const steps = 5;
  return (
    <div className="flex items-center gap-2">
      {Array.from({ length: steps }).map((_, i) => (
        <div
          key={i}
          className={`
            h-1.5 rounded-full transition-all duration-700 ease-out
            ${i < active
              ? 'w-6 bg-primary'
              : i === active
                ? 'w-4 bg-primary/60 animate-pulse'
                : 'w-2 bg-primary/20'}
          `}
        />
      ))}
    </div>
  );
}

export default function SyncingScreen() {
  const [tipIdx, setTipIdx] = useState(0);
  const [dotStep, setDotStep] = useState(0);
  const [elapsed, setElapsed] = useState(0);

  // Cycle through tips every 4 s
  useEffect(() => {
    const id = setInterval(() => {
      setTipIdx((prev) => (prev + 1) % TIPS.length);
    }, 4000);
    return () => clearInterval(id);
  }, []);

  // Advance dot step every 6 s
  useEffect(() => {
    const id = setInterval(() => {
      setDotStep((prev) => Math.min(prev + 1, 5));
    }, 6000);
    return () => clearInterval(id);
  }, []);

  // Elapsed seconds counter
  useEffect(() => {
    const id = setInterval(() => setElapsed((e) => e + 1), 1000);
    return () => clearInterval(id);
  }, []);

  const formattedElapsed = useMemo(() => {
    const m = Math.floor(elapsed / 60);
    const s = elapsed % 60;
    return m > 0 ? `${m}m ${s}s` : `${s}s`;
  }, [elapsed]);

  return (
    <div className="fixed inset-0 z-50 flex flex-col items-center justify-center bg-background">
      {/* Subtle radial glow behind the spinner */}
      <div className="absolute w-[400px] h-[400px] rounded-full bg-primary/5 blur-3xl pointer-events-none" />

      {/* Spinner */}
      <div className="relative mb-10">
        <OrbitalSpinner />
      </div>

      {/* Brand */}
      <h1 className="text-xl font-semibold tracking-tight text-foreground mb-1 select-none">
        AgentReplay
      </h1>

      {/* Cycling status tip with crossfade */}
      <p
        key={tipIdx}
        className="text-sm text-textSecondary mb-6 h-5 animate-[fade-slide-up_0.5s_ease-out_forwards]"
      >
        {TIPS[tipIdx]}
      </p>

      {/* Shimmer bar */}
      <ShimmerBar />

      {/* Step dots */}
      <div className="mt-5">
        <StepDots active={dotStep} />
      </div>

      {/* Elapsed timer (subtle) */}
      <span className="mt-8 text-xs text-textTertiary tabular-nums select-none">
        {formattedElapsed}
      </span>
    </div>
  );
}

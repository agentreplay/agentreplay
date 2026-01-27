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

import { ReactNode, useState } from 'react';
import { cn } from '../../lib/utils';

interface TooltipProps {
    content: ReactNode;
    children: ReactNode;
    className?: string;
    side?: 'top' | 'bottom' | 'left' | 'right';
    delayMs?: number;
}

export default function Tooltip({
    content,
    children,
    className,
    side = 'top',
    delayMs = 300
}: TooltipProps) {
    const [isVisible, setIsVisible] = useState(false);
    let timeoutId: NodeJS.Timeout;

    const handleMouseEnter = () => {
        timeoutId = setTimeout(() => setIsVisible(true), delayMs);
    };

    const handleMouseLeave = () => {
        clearTimeout(timeoutId);
        setIsVisible(false);
    };

    const sideStyles = {
        top: 'bottom-full left-1/2 -translate-x-1/2 mb-2',
        bottom: 'top-full left-1/2 -translate-x-1/2 mt-2',
        left: 'right-full top-1/2 -translate-y-1/2 mr-2',
        right: 'left-full top-1/2 -translate-y-1/2 ml-2',
    };

    return (
        <div
            className="relative inline-flex"
            onMouseEnter={handleMouseEnter}
            onMouseLeave={handleMouseLeave}
        >
            {children}
            {isVisible && content && (
                <div
                    className={cn(
                        'absolute z-50 px-3 py-2 text-xs font-medium',
                        'bg-surface border border-border/60 rounded-lg shadow-lg',
                        'text-textPrimary whitespace-nowrap',
                        'transition-opacity duration-150',
                        'pointer-events-none',
                        sideStyles[side],
                        className
                    )}
                >
                    {content}
                    {/* Arrow */}
                    <div
                        className={cn(
                            'absolute w-2 h-2 bg-surface border-border/60',
                            'rotate-45',
                            side === 'top' && 'top-full left-1/2 -translate-x-1/2 -mt-1 border-r border-b',
                            side === 'bottom' && 'bottom-full left-1/2 -translate-x-1/2 -mb-1 border-l border-t',
                            side === 'left' && 'left-full top-1/2 -translate-y-1/2 -ml-1 border-r border-t',
                            side === 'right' && 'right-full top-1/2 -translate-y-1/2 -mr-1 border-l border-b'
                        )}
                    />
                </div>
            )}
        </div>
    );
}

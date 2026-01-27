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

import { useState } from 'react';
import { Check, Copy } from 'lucide-react';
import { cn } from '../../lib/utils';

interface CopyButtonProps {
    value: string;
    className?: string;
    size?: 'sm' | 'md';
}

export default function CopyButton({ value, className, size = 'sm' }: CopyButtonProps) {
    const [copied, setCopied] = useState(false);

    const handleCopy = async (e: React.MouseEvent) => {
        e.stopPropagation();
        try {
            await navigator.clipboard.writeText(value);
            setCopied(true);
            setTimeout(() => setCopied(false), 2000);
        } catch (err) {
            console.warn('Failed to copy', err);
        }
    };

    const sizeClasses = {
        sm: 'h-3.5 w-3.5',
        md: 'h-4 w-4',
    };

    return (
        <button
            onClick={handleCopy}
            className={cn(
                'inline-flex items-center justify-center rounded p-1 transition-colors',
                'hover:bg-surface-hover active:bg-surface',
                'text-textTertiary hover:text-textSecondary',
                'opacity-0 group-hover:opacity-100',
                className
            )}
            title={copied ? 'Copied!' : 'Copy to clipboard'}
        >
            {copied ? (
                <Check className={cn(sizeClasses[size], 'text-success')} />
            ) : (
                <Copy className={sizeClasses[size]} />
            )}
        </button>
    );
}

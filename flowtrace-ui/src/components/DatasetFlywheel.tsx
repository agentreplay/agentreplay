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

'use client';

import { Database, Sparkles } from 'lucide-react';

export function DatasetFlywheel() {
  return (
    <div className="p-6 bg-gradient-to-r from-primary/5 to-primary/10 rounded-xl border border-primary/20">
      <div className="flex items-center gap-3 mb-3">
        <div className="p-2 bg-primary/20 rounded-lg">
          <Sparkles className="w-5 h-5 text-primary" />
        </div>
        <div>
          <h3 className="font-semibold text-textPrimary">Dataset Flywheel</h3>
          <p className="text-sm text-textSecondary">Automatically curate fine-tuning data from production traces</p>
        </div>
      </div>
      <div className="flex items-center gap-4 text-sm">
        <div className="flex items-center gap-2 text-textTertiary">
          <Database className="w-4 h-4" />
          <span>0 examples auto-curated</span>
        </div>
        <button className="text-primary hover:underline">
          Configure auto-curation →
        </button>
      </div>
    </div>
  );
}

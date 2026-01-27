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

import React, { useState } from 'react';

// Types derived from Rust structs
interface ManagedDataset {
  id: string;
  name: string;
  semantic_version: string;
  description: string;
  test_cases: TestCase[];
  metadata: DatasetMetadata;
  parent_version?: string;
  created_at: number;
  updated_at: number;
}

interface TestCase {
  id: string;
  input: string;
  expected_output?: string;
  metadata: any;
  tags: string[];
}

interface DatasetMetadata {
  author: string;
  tags: string[];
  source: any;
  size: number;
  quality_metrics: DatasetQualityMetrics;
}

interface DatasetQualityMetrics {
  coverage: number;
  diversity: number;
  difficulty: number;
  staleness_days: number;
}

const DatasetVersionHistory: React.FC<{
  dataset: ManagedDataset;
  versions: ManagedDataset[];
  onSelectVersion: (version: ManagedDataset) => void;
}> = ({ dataset, versions, onSelectVersion }) => {
  return (
    <div className="flex flex-col gap-4">
      <div className="flex items-center justify-between">
        <h2 className="text-xl font-bold">{dataset.name}</h2>
        <span className="text-sm text-gray-500">
          Latest: v{dataset.semantic_version}
        </span>
      </div>

      <div className="flex gap-2 overflow-x-auto pb-4">
        {versions.map(v => (
          <div
            key={v.id}
            className="flex-shrink-0 p-4 border rounded cursor-pointer hover:bg-gray-50"
            onClick={() => onSelectVersion(v)}
          >
            <div className="font-bold">v{v.semantic_version}</div>
            <div className="text-sm text-gray-500">{new Date(v.created_at / 1000).toLocaleDateString()}</div>
            <div className="text-xs mt-2 bg-blue-100 text-blue-800 px-2 py-1 rounded inline-block">
              {v.test_cases.length} cases
            </div>
          </div>
        ))}
      </div>
    </div>
  );
};

export default function Datasets() {
  const [selectedVersion, setSelectedVersion] = useState<ManagedDataset | null>(null);

  // Mock data
  const currentDataset: ManagedDataset = {
    id: "1",
    name: "Customer Support Golden Set",
    semantic_version: "1.2.0",
    description: "Verified cases for support bot",
    test_cases: [],
    metadata: {
      author: "bob",
      tags: ["support", "golden"],
      source: { type: "Manual" },
      size: 150,
      quality_metrics: { coverage: 0.85, diversity: 0.9, difficulty: 0.3, staleness_days: 2 }
    },
    created_at: Date.now() * 1000,
    updated_at: Date.now() * 1000,
  };

  const versions = [currentDataset];

  return (
    <div className="container mx-auto p-6">
      <h1 className="text-2xl font-bold mb-6">Datasets</h1>

      <DatasetVersionHistory
        dataset={currentDataset}
        versions={versions}
        onSelectVersion={setSelectedVersion}
      />

      {selectedVersion && (
        <div className="mt-8 border-t pt-8">
          <h3 className="text-lg font-bold mb-4">Version Details: v{selectedVersion.semantic_version}</h3>

          <div className="grid grid-cols-4 gap-4 mb-8">
            <div className="p-4 bg-gray-50 rounded">
              <div className="text-sm text-gray-500">Coverage</div>
              <div className="text-xl font-bold">{(selectedVersion.metadata.quality_metrics.coverage * 100).toFixed(1)}%</div>
            </div>
             <div className="p-4 bg-gray-50 rounded">
              <div className="text-sm text-gray-500">Diversity</div>
              <div className="text-xl font-bold">{selectedVersion.metadata.quality_metrics.diversity.toFixed(2)}</div>
            </div>
             <div className="p-4 bg-gray-50 rounded">
              <div className="text-sm text-gray-500">Size</div>
              <div className="text-xl font-bold">{selectedVersion.metadata.size}</div>
            </div>
             <div className="p-4 bg-gray-50 rounded">
              <div className="text-sm text-gray-500">Staleness</div>
              <div className="text-xl font-bold">{selectedVersion.metadata.quality_metrics.staleness_days} days</div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

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

export interface PromptVersion {
  id: string;
  version: number;
  author: string;
  createdAt: number;
  notes?: string;
  content: string;
}

export interface PromptRecord {
  id: string;
  name: string;
  tags: string[];
  description?: string;
  lastEdited: number;
  deployedVersion: number | null;
  activeVersion: number;
  owner?: string;
  content: string;
  variables: Array<{ key: string; description?: string; required?: boolean }>;
  history: PromptVersion[];
}

const STORAGE_KEY = 'agentreplay.prompts';

const samplePrompts: PromptRecord[] = [
  {
    id: 'onboarding-assistant',
    name: 'Onboarding Agent',
    tags: ['support', 'flows'],
    description: 'Guides newly activated workspaces through the initial data sync checklist.',
    lastEdited: Date.now() - 1000 * 60 * 60 * 3,
    deployedVersion: 7,
    activeVersion: 9,
    owner: 'maya@agentreplay.ai',
    content: `You are Agentreplay Onboarding Agent.

Steps:
1. Greet the operator by name.
2. Inspect the workspace health card and highlight anything red.
3. Offer to run "smart verify" if the ingestion lag is > 60s.
4. Close with an emoji from the ops playbook.

Context:
- Workspace name: {{workspace_name}}
- Primary model: {{model_family}}
- Latest trace: {{last_trace_timestamp}}

Respond with:
<summary>
  <status></status>
  <actions>
    <action priority="high">...</action>
  </actions>
</summary>`,
    variables: [
      { key: 'workspace_name', required: true },
      { key: 'model_family', required: true },
      { key: 'last_trace_timestamp', required: false },
    ],
    history: [
      {
        id: 'onboarding-assistant-v9',
        version: 9,
        author: 'maya@agentreplay.ai',
        createdAt: Date.now() - 1000 * 60 * 60 * 3,
        notes: 'Added emoji policy and stricter lag alert copy.',
        content: '',
      },
      {
        id: 'onboarding-assistant-v8',
        version: 8,
        author: 'sam@agentreplay.ai',
        createdAt: Date.now() - 1000 * 60 * 60 * 24,
        content: '',
      },
    ],
  },
  {
    id: 'rag-grounding',
    name: 'RAG Grounding Validator',
    tags: ['evals', 'llm-as-judge'],
    description: 'LLM-as-judge rubric for groundedness and hallucination tagging.',
    lastEdited: Date.now() - 1000 * 60 * 45,
    deployedVersion: 12,
    activeVersion: 12,
    owner: 'evals@agentreplay.ai',
    content: `You are the strict grounding reviewer for Agentreplay.

Given:
- Question
- Retrieved context snippets
- Model response

Return JSON with:
{
  "grounded": boolean,
  "score": number (0-1),
  "citations": [array of snippet ids],
  "notes": string,
  "hallucination": boolean
}

Guidelines:
- If the response adds facts not present in context, mark hallucination true.
- Penalize if citations do not cover every factual claim.
- Encourage concise, actionable notes.`,
    variables: [
      { key: 'question', required: true },
      { key: 'context', required: true },
      { key: 'response', required: true },
    ],
    history: [
      {
        id: 'rag-grounding-v12',
        version: 12,
        author: 'evals@agentreplay.ai',
        createdAt: Date.now() - 1000 * 60 * 45,
        notes: 'Strict citation coverage requirement.',
        content: '',
      },
    ],
  },
];

function hasBrowserStorage() {
  return typeof window !== 'undefined' && typeof window.localStorage !== 'undefined';
}

export function loadPrompts(): PromptRecord[] {
  if (!hasBrowserStorage()) {
    return samplePrompts;
  }

  const stored = window.localStorage.getItem(STORAGE_KEY);
  if (!stored) {
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(samplePrompts));
    return samplePrompts;
  }

  try {
    const parsed = JSON.parse(stored);
    return Array.isArray(parsed) ? parsed : samplePrompts;
  } catch (error) {
    console.warn('Failed to parse prompt store, resetting', error);
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(samplePrompts));
    return samplePrompts;
  }
}

export function savePrompts(prompts: PromptRecord[]) {
  if (!hasBrowserStorage()) {
    return;
  }
  window.localStorage.setItem(STORAGE_KEY, JSON.stringify(prompts));
}

export function upsertPrompt(prompt: PromptRecord) {
  const prompts = loadPrompts();
  const idx = prompts.findIndex((item) => item.id === prompt.id);
  if (idx >= 0) {
    prompts[idx] = prompt;
  } else {
    prompts.unshift(prompt);
  }
  savePrompts(prompts);
}

export function deletePrompt(promptId: string) {
  const prompts = loadPrompts().filter((prompt) => prompt.id !== promptId);
  savePrompts(prompts);
}

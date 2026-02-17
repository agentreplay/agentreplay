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

import { BookOpen, Brain, Code, Database, Settings, Zap, ExternalLink, Copy, CheckCircle2, HelpCircle, Eye, Activity, BarChart3, Server } from 'lucide-react';
import { useState, useEffect, useRef } from 'react';
import { useProjects } from '../context/project-context';
import { useLocation } from 'react-router-dom';
import { VideoHelpButton } from '../components/VideoHelpButton';

type Section = 'quick-start' | 'sdk' | 'api' | 'configuration' | 'ui-guide' | 'skill-memory';

export default function Docs() {
  const { currentProject } = useProjects();
  const location = useLocation();
  const [copiedIndex, setCopiedIndex] = useState<number | null>(null);
  const [activeSection, setActiveSection] = useState<Section>('quick-start');
  const scrollContainerRef = useRef<HTMLDivElement>(null);

  // Handle hash navigation (e.g., #sdk, #api)
  useEffect(() => {
    const hash = location.hash.replace('#', '') as Section;
    if (hash && ['quick-start', 'sdk', 'api', 'configuration', 'ui-guide', 'skill-memory'].includes(hash)) {
      handleScrollToSection(hash);
    }
  }, [location.hash]);

  const handleScrollToSection = (sectionId: Section) => {
    setActiveSection(sectionId);

    // Use setTimeout to ensure DOM is ready and to decouple from current event loop
    setTimeout(() => {
      const element = document.getElementById(sectionId);
      if (element && scrollContainerRef.current) {
        // Manual scroll on the specific container
        scrollContainerRef.current.scrollTo({
          top: element.offsetTop - 24, // Add padding
          behavior: 'smooth'
        });
      }
    }, 100);
  };

  const copyToClipboard = async (text: string, index: number) => {
    await navigator.clipboard.writeText(text);
    setCopiedIndex(index);
    setTimeout(() => setCopiedIndex(null), 2000);
  };

  const onSectionClick = (sectionId: Section) => {
    // 1. Manually scroll
    handleScrollToSection(sectionId);

    // 2. Update URL without triggering default browser scroll behavior
    // window.location.hash = sectionId; // <-- This causes the parent 'main' to scroll!
    history.pushState(null, '', `#${sectionId}`);
  };

  const [activeLang, setActiveLang] = useState<'python' | 'ts'>('python');

  const quickStartCode = {
    python: `# your_app.py - NO AGENTREPLAY IMPORTS NEEDED!
from openai import OpenAI

client = OpenAI()
response = client.chat.completions.create(
    model="gpt-4",
    messages=[{"role": "user", "content": "Hello!"}]
)
print(response.choices[0].message.content)`,
    ts: `// app.ts
import OpenAI from 'openai';

const client = new OpenAI();
const response = await client.chat.completions.create({
  model: "gpt-4",
  messages: [{ role: "user", content: "Hello!" }],
});
console.log(response.choices[0].message.content);`
  };

  const installCmd = {
    python: 'pip install agentreplay && agentreplay-install',
    ts: 'npm install @agentreplay/agentreplay'
  };

  const sections: Array<{
    id: Section;
    icon: JSX.Element;
    title: string;
    content: JSX.Element;
  }> = [
      {
        id: 'quick-start',
        icon: <Zap className="w-6 h-6 text-primary" />,
        title: "Quick Start",
        content: (
          <>
            <p className="text-textSecondary mb-4">
              Get started with Agentreplay in minutes. <strong className="text-textPrimary">Zero code changes required!</strong>
            </p>

            {/* Language Selector */}
            <div className="flex gap-2 mb-6 border-b border-border">
              {(['python', 'ts'] as const).map((lang) => (
                <button
                  key={lang}
                  onClick={() => setActiveLang(lang)}
                  className={`px-4 py-2 text-sm font-medium border-b-2 transition-colors \${
                    activeLang === lang
                      ? 'border-primary text-primary'
                      : 'border-transparent text-textTertiary hover:text-textPrimary'
                  }`}
                >
                  {lang === 'ts' ? 'Node.js' : lang.charAt(0).toUpperCase() + lang.slice(1)}
                </button>
              ))}
            </div>

            <div className="space-y-4">
              <div>
                <h4 className="text-sm font-semibold text-textPrimary mb-2">1. Install the SDK</h4>
                <div className="relative">
                  <pre className="bg-surface-elevated border border-border rounded-lg p-4 text-sm overflow-x-auto">
                    <code>{installCmd[activeLang]}</code>
                  </pre>
                  <button
                    onClick={() => copyToClipboard(installCmd[activeLang], 0)}
                    className="absolute top-2 right-2 p-2 hover:bg-surface-hover rounded-lg transition-colors"
                  >
                    {copiedIndex === 0 ? (
                      <CheckCircle2 className="w-4 h-4 text-green-500" />
                    ) : (
                      <Copy className="w-4 h-4 text-textSecondary" />
                    )}
                  </button>
                </div>
              </div>
              <div>
                <h4 className="text-sm font-semibold text-textPrimary mb-2">2. Set environment variables</h4>
                <div className="relative">
                  <pre className="bg-surface-elevated border border-border rounded-lg p-4 text-sm overflow-x-auto">
                    <code>{`export AGENTREPLAY_ENABLED=true
export AGENTREPLAY_OTLP_ENDPOINT=localhost:47117
export AGENTREPLAY_PROJECT_ID="${currentProject?.project_id || 'your-project-id'}"`}</code>
                  </pre>
                  <button
                    onClick={() => copyToClipboard(`export AGENTREPLAY_ENABLED=true\nexport AGENTREPLAY_OTLP_ENDPOINT=localhost:47117\nexport AGENTREPLAY_PROJECT_ID="${currentProject?.project_id || 'your-project-id'}"`, 1)}
                    className="absolute top-2 right-2 p-2 hover:bg-surface-hover rounded-lg transition-colors"
                  >
                    {copiedIndex === 1 ? (
                      <CheckCircle2 className="w-4 h-4 text-green-500" />
                    ) : (
                      <Copy className="w-4 h-4 text-textSecondary" />
                    )}
                  </button>
                </div>
              </div>
              <div>
                <h4 className="text-sm font-semibold text-textPrimary mb-2">3. Run your existing code - that&apos;s it!</h4>
                <div className="relative">
                  <pre className="bg-surface-elevated border border-border rounded-lg p-4 text-sm overflow-x-auto">
                    <code className={`language-${activeLang === 'ts' ? 'typescript' : activeLang}`}>
                      {quickStartCode[activeLang]}
                    </code>
                  </pre>
                  <button
                    onClick={() => copyToClipboard(quickStartCode[activeLang], 2)}
                    className="absolute top-2 right-2 p-2 hover:bg-surface-hover rounded-lg transition-colors"
                  >
                    {copiedIndex === 2 ? (
                      <CheckCircle2 className="w-4 h-4 text-green-500" />
                    ) : (
                      <Copy className="w-4 h-4 text-textSecondary" />
                    )}
                  </button>
                </div>
                <p className="text-xs text-textTertiary mt-2">
                  Run with: <code className="px-1 py-0.5 bg-surface-elevated rounded">{activeLang === 'python' ? 'python your_app.py' : 'npx ts-node app.ts'}</code> — traces appear automatically!
                </p>
              </div>
            </div>
          </>
        ),
      },
      {
        id: 'sdk',
        icon: <Code className="w-6 h-6 text-primary" />,
        title: "SDK Integration",
        content: (
          <>
            <p className="text-textSecondary mb-4">
              Agentreplay provides <strong className="text-textPrimary">true zero-code observability</strong>. Auto-instruments OpenAI, Anthropic, LangChain, LangGraph, LlamaIndex, CrewAI, and AutoGen.
            </p>
            <div className="space-y-6">
              <div>
                <div className="flex items-center gap-2 mb-2">
                  <h4 className="text-sm font-semibold text-textPrimary">Supported Frameworks</h4>
                  <span className="px-2 py-0.5 bg-green-500/20 text-green-500 rounded text-xs font-medium">Auto-instrumented</span>
                </div>
                <div className="bg-surface-elevated border border-border rounded-lg p-4 grid grid-cols-2 gap-3">
                  <div className="flex items-center gap-2">
                    <span className="text-green-500">✓</span>
                    <span className="text-sm text-textSecondary">OpenAI</span>
                  </div>
                  <div className="flex items-center gap-2">
                    <span className="text-green-500">✓</span>
                    <span className="text-sm text-textSecondary">Anthropic</span>
                  </div>
                  <div className="flex items-center gap-2">
                    <span className="text-green-500">✓</span>
                    <span className="text-sm text-textSecondary">LangChain</span>
                  </div>
                  <div className="flex items-center gap-2">
                    <span className="text-green-500">✓</span>
                    <span className="text-sm text-textSecondary">LangGraph</span>
                  </div>
                  <div className="flex items-center gap-2">
                    <span className="text-green-500">✓</span>
                    <span className="text-sm text-textSecondary">LlamaIndex</span>
                  </div>
                  <div className="flex items-center gap-2">
                    <span className="text-green-500">✓</span>
                    <span className="text-sm text-textSecondary">CrewAI</span>
                  </div>
                  <div className="flex items-center gap-2">
                    <span className="text-green-500">✓</span>
                    <span className="text-sm text-textSecondary">AutoGen</span>
                  </div>
                  <div className="flex items-center gap-2">
                    <span className="text-green-500">✓</span>
                    <span className="text-sm text-textSecondary">Azure OpenAI</span>
                  </div>
                </div>
              </div>
              <div>
                <h4 className="text-sm font-semibold text-textPrimary mb-2">LangGraph Multi-Agent Example</h4>
                <pre className="bg-surface-elevated border border-border rounded-lg p-4 text-sm overflow-x-auto">
                  <code className="language-python">{`# NO AGENTREPLAY IMPORTS NEEDED!
from langgraph.graph import StateGraph, START, END
from langchain_openai import ChatOpenAI
from langgraph.prebuilt import ToolNode, tools_condition

# Your existing LangGraph code works unchanged
workflow = StateGraph(State)
workflow.add_node("agent", agent)
workflow.add_node("tools", ToolNode(tools))
workflow.add_edge(START, "agent")
workflow.add_conditional_edges("agent", tools_condition)

graph = workflow.compile()
result = graph.invoke({"messages": [("user", "Hello!")]})
# ✅ All agents, tools, and LLM calls automatically traced!`}</code>
                </pre>
              </div>
            </div>
          </>
        ),
      },
      {
        id: 'api',
        icon: <Database className="w-6 h-6 text-primary" />,
        title: "API Reference",
        content: (
          <>
            <p className="text-textSecondary mb-4">
              REST API for programmatic access to traces, projects, and analytics data.
            </p>
            <div className="space-y-6">
              <div>
                <h4 className="text-sm font-semibold text-textPrimary mb-2">Base URL</h4>
                <div className="bg-surface-elevated border border-border rounded-lg p-4">
                  <code className="text-sm text-primary">http://localhost:47100/api/v1</code>
                </div>
              </div>
              <div>
                <h4 className="text-sm font-semibold text-textPrimary mb-3">Endpoints</h4>
                <div className="space-y-3">
                  <div className="bg-surface-elevated border border-border rounded-lg p-4">
                    <div className="flex items-center gap-2 mb-2">
                      <span className="px-2 py-1 bg-blue-500/20 text-blue-500 rounded text-xs font-bold">GET</span>
                      <code className="text-sm text-textPrimary">/projects</code>
                    </div>
                    <p className="text-sm text-textSecondary">List all projects with trace counts</p>
                  </div>
                  <div className="bg-surface-elevated border border-border rounded-lg p-4">
                    <div className="flex items-center gap-2 mb-2">
                      <span className="px-2 py-1 bg-green-500/20 text-green-500 rounded text-xs font-bold">POST</span>
                      <code className="text-sm text-textPrimary">/projects</code>
                    </div>
                    <p className="text-sm text-textSecondary mb-2">Create a new project</p>
                    <pre className="bg-background border border-border-subtle rounded p-2 text-xs">
                      <code>{`{
  "name": "My Project",
  "description": "Project description"
}`}</code>
                    </pre>
                  </div>
                  <div className="bg-surface-elevated border border-border rounded-lg p-4">
                    <div className="flex items-center gap-2 mb-2">
                      <span className="px-2 py-1 bg-blue-500/20 text-blue-500 rounded text-xs font-bold">GET</span>
                      <code className="text-sm text-textPrimary">/traces</code>
                    </div>
                    <p className="text-sm text-textSecondary mb-2">List traces with filters</p>
                    <div className="text-xs text-textTertiary">
                      Query params: <code className="px-1 py-0.5 bg-background rounded">project_id</code>, <code className="px-1 py-0.5 bg-background rounded">limit</code>, <code className="px-1 py-0.5 bg-background rounded">offset</code>
                    </div>
                  </div>
                  <div className="bg-surface-elevated border border-border rounded-lg p-4">
                    <div className="flex items-center gap-2 mb-2">
                      <span className="px-2 py-1 bg-blue-500/20 text-blue-500 rounded text-xs font-bold">GET</span>
                      <code className="text-sm text-textPrimary">/traces/:trace_id</code>
                    </div>
                    <p className="text-sm text-textSecondary">Get detailed trace information</p>
                  </div>
                  <div className="bg-surface-elevated border border-border rounded-lg p-4">
                    <div className="flex items-center gap-2 mb-2">
                      <span className="px-2 py-1 bg-blue-500/20 text-blue-500 rounded text-xs font-bold">GET</span>
                      <code className="text-sm text-textPrimary">/sessions</code>
                    </div>
                    <p className="text-sm text-textSecondary">List conversation sessions grouped by session_id</p>
                  </div>
                  <div className="bg-surface-elevated border border-border rounded-lg p-4">
                    <div className="flex items-center gap-2 mb-2">
                      <span className="px-2 py-1 bg-blue-500/20 text-blue-500 rounded text-xs font-bold">GET</span>
                      <code className="text-sm text-textPrimary">/analytics/metrics</code>
                    </div>
                    <p className="text-sm text-textSecondary mb-2">Get aggregated metrics</p>
                    <div className="text-xs text-textTertiary">
                      Query params: <code className="px-1 py-0.5 bg-background rounded">time_range</code> (24h, 7d, 30d)
                    </div>
                  </div>
                </div>
              </div>
            </div>
          </>
        ),
      },
      {
        id: 'configuration',
        icon: <Settings className="w-6 h-6 text-primary" />,
        title: "Configuration",
        content: (
          <>
            <p className="text-textSecondary mb-4">
              Configure Agentreplay using environment variables. No code changes needed!
            </p>
            <div className="space-y-4">
              <div>
                <div className="flex items-center gap-2 mb-2">
                  <h4 className="text-sm font-semibold text-textPrimary">Required</h4>
                  <span className="px-2 py-0.5 bg-primary/20 text-primary rounded text-xs font-medium">Must set</span>
                </div>
                <div className="bg-surface-elevated border border-border rounded-lg p-4 space-y-3">
                  <div>
                    <code className="text-xs font-mono text-primary">AGENTREPLAY_ENABLED</code>
                    <p className="text-sm text-textSecondary mt-1">
                      Set to <code className="px-1 py-0.5 bg-background rounded">true</code> to enable auto-instrumentation
                    </p>
                  </div>
                </div>
              </div>
              <div>
                <h4 className="text-sm font-semibold text-textPrimary mb-2">Optional</h4>
                <div className="bg-surface-elevated border border-border rounded-lg p-4 space-y-3">
                  <div>
                    <code className="text-xs font-mono text-primary">AGENTREPLAY_OTLP_ENDPOINT</code>
                    <p className="text-sm text-textSecondary mt-1">
                      OTLP endpoint for sending traces (default: <code className="px-1 py-0.5 bg-background rounded">localhost:47117</code>)
                    </p>
                  </div>
                  <div>
                    <code className="text-xs font-mono text-primary">AGENTREPLAY_PROJECT_ID</code>
                    <p className="text-sm text-textSecondary mt-1">
                      The project ID to associate traces with (default: <code className="px-1 py-0.5 bg-background rounded">0</code>)
                    </p>
                  </div>
                  <div>
                    <code className="text-xs font-mono text-primary">AGENTREPLAY_SERVICE_NAME</code>
                    <p className="text-sm text-textSecondary mt-1">
                      Your application name in traces (default: <code className="px-1 py-0.5 bg-background rounded">app</code>)
                    </p>
                  </div>
                  <div>
                    <code className="text-xs font-mono text-primary">AGENTREPLAY_LOG_LEVEL</code>
                    <p className="text-sm text-textSecondary mt-1">
                      Logging verbosity: DEBUG, INFO, WARNING, ERROR (default: <code className="px-1 py-0.5 bg-background rounded">INFO</code>)
                    </p>
                  </div>
                </div>
              </div>
              <div>
                <h4 className="text-sm font-semibold text-textPrimary mb-2">Complete Example</h4>
                <div className="bg-surface-elevated border border-border rounded-lg p-4">
                  <pre className="text-sm overflow-x-auto">
                    <code className="language-bash">{`# Enable auto-instrumentation
export AGENTREPLAY_ENABLED=true

# Point to your Agentreplay server
export AGENTREPLAY_OTLP_ENDPOINT=localhost:47117

# Associate traces with your project
export AGENTREPLAY_PROJECT_ID=${currentProject?.project_id || 'your-project-id'}

# Run your existing code - no changes needed!
python your_app.py`}</code>
                  </pre>
                </div>
              </div>
            </div>
          </>
        ),
      },
      {
        id: 'ui-guide',
        icon: <HelpCircle className="w-6 h-6 text-primary" />,
        title: "UI Guide — Interactive Training",
        content: (
          <>
            {/* Course Introduction */}
            <div className="bg-gradient-to-r from-primary/10 to-blue-500/10 border border-primary/20 rounded-lg p-6 mb-8">
              <h3 className="text-xl font-bold text-textPrimary mb-3">Master Agent Replay: Zero to Hero 🚀</h3>
              <p className="text-textSecondary mb-6">
                Whether you're just starting or managing production agents, we've structured this guide to match your journey.
              </p>
              <div className="grid md:grid-cols-3 gap-4">
                <div className="bg-surface-elevated/50 p-4 rounded-lg border border-border/50">
                  <div className="text-xs font-bold text-green-500 uppercase tracking-wider mb-2">Level 1: Beginner</div>
                  <div className="font-semibold text-textPrimary mb-1">The Observer 👁️</div>
                  <p className="text-xs text-textSecondary">Learn to see what your AI is doing. Master the basics of <strong>Traces</strong> and <strong>Conversations</strong>.</p>
                </div>
                <div className="bg-surface-elevated/50 p-4 rounded-lg border border-border/50">
                  <div className="text-xs font-bold text-blue-500 uppercase tracking-wider mb-2">Level 2: Intermediate</div>
                  <div className="font-semibold text-textPrimary mb-1">The Builder 🛠️</div>
                  <p className="text-xs text-textSecondary">Learn to fix issues. Master <strong>Debugging</strong>, <strong>Prompts</strong>, and <strong>Graph View</strong>.</p>
                </div>
                <div className="bg-surface-elevated/50 p-4 rounded-lg border border-border/50">
                  <div className="text-xs font-bold text-purple-500 uppercase tracking-wider mb-2">Level 3: Advanced</div>
                  <div className="font-semibold text-textPrimary mb-1">The Architect 🏛️</div>
                  <p className="text-xs text-textSecondary">Learn to optimize systems. Master <strong>Evaluations</strong>, <strong>Costs</strong>, and <strong>Analytics</strong>.</p>
                </div>
              </div>
            </div>

            {/* LEVEL 1: THE OBSERVER */}
            <div className="mb-12 border-l-4 border-green-500 pl-6 py-2">
              <div className="flex items-center gap-2 mb-4">
                <div className="bg-green-100 text-green-700 px-3 py-1 rounded-full text-xs font-bold uppercase tracking-wide">Level 1</div>
                <h3 className="text-2xl font-bold text-textPrimary">The Observer: Seeing the Matrix</h3>
              </div>

              <p className="text-textSecondary mb-6">
                <strong>The Problem:</strong> AI is a black box. You send a prompt, you get a response. But what happened in between?
                <br /><strong>The Solution:</strong> Traces. A trace is simply a recording of everything that happened.
              </p>

              {/* Dashboard */}
              <div className="mb-8">
                <h4 className="text-lg font-semibold text-textPrimary mb-3">1. The Dashboard (Your Command Center)</h4>
                <img src="/screenshots/traces_dashboard.png" alt="Traces Dashboard" className="w-full rounded-lg border border-border mb-4 shadow-sm" />
                <div className="bg-surface-elevated border border-border rounded-lg p-4">
                  <p className="text-sm text-textSecondary mb-2"><strong>How to read it:</strong></p>
                  <ul className="text-sm text-textSecondary space-y-1 ml-4 list-disc">
                    <li><strong>Duration:</strong> How long did it take? (If it's red/high, users are waiting).</li>
                    <li><strong>Cost:</strong> How much $$$ was burned?</li>
                    <li><strong>Status:</strong> Did it crash? (Look for red X's).</li>
                  </ul>
                </div>
              </div>

              {/* Conversation View */}
              <div className="mb-8">
                <h4 className="text-lg font-semibold text-textPrimary mb-3">2. Conversation View (The Chat Log)</h4>
                <img src="/screenshots/level1_conversation_view_1770661724251.png" alt="Conversation View" className="w-full rounded-lg border border-border mb-4 shadow-sm" />
                <p className="text-sm text-textSecondary mb-2">
                  This is exactly what you expect: The chat history.
                </p>
                <div className="bg-green-500/10 border border-green-500/20 rounded-lg p-4">
                  <p className="text-sm text-textPrimary">
                    <strong>💡 Beginner Tip:</strong> Always look at the <strong>System Prompt</strong> (the first message). This is the "hidden instruction" telling the AI how to behave. If the AI is acting weird, the issue is usually here.
                  </p>
                </div>
              </div>
            </div>

            {/* LEVEL 2: THE BUILDER */}
            <div className="mb-12 border-l-4 border-blue-500 pl-6 py-2">
              <div className="flex items-center gap-2 mb-4">
                <div className="bg-blue-100 text-blue-700 px-3 py-1 rounded-full text-xs font-bold uppercase tracking-wide">Level 2</div>
                <h3 className="text-2xl font-bold text-textPrimary">The Builder: Fixing & Iterating</h3>
              </div>

              <p className="text-textSecondary mb-6">
                <strong>The Problem:</strong> The AI is "working", but it's slow, or it's choosing the wrong tool.
                <br /><strong>The Solution:</strong> Deep introspection tools (Graph & Flame Graph) and Prompt Management.
              </p>

              {/* Graph View */}
              <div className="mb-8">
                <h4 className="text-lg font-semibold text-textPrimary mb-3">1. Graph View (The Logic Map)</h4>
                <img src="/screenshots/level2_graph_view_1770661734961.png" alt="Graph View" className="w-full rounded-lg border border-border mb-4 shadow-sm" />
                <p className="text-sm text-textSecondary mb-4">
                  Agents aren't linear. They loop, they branch, they retry. The Graph View visualizes this logic.
                </p>
                <div className="grid md:grid-cols-2 gap-4">
                  <div className="bg-surface-elevated border border-border rounded-lg p-3">
                    <div className="font-semibold text-textPrimary text-sm mb-1">Infinite Loops</div>
                    <div className="text-xs text-textSecondary">See a cycle of identical nodes? Your agent is stuck.</div>
                  </div>
                  <div className="bg-surface-elevated border border-border rounded-lg p-3">
                    <div className="font-semibold text-textPrimary text-sm mb-1">Tool Failures</div>
                    <div className="text-xs text-textSecondary">Red nodes show exactly where an API call failed.</div>
                  </div>
                </div>
              </div>

              {/* Flame Graph */}
              <div className="mb-8">
                <h4 className="text-lg font-semibold text-textPrimary mb-3">2. Flame Graph (The Speedometer)</h4>
                <img src="/screenshots/level2_flame_graph_1770661745194.png" alt="Flame Graph" className="w-full rounded-lg border border-border mb-4 shadow-sm" />
                <div className="bg-blue-500/10 border border-blue-500/20 rounded-lg p-4">
                  <p className="text-sm text-textPrimary">
                    <strong>⚡ Speed Tuning:</strong> Look for the <strong>widest bar</strong>. That is your bottleneck.
                    <br /><span className="text-textSecondary text-xs">Common culprit: Waiting 3s for a Database tool when it should take 0.1s.</span>
                  </p>
                </div>
              </div>

              {/* Prompts */}
              <div className="mb-8">
                <h4 className="text-lg font-semibold text-textPrimary mb-3">3. Managing Prompts (The Instructions)</h4>
                <img src="/screenshots/level2_prompts_page_1770661755307.png" alt="Prompts Page" className="w-full rounded-lg border border-border mb-4 shadow-sm" />
                <p className="text-sm text-textSecondary mb-2">
                  Don't hardcode prompts in your code. Use the Registry.
                </p>
                <ul className="list-disc ml-5 text-sm text-textSecondary space-y-1">
                  <li><strong>Version Control:</strong> Rollback to v1 if v2 breaks.</li>
                  <li><strong>Playground:</strong> Test changes immediately without redeploying code.</li>
                </ul>
              </div>
            </div>

            {/* LEVEL 3: THE ARCHITECT */}
            <div className="mb-12 border-l-4 border-purple-500 pl-6 py-2">
              <div className="flex items-center gap-2 mb-4">
                <div className="bg-purple-100 text-purple-700 px-3 py-1 rounded-full text-xs font-bold uppercase tracking-wide">Level 3</div>
                <h3 className="text-2xl font-bold text-textPrimary">The Architect: Scaling & Optimizing</h3>
              </div>

              <p className="text-textSecondary mb-6">
                <strong>The Problem:</strong> You have users. Costs are rising. Quality is inconsistent.
                <br /><strong>The Solution:</strong> Data-driven engineering.
              </p>

              {/* Evaluations */}
              <div className="mb-8">
                <h4 className="text-lg font-semibold text-textPrimary mb-3">1. Evaluations (Unit Tests)</h4>
                <img src="/screenshots/level3_evaluations_page_1770661764516.png" alt="Evaluations Page" className="w-full rounded-lg border border-border mb-4 shadow-sm" />
                <div className="bg-surface-elevated border border-border rounded-lg p-4">
                  <h5 className="font-semibold text-textPrimary text-sm mb-2">The Golden Rule of AI Engineering:</h5>
                  <p className="text-sm text-textSecondary italic mb-2">"You cannot improve what you do not measure."</p>
                  <p className="text-sm text-textSecondary">
                    Run <strong>Evaluations</strong> before every deployment. If your "Accuracy" score drops from 95% to 80%, <strong>do not deploy</strong>.
                  </p>
                </div>
              </div>

              {/* Costs & Analytics */}
              <div className="grid md:grid-cols-2 gap-8 mb-8">
                <div>
                  <h4 className="text-lg font-semibold text-textPrimary mb-3">2. Cost Control</h4>
                  <img src="/screenshots/level3_costs_page_1770661784485.png" alt="Costs Page" className="w-full rounded-lg border border-border mb-3 shadow-sm" />
                  <p className="text-sm text-textSecondary">
                    See exactly which team/project is spending money.
                  </p>
                </div>
                <div>
                  <h4 className="text-lg font-semibold text-textPrimary mb-3">3. Analytics</h4>
                  <img src="/screenshots/level3_analytics_page_1770661775164.png" alt="Analytics Page" className="w-full rounded-lg border border-border mb-3 shadow-sm" />
                  <p className="text-sm text-textSecondary">
                    Monitor error rates and latency trends over time.
                  </p>
                </div>
              </div>
            </div>

            {/* Final Certification */}
            <div className="bg-gradient-to-r from-green-500/10 to-blue-500/10 border border-green-500/20 rounded-lg p-8 text-center mb-8">
              <div className="text-4xl mb-4">🙌</div>
              <h3 className="text-2xl font-bold text-textPrimary mb-2">You Made It!</h3>
              <p className="text-textSecondary mb-6 max-w-2xl mx-auto">
                By mastering these 3 levels, you've moved from "guessing" to "engineering". You are now ready to build production-grade AI agents.
              </p>
              <button
                onClick={() => onSectionClick('quick-start')}
                className="bg-primary text-white px-8 py-3 rounded-lg font-medium hover:bg-primary/90 transition-colors shadow-lg shadow-primary/20"
              >
                Start Building →
              </button>
            </div>

            {/* Resources Footer */}
            <div className="border-t border-border pt-8">
              <h4 className="text-sm font-semibold text-textPrimary mb-4">📚 Additional Resources</h4>
              <div className="grid md:grid-cols-2 gap-4">
                <a href="https://agentreplay.dev/docs" target="_blank" rel="noreferrer" className="flex items-center gap-3 p-3 rounded-lg border border-border hover:bg-surface-hover transition-colors group">
                  <BookOpen className="w-5 h-5 text-textSecondary group-hover:text-primary" />
                  <div>
                    <div className="text-sm font-medium text-textPrimary">Official Documentation</div>
                    <div className="text-xs text-textSecondary">Deep detailed API references</div>
                  </div>
                  <ExternalLink className="w-4 h-4 text-textTertiary ml-auto" />
                </a>
                <a href="https://github.com/agentreplay/agentreplay" target="_blank" rel="noreferrer" className="flex items-center gap-3 p-3 rounded-lg border border-border hover:bg-surface-hover transition-colors group">
                  <Code className="w-5 h-5 text-textSecondary group-hover:text-primary" />
                  <div>
                    <div className="text-sm font-medium text-textPrimary">GitHub Repository</div>
                    <div className="text-xs text-textSecondary">Star us and report issues!</div>
                  </div>
                  <ExternalLink className="w-4 h-4 text-textTertiary ml-auto" />
                </a>
              </div>
            </div>
          </>
        ),
      },
      // OpenClaw docs section hidden — not yet tested for release
      {
        id: 'skill-memory',
        icon: <Brain className="w-6 h-6 text-primary" />,
        title: "Skill Memory",
        content: (
          <>
            <p className="text-textSecondary mb-4">
              The <strong className="text-textPrimary">Skill Memory</strong> page is your central hub for managing learned AI capabilities. It is backed by SochDB and uses hierarchical memory with semantic search (HNSW 384-dim) for discovery.
            </p>

            {/* Overview Tab */}
            <div className="mb-8">
              <h4 className="text-lg font-semibold text-textPrimary mb-3">Overview Tab</h4>
              <div className="bg-surface-elevated border border-border rounded-lg p-4">
                <ul className="list-disc ml-5 text-sm text-textSecondary space-y-2">
                  <li><strong>Stats Cards</strong> — Total skills, invocations, evolutions, and overall success rate at a glance.</li>
                  <li><strong>Skills by Source</strong> — Cards showing how many skills each source/agent has learned.</li>
                  <li><strong>Skills by Category</strong> — Tag cloud of categories (e.g. code-generation, debugging, summarization).</li>
                  <li><strong>Recent Invocations</strong> — A live feed of the most recent skill executions with success/failure, duration, and token counts.</li>
                  <li><strong>Architecture</strong> — Technical details: SochDB MemoryStore schema, HNSW index dimensions, and hierarchical memory levels.</li>
                </ul>
              </div>
            </div>

            {/* Skills Tab */}
            <div className="mb-8">
              <h4 className="text-lg font-semibold text-textPrimary mb-3">Skills Tab</h4>
              <p className="text-sm text-textSecondary mb-3">
                Browse, search, and filter all registered skills. The left panel lists skills; clicking one opens a detail view on the right.
              </p>
              <div className="bg-surface-elevated border border-border rounded-lg p-4 space-y-3">
                <div>
                  <h5 className="font-semibold text-textPrimary text-sm">Searching &amp; Filtering</h5>
                  <p className="text-xs text-textSecondary">Use the search bar to find skills by name. Filter by source (type an agent name) or by status (Draft, Active, Deprecated, Archived).</p>
                </div>
                <div>
                  <h5 className="font-semibold text-textPrimary text-sm">Skill Detail Panel</h5>
                  <p className="text-xs text-textSecondary">Shows description, source, category, version, creation date, tags, shared agents, and the full definition/procedure. Separate sub-tabs display invocation history and evolution log.</p>
                </div>
                <div>
                  <h5 className="font-semibold text-textPrimary text-sm">Creating a Skill</h5>
                  <p className="text-xs text-textSecondary">Click <strong>+ New Skill</strong> in the header. Provide a name, description, source/agent name, category, tags (comma-separated), and the skill definition or prompt template.</p>
                </div>
              </div>
            </div>

            {/* Activity Tab */}
            <div className="mb-8">
              <h4 className="text-lg font-semibold text-textPrimary mb-3">Activity Tab</h4>
              <p className="text-sm text-textSecondary mb-3">
                A timeline of recent skill invocations across all agents. Each entry shows success/failure status, source, skill ID, latency, and token usage.
              </p>
            </div>

            {/* Key Concepts */}
            <div className="mb-8">
              <h4 className="text-lg font-semibold text-textPrimary mb-3">Key Concepts</h4>
              <div className="grid md:grid-cols-2 gap-4">
                <div className="bg-surface-elevated border border-border rounded-lg p-4">
                  <div className="font-semibold text-textPrimary text-sm mb-1">Skill</div>
                  <p className="text-xs text-textSecondary">A reusable AI capability — a procedure, prompt template, or tool-use pattern that has been registered and versioned.</p>
                </div>
                <div className="bg-surface-elevated border border-border rounded-lg p-4">
                  <div className="font-semibold text-textPrimary text-sm mb-1">Invocation</div>
                  <p className="text-xs text-textSecondary">A record of a skill being executed — captures input, output, duration, tokens used, and success/failure.</p>
                </div>
                <div className="bg-surface-elevated border border-border rounded-lg p-4">
                  <div className="font-semibold text-textPrimary text-sm mb-1">Evolution</div>
                  <p className="text-xs text-textSecondary">A version upgrade of a skill — tracks which agent updated it, the reason, and what changed between versions.</p>
                </div>
                <div className="bg-surface-elevated border border-border rounded-lg p-4">
                  <div className="font-semibold text-textPrimary text-sm mb-1">Skill Sharing</div>
                  <p className="text-xs text-textSecondary">Skills can be shared across agents via the shared memory store, enabling cross-agent knowledge transfer.</p>
                </div>
              </div>
            </div>

            {/* API Endpoints */}
            <div>
              <h4 className="text-lg font-semibold text-textPrimary mb-3">API Endpoints</h4>
              <div className="bg-surface-elevated border border-border rounded-lg p-4">
                <pre className="text-xs text-textSecondary whitespace-pre-wrap font-mono">{`GET  /api/v1/skill-memory/stats          — Aggregate stats
GET  /api/v1/skill-memory/skills         — List skills (search, bot, status params)
POST /api/v1/skill-memory/skills         — Create a new skill
GET  /api/v1/skill-memory/skills/:id     — Get skill detail
GET  /api/v1/skill-memory/skills/:id/invocations  — Invocation history
GET  /api/v1/skill-memory/skills/:id/evolutions   — Evolution log`}</pre>
              </div>
            </div>
          </>
        ),
      },
    ];

  const resources = [
    {
      title: "API Reference",
      href: "https://agentreplay.dev/docs/api-reference",
      external: true,
    },
    {
      title: "GitHub",
      href: "https://github.com/agentreplay/agentreplay",
      external: true,
    },
    {
      title: "Architecture",
      href: "https://agentreplay.dev/docs/architecture",
      external: true,
    },
  ];

  return (
    <div className="h-full flex flex-col bg-background">
      {/* Header */}
      <div className="border-b border-border bg-surface-elevated flex-shrink-0">
        <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-6">
          <div className="flex items-center justify-between">
            <div>
              <div className="flex items-center gap-3 mb-2">
                <BookOpen className="w-8 h-8 text-primary" />
                <h1 className="text-3xl font-bold text-textPrimary">Documentation</h1>
              </div>
              <p className="text-textSecondary">
                Everything you need to know about using Agentreplay for LLM observability
              </p>
            </div>
            <VideoHelpButton pageId="docs" />
          </div>
        </div>
      </div>

      {/* Main Content */}
      <div
        ref={scrollContainerRef}
        className="flex-1 overflow-y-auto relative"
      >
        <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-8">
          <div className="flex gap-8">
            {/* Sidebar Navigation */}
            <aside className="w-64 flex-shrink-0 sticky top-0 h-fit">
              <nav className="bg-surface border border-border rounded-xl p-4 shadow-sm">
                <h3 className="text-sm font-semibold text-textPrimary mb-3 px-2">Contents</h3>
                <ul className="space-y-1">
                  {sections.map((section) => (
                    <li key={section.id}>
                      <button
                        onClick={() => onSectionClick(section.id)}
                        className={`w-full flex items-center gap-2 px-3 py-2 rounded-lg text-sm transition-colors text-left ${activeSection === section.id
                          ? 'bg-primary/10 text-primary font-medium'
                          : 'text-textSecondary hover:text-textPrimary hover:bg-surface-hover'
                          }`}
                      >
                        {section.icon}
                        {section.title}
                      </button>
                    </li>
                  ))}
                </ul>

                <div className="mt-6 pt-4 border-t border-border">
                  <h4 className="text-sm font-semibold text-textPrimary mb-3 px-2">Resources</h4>
                  <div className="space-y-2">
                    {resources.map((resource, index) => (
                      <a
                        key={index}
                        href={resource.href}
                        target={resource.external ? "_blank" : undefined}
                        rel={resource.external ? "noopener noreferrer" : undefined}
                        className="flex items-center gap-2 px-3 py-2 text-sm text-textSecondary hover:text-primary hover:bg-surface-hover rounded-lg transition-colors group"
                      >
                        <span className="flex-1">{resource.title}</span>
                        {resource.external && (
                          <ExternalLink className="w-3.5 h-3.5 opacity-50 group-hover:opacity-100" />
                        )}
                      </a>
                    ))}
                  </div>
                </div>

                {/* Current Project Info */}
                {currentProject && (
                  <div className="mt-6 pt-4 border-t border-border">
                    <div className="px-2">
                      <h4 className="text-xs font-semibold text-textTertiary uppercase tracking-wider mb-2">
                        Current Project
                      </h4>
                      <p className="text-sm font-medium text-textPrimary mb-1">{currentProject.name}</p>
                      <code className="text-xs text-textTertiary font-mono">{currentProject.project_id}</code>
                    </div>
                  </div>
                )}
              </nav>
            </aside>

            {/* Documentation Content */}
            <main className="flex-1 min-w-0 space-y-8 pb-12">
              {sections.map((section) => (
                <section
                  key={section.id}
                  id={section.id}
                  className="bg-surface border border-border rounded-xl p-6 shadow-sm hover:shadow-md transition-shadow scroll-mt-4"
                >
                  <div className="flex items-center gap-3 mb-4">
                    {section.icon}
                    <h2 className="text-xl font-semibold text-textPrimary">{section.title}</h2>
                  </div>
                  <div>{section.content}</div>
                </section>
              ))}
            </main>
          </div>
        </div>
      </div>
    </div>
  );
}

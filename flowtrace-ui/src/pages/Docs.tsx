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

import { BookOpen, Code, Database, Settings, Zap, ExternalLink, Copy, CheckCircle2 } from 'lucide-react';
import { useState, useEffect, useRef } from 'react';
import { useProjects } from '../context/project-context';
import { useLocation } from 'react-router-dom';
import { VideoHelpButton } from '../components/VideoHelpButton';

type Section = 'quick-start' | 'sdk' | 'api' | 'configuration';

export default function Docs() {
  const { currentProject } = useProjects();
  const location = useLocation();
  const [copiedIndex, setCopiedIndex] = useState<number | null>(null);
  const [activeSection, setActiveSection] = useState<Section>('quick-start');
  const scrollContainerRef = useRef<HTMLDivElement>(null);

  // Handle hash navigation (e.g., #sdk, #api)
  useEffect(() => {
    const hash = location.hash.replace('#', '') as Section;
    if (hash && ['quick-start', 'sdk', 'api', 'configuration'].includes(hash)) {
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

  const [activeLang, setActiveLang] = useState<'python' | 'ts' | 'go' | 'rust'>('python');

  const quickStartCode = {
    python: `# your_app.py - NO FLOWTRACE IMPORTS NEEDED!
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
console.log(response.choices[0].message.content);`,
    go: `// main.go
package main
import "github.com/sashabaranov/go-openai"

// Use standard OpenAI client
// Flowtrace automatically instruments correctly configured clients`,
    rust: `// main.rs
// Rust support coming soon via OpenTelemetry auto-instrumentation`
  };

  const installCmd = {
    python: 'pip install flowtrace-client && flowtrace-install',
    ts: 'npm install flowtrace-client',
    go: 'go get github.com/sushanthpy/flowtrace-go',
    rust: 'cargo add flowtrace-client'
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
              Get started with Flowtrace in minutes. <strong className="text-textPrimary">Zero code changes required!</strong>
            </p>

            {/* Language Selector */}
            <div className="flex gap-2 mb-6 border-b border-border">
              {(['python', 'ts', 'go', 'rust'] as const).map((lang) => (
                <button
                  key={lang}
                  onClick={() => setActiveLang(lang)}
                  className={`px-4 py-2 text-sm font-medium border-b-2 transition-colors \${
                    activeLang === lang
                      ? 'border-primary text-primary'
                      : 'border-transparent text-textTertiary hover:text-textPrimary'
                  }`}
                >
                  {lang === 'ts' ? 'TypeScript' : lang.charAt(0).toUpperCase() + lang.slice(1)}
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
                    <code>{`export FLOWTRACE_ENABLED=true
export FLOWTRACE_OTLP_ENDPOINT=localhost:4317
export FLOWTRACE_PROJECT_ID="${currentProject?.project_id || 'your-project-id'}"`}</code>
                  </pre>
                  <button
                    onClick={() => copyToClipboard(`export FLOWTRACE_ENABLED=true\nexport FLOWTRACE_OTLP_ENDPOINT=localhost:4317\nexport FLOWTRACE_PROJECT_ID="${currentProject?.project_id || 'your-project-id'}"`, 1)}
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
                  Run with: <code className="px-1 py-0.5 bg-surface-elevated rounded">{activeLang === 'python' ? 'python your_app.py' : activeLang === 'ts' ? 'ts-node app.ts' : 'go run main.go'}</code> — traces appear automatically!
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
              Flowtrace provides <strong className="text-textPrimary">true zero-code observability</strong>. Auto-instruments OpenAI, Anthropic, LangChain, LangGraph, LlamaIndex, CrewAI, and AutoGen.
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
                  <code className="language-python">{`# NO FLOWTRACE IMPORTS NEEDED!
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
                  <code className="text-sm text-primary">http://localhost:9600/api/v1</code>
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
              Configure Flowtrace using environment variables. No code changes needed!
            </p>
            <div className="space-y-4">
              <div>
                <div className="flex items-center gap-2 mb-2">
                  <h4 className="text-sm font-semibold text-textPrimary">Required</h4>
                  <span className="px-2 py-0.5 bg-primary/20 text-primary rounded text-xs font-medium">Must set</span>
                </div>
                <div className="bg-surface-elevated border border-border rounded-lg p-4 space-y-3">
                  <div>
                    <code className="text-xs font-mono text-primary">FLOWTRACE_ENABLED</code>
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
                    <code className="text-xs font-mono text-primary">FLOWTRACE_OTLP_ENDPOINT</code>
                    <p className="text-sm text-textSecondary mt-1">
                      OTLP endpoint for sending traces (default: <code className="px-1 py-0.5 bg-background rounded">localhost:4317</code>)
                    </p>
                  </div>
                  <div>
                    <code className="text-xs font-mono text-primary">FLOWTRACE_PROJECT_ID</code>
                    <p className="text-sm text-textSecondary mt-1">
                      The project ID to associate traces with (default: <code className="px-1 py-0.5 bg-background rounded">0</code>)
                    </p>
                  </div>
                  <div>
                    <code className="text-xs font-mono text-primary">FLOWTRACE_SERVICE_NAME</code>
                    <p className="text-sm text-textSecondary mt-1">
                      Your application name in traces (default: <code className="px-1 py-0.5 bg-background rounded">app</code>)
                    </p>
                  </div>
                  <div>
                    <code className="text-xs font-mono text-primary">FLOWTRACE_LOG_LEVEL</code>
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
export FLOWTRACE_ENABLED=true

# Point to your Flowtrace server
export FLOWTRACE_OTLP_ENDPOINT=localhost:4317

# Associate traces with your project
export FLOWTRACE_PROJECT_ID=${currentProject?.project_id || 'your-project-id'}

# Run your existing code - no changes needed!
python your_app.py`}</code>
                  </pre>
                </div>
              </div>
            </div>
          </>
        ),
      },
    ];

  const resources = [
    {
      title: "API Reference",
      href: "https://github.com/sushanthpy/flowtrace/blob/main/docs/API_REFERENCE.md",
      external: true,
    },
    {
      title: "GitHub",
      href: "https://github.com/sushanthpy/flowtrace",
      external: true,
    },
    {
      title: "Architecture",
      href: "https://github.com/sushanthpy/flowtrace/blob/main/docs/ARCHITECTURE.md",
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
                Everything you need to know about using Flowtrace for LLM observability
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

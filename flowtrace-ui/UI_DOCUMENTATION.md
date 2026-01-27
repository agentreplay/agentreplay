# Flowtrace UI - Best-in-Class Experience

This UI has been redesigned to follow the best practices from top LLM observability platforms like Helicone, Langfuse, Lunary, Phoenix, TruLens, Braintrust, and LangSmith.

## 🎯 Design Philosophy

- **3-Minute Setup**: Get value immediately with 1-line integration
- **Simplicity First**: Clean, uncluttered interfaces
- **Discoverability**: Intuitive navigation and workflows
- **Developer-Focused**: Built for debugging and iteration

## 📄 Pages Overview

### 1. Get Started (`/get-started`)
**Inspired by**: Helicone & Langfuse

**Purpose**: Onboarding and project dashboard

**Features**:
- 1-line setup instructions with copy-paste buttons
- Live-updating project cards showing Cost, Latency, Requests, Errors
- "Aha moment" - instant feedback when code runs
- Links to documentation and resources

**API Endpoints Used**:
- `GET /api/v1/projects` - List all projects with stats

---

### 2. Analytics Dashboard (`/analytics`)
**Inspired by**: Langfuse & Lunary

**Purpose**: Mission control - high-level health overview

**Features**:
- **The Big 4 Metrics**: Total Cost, Avg Latency, Total Requests, Error Rate
- Time-series charts for each metric
- **"What to Fix" Widgets**:
  - Top 5 Most Expensive Users
  - Top 5 Slowest Endpoints
  - Top 5 Erroring Models
- Click any widget to filter traces

**API Endpoints Used**:
- `GET /api/v1/metrics/timeseries?start_ts={}&end_ts={}&interval_seconds={}` - Time-series data
- `GET /api/v1/analytics/top-users?start_ts={}&end_ts={}` - Top users by cost
- `GET /api/v1/analytics/top-slow-endpoints?start_ts={}&end_ts={}` - Slowest endpoints
- `GET /api/v1/analytics/top-errors?start_ts={}&end_ts={}` - Models with most errors

---

### 3. Traces (`/traces`)
**Inspired by**: LangSmith & Langfuse

**Purpose**: Deep debugging - 80% of your time is spent here

**Features**:
- Dense, filterable table of all traces
- **Powerful Filter Bar** - natural language queries:
  - `cost > $0.10`
  - `user_id = "user_123"`
  - `user_feedback = 👎`
  - `latency > 1000ms`
- Sortable columns (timestamp, cost, latency)
- Click trace → opens **Trace Waterfall Modal**
- Real-time WebSocket updates with "Live" indicator

**Trace Waterfall Modal**:
- Nested, collapsible view of all spans
- Visual timeline showing duration and timing
- Click any span to see:
  - Full input/output (with copy button)
  - Metadata
  - Cost, latency, tokens
- Perfect for debugging RAG pipelines, agent chains, etc.

**API Endpoints Used**:
- `WebSocket /ws/traces` - Real-time trace streaming
- `GET /api/v1/traces/{trace_id}` - Full trace details with spans

---

### 4. Evals & Testing (`/evals`)
**Inspired by**: Braintrust & TruLens

**Purpose**: Close the loop from production to development

**Features**:
- **Two Tabs**: Datasets and Eval Runs

**The "Golden Workflow"**:
1. Find a failed trace in `/traces`
2. Click "Add to Test Dataset" (future feature)
3. Create a new experiment testing prompt_v1 vs prompt_v2
4. Run eval and get automatic scores:
   - **Groundedness** - Is the answer grounded in the context?
   - **Context Relevance** - Is the retrieved context relevant?
   - **Answer Relevance** - Does the answer address the question?
5. See side-by-side comparison and proof your fix works

**API Endpoints Used**:
- `GET /api/v1/datasets` - List datasets
- `POST /api/v1/datasets` - Create dataset
- `GET /api/v1/evals/runs` - List eval runs
- `POST /api/v1/evals/runs` - Create and run eval
- `GET /api/v1/evals/runs/{run_id}` - Get eval results

---

### 5. Sessions (`/sessions`)
**Inspired by**: Lunary & Langfuse

**Purpose**: Chatbot-specific view for conversational agents

**Features**:
- Left sidebar: List of conversations grouped by `session_id` or `user_id`
- Right panel: **Chat Replay** - looks like ChatGPT
- Click any AI message → see full trace waterfall for that turn
- See where conversations went wrong
- User feedback indicators (👍/👎)

**API Endpoints Used**:
- `GET /api/v1/sessions` - List all sessions with metadata
- `GET /api/v1/sessions/{session_id}/messages` - Get conversation history

---

## 🔌 API Requirements

### New Endpoints Needed

The UI expects these endpoints to exist. If they don't, you'll need to create them:

#### Projects
```
GET /api/v1/projects
Response: {
  "projects": [
    {
      "id": "proj_123",
      "name": "Production Chatbot",
      "description": "Main customer-facing bot",
      "created_at": "2025-01-01T00:00:00Z",
      "stats": {
        "total_requests": 1500,
        "total_cost": 2.45,
        "avg_latency_ms": 850,
        "error_rate": 0.02
      }
    }
  ]
}
```

#### Analytics - Top Items
```
GET /api/v1/analytics/top-users?start_ts={}&end_ts={}
Response: {
  "users": [
    { "id": "user_123", "name": "user_123", "value": 5.23 }
  ]
}

GET /api/v1/analytics/top-slow-endpoints?start_ts={}&end_ts={}
Response: {
  "endpoints": [
    { "id": "rag_agent", "name": "RAG Agent", "value": 2500 }
  ]
}

GET /api/v1/analytics/top-errors?start_ts={}&end_ts={}
Response: {
  "models": [
    { "id": "gpt-4", "name": "gpt-4 (429)", "value": 45 }
  ]
}
```

#### Traces with Full Details
```
GET /api/v1/traces/{trace_id}
Response: {
  "trace_id": "trace_123",
  "root_span": {
    "span_id": "span_1",
    "name": "AgentExecutor",
    "type": "agent",
    "start_time_us": 1234567890000000,
    "duration_ms": 2500,
    "cost": 0.012,
    "tokens": 1500,
    "status": "success",
    "input": { "query": "What is...?" },
    "output": { "answer": "..." },
    "metadata": {},
    "children": [
      {
        "span_id": "span_2",
        "name": "RetrieveDocuments",
        "type": "retrieval",
        "start_time_us": 1234567890050000,
        "duration_ms": 120,
        "cost": 0,
        "status": "success",
        "input": { "query": "..." },
        "output": { "documents": [...] },
        "children": []
      }
    ]
  },
  "total_duration_ms": 2500,
  "total_cost": 0.012,
  "total_tokens": 1500,
  "status": "success"
}
```

#### Datasets & Evals
```
GET /api/v1/datasets
Response: {
  "datasets": [
    {
      "id": "dataset_123",
      "name": "Production Failures",
      "description": "Failed cases from prod",
      "size": 25,
      "created_at": "2025-01-01T00:00:00Z",
      "source": "production"
    }
  ]
}

GET /api/v1/evals/runs
Response: {
  "runs": [
    {
      "id": "run_123",
      "name": "Prompt V2 Test",
      "dataset_id": "dataset_123",
      "dataset_name": "Production Failures",
      "status": "completed",
      "created_at": "2025-01-01T00:00:00Z",
      "completed_at": "2025-01-01T00:05:00Z",
      "metrics": {
        "total_cases": 25,
        "passed": 23,
        "failed": 2,
        "avg_latency_ms": 850,
        "avg_cost": 0.008,
        "groundedness": 0.95,
        "context_relevance": 0.88,
        "answer_relevance": 0.92
      }
    }
  ]
}
```

#### Sessions
```
GET /api/v1/sessions
Response: {
  "sessions": [
    {
      "id": "session_123",
      "user_id": "user_123",
      "started_at": 1234567890000000,
      "last_message_at": 1234568000000000,
      "message_count": 12,
      "total_cost": 0.15,
      "avg_latency_ms": 950,
      "status": "ended"
    }
  ]
}

GET /api/v1/sessions/{session_id}/messages
Response: {
  "messages": [
    {
      "id": "msg_1",
      "role": "user",
      "content": "Hello!",
      "timestamp_us": 1234567890000000
    },
    {
      "id": "msg_2",
      "role": "assistant",
      "content": "Hi! How can I help?",
      "timestamp_us": 1234567891000000,
      "trace_id": "trace_123",
      "cost": 0.002,
      "latency_ms": 850,
      "feedback": "positive"
    }
  ]
}
```

---

## 🎨 Design System

### Colors
The UI uses a consistent color palette:
- **Primary**: Main brand color (links, buttons, highlights)
- **Success/Green**: Cost metrics, passed tests
- **Warning/Yellow**: Latency metrics
- **Info/Blue**: Request counts
- **Error/Red**: Errors, failures

### Components
All major components are built with:
- **Framer Motion** for smooth animations
- **Recharts** for data visualization
- **Tailwind CSS** for styling
- **Lucide React** for icons

---

## 🚀 Getting Started

1. **Install dependencies** (if not already done):
   ```bash
   cd ui
   npm install
   ```

2. **Start the dev server**:
   ```bash
   npm run dev
   ```

3. **Configure API endpoints**:
   - WebSocket: Set `NEXT_PUBLIC_WS_URL` in `.env.local`
   - HTTP API: Uses Next.js API routes or configure proxy in `next.config.js`

4. **Implement missing API endpoints** on the backend

---

## 🔄 Migration from Old UI

### Old Dashboard → New Structure
- Old `/dashboard` → Now split into:
  - `/get-started` - First-time setup
  - `/analytics` - High-level metrics
  - `/traces` - Detailed debugging

### Preserved Features
- ✅ Real-time WebSocket trace streaming
- ✅ Time-series metrics with configurable ranges
- ✅ Semantic search (can be added to filter bar)
- ✅ Agent filtering
- ✅ Cost/latency tracking

### Enhanced Features
- ✨ Natural language filter queries
- ✨ Trace waterfall visualization
- ✨ RAG eval scores
- ✨ Session/conversation view
- ✨ Dataset management for testing

---

## 📝 Notes

- All API calls maintain backward compatibility with existing endpoints
- New endpoints are additions, not replacements
- The UI gracefully handles missing data (shows empty states)
- Mobile-responsive design throughout
- Accessibility considerations (keyboard navigation, ARIA labels)

---

## 🛠️ Customization

### Adding New Metrics
Edit `/app/analytics/page.tsx` and add to the "Big 4" or create new widgets.

### Custom Filters
Update the `parseFilterQuery` function in `/app/traces/page.tsx` to support new filter types.

### Eval Metrics
Add custom eval functions in your backend and display them in `/app/evals/page.tsx`.

---

## 🐛 Troubleshooting

### "No traces found"
- Check WebSocket connection (look for "Live" indicator)
- Verify `NEXT_PUBLIC_WS_URL` is set correctly
- Ensure backend is running on correct port

### Charts not loading
- Verify `/api/v1/metrics/timeseries` endpoint exists
- Check browser console for API errors
- Ensure time range parameters are valid

### Projects not showing
- Implement `GET /api/v1/projects` endpoint
- Check API key authentication if required

---

Enjoy your new best-in-class UI! 🎉

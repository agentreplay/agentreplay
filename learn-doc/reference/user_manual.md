# Agentreplay: The Exhaustive User Manual

**Version**: 1.0
**Target Audience**: Developers, QA, and Data Scientists.

This manual provides a pixel-level reference for the entire Agentreplay platform. It covers every button, tab, and panel available in the application, from Observability to Tool Testing and System Configuration.

---

## Part 1: The Traces Dashboard
**URL**: `/projects/{id}/traces`

The dashboard is your entry point. It aggregates all agent activities into a searchable, filterable list.

### 1.1 The Header & Global Controls
![Header Actions](traces_header_actions_1770873990392.png)
*   **Search Bar**: Located at the top.
    *   *Usage*: Type keywords like "Write", "Error", or a specific Trace ID.
    *   *Behavior*: The list filters in real-time as you type (see `traces_search_bar` below).
*   **Live Toggle**:
    *   *State*: When active (green), the list auto-refreshes as new traces arrive.
    *   *Action*: Click to pause updates for stable analysis.
*   **Refresh Button**:
    *   *Action*: Manually fetches the latest data from the backend.
*   **Delete All**:
    *   *Action*: The trash icon permanently clears the *entire* trace history for the project. **Use with caution.**

### 1.2 The Trace List
![Search Results](traces_search_bar_1770873986011.png)
*   **Trace / Time Column**:
    *   *Top Line*: The unique 16-character Trace ID.
    *   *Bottom Line*: Relative time (e.g., "5 minutes ago").
*   **Model Column**: The specific LLM used (e.g., `gpt-4`). Hover to see the full version string.
*   **Input/Output Columns**: Truncated previews. Designed for scanning, not reading.
*   **Duration Column**:
    *   *Color Coding*: Green (<1s), Yellow (<5s), Red (>5s).
    *   *Sorting*: Click the header to sort by latency (critical for performance debugging).
*   **Cost Column**: Total USD cost of the run, calculated from token usage.
*   **Status Column**:
    *   *Green Check*: Successful execution.
    *   *Red X*: Uncaught exception or error.

### 1.3 Pagination
![Pagination](traces_pagination_1770873990790.png)
*   **Location**: Bottom of the screen.
*   **Controls**: Standard "First", "Previous", "Page Numbers", "Next", "Last" navigation.
*   **Count**: Shows "1-XYZ of Total" to give context on the dataset size.

---

## Part 2: The Trace Detail View
**URL**: `/projects/{id}/traces/{trace_id}`

Clicking any row in the dashboard opens this view. It is divided into three zones: **Header**, **Left Panel**, and **Right Panel**.

### 2.1 The Action Bar (Header)
![Action Bar](trace_detail_actions_1770874013336.png)
*   **Commit Trace**:
    *   *Icon*: Git branch symbol.
    *   *Function*: Saves a snapshot of the trace, useful for versioning "Golden Traces" for testing.
*   **Add to Dataset**:
    *   *Icon*: Database with plus sign.
    *   *Function*: Opens the Dataset Modal (see below).
*   **Evaluate**:
    *   *Icon*: Beaker/Test tube.
    *   *Function*: Opens the Evaluation Modal (see below).
*   **Delete**:
    *   *Icon*: Trash can.
    *   *Function*: Deletes this specific trace.

### 2.2 Modals & Workflows

#### Add to Dataset Modal
![Dataset Modal](trace_detail_dataset_modal_1770874018524.png)
*   **Usage**: Select a target dataset (e.g., "Golden Set v1") to add this trace as a test case.
*   **Inputs**: Allows mapping specific trace inputs/outputs to dataset fields.

#### Evaluate Trace Modal
![Evaluate Modal](trace_detail_evaluate_modal_1770874025139.png)
*   **Usage**: Run automated graders against this trace.
*   **Scorers**: Select from "RAG Quality", "Hallucination", "Toxicity", etc.

### 2.3 The Left Panel (Visualization)
This panel visualizes the trace structure.

*   **List View** (Default): Tree structure of spans.
*   **Session Replay**:
    *   *Screenshot*: ![Replay](trace_detail_replay_1770873758253.png)
    *   *Feature*: Chat-bubble interface for non-technical stakeholders.
*   **Dependency Graph**:
    *   *Screenshot*: ![Graph](trace_detail_graph_1770873766850.png)
    *   *Feature*: Node-link diagram showing execution flow.
*   **Flame Graph**:
    *   *Screenshot*: ![Flame Graph](trace_detail_flamegraph_1770873775410.png)
    *   *Feature*: Time-based visualization for identifying latency.
*   **AI Analysis**:
    *   *Screenshot*: ![AI Analysis](trace_detail_ai_analysis_unconfigured_1770873784695.png)
    *   *Feature*: Auto-generated summary of the trace (requires API key).

### 2.4 The Right Panel (Span Inspector)
Details update based on the *selected span* in the Left Panel.

#### Conversation Tab
![Conversation](trace_detail_conversation_1770873750425.png)
*   **Content**: Full text of User Prompt and Assistant Response.
*   **Action**: **"Try in Playground"** button.
    *   *Result*: Opens the Playground with context pre-loaded (see below).

#### Attributes Tab
![Attributes](trace_detail_attributes_scroll_start_1770873808514.png)
*   **Data**: Key-value pairs from OpenTelemetry (e.g., `gen_ai.model`, `tool.name`).
*   **Usage**: Debugging low-level instrumentation data.

#### Raw Tab
![Raw JSON](trace_detail_raw_json_1770873817193.png)
*   **Data**: The complete JSON object stored in SQLite.

---

## Part 3: The Playground
**Access**: Click "Try in Playground" from the Trace Detail.

![Playground](trace_detail_playground_1770874031900.png)
*   **Context**: The System Prompt and User Message from the trace are auto-filled.
*   **Experimentation**: Modify the prompt or model parameters (Temperature, Top P) and click "Run" to test fixes immediately.

---

## Part 4: MCP Tester
**URL**: `/projects/{id}/mcp-tester`

This experimentation lab allows you to test Model Context Protocol tools in isolation.

### 4.1 Connection & Transport
![Transport Selection](mcp_transport_dropdown_1770874089342.png)
*   **Transport Dropdown**:
    *   *Options*: HTTP (SSE) or WebSocket.
    *   *Usage*: Match this to your MCP server's implementation.
*   **Connect Button**: Initiates the handshake.
*   **Status Indicator**: Shows exact connection state (`Connected`, `Connecting`, `Disconnected`).

### 4.2 Experimentation Tools
![Tools List](persona_3_experimentation_1770873601885.png)
*   **Method Catalog**: Browsable list of all capabilities exposed by the server (e.g., `tools/list`, `resources/read`).
*   **Request Composer**: Auto-generates JSON payloads based on the selected method's schema.
*   **Response Inspector**: Shows the raw JSON-RPC response.

### 4.3 History Tab
![History Log](mcp_history_tab_1770874099830.png)
*   **Usage**: Keeps a session log of every request sent.
*   **Feature**: Click any past request to reload it into the Composer for regression testing.

---

## Part 5: Settings
**URL**: `/projects/{id}/settings`

Configure the behavior of your Agentreplay instance.

### 5.1 LLM Providers
![LLM Config](settings_llm_provider_1770874116051.png)
*   **Providers**: Configure API keys for OpenAI, Anthropic, or Ollama.
*   **Usage**: These keys power the "AI Analysis" and "Playground" features.

### 5.2 Backup & Restore
![Backup Section](settings_backup_1770874123918.png)
*   **Create Backup**: Generates a `.sqlite` snapshot of your entire project.
*   **Restore**: Upload a previous snapshot to roll back state.
*   **Usage**: Critical for saving state before running destructive tests.

### 5.3 Theme & UI
![Theme Toggle](settings_theme_toggle_1770874137761.png)
*   **Theme**: Toggle between Light and Dark modes.
*   **Visual density**: Adjust how much information is packed into the trace list rows.

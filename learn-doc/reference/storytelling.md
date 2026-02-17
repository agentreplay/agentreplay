# The Complete Tour: Inside Agentreplay Traces

**So, you've just run your agent.**
You watched the terminal spin, you saw the "Success" message, but what *actually* happened?

This guide walks you through every single pixel of the **Traces** experience. We'll start at the front door and go all the way down to the raw JSON.

---

## 1. The Landing: Traces List
**URL**: `/projects/{id}/traces`

This is your inbox. It's designed to answer one question: *"Is everything okay?"*

![Traces List](traces_list_view_1770872130082.png)

*   **The Status Column**: Your first check. Green checks mean verified success. Red crosses mean exceptions.
*   **Duration**: This isn't just a number; it's a heatmap.
    *   **Green (<1s)**: Instant.
    *   **Yellow (<5s)**: Normal thinking time.
    *   **Red (>5s)**: Potential bottlenecks.
*   **Cost**: Real-time USD tracking based on the model used (e.g., GPT-4o vs. Haiku).
*   **Input/Output**: We deliberately truncate this. You don't need the full novel here; you just need to identify *which* run this was.

---

## 2. The Deep Dive: Trace Detail View
**Action**: Click on any row.
**Result**: You enter the **Trace Detail** view. This is the heart of Agentreplay.

### A. The Header (Global Context)
At the very top, you have your "Flight Recorder" data:
*   **Trace ID**: The unique fingerprint. Click the **Copy** icon next to it to share with your team.
*   **Global Actions**:
    *   **Export**: Downloads `trace.json` for offline analysis.
    *   **Replay**: (If configured) Reruns the exact inputs.
    *   **Delete**: Nukes the trace from your local database.

---

### B. The Left Panel: Views of Reality
This panel lets you see the *same* execution through five different lenses.

#### 🔭 1. List View (The Default)
This is the chronological truth. It shows a tree of every span.
*   **Root**: The user's initial command.
*   **Branches**: The agent's "Thoughts" (Reasoning traces).
*   **Leaves**: The external actions (Tools like `Write File`, `Run Command`).
*   **Why use it?** To verify the *order* of operations. Did it check the file *before* trying to write to it?

#### 🎬 2. Session Replay
![Session Replay](trace_detail_replay_1770873758253.png)
*   **What it is**: A "Movie Mode" for your trace. It strips away the code and shows a chat interface.
*   **Why use it?** To see what the *User* experienced. This is great for showing stakeholders demo results without overwhelming them with JSON.

#### 🕸️ 3. Dependency Graph
![Dependency Graph](trace_detail_graph_1770873766850.png)
*   **What it is**: A flowchart. Nodes are actions; arrows are dependencies.
*   **Why use it?** Debugging async race conditions. If two tools ran in parallel, they appear side-by-side here.

#### 🔥 4. Flame Graph
![Flame Graph](trace_detail_flamegraph_1770873775410.png)
*   **What it is**: A stack trace for time. Wider bars = longer time.
*   **Why use it?** Optimization. That skinny bar at the bottom? That's your LLM token generation. That massive wide bar on top? That's your database tool timing out. Fix the tool, not the prompt.

#### 🧠 5. AI Analysis
![AI Analysis](trace_detail_ai_analysis_unconfigured_1770873784695.png)
*   **What it is**: We feed the trace metadata *back* into an LLM.
*   **Why use it?** To get a plain-English explanation: "The agent failed because it tried to read a file that didn't exist." It's like having a senior engineer review your logs.

---

### C. The Right Panel: The Inspector
Click on *any* span in the Left Panel (e.g., a "Tool Call"), and this Right Panel updates instantly.

#### 💬 1. Conversation Tab
![Conversation View](trace_detail_conversation_1770873750425.png)
*   **The Content**: Shows the exact "User" prompt and "Assistant" response for that specific step.
*   **Code Highlighting**: Detects code blocks (Python, JSON, Markdown) and formats them for readability.

#### ℹ️ 2. Overview Tab
![Overview Stats](trace_detail_overview_1770873798823.png)
*   **The Metadata**: Start time, End time, exact duration to the microsecond.
*   **Status**: If a specific tool failed, the error message appears here.

#### 🏷️ 3. Attributes Tab
![Attributes Data](trace_detail_attributes_scroll_start_1770873808514.png)
*   **The Structured Data**: This is your OpenTelemetry playground.
*   **Key Fields**:
    *   `gen_ai.system`: The provider (e.g., `openai`).
    *   `gen_ai.request.model`: The specific model (e.g., `gpt-4-0613`).
    *   `tool.name`: The function called (e.g., `fs_write`).

#### 📝 4. Raw Tab
![Raw JSON](trace_detail_raw_json_1770873817193.png)
*   **The Source of Truth**: The unadulterated JSON object stored in the database.
*   **Why use it?** When you suspect the UI is hiding something. This is the raw data straight from the wire.

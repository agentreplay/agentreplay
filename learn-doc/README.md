# Agent Development with Agentreplay: The Complete Workflow

**Welcome to your observability cockpit.**

This guide outlines the standard workflow for an Agent Developer using Agentreplay. It is designed to help you move from "it runs" to "it works perfectly".

## The Core Loop
Building reliable agents requires a cycle of **Observation**, **Debugging**, and **Evaluation**.

```mermaid
graph LR
    A[Run Agent] --> B[Observe Trace]
    B --> C{Success?}
    C -->|No| D[Debug Detail]
    D --> E[Test Fix (MCP)]
    E --> A
    C -->|Yes| F[Evaluate Quality]
    F --> G[Add to Dataset]
```

---

## Phase 1: Observation (The "Pulse Check")
**Goal**: Verify that your agent is running, staying within budget, and responding quickly.

1.  **Check the Dashboard**: Go to the **Traces List**.
    *   *Look for*: Red status icons (errors) or high "Duration" numbers (latency).
    *   *Action*: Sort by **Duration** to find the slowest traces.
2.  **Verify Costs**: Glance at the "Cost" column.
    *   *Insight*: If costs are spiking, your agent might be getting stuck in loops or using expensive models unnecessarily.

> 📘 **Deep Dive**: See [The Traces Dashboard](reference/user_manual.md#part-1-the-traces-dashboard) in the User Manual.

---

## Phase 2: Debugging (The "Deep Dive")
**Goal**: Understand *why* an agent failed or hallucinated.

1.  **Inspect the Trace**: Click into a trace.
2.  **Trace the Logic**: Use the **Dependency Graph** or **List View** (Left Panel) to see the order of operations.
    *   *Question*: Did the agent call `fs.readFile` before `fs.writeFile`?
3.  **Check the Prompt**: Open the **Conversation Tab** (Right Panel).
    *   *Question*: Did the system prompt actually contain the instructions you thought it did?
4.  **Analyze Performance**: Switch to **Flame Graph**.
    *   *Insight*: Identify if the bottleneck is the LLM (thin bars) or the Tool execution (wide bars).

> 📘 **Deep Dive**: See [The Trace Detail View](reference/user_manual.md#part-2-the-trace-detail-view) in the User Manual.

---

## Phase 3: Experimentation (The "Lab")
**Goal**: Fix the bug without running the entire agent again.

1.  **Isolate the Tool**: If a tool call failed, go to the **MCP Tester**.
2.  **Reproduce**: Use the **History** tab to reload the exact payload that failed.
3.  **Tweak**: Modify the JSON in the **Request Composer** and hit Send.
4.  **Verify**: Once it works here, it will work in your agent.

> 📘 **Deep Dive**: See [MCP Tester](reference/user_manual.md#part-4-mcp-tester) in the User Manual.

---

## Phase 4: Evaluation (The "Quality Gate")
**Goal**: Ensure your agent is strictly better than before.

1.  **Capture Gold**: When you see a perfect trace, click **"Add to Dataset"**.
    *   *Why*: This becomes your ground truth for regression testing.
2.  **Run Evals**: Click **"Evaluate"** on a trace.
    *   *Scorers*: Run "Hallucination" or "RAG Quality" checks.
3.  **Review**: Check the scores in the trace header.

> 📘 **Deep Dive**: See [Modals & Workflows](reference/user_manual.md#22-modals--workflows) in the User Manual.

---

## Where to go next?
*   **[User Manual](reference/user_manual.md)**: The exhaustive, button-by-button reference guide.
*   **[Developer Storyboard](reference/storytelling.md)**: A "Day in the Life" narrative walkthrough.

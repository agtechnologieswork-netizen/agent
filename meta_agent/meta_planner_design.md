# Claude‑Style Planner — Design Doc (meta_agent / meta_draft)

**Goal**: A single-module, compile‑time integrated planner that:
- Accepts plain text input.
- (Future) Accepts links, image refs, and file attachments.
- Produces an ordered list of **text tasks** with attachment metadata.
- Drives execution via `PipelineCmd` → receives `PipelineEvent`.
- Pauses for **user clarification** when needed, then resumes.
- **Compacts context** between steps to stay within token/memory limits.
- Runs a simple **sequential loop** until all tasks are done.

---

## 1) Public Interfaces & Data Types

> Integrate into `meta_draft/src/actors.rs` (or adjacent module). Enums below extend your existing pipeline types.

```rust
/// Commands emitted by the planner to the pipeline/executor.
pub enum PipelineCmd {
    ExecuteTask { node_id: u64, kind: NodeKind, parameters: String },
    RequestClarification { node_id: u64, question: String },
    // (Optional) Cancel/Abort, SaveCheckpoint, etc.
}

/// Events received by the planner from the executor/UI.
pub enum PipelineEvent {
    TaskCompleted { node_id: u64, result: String },
    TaskFailed { node_id: u64, error: String },
    NeedsClarification { node_id: u64, question: String },
    ClarificationProvided { node_id: u64, answer: String },
    // (Optional) CheckpointSaved, ToolOutput, etc.
}

/// Classification for routing & tooling (v1 minimal set)
#[derive(Debug, Clone, Copy)]
pub enum NodeKind {
    CodeImplementation,
    UnitTest,
    Retrieval,       // fetch URLs, parse
    Analysis,        // analyze inputs, summarize
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatus { Planned, Running, Completed, NeedsClarification }

// (Future)
#[derive(Debug, Clone)]
pub enum AttachmentKind {
    Link(String),        // URL
    ImageRef(String),    // URL or opaque id
    FileRef(String),     // path or opaque id
}

// (Future)
#[derive(Debug, Clone)]
pub struct Attachment {
    pub kind: AttachmentKind,
    pub label: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Task {
    pub id: u64,
    pub description: String,       // plain text step
    pub kind: NodeKind,
    pub status: TaskStatus,
    pub attachments: Vec<Attachment>,
}

#[derive(Debug, Default)]
pub struct PlannerState {
    pub tasks: Vec<Task>,
    pub cursor: usize,
    pub waiting_for_clarification: bool,
    pub next_id: u64,
    pub context_summary: String, // compacted rolling summary
}
```

---

## 2) Control Flow

### Activity (PlantUML)
```plantuml
@startuml
start
:Parse input -> Vec<Task>;
repeat
  :Pick next Planned task;
  :Emit PipelineCmd.ExecuteTask(node_id, kind, params);
  -> Wait for PipelineEvent;
  if (TaskCompleted?) then (yes)
    :Mark Completed;
    :Compact context;
    :Advance;
  elseif (NeedsClarification?) then (yes)
    :Emit RequestClarification(question);
    -> Wait ClarificationProvided;
    :Merge answer into task/context;
    :Retry ExecuteTask;
  elseif (TaskFailed?) then (yes)
    :Record error; Advance/Abort policy;
  endif
repeat while (Tasks left?)
:Emit final summary/result;
stop
@enduml
```

### Minimal Pseudocode
```rust
impl PlannerState {
    pub fn plan_tasks(&mut self, input: &str, atts: &[Attachment]) {
        // 1) parse input → steps
        // 2) map steps → NodeKind
        // 3) v1: extract URLs only (files/images deferred)
        // 4) push Task { id: self.alloc_id(), ... }
    }

    pub fn on_tick_or_ready(&mut self, ctx: &mut Ctx) {
        if self.waiting_for_clarification { return; }
        if let Some(t) = self.tasks.get_mut(self.cursor) {
            if t.status == TaskStatus::Planned {
                t.status = TaskStatus::Running;
                ctx.emit(PipelineCmd::ExecuteTask {
                    node_id: t.id, kind: t.kind, parameters: t.description.clone()
                });
            }
        }
    }

    pub fn on_event(&mut self, evt: PipelineEvent, ctx: &mut Ctx) {
        match evt {
          PipelineEvent::TaskCompleted { node_id, result } => {
            if let Some(t) = self.by_id_mut(node_id) {
               t.status = TaskStatus::Completed;
               self.compact_with(&result);
               self.cursor += 1;
               self.on_tick_or_ready(ctx);
            }
          }
          PipelineEvent::NeedsClarification { node_id, question } => {
            if let Some(t) = self.by_id_mut(node_id) {
               t.status = TaskStatus::NeedsClarification;
               self.waiting_for_clarification = true;
               ctx.emit(PipelineCmd::RequestClarification { node_id, question });
            }
          }
          PipelineEvent::ClarificationProvided { node_id, answer } => {
            if let Some(t) = self.by_id_mut(node_id) {
               t.status = TaskStatus::Planned;
               // incorporate answer
               t.description.push_str(&format!("\nClarification: {}", answer));
               self.waiting_for_clarification = false;
               self.on_tick_or_ready(ctx);
            }
          }
          PipelineEvent::TaskFailed { node_id, error } => {
            // policy: skip, retry, or abort. Start simple: log+advance
            self.cursor += 1;
            self.on_tick_or_ready(ctx);
          }
          _ => {}
        }
    }
}
```

---

## 3) Planning & Attachments

**Parsing strategy (v1):**
- Split input into steps via bullets, sentences, or “then/next” cues.
- Infer `NodeKind` - for now simplify, later:
  - contains “test/verify” → `UnitTest`
  - “fetch/read/scrape” or URL present → `Retrieval`
  - “analyze/summarize” → `Analysis`
  - default → `CodeImplementation`
- Attachments - for future:
  - URLs → `AttachmentKind::Link`
  - image refs (e.g., `.png`, `.jpg`, known ids) → `ImageRef`
  - explicit files → `FileRef`

**Context compaction (v1):**
- After each completion, append `result` summary to `context_summary` (truncate to N chars).
- Future prompts include `context_summary` + current task only.

---

## 4) Integration Points

- Create `PlannerState` inside `actors.rs` (or `planner.rs` re-exported).
- When new **user input** arrives: build `attachments`, call `plan_tasks`, then call `on_tick_or_ready`.
- Route all `PipelineEvent` instances to `PlannerState::on_event`.
- Ensure executor maps `NodeKind` → suitable actor/tool (code agent, test runner, retriever, etc.).

---

## 5) Error & Clarification Policy

- **NeedsClarification** pauses the loop; only resume on `ClarificationProvided`.
- **TaskFailed** policy (v1): log, mark failed, continue; (later add retries/backoff).
- Validate attachments exist/accessible before dispatch; if not, ask for re-upload or alt link.

---

## 6) Minimal Example

**Input**: “Add login with session cookies. Use basic auth. Read API spec at https://example.com/spec.pdf. Then write unit tests.”

**Planned tasks** (example):
1. Retrieval (spec) — parse key constraints.  
2. CodeImplementation — backend login.  
3. CodeImplementation — frontend form & wiring.  
4. UnitTest — cover happy/edge cases.

If ambiguity (e.g., *cookie expiry?*), emit `RequestClarification` and wait.

---

## 7) Extensibility

See section 11 (Future Work) for planned extensions beyond v1.

---

## 8) Testing

- Unit: parse → tasks mapping; event handling transitions.
- Integration: scripted sequence (Completed → NeedsClarification → ClarificationProvided → Completed).
- Load: long task lists + compaction threshold respected.

---

## 9) Definition of Done

- Enums extended; planner compiles and is called on new input.
- Sequential loop executes tasks; clarification pause/resume works.
- Context compaction active; final summary emitted.
- Basic tests passing (unit + one integration path).

---

## 10) Scope: Not Now (v1)

- Advanced `NodeKind` variants (e.g., `Refactor`, `ToolCall`, `Clarification`).
- Non-URL attachments (image refs, file refs) and parsing of local files.
- Checkpointing, cancellation/abort flows, or persistence of planner state.
- Retries/backoff policies beyond simple log-and-advance on failure.
- Parallel or graph/DAG execution; v1 is strictly sequential.
- LLM-backed planning; v1 uses deterministic parsing heuristics.
- Long-term memory/vector store; v1 uses a rolling compact string summary.
- Rich metrics/telemetry; v1 may include minimal logging only.

---

## 11) Future Work

- Expand `NodeKind` as new tools/agents ship (e.g., `Refactor`, `ToolCall`).
- Introduce `AttachmentKind` and `Attachment` handling for images and files.
- Add retry policies with caps/backoff and failure classification.
- Checkpoint/save/restore planner state and cancellation support.
- Optional parallelization or partial ordering once executors support it.
- Replace heuristic `plan_tasks` with an LLM-backed planner (same `Task` API).
- Upgrade `context_summary` to a vector store or structured memory.
- Add richer metrics, tracing, and UI affordances for clarifications.

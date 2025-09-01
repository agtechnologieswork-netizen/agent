# Implementation Tasks — Claude‑Style Planner (meta_agent / meta_draft)

> Paste into a GitHub Issue or run as a checklist for your coding agent.

## Milestone A — Types & State (v1 scope)
- [ ] Extend **`PlannerCmd`** with:
  - [ ] `ExecuteTask { node_id, kind, parameters }`
  - [ ] `RequestClarification { node_id, question }`
- [ ] Extend **`ExecutorEvent`** with:
  - [ ] `TaskCompleted { node_id, result }`
  - [ ] `TaskFailed { node_id, error }`
  - [ ] `NeedsClarification { node_id, question }`
  - [ ] `ClarificationProvided { node_id, answer }`
- [ ] Add **planner structs**:
  - [ ] `TaskStatus`, `Task`
  - [ ] `PlannerState { tasks, cursor, waiting_for_clarification, pending_clarification_for, next_id, context_summary }`
  - [ ] `PlannerConfig { system_prompt }`
  - [ ] `Planner<L, C> { state, llm, compactor, config }`

## Milestone B — Planning & Dispatch (v1 scope)
- [ ] Implement `PlannerState::plan_tasks(input: &str, atts: &[Attachment])`
  - [ ] Parse steps (heuristic)
  - [ ] Extract URLs only for attachments (defer files/images)
- [ ] Implement `PlannerState::step(ctx, incoming: Option<ExecutorEvent>)`
  - [ ] Route incoming events, then pick next Planned task -> emit `PlannerCmd::ExecuteTask` -> mark Running
- [ ] Wire initial call on new user input:
  - [ ] Build attachments from message
  - [ ] `plan_tasks` then `step(ctx, None)`

- [ ] Implement event handling within `step`:
  - [ ] `TaskCompleted` -> mark Completed -> `compact_with(result)` -> advance
  - [ ] `NeedsClarification` -> mark NeedsClarification -> set waiting flag -> emit `RequestClarification`
  - [ ] `ClarificationProvided` -> merge answer into task -> clear waiting
  - [ ] `TaskFailed` -> record + advance (v1 policy)
- [ ] Implement `compact_with(&mut self, result: &str)`
  - [ ] Append summarized result to `context_summary` with max length
  - [ ] Use Compactor abstraction (no naive truncation)
  - [ ] Pause semantics: set `waiting_for_clarification = true` and `pending_clarification_for = Some(node_id)`

## Milestone D — Integration & Routing (v1 scope)
- [ ] Route **all `ExecutorEvent`** to `PlannerState::step`
- [ ] Ensure executor routes **`NodeKind`** to correct handler/agent
- [ ] Add minimal logging/metrics

## Milestone E — Tests (v1 scope)
- [ ] Unit tests: planning, attachment parsing, state transitions
- [ ] Integration test: Completed -> NeedsClarification -> ClarificationProvided -> Completed path
- [ ] Large input test: compaction keeps prompts under limit
  - [ ] Compactor budget respected, no naive truncation

## Acceptance Criteria
- [ ] End-to-end: given input + attachments -> tasks execute sequentially
- [ ] Clarification pauses and resumes correctly
- [ ] Final summary/event emitted on completion
- [ ] All tests pass in CI

## Copy‑paste Issue Template

**Title:** Implement Claude‑Style Planner in `meta_draft`

**Body:**
- Implement planner per design doc.
- Extend enums, add planner state/structs.
- Sequential execution loop with clarification handling.
- Context compaction after each task.
- Wire into `actors.rs` user input flow.
- Add tests (unit + integration).

## Not Now (v1)
- [ ] Add advanced `NodeKind` variants (`Refactor`, `Clarification`, `ToolCall`)
- [ ] Add non-URL attachments (`AttachmentKind`, `Attachment` for images/files)
- [ ] Add retries/backoff beyond simple advance-on-fail
- [ ] Add checkpointing/cancellation/persistence of planner state
- [ ] Add parallel or DAG execution features
- [ ] Replace heuristic planner with LLM-backed planning
- [ ] Replace rolling summary with vector-store memory

## Future Work
- [ ] Extend **`NodeKind`** as new tools/agents are added
- [ ] Introduce **`AttachmentKind`** and **`Attachment`** for richer attachments
- [ ] Implement robust retry/backoff and failure classification
- [ ] Checkpoint/save/restore and cancellation support
- [ ] Optional parallelization / partial ordering support
- [ ] LLM-backed planner maintaining `Task` API
- [ ] Memory upgrade to vector-store / structured context


**Checklist:** (see above tasks)


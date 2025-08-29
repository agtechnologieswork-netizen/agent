# Implementation Tasks — Claude‑Style Planner (meta_agent / meta_draft)

> Paste into a GitHub Issue or run as a checklist for your coding agent.

## Milestone A — Types & State
- [ ] Extend **`PipelineCmd`** with:
  - [ ] `ExecuteTask { node_id, kind, parameters }`
  - [ ] `RequestClarification { node_id, question }`
- [ ] Extend **`PipelineEvent`** with:
  - [ ] `TaskCompleted { node_id, result }`
  - [ ] `TaskFailed { node_id, error }`
  - [ ] `NeedsClarification { node_id, question }`
  - [ ] `ClarificationProvided { node_id, answer }`
- [ ] Add **planner structs**:
  - [ ] `TaskStatus`, `Task`
  - [ ] `PlannerState { tasks, cursor, waiting_for_clarification, next_id, context_summary }`

## Milestone B — Planning & Dispatch
- [ ] Implement `PlannerState::plan_tasks(input: &str, atts: &[Attachment])`
  - [ ] Parse steps
- [ ] Implement `PlannerState::on_tick_or_ready(ctx)`
  - [ ] Pick next Planned task -> emit `PipelineCmd::ExecuteTask` -> mark Running
- [ ] Wire initial call on new user input:
  - [ ] Build attachments from message
  - [ ] `plan_tasks` then `on_tick_or_ready`

## Milestone C — Event Handling
- [ ] Implement `PlannerState::on_event(evt, ctx)`:
  - [ ] `TaskCompleted` -> mark Completed -> `compact_with(result)` -> advance -> `on_tick_or_ready`
  - [ ] `NeedsClarification` -> mark NeedsClarification -> set waiting flag -> emit `RequestClarification`
  - [ ] `ClarificationProvided` -> merge answer into task -> clear waiting -> `on_tick_or_ready`
  - [ ] `TaskFailed` -> record + advance (v1 policy)
- [ ] Implement `compact_with(&mut self, result: &str)`
  - [ ] Append summarized result to `context_summary` with max length

## Milestone D — Integration & Routing
- [ ] Route **all `PipelineEvent`** to planner in `actors.rs`
- [ ] Ensure executor routes **`NodeKind`** to correct handler/agent
- [ ] Add minimal logging/metrics

## Milestone E — Tests
- [ ] Unit tests: planning, attachment parsing, state transitions
- [ ] Integration test: Completed -> NeedsClarification -> ClarificationProvided -> Completed path
- [ ] Large input test: compaction keeps prompts under limit

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

## Future
- [ ] Extend **`NodeKind`** with at least:
  - [ ] `CodeImplementation`, `UnitTest`, `Refactor`, `Retrieval`, `Analysis`, `Clarification`, `ToolCall`
- [ ] Add **planner structs**:
  - [ ] `AttachmentKind`, `Attachment`


**Checklist:** (see above tasks)


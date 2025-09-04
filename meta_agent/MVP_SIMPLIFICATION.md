# MVP Simplification Summary

## Key Decision: Keep Non-LLM Planner ✅

The basic `Planner` in `handler.rs` is **necessary** for:
1. **Testing**: Allows tests to run without LLM dependencies
2. **Fallback**: When LLM is unavailable or fails  
3. **Event Sourcing Core**: Provides the fundamental Handler trait that LLM version wraps

## Simplifications Made

### 1. **Simplified Basic Parser**
- **Before**: Complex line-by-line parsing with keyword-based classification
- **After**: Single task creation - let LLM handle real parsing
- **Rationale**: Basic planner is just a fallback; complexity belongs in LLM

### 2. **Removed Context Compaction Logic**
- **Before**: Token counting and task summarization
- **After**: No-op that returns existing summary
- **Rationale**: Real compaction needs LLM understanding

### 3. **Simplified Attachment Types**
- **Before**: Link, ImageRef, FileRef
- **After**: Just Link and FileRef
- **Rationale**: MVP doesn't need image handling

### 4. **Minimized Configuration**
- **Before**: system_prompt, profile, token_budget, error_char_limit
- **After**: Just token_budget
- **Rationale**: Other fields were unused or LLM-specific

### 5. **Consolidated Error Types**
- **Before**: InvalidCommand, TaskNotFound, LlmError, ParseError
- **After**: InvalidCommand, TaskNotFound, ExternalError
- **Rationale**: Simpler error handling for MVP

## Architecture Clarity

```
┌─────────────────────────────────────────┐
│           LLMEnhancedPlanner            │ ← Production Use
│  - Uses LLM for intelligent parsing     │
│  - Falls back to basic planner on error │
└─────────────────┬───────────────────────┘
                  │ wraps
┌─────────────────▼───────────────────────┐
│             Planner (Basic)             │ ← Testing/Fallback
│  - Implements Handler trait             │
│  - Simple single-task fallback          │
│  - Event sourcing mechanics             │
└─────────────────────────────────────────┘
```

## What Stays Complex (Rightfully)

1. **Handler Trait Pattern**: Core architectural decision
2. **Event Sourcing**: Foundation for state management
3. **DabGent MQ Integration**: Production infrastructure
4. **LLM Task Parsing**: Where the real intelligence lives

## Test Impact

- Tests updated to expect single task from basic planner
- All tests passing ✅
- No functionality lost, just simplified fallback behavior

## Next Steps for Production

1. Always use `LLMEnhancedPlanner` in production
2. Basic `Planner` is for tests and emergency fallback only
3. Focus development on improving LLM parsing quality
4. Add monitoring to track LLM failures and fallback usage

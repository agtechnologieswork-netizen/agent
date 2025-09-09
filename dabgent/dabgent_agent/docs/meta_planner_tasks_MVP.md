# Event-Sourced LLM Planner — MVP Tasks

## ✅ DONE (What We Have)

### Core Implementation
- [x] Handler trait with process/fold
- [x] Basic Planner with event sourcing
- [x] LLM integration for task parsing
- [x] DabGent MQ event persistence
- [x] Tests passing

## 🚀 Ship MVP (What's Left)

### Integration Tasks (2-4 hours)
- [x] Create simple CLI demo
- [ ] End-to-end test with real LLM
- [ ] Basic error handling
- [x] Minimal documentation

## ❌ CUT from MVP (Do Later)

### Phase 2+ (After MVP Ships)
- [ ] Task execution framework
- [ ] Executor routing
- [ ] Context compaction
- [ ] Dependency analysis
- [ ] Parallel execution
- [ ] Checkpoint/restore
- [ ] Multi-agent coordination
- [ ] Vector stores
- [ ] RAG integration
- [ ] Monitoring/metrics
- [ ] UI/visualization
- [ ] Advanced error recovery
- [ ] Task templates
- [ ] Learning from history
- [ ] Profile-based strategies
- [ ] Attachment validation
- [ ] URL fetching
- [ ] Document parsing
- [ ] Embedding generation
- [ ] Semantic search
- [ ] Time-travel debugging UI
- [ ] A/B testing framework
- [ ] Performance optimization
- [ ] Horizontal scaling
- [ ] PostgreSQL support
- [ ] Migration tools
- [ ] Admin dashboard
- [ ] GraphQL API
- [ ] WebSocket subscriptions
- [ ] Event replay tools
- [ ] Audit logging
- [ ] Compliance features
- [ ] Multi-tenancy
- [ ] Rate limiting
- [ ] Cost tracking
- [ ] Usage analytics

## MVP Definition of Done

```bash
# This works:
echo "Build a todo app" | cargo run --features mq
# Output: Events saved to DabGent MQ

# This passes:
cargo test --features mq

# This exists:
README.md with 10 lines of "how to use"
```

## Time Estimate

- MVP Integration: 2-4 hours
- Everything Else: 2-4 months

## Focus

Just ship the MVP. Everything else can wait.

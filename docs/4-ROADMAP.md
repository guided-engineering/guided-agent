# Guided Agent CLI — Roadmap

This roadmap expands the previously defined short milestone list into a structured plan, written in clear, incremental phases. Each stage is intentionally small to keep the system stable, predictable, and easy to evolve.

---

## Phase 0 — Initialization

1. Create the Rust monorepo using Cargo workspaces.
2. Create initial crates (`core`, `cli`) with minimal scaffolding.
3. Set up `tracing` logging, configuration loader, and unified error types.

---

## Phase 1 — Core Foundations

4. Implement base CLI structure with empty commands (`ask`, `task`, `knowledge`, `stats`).
5. Add support for global options: workspace, config, log level, provider/model overrides.
6. Enforce stdout (results) vs stderr (logs) separation.

---

## Phase 2 — LLM Integration

7. Create the `llm` crate with a generic `LLMClient` trait.
8. Implement the first provider: **Ollama** (local-first default).
9. Add streaming support for LLM responses.

---

## Phase 3 — Prompt System

10. Create `.guided/prompts/` structure and default prompts.
11. Implement the prompt loader (YAML + schema enforcement).
12. Implement the prompt builder (context injection + Handlebars templates).
13. Integrate prompt builder into `/ask`.

---

## Phase 4 — `/ask` Command

14. Implement `/ask` with streaming and JSON output modes.
15. Add workspace context support.
16. Add optional knowledge context support (stub until knowledge base is ready).

---

## Phase 5 — Knowledge Base (RAG)

17. Create the `knowledge` crate with config + SQLite index.
18. Implement `knowledge learn` with chunking + embeddings.
19. Implement `knowledge ask` with top‑k retrieval.
20. Implement `knowledge clean` and `knowledge stats`.
21. Connect `/ask` to knowledge retrieval.

---

## Phase 6 — Task System

22. Create the `task` crate with `TaskPlan` model and `TaskStep` structures.
23. Implement `task plan` using a dedicated prompt.
24. Implement `task run` with atomic file writes and dry‑run mode.
25. Implement `task show` with JSON output.

---

## Phase 7 — Stats & Telemetry

26. Implement usage metrics stored under `.guided/operation/stats.json`.
27. Implement `/stats` with JSON and detailed output.
28. Add command-level metrics for `/ask`, `/task`, `/knowledge`.

---

## Phase 8 — Documentation

29. Populate `.guided/architecture/` with architecture, entities, data dictionary, and specs.
30. Add integration guide for IDEs/CI/script automations.
31. Add developer guide for Rust contributors.

---

## Phase 9 — Stability & Polishing

32. Ensure consistent error handling across all crates.
33. Add snapshot testing for prompts and integration tests for commands.
34. Perform final cleanup: remove dead code, enforce minimal logging, validate JSON outputs.

---

## Phase 10 — Optional Future Work

35. Add `serve` mode (local HTTP server).
36. Add multiple LLM providers (OpenAI, Claude, Gemini).
37. Implement incremental RAG updates and batch embeddings.
38. Add file watchers for knowledge auto-refresh.
39. Add plugin ecosystem or agent marketplace.

---

**This roadmap supports a predictable, minimal, and scalable evolution of the Guided Agent CLI, ensuring each milestone delivers stable value without architectural churn.**

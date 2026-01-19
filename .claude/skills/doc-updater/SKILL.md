---
name: doc-updater
description: Guide for updating documentation to maintain consistency with code changes. Use when implementation is done but docs need syncing.
---

# Documentation Update Guide

When code is modified, related documentation MUST be updated to maintain strict consistency.

## 1. Mapping: Code to Docs

Identify which documents need updates based on the type of code change.

| Code Change Type    | Affected Files (src/)              | Target Doc (docs/)       | Action                                                    |
| :------------------ | :--------------------------------- | :----------------------- | :-------------------------------------------------------- |
| **Concept/Vision**  | N/A                                | `concept.md`             | Update high-level vision or core philosophy.              |
| **Architecture**    | `main.rs`, `core/`, `orchestrator/`| `architecture.md`        | Update component design, data flow, or tech stack details.|
| **Requirements**    | New features, CLI args             | `requirements.md`        | Update functional requirements list.                      |
| **UI/TUI Layout**   | `ui/`, `tui/`                      | `architecture.md`        | Update TUI layer descriptions.                            |

## 2. Update Process

1.  **Read the Doc**: Read the target document first to understand the current state.
2.  **Edit**: Update the document to reflect the code changes.
    - Keep the format consistent with the existing document.
    - If diagrams (Mermaid) are involved, ensure they are syntactically correct.
3.  **Verify**: Check if there are any contradictions between the new doc and other docs.

## 3. Consistency Checklist

- [ ] Does `architecture.md` accurately reflect the current Rust struct/module structure?
- [ ] Are all implemented features marked as completed in `requirements.md`?
- [ ] Do unit/integration tests match the requirements?

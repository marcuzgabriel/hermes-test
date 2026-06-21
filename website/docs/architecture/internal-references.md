---
title: Internal references
---

# Internal references

These repository references are the project memory for hermes-test.  
Use them to understand **why** choices were made, not only **what** exists in code.

## How to use this section

1. Start with **Scope** and **Decisions** before implementing new features.
2. Use **Roadmap** to understand sequencing and delivery expectations.
3. Use the strategy/fix references when touching mocking, rendering, or coverage behavior.

## Core references

| Reference | What it contains | When to read it |
|---|---|---|
| [Scope](https://github.com/marcuzgabriel/hermes-test/blob/main/.claude/references/scope.md) | v0/v1 boundaries and out-of-scope items | Before planning any new work |
| [Decisions](https://github.com/marcuzgabriel/hermes-test/blob/main/.claude/references/decisions.md) | Settled architectural decisions and non-goals | Before proposing alternatives |
| [Roadmap](https://github.com/marcuzgabriel/hermes-test/blob/main/.claude/references/roadmap.md) | Time-based milestones and priorities | When sequencing implementation |
| [Challenges](https://github.com/marcuzgabriel/hermes-test/blob/main/.claude/references/challenges.md) | Day-by-day debugging history and solutions | When a regression looks familiar |

## Mocking and rendering references

| Reference | What it contains | When to read it |
|---|---|---|
| [Mock strategies](https://github.com/marcuzgabriel/hermes-test/blob/main/.claude/references/mock-strategies.md) | Current working mock strategy and operational notes | Before editing mock infrastructure |
| [Mock strategy history](https://github.com/marcuzgabriel/hermes-test/blob/main/.claude/references/mock-strategy.md) | Full strategy timeline, including failed approaches | When evaluating alternative mock designs |
| [Shadow wrappers](https://github.com/marcuzgabriel/hermes-test/blob/main/.claude/references/shadow-wrappers.md) | Proxy wrapper mechanics and alias/package shim model | Before touching bundler wrapper generation |
| [Shallow rendering fixes](https://github.com/marcuzgabriel/hermes-test/blob/main/.claude/references/shallow-rendering-fixes.md) | Root causes and fixes for `ht.shallow()` failures | Before changing shallow rendering behavior |
| [Component rendering](https://github.com/marcuzgabriel/hermes-test/blob/main/.claude/references/component-rendering.md) | Design notes for component rendering support | When extending render APIs |

## Performance and coverage references

| Reference | What it contains | When to read it |
|---|---|---|
| [Performance](https://github.com/marcuzgabriel/hermes-test/blob/main/.claude/references/performance.md) | Performance findings and optimization context | Before making performance claims |
| [Coverage approaches](https://github.com/marcuzgabriel/hermes-test/blob/main/.claude/references/coverage-approaches.md) | Explored coverage models and tradeoffs | Before changing coverage implementation |
| [V8 evaluation summary](https://github.com/marcuzgabriel/hermes-test/blob/main/.claude/references/v8-evaluation-summary.md) | Hermes vs V8 benchmark evaluation and conclusions | When discussing engine strategy |

## Working notes

| Reference | What it contains | When to read it |
|---|---|---|
| [Todo](https://github.com/marcuzgabriel/hermes-test/blob/main/.claude/references/todo.md) | In-flight notes and short-term tasks | For current task context only |

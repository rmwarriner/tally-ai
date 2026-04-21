# UI Foundation Design (T-037, T-030, T-031)

Date: 2026-04-21
Branch: codex/ui-foundation-phase1

## Scope

This design covers the first sequential UI foundation tickets:

1. T-037: shared `InfoCircle` + `Tooltip` primitives
2. T-030: app shell, sidebar/chat placeholders, Zustand UI store, keyboard toggle
3. T-031: health sidebar data panels (accounts, envelopes, coming up) with TanStack Query

## Goals

- Establish visible interactive affordances from day one
- Ship a working desktop shell with no horizontal overflow at minimum width
- Replace sidebar placeholder with production-ready read-only health panels
- Keep each ticket isolated in small commits for clean handoffs

## Architecture

### Component layers

- `components/ui`: reusable primitives (`InfoCircle`, `Tooltip`)
- `components/sidebar`: shell + toggle + health data panels
- `components/chat`: chat thread placeholder in T-030; full thread later tickets
- `hooks`: sidebar query hooks + IPC fetchers
- `stores`: `uiStore` for sidebar state and toggles
- `utils`: display-only formatters (`formatCents`)

### Data flow

- UI state (`sidebarOpen` initially) in Zustand
- Sidebar panel data via TanStack Query hooks
- Query functions call Tauri IPC using `invoke`
- Panel-level loading/error/empty UI isolation

## UX decisions

- Affordance-first rule: `InfoCircle` is always visible at rest
- Sidebar transitions use CSS width animation only
- Financial data prioritizes scanability: right-aligned formatted amounts, compact sections
- Over-budget and liability states use caution/danger color semantics

## Testing strategy

- TDD for each ticket: failing tests first, then implementation
- Keep tests colocated with components/hooks/utilities
- Validate command/keyboard behavior with interaction tests
- Keep coverage >= 80% for new files and touched surfaces

## Risks and mitigations

- Risk: CSS regressions while shell and sidebar evolve
  - Mitigation: explicit role-based tests and width assertions
- Risk: IPC not yet implemented for sidebar data
  - Mitigation: panel-level error states and hook-level test mocks
- Risk: cross-ticket churn
  - Mitigation: shared primitives built first, then shell, then data panels

## Handoff protocol

- Commit frequently, scoped by ticket/step
- Include ticket ID in commit body/message where useful
- Keep untracked local-only files out of commits
- At handoff points, include current status + next commandable step

# React Component Behavior Matrix — T-061

Every component listed here has a test file enforcing the requirements below.
Every test file wraps at least one render with
`expectNoA11yViolations(await checkA11y(container))` so that a11y regressions
are caught alongside behavioral ones. The axe helper lives at
`apps/desktop/src/test/axe.ts`.

When you add a new component to the chat surface or input bar, add a row here
and update the corresponding test file (or create one). Same discipline as
`core::validation_matrix` on the Rust side.

## TransactionCard (`src/components/chat/TransactionCard.tsx`)

- Render in 4 states: `posted`, `pending`, `voided`, `correction_pair`.
- Each state passes axe with no violations.
- `pending` state exposes a visible info-circle affordance (drawer toggle has
  an `<InfoCircle />` whose `role="img"` carries an `aria-label`).
- Journal-line drawer is collapsed by default; clicking the toggle expands it
  and reveals each line with its `side` (debit|credit) and amount.
- Card-local error: when `commitError` is set on the pending state, an
  `role="alert"` element renders with the message text. (Recovery actions are
  surfaced separately as a proactive advisory message via `appendAdvisory`;
  see `ProactiveMessage` for the rendering of `recovery[]`.)

## ChatThread (`src/components/chat/ChatThread.tsx`)

- Renders all message kinds in the `ChatMessage` union: `user`, `ai`,
  `proactive`, `system`, `transaction`, `artifact`, `setup_card`, `handoff`,
  `gnucash_mapping`, `gnucash_reconcile`. (Per-kind rendering coverage lives
  in `MessageList.test.tsx`; ChatThread asserts the union flows through.)
- Date separators appear between messages on different local-day boundaries.
- Auto-scrolls to the bottom when a new message arrives and the user is near
  the bottom.
- Shows a "new message" pill when the user is scrolled up and a new message
  arrives; clicking it scrolls to the bottom and hides the pill.
- Calls `fetchNextPage` (infinite-scroll callback) when the "Load earlier
  messages" button is activated at the top of the thread.

## InputBar (`src/components/input/InputBar.tsx`)

- Slash palette opens on `/` at the start of the input and filters as the
  user types.
- Arrow keys navigate the palette; Enter selects the highlighted option;
  Escape closes the palette.
- Chip strip renders chips from `useUIStore`; clicking a chip's remove
  button dismisses it from the store.
- Textarea grows with content up to a max height (`MAX_HEIGHT_PX = 144` in
  `ChatTextarea.tsx`). jsdom does not measure layout, so the test asserts
  the inline height-resize side effect runs and respects the cap; visual
  growth is observed in Playwright (Task 23).

## safeInvoke (`src/lib/safeInvoke.ts`) — covered in Task 10.

## ErrorBoundary (`src/components/ErrorBoundary.tsx`) — covered in Task 11.

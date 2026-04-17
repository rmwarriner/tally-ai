**tally.ai**

Phase 1 Build Specification

  ------------------ ----------------------------------------------------
  **Version**        1.0 --- Draft

  **Date**           April 17, 2026

  **Author**         Tulip Design

  **Status**         Ready for implementation

  **Target**         Claude Code + Codex orchestration
  ------------------ ----------------------------------------------------

> Phase 1 delivers a working desktop application: Tauri 2 shell,
> encrypted SQLite schema, Rust accounting core, Claude AI backend, and
> manual transaction entry via natural language. All subsequent phases
> build on this foundation.

**1. Overview**

Tally.ai is a conversational household finance application. The user
interacts exclusively through a chat interface --- entering transactions
in natural language, querying budgets, and receiving proactive financial
insights. There are no forms, no ledger edit screens, and no traditional
navigation menus. The accounting core is a GAAP-compliant double-entry
ledger implemented in Rust. The AI layer parses intent, constructs
transaction proposals, and generates responses. The UI is a Tauri 2
desktop application with a React/TypeScript frontend.

Phase 1 scope is deliberately narrow. The goal is a working vertical
slice --- one AI backend, manual entry only, desktop only, single user
--- that proves the core interaction model before complexity is added.

**1.1 What Phase 1 Delivers**

- Tauri 2 desktop shell (macOS, Windows, Linux)

- SQLCipher encrypted database with full v1 schema

- Rust accounting core: double-entry validation, envelope tracking,
  audit log

- React/TypeScript chat UI with health sidebar

- Claude AI backend: intent classification, TransactionProposal output
  contract

- Natural language transaction entry --- simple and split

- Slash command surface: /budget, /balance, /recent, /undo, /fix, /help,
  /defaults

- Onboarding conversation --- fresh start and hledger migration paths

- Morning briefing and basic proactive insight engine

- Error message contract --- plain language, typed RecoveryActions

**1.2 What Phase 1 Explicitly Excludes**

- Mobile (iOS/Android) --- Phase 2

- Multi-user / sync --- Phase 2

- SimpleFIN bank connectivity --- Phase 2

- File import (CSV, OFX, PDF) --- Phase 2

- Scheduled/recurring transactions --- Phase 2

- GPT, Gemini, Ollama backends --- Phase 2

- Pinned panel / three-column layout --- Phase 2

- Folder watch, receipt camera --- Phase 2

**2. Technology Stack**

  ------------------ ----------------------------------------------------
  **Desktop shell**  Tauri 2 (Rust backend + WebView frontend)

  **UI framework**   React 18 + TypeScript 5

  **State            Zustand
  management**       

  **Data fetching**  TanStack Query v5

  **Database**       SQLite via sqlx + SQLCipher encryption

  **Accounting       Rust (Tauri backend commands)
  core**             

  **AI backend**     Anthropic Claude API (claude-sonnet-4-5)

  **Styling**        CSS custom properties --- no Tailwind in desktop
                     shell

  **Testing**        Vitest (frontend) + cargo test (Rust) + Playwright
                     (E2E)

  **Package          pnpm (frontend) + Cargo (Rust)
  manager**          

  **Minimum          80% --- enforced via pre-commit hooks
  coverage**         
  ------------------ ----------------------------------------------------

**2.1 Repository Structure**

tally/

apps/

desktop/ \# Tauri app

src-tauri/ \# Rust backend

src/

commands/ \# Tauri command handlers

core/ \# Accounting core (pure Rust, no Tauri deps)

ledger.rs \# Double-entry engine

validation.rs \# Three-tier validation

envelope.rs \# Envelope budget logic

db/ \# SQLx migrations + queries

ai/ \# AI orchestration layer

prompt.rs \# Context assembly

parser.rs \# TransactionProposal parser

adapter/ \# Backend adapters (claude.rs + trait)

scheduler/ \# Recurrence engine (Phase 2 stub)

error.rs \# Structured error types + RecoveryAction

src/ \# React frontend

components/

chat/ \# Chat thread, bubbles, transaction cards

sidebar/ \# Health panel

input/ \# Input bar, chips, slash commands

artifacts/ \# Inline charts, ledger tables, reports

onboarding/ \# Setup conversation

stores/ \# Zustand stores

hooks/ \# TanStack Query hooks

packages/

core-types/ \# Shared TypeScript types (mirrors Rust structs)

CLAUDE.md \# AI coding context --- see Section 7

**3. Database Schema**

All tables use ULID primary keys, integer cents for monetary amounts,
and unix milliseconds for timestamps. No floating-point money values
anywhere in the schema. The audit_log table is INSERT-only --- enforced
via SQLite trigger.

**3.1 Key Design Decisions**

- ULIDs: time-sortable, globally unique, collision-safe for future sync

- Money as INTEGER cents: eliminates floating-point rounding errors

- households.timezone: IANA string (e.g. America/Chicago) --- required,
  no UTC default

- All calendar dates normalized to UTC midnight of the local date before
  storage

- is_placeholder on accounts: grouping-only accounts cannot receive
  journal lines

- append-only audit_log: INSERT trigger rejects UPDATE and DELETE
  statements

- journal_lines.amount always positive; side field (debit\|credit)
  encodes direction

- envelope_periods.spent updated atomically via SQLite trigger on
  journal_lines INSERT

**3.2 Phase 1 Tables**

**Core identity**

  -----------------------------------------------------------------------
  **Column**             **Type**      **Notes**
  ---------------------- ------------- ----------------------------------
  households: id         ULID PK       

  households: name       TEXT          e.g. Warriner household

  households: timezone   TEXT NOT NULL IANA --- e.g. America/Chicago.
                                       Required.

  households:            INTEGER       Migration tracking
  schema_version                       

  users: id              ULID PK       

  users: household_id    ULID FK       

  users: display_name    TEXT          e.g. Robert, Katie

  users: role            TEXT          owner \| member

  users: is_active       BOOLEAN       Soft disable only --- never delete
  -----------------------------------------------------------------------

**Ledger core --- transactions + journal_lines**

transactions holds the header; journal_lines holds the debit/credit
rows. Every committed transaction must have balanced DR = CR ---
enforced by the Rust core before any DB write.

Key fields: txn_date (when it happened), entry_date (when recorded),
status (pending\|posted\|void), source (manual\|ai\|scheduled\|import),
corrects_txn_id (GAAP correction chain), ai_confidence (0.0--1.0).

journal_lines key fields: account_id (leaf accounts only ---
is_placeholder must be false), envelope_id (nullable, expense lines
only), amount (INTEGER cents, always positive), side (debit\|credit).

**Chart of accounts**

Hierarchical via parent_id self-reference. is_placeholder=true means
grouping only --- the Rust core rejects any journal line referencing a
placeholder account with ERR_PLACEHOLDER_ACCOUNT. normal_balance
(debit\|credit) is stored per account type and validated at commit time.

**Envelopes + envelope_periods**

envelopes are first-class entities linked to a CoA leaf account.
envelope_periods materializes the spent balance per period --- updated
atomically via SQLite trigger rather than computed on query. This keeps
the health sidebar fast.

Trigger definition: after INSERT on journal_lines where envelope_id IS
NOT NULL and status=posted, UPDATE envelope_periods SET spent = spent +
NEW.amount WHERE envelope_id = NEW.envelope_id AND period_start \<=
NEW.txn_date AND period_end \>= NEW.txn_date.

**Audit log**

INSERT-only. SQLite trigger: BEFORE UPDATE OR DELETE ON audit_log
RAISE(ABORT, \'audit_log is immutable\'). Every AI-sourced action stores
ai_prompt_hash (SHA-256 of the prompt). Import-sourced transactions
store import_id and source_line (raw unparsed input, capped at 4KB).

**4. Accounting Core (Rust)**

The accounting core is a pure Rust library with no Tauri dependencies.
It can be tested in isolation with cargo test. All financial logic lives
here --- the AI layer and UI layer never write directly to the database.
They submit proposals; the core validates and commits.

**4.1 Three-Tier Validation**

**Tier 1 --- Hard rules (blocking, Rust only)**

- ERR_UNBALANCED: sum(DR) must equal sum(CR). Reports delta in cents.

- ERR_PLACEHOLDER_ACCOUNT: journal line references is_placeholder=true
  account

- ERR_INVALID_ACCOUNT: account not found, not active, or wrong household

- ERR_INVALID_AMOUNT: amount must be INTEGER cents \> 0

- ERR_INSUFFICIENT_LINES: minimum 2 lines (one DR, one CR)

- ERR_ABNORMAL_BALANCE: hard block when AI produces abnormal direction
  for clear intent

**Tier 2 --- Soft warnings (non-blocking)**

- WARN_FUTURE_DATE: txn_date \> entry_date + 1 day

- WARN_LARGE_AMOUNT: amount exceeds household threshold (default \$50000
  cents)

- WARN_ENVELOPE_OVERDRAFT: posting would push envelope past allocated
  amount

- WARN_STALE_DATE: txn_date more than 90 days in the past

- WARN_POSSIBLE_DUPLICATE: same payee + amount + date ±1 day already
  exists

**Tier 3 --- AI advisories (informational, AI layer only)**

- ADV_PAYEE_ACCOUNT: payee has prior mappings to a different account

- ADV_AMOUNT_DEVIATION: amount \> 2 std dev above 90-day rolling average
  for payee

- ADV_NO_ENVELOPE: expense line has no envelope_id --- AI suggests most
  likely

- ADV_SCHEDULED_MATCH: looks like a manual entry of a scheduled
  transaction

**4.2 ValidationResult Type**

Returned by the Rust core to the AI layer after every proposal. The AI
layer appends tier 3 advisories before the result reaches the UI.

ValidationResult {

status: approved \| rejected \| pending_confirmation

transaction_id: Option\<ULID\> // set if written to DB

hard_errors: Vec\<HardError\> // code, message, field, suggestion

warnings: Vec\<SoftWarning\> // code, message, severity,
auto_commit_blocked

advisories: Vec\<AIAdvisory\> // code, message, suggested_fix:
Option\<TransactionPatch\>

confidence: Option\<f32\> // 0.0--1.0, None if user-entered

auto_commit_at: Option\<i64\> // unix ms, None if explicit confirm
required

}

**4.3 Error Message Contract**

Every HardError, SoftWarning, and AIAdvisory MUST carry at least one
RecoveryAction. Enforced via NonEmpty\<Vec\<RecoveryAction\>\> in Rust
--- zero-action errors are a compile error.

RecoveryAction kinds: CreateMissing, UseSuggested (with
TransactionPatch), EditField, PostAnyway (with caveat), Discard,
ShowHelp.

Rules: plain language only --- no error codes in user-facing messages.
Internal codes appear in audit_log and console only. Every message names
the specific value that failed. One RecoveryAction marked
is_primary=true per error.

**5. AI Orchestration Layer**

The AI layer sits between the UI and the Rust core. It never writes to
the database directly. It constructs prompts, calls the Claude API,
parses the response into a typed TransactionProposal, appends tier 3
advisories, and hands off to the Rust core for validation and commit.

**5.1 Request Pipeline**

1.  Pre-classify: lightweight local classifier detects intent type
    (Rust, pattern matching)

2.  Build context: assemble 5-layer prompt (base + snapshot + intent +
    history + memory)

3.  Call Claude API: structured output via tool use

4.  Parse response: extract TransactionProposal or query answer

5.  Append tier 3 advisories: AI layer adds ADV\_ items before Rust sees
    proposal

6.  Submit to Rust core: validate and commit

7.  Update memory: async --- payee mappings, amount patterns, session
    summary

**5.2 Context Window Assembly**

  -------------------------------------------------------------------------
  **Layer**   **Contents**       **Budget**          **Notes**
  ----------- ------------------ ------------------- ----------------------
  BASE        System identity,   \~400 tokens        Static --- never
              hard rules, output                     trimmed
              contract                               

  SNAPSHOT    Account balances,  \~300 tokens        JSON, not prose.
              envelope health,                       Refreshed each
              next 3 scheduled                       request.

  INTENT      CoA leaves, payee  200--800 tokens     Loaded only for
              memory, relevant                       detected intent type
              history                                

  HISTORY     Last 10 messages   \~800 tokens        Verbatim trimmed to 5
              verbatim + rolling                     under Ollama budget
              summary                                

  MEMORY      Payee mappings,    \~250 tokens        Household + user
              vocabulary,                            scoped, async writes
              correction history                     
  -------------------------------------------------------------------------

**5.3 TransactionProposal Contract**

The AI must always return a TransactionProposal for entry intents ---
never free-form text. Enforced via Claude tool use. If tool use fails,
the adapter retries with explicit JSON schema in the user turn. Two
consecutive failures surface a structured error with RecoveryAction.

TransactionProposal {

payee: String

txn_date: ISODate // resolved from \'today\', \'last week\', etc.

memo: Option\<String\>

lines: Vec\<ProposedLine\> { account_id, envelope_id?, amount, side,
line_memo? }

confidence: f32 // 0.0--1.0

confidence_notes: Vec\<String\>

needs_clarification: bool

clarification_prompt: Option\<String\>

advisories: Vec\<AIAdvisory\>

}

**6. UI Layer**

**6.1 App Shell**

Two-column layout: health sidebar (left, collapsible) + chat thread
(right, full height). Three shell states: chat-only, chat + health,
chat + health + pinned panel (three-column). Pinned panel is Phase 2 ---
stub the column slot in Phase 1.

Sidebar header: tally.ai · health (Option C --- inline separator, muted
weight on \'health\'). Collapsed state: 44px wide, dot indicators only
--- green/amber/red per account and envelope. Always read-only --- no
interactive elements except the collapse toggle.

Last-sync-at timestamp shown in sidebar footer when last_sync_at is more
than 1 hour old. Stale state indicator for multi-user households (Phase
2).

**6.2 Chat Thread**

Single persistent log, infinite scroll upward. Three message visual
types:

- Plain prose bubble: answers, questions, confirmations

- Transaction card: every committed entry --- icon, payee, envelope,
  date, amount

- Artifact card: reports, charts, ledger tables --- framed inline panel

Proactive messages (Tally-initiated): amber avatar (vs blue for
responses), squared top-left corner (4px vs 14px radius), left border
accent encoding category (amber=insight, red=alert, blue=briefing),
micro-label inside bubble (insight · mid-session, alert · always-on,
briefing · morning).

Interactive affordance: info circle (ⓘ, 14×14px) always visible on
interactive bubbles and transaction cards. Turns blue on hover. Ledger
rows use hover-reveal \'fix in chat ↗\' label instead --- table density
exception, documented in component spec.

**6.3 Input Bar**

Chip strip above text box: /report, /budget, /ledger, /fix, quick-tap
shortcuts. Text box: single-line until wrap, Shift+Enter for newline,
Enter to send. Typing / alone reveals command palette inline.
Tab-completion for command names.

**6.4 Transaction Cards**

Four states: posted (green icon, checkmark), pending (amber icon,
clock), voided (gray, crossed out, voided badge), correction pair
(voided original + green replacement, GAAP reversal bridge label between
them).

Clicking a transaction card pre-fills /fix \[payee date\] in the chat
input and focuses the input. Never opens a form. Never opens an edit
modal. The info circle is the click affordance.

**6.5 Phase 1 Slash Commands**

  -------------------------------------------------------------------------
  **Command**   **Description**              **Example**
  ------------- ---------------------------- ------------------------------
  /budget       Envelope status --- all or   /budget · /budget groceries
                one                          

  /balance      Account balances             /balance · /balance checking

  /recent       Last N transactions          /recent · /recent 20 · /recent
                                             walmart

  /fix          Natural language correction  /fix that Shell charge was on
                                             Visa

  /undo         Reverse last entry           /undo

  /help         Command reference inline     /help

  /defaults     View or set defaults         /defaults · /defaults payment
                                             account = checking
  -------------------------------------------------------------------------

**7. CLAUDE.md --- AI Coding Context**

The CLAUDE.md file lives at the repository root and is loaded as context
by Claude Code at the start of every session. It must be kept current as
the codebase evolves. The following content should be used as the
initial CLAUDE.md.

\# Tally.ai --- CLAUDE.md

\## Project identity

Tally.ai is a conversational household finance app built with Tauri 2
(Rust backend),

React/TypeScript frontend, and Claude AI. The user interacts exclusively
through

a chat interface. There are no forms and no edit screens --- all writes
go through chat.

\## Non-negotiable architectural rules

\- Money is ALWAYS stored as INTEGER cents. Never REAL or FLOAT for
amounts.

\- The AI layer NEVER writes to the database directly. It submits
proposals.

The Rust core validates and commits. This boundary must never be
crossed.

\- audit_log is INSERT-only. Never issue UPDATE or DELETE on audit_log.

\- journal_lines.amount is always positive. The side field
(debit\|credit)

encodes direction. Never use negative amounts.

\- Every hard error, warning, and advisory must carry
NonEmpty\<RecoveryAction\>.

Zero-action errors are a compile error by design.

\- Error messages shown to the user must be plain language. No error
codes,

no runtime text, no field names. Internal codes go to logs only.

\- Interactive UI elements must always have a visible affordance (info
circle).

No invisible clickables anywhere in the app.

\## Code conventions

\- TDD-first: write tests before implementation. 80% coverage enforced
pre-commit.

\- Rust: use thiserror for error types. No unwrap() in production paths.

\- TypeScript: strict mode. No any. Use core-types package for shared
types.

\- React: functional components only. No class components.

\- State: Zustand for UI state. TanStack Query for server/DB state.

\- Feature branches: never commit directly to main.

\- Commit messages: conventional commits format (feat:, fix:, test:,
docs:).

\## Key types (Rust)

\- TransactionProposal: what the AI returns for entry intents

\- ValidationResult: what the Rust core returns after validation

\- RecoveryAction: typed next-step for every error (CreateMissing,
UseSuggested,

EditField, PostAnyway, Discard, ShowHelp)

\- HardError / SoftWarning / AIAdvisory: three-tier validation results

\## Database rules

\- All dates stored as unix milliseconds UTC midnight of local date.

Use household.timezone (IANA) for all local date conversions.

\- ULID for all primary keys. Use ulid crate in Rust, ulid package in
TS.

\- SQLCipher encryption key derived from user passphrase via Argon2id.

\- Migrations live in src-tauri/src/db/migrations/. Never edit past
migrations.

\## AI orchestration

\- Claude API: always use tool use for TransactionProposal output.

\- Never parse free-form text to extract transaction data.

\- Prompt assembly order: BASE \> SNAPSHOT \> INTENT \> HISTORY \>
MEMORY.

\- BASE and SNAPSHOT are never trimmed. Others trim under token budget.

\- Memory writes are always async --- never block the response path.

\## Phase 1 scope

\- Desktop only (Tauri). No mobile, no sync, no multi-user.

\- Claude backend only. No GPT, Gemini, or Ollama yet.

\- Manual entry only. No SimpleFIN, no file import, no folder watch.

\- No scheduled/recurring transactions yet.

\- Stub Phase 2 extension points with clear TODO(phase2) comments.

**8. Pre-Build Blockers**

These five issues from the technical design review must be resolved
before production code is written. Each has a specified resolution ---
implement it exactly.

  -----------------------------------------------------------------------
  **Issue**             **Resolution**
  --------------------- -------------------------------------------------
  **UTC / local time**  Store all calendar dates as UTC midnight of the
                        local date. Use households.timezone (IANA) for
                        all conversions. Never store raw event timestamps
                        as calendar dates.

  **Missing timezone    Add households.timezone TEXT NOT NULL. No default
  field**               of UTC. Required during onboarding setup. Error
                        if missing --- do not silently fall back.

  **Envelope balance    SQLite trigger on journal_lines INSERT/UPDATE:
  atomicity**           atomically update envelope_periods.spent. Same
                        trigger fires on void (spent -= amount).
                        Implemented in Migration 001.

  **Opening balances    transactions.source = \'opening_balance\' with
  not modelled**        immutable single journal line. Created by
                        onboarding wizard. Rust core treats these as
                        non-correctable, non-undoable.

  **Household key       Argon2id key derivation. Salt in households
  management**          table. New device via QR code scan (same-room
                        exchange). Recovery code: BIP-39 12-word mnemonic
                        generated at setup. Server never holds the key.
  -----------------------------------------------------------------------

**9. Phase 1 Ticket List**

Ordered by dependency. P0 = must complete before anything else builds on
top. P1 = core functionality. P2 = UI polish and non-critical paths.
Estimates are in ideal engineering days.

**9.1 Foundation (P0 --- do first)**

  ------------------------------------------------------------------------------
  **ID**    **Description**                **Layer**   **Priority**   **Est.**
  --------- ------------------------------ ----------- -------------- ----------
  T-001     Tauri 2 project scaffold ---   Infra       **P0**         0.5d
            workspace, pnpm, Cargo                                    

  T-002     SQLCipher integration ---      Data        **P0**         1d
            Argon2id key derivation, DB                               
            open/create                                               

  T-003     Migration 001 --- full Phase 1 Data        **P0**         1d
            schema including timezone                                 
            field                                                     

  T-004     audit_log INSERT-only trigger  Data        **P0**         0.5d

  T-005     envelope_periods.spent atomic  Data        **P0**         0.5d
            trigger                                                   

  T-006     ULID generation utilities      Infra       **P0**         0.5d
            (Rust + TypeScript)                                       

  T-007     core-types package --- shared  Infra       **P0**         0.5d
            TS types mirroring Rust                                   
            structs                                                   

  T-008     Pre-commit hooks --- cargo     Infra       **P0**         0.5d
            test, vitest, 80% coverage                                
            gate                                                      
  ------------------------------------------------------------------------------

**9.2 Accounting Core (P0/P1)**

  ------------------------------------------------------------------------------
  **ID**    **Description**                **Layer**   **Priority**   **Est.**
  --------- ------------------------------ ----------- -------------- ----------
  T-010     TransactionProposal and        Core        **P0**         0.5d
            ValidationResult types                                    

  T-011     RecoveryAction type ---        Core        **P0**         0.5d
            NonEmpty enforcement                                      
            compile-time                                              

  T-012     Tier 1 hard validation --- all Core        **P0**         1.5d
            6 rules with tests                                        

  T-013     Tier 2 soft warnings --- all 5 Core        **P1**         1d
            rules with tests                                          

  T-014     Ledger commit --- write        Core        **P0**         1d
            transactions + journal_lines                              
            atomically                                                

  T-015     GAAP correction --- reversal + Core        **P1**         1d
            replacement chain                                         

  T-016     Undo --- last entry reversal   Core        **P1**         0.5d
            with confirmation                                         

  T-017     CoA seed data --- standard     Core        **P1**         1d
            household chart of accounts                               

  T-018     Opening balance creation ---   Core        **P0**         0.5d
            immutable,                                                
            source=opening_balance                                    
  ------------------------------------------------------------------------------

**9.3 AI Orchestration (P1)**

  ------------------------------------------------------------------------------
  **ID**    **Description**                **Layer**   **Priority**   **Est.**
  --------- ------------------------------ ----------- -------------- ----------
  T-020     Claude adapter --- tool use,   AI          **P1**         1d
            TransactionProposal schema                                

  T-021     Intent pre-classifier --- 8    AI          **P1**         1d
            intent types, pattern matching                            

  T-022     Prompt assembly --- 5-layer    AI          **P1**         1.5d
            context builder                                           

  T-023     Financial snapshot builder --- AI          **P1**         0.5d
            balances + envelopes +                                    
            scheduled                                                 

  T-024     Payee memory ---               AI          **P1**         1d
            household-scoped mappings, LRU                            
            500 entries                                               

  T-025     Tier 3 AI advisories --- 4     AI          **P1**         1d
            advisory types                                            

  T-026     Two-pass fallback --- retry    AI          **P1**         0.5d
            with explicit JSON schema on                              
            tool use failure                                          

  T-027     Session summary compression    AI          **P2**         1d
            --- async, rolling 12-month                               
  ------------------------------------------------------------------------------

**9.4 UI --- Shell and Chat (P1)**

  ------------------------------------------------------------------------------
  **ID**    **Description**                **Layer**   **Priority**   **Est.**
  --------- ------------------------------ ----------- -------------- ----------
  T-030     App shell --- two-column       UI          **P1**         1d
            layout, health sidebar, chat                              
            column                                                    

  T-031     Health sidebar --- accounts,   UI          **P1**         1d
            envelopes with progress bars,                             
            coming up                                                 

  T-032     Sidebar collapse --- icon-only UI          **P1**         0.5d
            state with dot indicators                                 

  T-033     Chat thread --- message        UI          **P1**         1d
            rendering, scroll, infinite                               
            history                                                   

  T-034     Transaction card --- 4 states  UI          **P1**         1d
            (posted, pending, voided,                                 
            correction pair)                                          

  T-035     Artifact card --- framed       UI          **P1**         0.5d
            inline panel for reports and                              
            ledger views                                              

  T-036     Proactive message visual ---   UI          **P1**         0.5d
            amber avatar, corner geometry,                            
            border accent                                             

  T-037     Interactive affordance ---     UI          **P1**         0.5d
            info circle (14px, always                                 
            visible, blue on hover)                                   

  T-038     Input bar --- text box, chip   UI          **P1**         1d
            strip, slash command palette                              

  T-039     Slash command routing --- all  UI          **P1**         1d
            7 Phase 1 commands                                        
  ------------------------------------------------------------------------------

**9.5 UI --- Onboarding (P1)**

  ------------------------------------------------------------------------------
  **ID**    **Description**                **Layer**   **Priority**   **Est.**
  --------- ------------------------------ ----------- -------------- ----------
  T-040     Onboarding conversation engine UI          **P1**         1.5d
            --- adaptive phase detection                              

  T-041     Fresh start path --- accounts, UI          **P1**         1.5d
            balances, envelopes, scheduled                            
            stub                                                      

  T-042     Migration path --- hledger     UI          **P1**         1.5d
            import + CoA mapping session                              

  T-043     Setup cards --- account        UI          **P1**         0.5d
            created, opening balances,                                
            envelope summary                                          

  T-044     Handoff message --- summary    UI          **P1**         0.5d
            card with starter prompts                                 
  ------------------------------------------------------------------------------

**9.6 Proactive Engine (P2)**

  ------------------------------------------------------------------------------
  **ID**    **Description**                **Layer**   **Priority**   **Est.**
  --------- ------------------------------ ----------- -------------- ----------
  T-050     Morning briefing ---           AI          **P2**         1d
            session-open trigger, 4-item                              
            max                                                       

  T-051     Envelope approaching/over      AI          **P2**         0.5d
            triggers --- always-on alerts                             

  T-052     Duplicate detection trigger    AI          **P2**         0.5d
            --- same payee+amount+day                                 

  T-053     Insight log table and dedup    Data        **P2**         0.5d
            rules                                                     

  T-054     Sensitivity level ---          UI          **P2**         0.5d
            quiet/normal/proactive user                               
            setting                                                   
  ------------------------------------------------------------------------------

**9.7 Testing and Polish (P2)**

  ------------------------------------------------------------------------------
  **ID**    **Description**                **Layer**   **Priority**   **Est.**
  --------- ------------------------------ ----------- -------------- ----------
  T-060     Rust unit tests --- full       Test        **P2**         1d
            coverage of validation tiers                              

  T-061     React component tests ---      Test        **P2**         1d
            transaction card states, chat                             
            thread                                                    

  T-062     Playwright E2E --- onboarding  Test        **P2**         1.5d
            flow, entry, /fix, /undo                                  

  T-063     Accessibility audit --- WCAG   UI          **P2**         1d
            2.1 AA, aria-labels, contrast                             
            check                                                     

  T-064     Error boundary --- all Tauri   UI          **P2**         0.5d
            command failures surface                                  
            RecoveryAction                                            

  T-065     CLAUDE.md --- keep current as  Docs        **P2**         ongoing
            tickets complete                                          
  ------------------------------------------------------------------------------

**10. Phase 2 Preview**

Phase 2 scope is not fully specified here but the following items are
the natural next layer after Phase 1 ships. Each has been designed in
the concept documentation and is ready for a Phase 2 spec when Phase 1
is complete.

- Multi-user + sync: CRDT merge, three-tier sync transport, household
  key exchange

- Scheduler: recurring transactions, 4 recurrence patterns,
  weekend/holiday sliding, per-occurrence overrides

- SimpleFIN bank connectivity: 6-step poll pipeline, pending→cleared
  reconciliation

- File import: OFX, CSV, QIF, PDF, hledger, Beancount --- AI column
  mapper, ImportMapping

- Mobile: React Native, iOS share sheet, receipt camera

- Additional AI backends: GPT (function calling), Gemini (JSON schema),
  Ollama (grammar constraints)

- Pinned panel: three-column layout, read-only persistent views

- Proactive engine: full trigger catalogue, projections, pattern
  analysis

- Envelope rollover: envelope_periods.rolled_over computation

- Export: hledger journal, CSV, OFX --- /export command

- Reconciliation: match against bank statement --- /reconcile command

Tally.ai Phase 1 Build Specification --- Tulip Design --- April 2026

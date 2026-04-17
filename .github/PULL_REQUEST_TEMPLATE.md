## What

Brief description of the change.

## Why

Context and motivation. What problem does this solve? Link to relevant issues.

## How

Implementation approach. Any tradeoffs or decisions worth explaining?

## Testing

- [ ] Added tests (or updated existing ones)
- [ ] Coverage is at 80%+
- [ ] Manual testing complete
- [ ] No regressions in other features

## Checklist

- [ ] Conventional commit message (`feat:`, `fix:`, `test:`, `docs:`, etc.)
- [ ] Types pass: `pnpm typecheck`
- [ ] Tests pass: `pnpm test`
- [ ] Rust tests pass: `cargo test --all`
- [ ] No `any` types introduced
- [ ] No `unwrap()` in production paths (Rust)
- [ ] Error messages are plain language (no error codes/field names)
- [ ] No direct writes to audit_log
- [ ] Money stored as integer cents

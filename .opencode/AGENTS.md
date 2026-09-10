# Opencode-specific agent instructions

This file contains instructions that apply when this repository is opened with
**opencode**. They complement, and may override, the repo-level [`AGENTS.md`](../AGENTS.md)
and `node_modules/@daochild/agents-config/AGENTS.md`.

## Scope

- Project: AiFinPay Solana Splitter v1.4
- Canonical program: `programs/splitter/`
- Non-canonical / removed: `splitter_light` must not be reintroduced without a
  documented architectural decision and audit.

## Authority chain

When conflicting instructions appear, resolve them in this order:

1. System prompt / safety guardrails (never override).
2. This file (`.opencode/AGENTS.md`) for opencode-specific workflow.
3. Repo-level [`AGENTS.md`](../AGENTS.md) and [`programs/splitter/AGENTS.md`](../programs/splitter/AGENTS.md).
4. `node_modules/@daochild/agents-config/AGENTS.md`.
5. Other project docs (`ARCHITECTURE.md`, `SECURITY.md`, `CONTRIBUTING.md`, ADRs).

## Mandatory local checks

Before declaring any code task complete, run the CI steps from the repo root:

```bash
cargo fmt --check
cargo test --locked
cargo clippy --all-targets -- -D warnings
anchor build
```

Treat any failure as a blocker. Do not ask the user to skip these checks.

## Permission boundaries

The local `.opencode/opencode.json` denies certain operations. Respect them:

- **Never** run git mutations (`commit`, `push`, `reset`, `rebase`, `checkout`,
  `merge`, `pull`, `cherry-pick`, `revert`, `stash`, `branch`, `tag`, `rm`,
  `restore`) without explicit user confirmation.
- **Never** run destructive system commands (`rm`, `rmdir`, `sudo`, `chmod`,
  `chown`, `dd`, `mkfs`, `fdisk`, `halt`, `reboot`, `shutdown`, `poweroff`,
  `kill`, `pkill`, `killall`, `passwd`, `useradd`, `usermod`, `userdel`,
  `groupadd`, `groupdel`, package managers, etc.).
- File edits under `~/.ssh`, `~/.aws`, `~/.gnupg`, `~/.kube`, `~/secrets`, and
  `**/.env*` are restricted per `opencode.json`.

When in doubt, ask before executing a bash command.

## Code change discipline

- Make **minimal** changes to achieve the goal.
- Follow existing code style; run `cargo fmt` before finishing.
- Never leave the codebase in a broken state. If a change must span multiple
  files, keep intermediate states compilable.
- Add or update tests for any new behavior.
- Update `docs/IMPLEMENTATION.md` and the relevant ADR when an architectural
  decision changes.

## Cross-chain sacred constants

These values are part of the cross-chain contract with the EVM v1.4 deployment.
Any change is a coordinated upgrade:

- `ROUTE_AGENT_X402`, `ROUTE_MERCHANT_AIFP1`
- `MAX_TREASURY_BPS = 500`, `MAX_IP_CREATOR_BPS = 100`
- `Quote` field order in Borsh serialization used by `quote_message_hash()`
- `MESSAGE_DOMAIN_TAG = b"AiFinPay-Solana-v1.4"`

If a task touches any of these, invoke the `senior-solidity-auditor` and
`senior-software-architect` skills and require two human approvals in the PR.

## Security-critical invariants

These properties are checked by `cargo test` and must remain green:

1. **Signer exclusivity** — every accepted settlement recovers to `config.signer`.
2. **Replay safety** — a `(payer, nonce)` pair cannot settle twice.
3. **No zero-value splits** — `merchant_amt > 0`; non-zero bps must yield non-zero fees.
4. **Fee-cap enforcement** — `treasury_bps <= 500`, `ip_creator_bps <= 100`.
5. **Role separation** — admin, pauser, and signer key material are operationally distinct.
6. **Pause enforcement** — settlement instructions reject while `is_paused`.
7. **Cross-chain quote parity** — `Quote` schema, route IDs, and fee caps match EVM v1.4.

## Documentation updates

After any architectural, security, or role-related change, update:

- `ARCHITECTURE.md` if layout / roles / digest / state changed.
- `SECURITY.md` if threat model or invariants changed.
- `CONTRIBUTING.md` if workflow or parity rules changed.
- `docs/IMPLEMENTATION.md` with current status.
- `docs/adr/` if a new architectural decision was made or an old one changed.

## Audit trail

The repo contains `AUDIT_REPORT_splitter.md`. When addressing audit findings:

- Prioritize Critical and High findings over cosmetic cleanup.
- After fixing a finding, update the audit report or add a note documenting the
  remediation commit / PR.
- Do not delete audit reports unless explicitly asked.

## Communication style

- Be concise and accurate.
- Use the same language as the user.
- Do not hallucinate facts about the codebase; verify with `read`, `grep`, or
  `bash` before stating them.
- When a task has multiple steps, use the todo tool to track progress and keep
  exactly one item `in_progress` at a time.

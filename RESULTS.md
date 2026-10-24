# Measured results — 24 October 2026

**23 tests passed in each of debug and release, zero failed, zero ignored, zero warnings.** The release build also completed successfully. These are author-side executions, not independent external reproduction or certification.

## Environment and commands

Windows, `x86_64-pc-windows-gnu`, Rust 1.98.0 (`88d9e12ae`, LLVM 22.1.8). [EXECUTION.json](evidence/EXECUTION.json) records compiler/Cargo versions, the hashes of every tested Rust source and Cargo file, and the tested binary hash. The binary is not required or distributed; build it from the source.

| Command | Exit | Result | Log |
|---|---:|---|---|
| `cargo test --locked --offline` | 0 | 16 unit + 7 process tests passed; 0 ignored; 0 warnings | [debug](evidence/cargo-test-verified-debug.log) |
| `cargo test --release --locked --offline` | 0 | 16 unit + 7 process tests passed; 0 ignored; 0 warnings | [release](evidence/cargo-test-verified-release.log) |
| `cargo build --release --locked --offline` | 0 | Release build passed; 0 warnings | [build](evidence/cargo-build-verified-release.log) |

These are 23 distinct tests executed in two configurations, not 46 distinct claims. There are no authored doctests. Dependency resolution is fixed by `Cargo.lock`; the reported runs used the existing local cache. CI is separately configured for Windows and Ubuntu; this local report does not predict their outcome.

## Five cycles, one persistent context

The release integration test starts actual subprocesses using the same initialized WAL. The controlled input identity is `principal:alice`, operation `persist`, resource `poc010-test-vault`, scope `poc010:persist`, context `five-cycle-context`. Logical requests are `r1`, `r2`, `r3`; their exact JSON inputs and executable argument arrays are retained in [evidence/five-cycles](evidence/five-cycles).

| Cycle | Observed result | Physical records afterwards |
|---|---|---:|
| 1 | Both independent authorizations were verified and consumed before the parent released either worker. One worker committed; the other received the same existing receipt. | 1 |
| 2 | A fresh process obtained fresh authorization for exact `r1`; returned its original receipt without changing WAL bytes. | 1 |
| 3 | Parent killed a paused worker at `before_commit` for `r2`. The WAL remained unchanged, count 1. A fresh retry committed `r2`. | 2 |
| 4 | Parent killed a paused worker after synchronized `r3` effect and receipt, before final caller response. A fresh retry returned the stored receipt without a new append. | 3 |
| 5 | Historical `r1` still returned its original receipt. Changed payload under `r1` was refused, with unchanged WAL. | 3 |

The actual structured result is [five-cycle-results.json](evidence/five-cycles/five-cycle-results.json). The actual final store is [five-cycle-final.wal](evidence/five-cycles/five-cycle-final.wal). Every child has stdout, stderr, command arguments and exit status. The two killed children exit unsuccessfully by design; the test asserts they were alive at the interruption point and had produced no final success result. Other intended refusals exit 2. A passing test does not mean every child command succeeded.

Both crash points are located in the local transaction protocol. In cycle 4, the effect **and completion receipt already share a durable record**; caller acknowledgement is missing. This does not demonstrate recovery from an external service effect with no corresponding completion record. Worker termination is not physical power-loss testing.

## Adverse controls

- Two fresh handles for the same request pass the original local source gate. This retained baseline establishes why the new shared action ledger is a separate integration requirement.
- Copied/consumed handles, wrong nonce, unknown generation and changed binding are refused. The process harness checks invalid permits against both an empty store and an existing receipt.
- Equal payloads with distinct logical request IDs produce two intentional records; deduplication does not erase a distinct action.
- Changed payload under an existing request, unauthorized operation/scope and wrong resource are refused without changing the store.
- Missing storage, corrupted frames, incomplete tails and even a rehashed duplicate physical claim are refused. No repair is hidden inside validation.
- A valid older prefix **is accepted**. The corresponding test passes by demonstrating this known limit, not by claiming rollback resistance.
- A trace callback failure before commit leaves no record; after commit, an exact retry reconciles the durable record. Cached success also calls `sync_all` before acknowledgement.

The scope and exclusions are explicit in [CLAIM_BOUNDARY.md](CLAIM_BOUNDARY.md). No `(1,1)` knowledge result, native Shadow admission, full LGC/Vula500 execution, or external exactly-once guarantee is manufactured by this capsule.

## Review and evidence discipline

An initial test build caught an undefined local name in the new process harness; it was corrected before any passing result was claimed. Subsequent review strengthened the barrier so both authorizations are verified before release, added corruption/rollback and malformed-trace controls, and required synchronization before an existing receipt is acknowledged. The final commands above were rerun after these runtime changes. Earlier intermediate runs are not substituted for the final results.

The four selected source blocks remain byte-identical to their source manifest. The four canonical source files examined for gate/transaction/storage order have identical before/after SHA-256 values in [canonical-readonly-check.json](provenance/canonical-readonly-check.json). No private runtime or predecessor implementation is included.

The public evidence copy changes only absolute test paths in argument arrays and the workspace path in Cargo logs. [COPY_PROVENANCE.json](evidence/COPY_PROVENANCE.json) records the original and public SHA-256 for each file and the exact transformation. Worker output, request content, receipts and final WAL bytes are unchanged. `<CASE_DIR>` and `<WORKSPACE>` are documented placeholders, not literal commands to execute.

`python tools/verify.py` checks the complete artifact file set, SHA-256 values, selected source blocks, and independently decodes the recorded Rust WAL in Python to recompute its chain, action identities and receipts. The manifest is not its own member; use the Git commit/release and separately reported manifest digest to identify the bundle. These checks establish internal consistency, not an independent authenticity root.

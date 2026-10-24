# ESS-MAI POC-010: Authority Non-Duplication and Lawful Execution Continuity

A source-derived Rust experiment responding to [MANU-de's concurrent authorization and crash-recovery proposal](https://github.com/gjatalegacy-create/ESS-MAI/discussions/13). Two independently issued, **already verified** authorizations compete for one local persistent action. Five successive cycles use the same store and context, including two actual worker terminations and fresh-process recovery.

**The effect tested is a local transaction append.** Claim, payload and completion receipt share a durable record. The crash after that record tests a missing caller acknowledgement. This is not a demonstration of exactly-once remote effects, native Shadow knowledge admission, or complete ESS-MAI runtime security.

## Why this experiment

POC005's source gate prevents reuse of a copied handle, but two fresh handles each possess their own one-use slot. POC010 retains that behavior as an explicit baseline. The new integration binds action identity and immutable material, then serializes a local persistent claim across processes. It does not grant a copied token native Vault authority.

The title is supported by ESS-MAI's [theory and method mapping](THEORY_AND_PROVENANCE.md), including Academic 1–3 and POC005's failure-to-advancement proposal. It is a descriptive synthesis, not an already proved universal theorem.

## Reproduce

Prerequisites: Rust/Cargo **1.89 or later** and Python 3 for artifact checks; local evidence uses Rust **1.98.0 on Windows GNU**. Use a local filesystem supporting operating-system file locks. Do not use a network share as an untested substitute.

```powershell
python tools/verify.py
cargo fetch --locked
cargo test --locked --offline
cargo test --release --locked --offline
cargo build --release --locked --offline
$env:POC010_TEST_EVIDENCE = Join-Path $PWD 'run-evidence'
cargo test --release --locked --offline --test process five_cycle_continuity_across_concurrency_crashes_and_historical_retry -- --exact --nocapture
```

Run each command separately and stop on nonzero exit. `cargo fetch` may need network once; the following commands use the locked cache. The final command executes real fresh worker processes, synchronizes their admission boundary, kills two paused workers, and writes inputs, commands, stdout, stderr, exit codes, final WAL and a five-cycle JSON result below `run-evidence`. On POSIX systems set `POC010_TEST_EVIDENCE="$PWD/run-evidence"` instead. The artifact checker performs no download or execution.

Manual bounded operation (on a new absent WAL path):

```powershell
target/release/poc010-action-continuity init sample.wal test-vault
target/release/poc010-action-continuity worker sample.wal examples/request.json worker-1
target/release/poc010-action-continuity worker sample.wal examples/request.json worker-2
target/release/poc010-action-continuity inspect sample.wal test-vault
```

The second worker has a fresh process-local authorization and must return `AlreadyCommitted` with the original receipt and one physical effect. `inspect` is read-only and never initializes or repairs a WAL. `init` refuses an existing file.

## Five-cycle contract

| Cycle | Action | Required observable result |
|---|---|---|
| 1 | Two independently verified authorizations, synchronized before the persistent claim | One commit and one existing receipt; **1** physical effect |
| 2 | Fresh process, exact retry of request r1 | Same receipt and byte-identical WAL; **1** effect |
| 3 | New request r2; kill worker before commit, then retry | No effect from killed attempt; successful retry; **2** total effects |
| 4 | New request r3; kill after durable effect/receipt but before caller acknowledgement | Recovery returns existing receipt without append; **3** total effects |
| 5 | Retry r1 after later commits; attempt changed payload under r1 | Original receipt retained; changed material refused; **3** total effects |

Recorded results and actual execution evidence are in [RESULTS.md](RESULTS.md). These are execution cycles for one persistence context, not a claim of natural-language conversation or scientific reasoning.

## Boundary and order

1. The declared local fixture policy accepts only `persist` within `poc010:persist`.
2. The exact source gate checks generation/nonce and atomically consumes its local slot. New glue checks the exact action-plus-payload binding before consumption.
3. Storage independently locks the one WAL, verifies its current chain and resource, and checks action identity against immutable material.
4. A new action gets one frame containing claim, effect and receipt. `write_all`, `flush`, `sync_all` complete before a successful response.
5. A retry with a fresh valid authorization returns the original receipt; a conflicting payload fails. Missing/corrupt/incomplete storage fails closed and is preserved.

The copied source token is never cloned or exported. The adapter does not turn the original legacy FFI `CompatibilityHold` into a knowledge write. Native `GCL > Shadow verdict > Vault sink` authority remains outside this local fixture; no `(1,1)` or `(0,0)` is fabricated to pass the test.

## Evidence and review

- [Source extraction and hashes](provenance/source_gate.json): four exact selected blocks, with new wrappers labeled.
- [Claim boundary](CLAIM_BOUNDARY.md): assumptions, adverse controls and what is not proved.
- [Theory and provenance](THEORY_AND_PROVENANCE.md): original method, established technology and new integration.
- [Response to the reviewer](REVIEW_RESPONSE.md): the specific local-transaction question answered.
- [Review protocol](REVIEW_PROTOCOL.md): how to challenge this result.
- [Citation](CITATION.cff) and [attribution](NOTICE.md).

SHA-256 checks detect inconsistency; they are not authorization signatures or independent authenticity roots. A valid older WAL prefix is accepted, and this limitation has its own test. No authenticated rollback protection is claimed.

## Project and author links

- [ESS-MAI source and POC family](https://github.com/gjatalegacy-create/ESS-MAI)
- [Hugging Face public archive](https://huggingface.co/datasets/gjata-legacy/ess-mai-executable-prior-art)
- [Zenodo predecessor POC003–006 archive](https://doi.org/10.5281/zenodo.22750188)
- [OpenAIRE predecessor metadata](https://explore.openaire.eu/search/result?pid=10.5281%2Fzenodo.22750188)
- [Bledar Gjata on LinkedIn](https://www.linkedin.com/in/bledar-gjata-1a527030b/)
- [ORCID 0009-0003-7854-266X](https://orcid.org/0009-0003-7854-266X)

These predecessor links do not mean POC010 already has its own DOI or indexing record. Bledar Gjata directs the architecture and reports reviewing code through shell execution. AI tools assisted extraction, implementation, tests and writing. Publication and passing tests are not independent peer review or certification.


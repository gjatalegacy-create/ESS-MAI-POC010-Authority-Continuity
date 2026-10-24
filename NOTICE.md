# Attribution and working method

Copyright 2026 Bledar Gjata / Gjata Legacy. Licensed under Apache-2.0.

Author and accountable architect: Bledar Gjata, Independent Researcher, Albania; ORCID [0009-0003-7854-266X](https://orcid.org/0009-0003-7854-266X).

Four selected source blocks are retained byte-for-byte from the ESS-MAI snapshot directory v1.8.9_ess_mai_phase_015, `shadow/src/sovereign_ffi_gate.rs`. The snapshot's internal Cargo version is 1.7.8; directory and internal versions are not silently equated. Exact ranges and hashes are recorded in `provenance/source_gate.json`.

The binding adapter, scoped local transaction ledger, cross-process harness, structured output and documentation are new POC010 material. The methodology follows the predecessor's separation of authority, checking, durable storage and receipts. Hashing, compare-and-swap, file locks, transactions and idempotency are established techniques, not new inventions claimed here. No novel universal theorem or patentability claim is made.

MANU-de / Manuela Schrittwieser is credited for the public test proposal in ESS-MAI Discussion #13. She is not listed as a coauthor or independent validator of this implementation.

Bledar Gjata directs the architecture and reports reviewing code through shell execution. AI tools, including OpenAI Codex, assisted extraction, engineering, tests and documentation. This is not translation-only assistance, exclusively manual coding or independent peer review.

Predecessor DOIs are citations to their own artifacts: POC003–006 software 10.5281/zenodo.22750188; POC007 software 10.5281/zenodo.23596907 and preprint 10.5281/zenodo.23597026. POC010 is not assigned those identifiers. Public licensing applies to this capsule; private runtime code and histories are excluded. Citation and review are voluntary, with no additional license restriction.


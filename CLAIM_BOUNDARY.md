# Claim boundary

## Tested invariant

Let `A = enc(principal, operation, resource, delegated_scope, context, logical_request_id)` where each UTF-8 field is preceded by its little-endian byte length, and `H = SHA256(domain || A)`.

For an unchanged, valid local WAL and cooperating processes using this library:

`physical_commits(A) <= 1`.

The identity excludes payload so changing payload cannot silently create a second execution under the same request. Exact `A` bytes and exact payload bytes must match for a retry. A new logical request ID represents an intentionally new operation. This is declared identity, not automatic recognition of equivalent natural-language intentions.

`receipt_i = SHA256(tx_domain || previous_head || LE32(byte_length(canonical_record_i)) || canonical_record_i)`.

The frame includes action, payload, authorization trace and sequence. A native OS file lock protects replay, comparison and append as one critical section. The complete frame's successful `sync_all` precedes acknowledgement. Duplicate physical records are rejected even when rehashed; counts are physical accepted commit records, not deduplicated projections.

## Exact limits

- This is a successor integration experiment, not extraction alone. The original source gate remains generation-local. The persistent action ledger, cross-process file lock and action binding are new POC010 code.
- The fixture issuer is trusted and local. It does not authenticate arbitrary principals. Nonces are deterministic module labels; generation plus issuer-instance identifies a log event, not a remote credential.
- The side effect is the local persistent append. It is not an external API call, payment, actuator or separate uncoordinated database operation.
- Claim, local effect and receipt are one record. The after-commit crash removes caller acknowledgement, **not the receipt**. Atomic local bundling avoids the separate external effect/completion gap; it does not solve that general distributed problem.
- Actual worker process kills exercise OS process termination. Power loss, disk controller cache failures, torn writes at every instruction, distributed partitions and exhaustive schedules are not demonstrated.
- A complete corrupt or incomplete tail refuses future execution without truncation. Availability after arbitrary partial writes is not claimed; evidence is preserved for recovery outside this capsule.
- Missing storage is refused, never silently recreated. Deliberate explicit initialization of a missing file and acceptance of an older valid prefix remain outside freshness protection. An actor able to rewrite the entire store and hashes is outside the integrity threat model.
- File locks coordinate cooperating processes opening the same local file. Path replacement, uncooperative writers, network filesystem locking differences and hostile kernel/in-process code are outside the claim.
- The WAL is bounded to 64 MiB, one frame to 256 KiB, each request field to 16 KiB. Replay is linear in retained history; no latency or unlimited scale claim.
- No native knowledge verdict, candidate retrieval, KiE scientific inference, iZ generation or Shadow cognitive judgement is implemented by this focused experiment. The existing native admission order is not bypassed or relabeled.
- No C ABI/exported-pointer behavior is claimed here. The retained `repr(C)` handle and non-copy token are tested through the extracted Rust gate; POC005 remains the separate C-boundary reference.
- Structured output is new test instrumentation. It is not falsely labeled a full native LGC/Vula500 transcript.

## Adverse controls remain results

Two fresh source handles for the same binding both pass the local gate. The new ledger, not the gate, prevents a second local effect. Invalid permits still fail even when a matching receipt exists. A valid older prefix is accepted. These controls are part of the result, not failures to hide.


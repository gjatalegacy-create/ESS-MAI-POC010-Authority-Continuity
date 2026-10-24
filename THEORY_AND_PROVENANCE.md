# ESS-MAI POC-010: Authority Non-Duplication and Lawful Execution Continuity

**Author and research architecture:** Bledar Gjata / Gjata Legacy. **Scope:** a source-derived, local persistence experiment. The title names the bounded question studied; it is not a claim of complete ESS-MAI execution or a newly established universal law.

## The documented question

POC005 separates the copyable representation of a capability from the process-local state that permits one consumption. Its [theory, lines 29–34][p5-theory] limits the demonstrated property to one issued generation and the existing verdict hold. Lines 64–66 distinguish that property from preventing a fresh authorization for the same semantic action. Its [reported counterexample, lines 74–84][p5-results] retains two fresh generations that both cross local authorization, then stop at `CompatibilityHold` without a Vault write.

POC010 follows the successor experiment already described in [POC005's advancement contract][p5-advance]: bind the action and authorization evidence into one transaction identity (lines 30–34), refuse a second admission under fresh authorization (line 44), recover after commit but before response (line 46), and coordinate concurrent calls into one transaction and sink effect (line 48). Those were proposed advancement requirements. They do not establish that POC005 or the canonical source already contained POC010's persistent action ledger.

MANU-de's 24 October 2026 contribution in [Discussion #13][discussion] adds a concrete adversarial test proposal: independent authorizations for one logical action, synchronized workers, and a precisely located crash. The reviewer distinguishes local transactional persistence from separate external effects and states that she has not independently executed or validated the original Rust POCs. That contribution is an experimental proposal, not a reproduced source defect or independent validation of POC010.

## Academic basis and contribution boundary

The following Academic 1–3 texts were read in the POC007 reference capsule and compared with its later public copies. These links point to the corresponding public `theory/` files on `main`, which is mutable; the separately archived historical software is [POC007 version 0.1.2][p7-doi]. These documents supply methodological context, not execution evidence for POC010.

| Source | Actual passage and its use here |
| --- | --- |
| [Academic 1 — Foundations Used][a1], lines 5–15 | Fail-safe defaults, state machines, parent-digest/trace binding, safety and liveness, and hashing are established foundations. The stated research question concerns their composition and executable boundary. POC010 does not claim invention of capabilities, idempotency, atomic file coordination, hashing, or transaction recovery. |
| [Academic 2 — ESS-MAI Method Composition][a2], lines 5–8 and 16–23 | `MethodBinding = Formula + Location + Purpose + Inputs + Outputs + AuthorityBoundary + FailureSemantics`. Separate roles bind a law instance, promise, reason, check fulfilment and verify the chain. Here, a local source gate, exact request binding and a durable adapter have distinct responsibilities. |
| [Academic 3 — Emergent Hypotheses][a3], lines 3, 19–21 and 29–33 | The text labels its propositions as hypotheses, treats fail-closed gaps as grounds for further inquiry, and proposes parent-bound traces with cross-case substitution as a falsifier. POC010 therefore retains adverse outcomes and binds recovery to a declared action identity. It does not claim to execute the POC007 Light/ASHT/NBES/Shadow chain. |

The project-specific research contribution is the inspectable composition and falsifiable boundary within ESS-MAI's documented authority discipline. The [POC005 prior-art comparison][p5-prior] already treats capability systems, one-use authorization, reference monitors and idempotent sinks as prior or adjacent mechanisms. This capsule makes no broader novelty determination.

The historical POC005 title includes “FFI LAW-0.” In the local v189 source, `gcl-constitution/src/constitution.rs:35–40` distinguishes LAW 0, uncertainty reduction, from LIGJI 0, zero-copy/move transport. The [published FFI gate][p5-gate] also uses `LAW_3` in its comments. POC010's title avoids equating these separate rules. It does not claim to close POC004's global uncertainty-continuity gap.

## What was extracted and what is new

The predecessor public source is pinned to ESS-MAI commit `d38c2bab85e7bc9e1683da2cdfccc43e958639ad`, the POC003–006 v1.1.1 collection. The [POC005 source map][p5-map] identifies `shadow/src/sovereign_ffi_gate.rs` from the locally named v1.8.9 snapshot, whose internal packages report 1.7.8. The complete source file is 24,080 bytes and has SHA-256 `88ed03f02697ca772fb8d6bfd8575f16ce6404e720265a423ecffaa001081142`.

POC010's [extraction manifest](provenance/source_gate.json) records selected unchanged source blocks, their source/output coordinates and hashes. [src/source_gate.rs](src/source_gate.rs) retains the handle, private slot and generation-local atomic consumption mechanism. It is a selective extract, not a byte-identical copy of the entire original module. The original C exports, singleton, native verdict dependencies, lineage/commit path and original test fixtures are omitted.

The `BoundGate` request-binding wrapper, its tests, [request identity and worker orchestration](src/lib.rs), [local durable adapter](src/ledger.rs), and [process experiments](tests/process.rs) are new POC010 material. The source gate's token is consumed and dropped locally. It is not exported to another process or promoted into persistence jurisdiction. Each worker has its own gate lifetime; an authorization trace identifies the issuer instance as well as the generation, because generation numbers restart with a new gate. The source nonce is a deterministic module label, not secret authentication.

The retained baseline intentionally allows two freshly issued, correctly bound handles to pass local consumption. The shared adapter supplies the separate action-level duplicate check. A successful cached result still requires a fresh valid local authorization; possession of an action identifier alone is insufficient at the worker entry point.

## Identity and local transaction contract

The declared logical identity is the length-delimited UTF-8 encoding of:

```text
domain || principal || operation || resource || scope || context || request_id
```

The domain is `ESSMAI_POC010_ACTION_V1`; the implementation hashes this encoding with SHA-256. The logical request ID distinguishes intentional repeated operations. Equal payloads with different logical request IDs are distinct actions. For an existing logical identity, a changed payload is a conflict rather than a new permissible execution. Exact action-and-payload bytes are also bound to the local authorization.

Equivalence means equality of these declared values and exact payload bytes after JSON string decoding. It does not infer meaning from natural-language synonyms, normalize Unicode, merge unrelated requests, or prove that a caller chose an honest principal, scope or request ID. The issuer is a trusted local fixture; native delegated authorization and remote identity authentication are outside this experiment.

For one initialized store and participating processes that respect its file lock, the proposed invariant is:

```text
physical committed records for logical action A <= 1
exact authorized retry(A) -> original receipt, no new record
changed immutable material under A -> refusal, no new record
```

The observable effect is the local append itself. With the exclusive file lock held, the adapter replays the existing frames, checks the action identity, and creates one frame containing the claim, request/payload and authorization trace. The frame also contains its SHA-256 completion receipt, bound to the preceding frame head. `flush` and `sync_all` precede the successful caller response. Thus the claim, payload effect and receipt are stored together in one local transaction frame. A retry can return the original receipt from replayed persistent state.

This is an atomic transaction boundary at the adapter level, not a claim that every filesystem write is physically indivisible. An incomplete or corrupt frame is preserved and causes refusal; the adapter does not silently repair it or promise liveness from every interrupted write. File locking, stable store identity and the tested operating-system durability behavior are assumptions. A party able to replace or roll back the store, bypass the cooperating writer, or recompute an unkeyed hash chain is outside the claimed protection. The chain is an integrity/continuity record, not a signature or external monotonic witness.

The crash boundary named `effect_durable_before_ack` occurs after the frame containing both the local effect and completion receipt has been synchronized, but before the caller receives a final result. It is not a crash after an independently committed external operation but before a separately stored completion record. POC010 does not resolve that external-effect gap; such integration would require the external system's idempotency or reconciliation support.

## Constitutional boundary

The governing requirement remains **`GCL > Shadow verdict > Vault sink`**, as stated in [POC005's advancement contract, line 49][p5-advance]. The canonical [FFI commit boundary, lines 296–319][p5-gate] continues to require a committed verdict; traceable capability lineage alone does not grant a knowledge write.

POC010 does not construct a native `LawConfirmedVerdict`, invent positive/negative evidence, supply synthetic `(1, 1)` verifier results, or invoke a native KnowledgeVault admission. Its local append and receipt are fixture artifacts, not verified ESS-MAI knowledge. “Lawful” in this experiment refers to the declared local authorization and persistence contract. Native constitutional execution remains a separate integration requirement; this POC cannot be used to bypass the original hold.

[POC006's advancement contract][p6-advance] supplies a related boundary: a receipt is evidence, not a new authority, and receipt/history binding belongs after the existing transaction commits. POC010's receipt follows that limited ordering principle without claiming to materialize POC006's complete negative-knowledge lifecycle.

## Five-cycle falsification protocol

The following are predictions to test, not measured outcomes in this document. Runtime evidence must separately identify the exact POC010 source revision, commands, environment, worker exits, structured traces and final store bytes. No successful-run count is inferred from authored tests.

| Cycle | Experiment | Predicted persistent effect count |
| --- | --- | --- |
| 1 | Two synchronized workers independently issue authorization for `r1` and race against the same store. One commits; the other obtains the original receipt. | 1 |
| 2 | A fresh process receives fresh authorization and exactly retries `r1`; the store remains byte-identical. | 1 |
| 3 | Kill the worker for `r2` at `before_commit`; a new authorized process retries. The first attempt adds nothing; the retry can commit. | 2 after retry |
| 4 | Kill the worker for `r3` at `effect_durable_before_ack`; a new authorized process reconstructs the existing record and returns its receipt. | 3, unchanged by retry |
| 5 | Retry historical `r1` after the later commits, then attempt changed payload under `r1`. Preserve its original receipt and refuse the changed material. | 3 |

A duplicate physical record, replacement receipt on exact retry, unauthorized cached result, accepted payload conflict, or lost earlier receipt falsifies the corresponding local claim. Fail-closed corruption cases and the accepted rollback limitation must remain visible alongside successful evidence. Passing this finite protocol would support only the declared local boundary; it would not prove all schedules, power-loss modes, native system closure or external exactly-once behavior.

## Attribution and identifiers

The retained source and prior theory are attributed to Bledar Gjata / Gjata Legacy. POC005's [notice][p5-notice] records Apache-2.0; this capsule's package metadata also declares Apache-2.0. Preserve the applicable license and notices when redistributing. The implementation, experiments and documentation were prepared with AI assistance and require human critical review.

The predecessor [POC003–006 v1.1.1 DOI][collection-doi], [POC007 software DOI][p7-doi] and [POC007 preprint DOI][p7-paper] identify their own artifacts. None is POC010's DOI. This file does not claim independent reproduction, peer review, security assurance or production readiness.

[p5-theory]: https://github.com/gjatalegacy-create/ESS-MAI/blob/d38c2bab85e7bc9e1683da2cdfccc43e958639ad/publications/executable-prior-art/poc-005-ffi-law0-authority-nonduplication/THEORY.md
[p5-results]: https://github.com/gjatalegacy-create/ESS-MAI/blob/d38c2bab85e7bc9e1683da2cdfccc43e958639ad/publications/executable-prior-art/poc-005-ffi-law0-authority-nonduplication/RESULTS.md
[p5-advance]: https://github.com/gjatalegacy-create/ESS-MAI/blob/d38c2bab85e7bc9e1683da2cdfccc43e958639ad/publications/executable-prior-art/poc-005-ffi-law0-authority-nonduplication/FAILURE_TO_ADVANCEMENT.md
[p5-prior]: https://github.com/gjatalegacy-create/ESS-MAI/blob/d38c2bab85e7bc9e1683da2cdfccc43e958639ad/publications/executable-prior-art/poc-005-ffi-law0-authority-nonduplication/PRIOR_ART.md
[p5-map]: https://github.com/gjatalegacy-create/ESS-MAI/blob/d38c2bab85e7bc9e1683da2cdfccc43e958639ad/publications/executable-prior-art/poc-005-ffi-law0-authority-nonduplication/SOURCE_MAP.md
[p5-gate]: https://github.com/gjatalegacy-create/ESS-MAI/blob/d38c2bab85e7bc9e1683da2cdfccc43e958639ad/publications/executable-prior-art/poc-005-ffi-law0-authority-nonduplication/source_core/src/sovereign_ffi_gate.rs
[p5-notice]: https://github.com/gjatalegacy-create/ESS-MAI/blob/d38c2bab85e7bc9e1683da2cdfccc43e958639ad/publications/executable-prior-art/poc-005-ffi-law0-authority-nonduplication/NOTICE.md
[p6-advance]: https://github.com/gjatalegacy-create/ESS-MAI/blob/d38c2bab85e7bc9e1683da2cdfccc43e958639ad/publications/executable-prior-art/poc-006-living-negative-knowledge-verified-reentry/FAILURE_TO_ADVANCEMENT.md
[a1]: https://github.com/gjatalegacy-create/AIGovern-Cyber-POC007/blob/main/theory/ACADEMIC_1_FOUNDATIONS.md
[a2]: https://github.com/gjatalegacy-create/AIGovern-Cyber-POC007/blob/main/theory/ACADEMIC_2_ESS_MAI_METHOD.md
[a3]: https://github.com/gjatalegacy-create/AIGovern-Cyber-POC007/blob/main/theory/ACADEMIC_3_EMERGENT_HYPOTHESES.md
[discussion]: https://github.com/gjatalegacy-create/ESS-MAI/discussions/13
[collection-doi]: https://doi.org/10.5281/zenodo.22750188
[p7-doi]: https://doi.org/10.5281/zenodo.23596907
[p7-paper]: https://doi.org/10.5281/zenodo.23597026

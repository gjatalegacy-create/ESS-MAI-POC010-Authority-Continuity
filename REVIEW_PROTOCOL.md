# Reproduction and challenge protocol

1. Record repository commit, `rustc --version --verbose`, `cargo --version`, OS and filesystem. Verify `artifact_hashes.sha256` with `python tools/verify.py` before execution.
2. Run the locked test/build commands in README. Preserve exit codes and all output. Do not change assertions or inputs to obtain green results.
3. Run the five-cycle process test with `POC010_TEST_EVIDENCE` pointing outside the immutable source tree. Check both authorization-verified events occur before either worker crosses the barrier.
4. Check two distinct authorization IDs share one action SHA-256, yet produce one physical record and the same receipt. Distinct logical requests must remain distinct even with identical payloads.
5. For each killed worker, preserve the precise last event, the nonzero process exit, absence of a final result and the restored store. Compare WAL bytes on exact retry.
6. Try benign variations of principal, resource, context, request ID and payload. Changed payload under the same logical ID must conflict; a missing/invalid permit must not access a cached result.
7. Retain negative controls. The accepted older-prefix test defines the absence of authenticated freshness. It must not be reported as rollback protection.
8. Report the earliest mismatch, exact command/input, expected vs observed behavior and full logs in a GitHub issue. A failure is as valuable as a successful reproduction. No favorable assessment is requested.

Review of source, execution by project tools, AI-assisted review and independent human reproduction are different evidence classes. MANU-de supplied the test proposal and explicitly did not report reproducing the original Rust implementation.


# Wrong RFC 8032 Public Key in Conformance Vectors

**Date:** 2026-03-06
**Attempts before resolution:** 6+

## What happened

The Ed25519 RFC 8032 TEST 1 public key in `specs/conformance/vectors.json` was wrong. The incorrect value `...daa3f4a18446b0b8d558cd8d` was a transcription error — the correct value per RFC 8032 is `...daa62325af021a68f707511a`. Three independent implementations (ring, ed25519-dalek, PyNaCl/libsodium) all derived the correct key, but the test asserted against the wrong expected value.

## Where it went astray

1. The wrong public key was introduced during Phase 0 when hand-populating `vectors.json` and was never cross-checked against the actual RFC at that time.
2. When the test failed, the assumption was "the implementations are wrong" rather than "the test vector is wrong." This led to investigating platform bugs, switching crypto libraries, and testing multiple backends — all unnecessary.
3. The correct signature (which matched RFC 8032) was treated as a puzzling anomaly rather than as strong evidence that the implementations were correct and the expected public key was wrong.

## How it was resolved

The user pointed out that three independent implementations agreeing constitutes strong evidence of correctness. Fetching the actual RFC 8032 text confirmed the test vector was wrong.

## Prevention

- **Verify test vectors against their source at the time of creation.** For RFC vectors, fetch the RFC and compare byte-for-byte before committing.
- **When N independent implementations agree and disagree with your expected value, suspect the expected value first.** The probability of N implementations having the same bug drops exponentially with N.
- **A correct signature with a "wrong" public key is a logical contradiction in Ed25519** — the public key is embedded in the signature hash. If the signature matches, the public key used internally must be correct. This should have immediately pointed to the test vector, not the implementation.

# Complete tile headers with undersized nonzero Psot are reported Available with unspecified data length

`DEV-DFA2D383B6` · [All findings](../../README.md)

## Requirement and observed deviation

**Standard:** t808-202212.txt § A.3.3

**Required behavior:** Automatically normalize received JPIP header bins into reconstructed decoder inputs without imposing the explicit caller-planned header preparation contract.

**Observed deviation:** Complete tile headers with undersized nonzero Psot are reported Available with unspecified data length

## Reproduce this finding

Run from the repository root with its native build tools and dependencies available. Use a Python environment with DeepSpec Core installed (`deepspec_core.rfcmap.replay` must be importable).

This command selects **only this finding's recipe**, copies the checkout to a temporary directory, installs the included test files, and runs its build and test. It does not modify production files in your checkout.

```sh
python doc_deviations/run_tests.py --repo . --finding DEV-DFA2D383B6 --output rfc-results/DEV-DFA2D383B6
```

The output directory must not already exist; choose another `--output` path when repeating the test.

**Success means the deviation reproduced:** the test fails and its output contains the recorded failure below; the replay command exits 0 and reports `"status": "reproduced"`. A passing test is an `unexpected_pass`, not a reproduction.

### Exact test invocation

After applying the packaged files and build steps in the temporary checkout, the runner executes:

```sh
cargo test --locked --lib jpip::tests::contract_exact_parse_and_errors -- --exact --nocapture
```

### Test input and recorded failure

The tested header is 22 actual bytes: a 12-byte SOT with Lsot=10, tile=7, Psot=12, TPsot=0 and TNsot=1; an 8-byte COM segment with valid length and registration; and 2-byte SOD. There is no earlier parse failure. The malformed Psot is intentionally the format predicate this public parser query must reject, not a cache-entry precondition. contract_cache uses DatabinCache::new(ResourceLimits::default()), inserts one final class-2 fragment at offset zero via JpipFragment::new and DatabinCache::insert, unwraps successful acceptance, and asserts is_complete for the matching ID. All storage/count/platform bounds are comfortably satisfied, and the shared Rust borrow is valid. The fixture never mutates private cache fields or fabricates a transition supplied to the production query.

**Recorded failure** (matched by the replay runner):

```text
bytes=[ff, 90, 00, 0a, 00, 07, 00, 00, 00, 0c, 00, 01, ff, 64, 00, 06, 00, 01, 6f, 6b, ff, 93]: expected Err(MalformedInput { offset: 0, kind: InvalidTilePart }); got Ok(JpipTilePartTransitionReport { status: Available, transitions: [JpipTilePartTransition { tile: 7, tile_part: 0, packet_count: None, data_length: None, functional: false, header: [255, 144, 0, 10, 0, 7, 0, 0, 0, 12, 0, 1, 255, 100, 0, 6, 0, 1, 111, 107, 255, 147] }], needs: [], missing: [] })
```

### Cited test code

These are the fixture and checks cited for this finding, not other tests to run.

[`src/jpip_transition_contract_tests.rs:L69-L74`](files/src/jpip_transition_contract_tests.rs#L69-L74)

```text
fn contract_cache(bytes: &[u8], stream: u64, tile: u16) -> DatabinCache {
    let mut cache = DatabinCache::new(ResourceLimits::default());
    contract_insert(&mut cache, 2, stream, tile, 0, bytes, true);
    assert!(cache.is_complete(DatabinId::new(2, stream, u64::from(tile)).unwrap()));
    cache
}
```

[`src/jpip_transition_contract_tests.rs:L257-L267`](files/src/jpip_transition_contract_tests.rs#L257-L267)

```text
    for declared in [1, 12, 13, valid.len() as u32 - 1] {
        for prefix in [Vec::new(), valid.clone()] {
            let (index, total) = if prefix.is_empty() { (0, 1) } else { (1, 2) };
            let mut undersized = contract_part(7, index, total, Some(0), false, false).header;
            undersized[6..10].copy_from_slice(&declared.to_be_bytes());
            let offset = prefix.len() as u64;
            let mut bytes = prefix;
            bytes.extend(undersized);
            failures.push((bytes, offset, MalformedKind::InvalidTilePart));
        }
    }
```

[`src/jpip_transition_contract_tests.rs:L269-L278`](files/src/jpip_transition_contract_tests.rs#L269-L278)

```text
    for (bytes, offset, kind) in failures {
        let cache = contract_cache(&bytes, 77, 7);
        eprintln!("complete malformed bytes={bytes:02x?}; expected {kind:?} at {offset}");
        let expected = Err(DecodeError::MalformedInput { offset, kind });
        let actual = cache.tile_part_transitions(77, 7);
        if actual != expected {
            errors.push(format!("bytes={bytes:02x?}: expected {expected:?}; got {actual:?}"));
        }
    }
    assert!(errors.is_empty(), "{}", errors.join("\n"));
```

<details>
<summary>Build steps and packaged files</summary>

The runner applies these files **before** the build steps. Helper scripts and patches are setup dependencies, not separate reproductions.

- [`src/jpip_transition_contract_tests.rs`](files/src/jpip_transition_contract_tests.rs)
- [`validation/build-jpip-transition-contract-tests.py`](files/validation/build-jpip-transition-contract-tests.py)

**Build commands** (in order):

```sh
python3 validation/build-jpip-transition-contract-tests.py
```

[Machine-readable recipe](recipe.json)

</details>

<details>
<summary>Why this is a deviation and supporting evidence</summary>

The failed check is the final assert!(errors.is_empty(), "{}", errors.join("\n")) in contract_exact_parse_and_errors, which accumulates exact Result mismatches. The beginning of the host output tail is truncated, so I do not identify an unseen first mismatch; the first fully visible mismatch is a single 22-byte SOT/COM/SOD header for tile 7 with Psot=12: expected MalformedInput { offset: 0, kind: InvalidTilePart }, but the query returned Available with data_length=None. This directly contradicts clause ecde50e8b39ea19cc97ae9a9's explicit nonzero-Psot-underflow rejection and zero-Psot/None distinction. Entry realism: each failing fixture stores just 22 or 44 bytes in one complete bin using one public fragment insertion, with one or two headers, no recursion and no large allocation. Default HARDENED limits allow 2,147,483,648 input bytes, 1,048,576 fragments and 16,384 tile parts. Large Psot values elsewhere in this test are scalar declarations, not requests to allocate payloads; the tested operation reads header-only databins. Existing native callers/tests likewise construct complete class-2 header bins using fragment/insert and directly call this public query. No owning caller, main header, decoded image or payload must precede this independent unit-contract entry. Fragment construction checks ranges/resource limits, not codestream syntax; accepting malformed network bytes into the cache is admissible and their rejection belongs to parsing. Route confirmed by source, describe_declaration and Rust MIR CFG: contract_cache creates the default cache, inserts class 2 / stream 77 / tile 7 at offset zero with final=true and asserts completion; the direct query takes the complete branch, snapshots databin_view, and calls parse_tile_part_transitions. SOT length, tile identity and COM framing succeed; SOD establishes H=22. For nonzero L=12, declared.checked_sub(header_length) yields None, and map preserves None without producing DecodeError. The transition is pushed and returned; the query's index>0 condition is false for its singleton, so needs remains empty and status becomes Available. The 44-byte cases additionally demonstrate the same error after a valid first transition, at offset 22. This is not a harness setup failure or an oracle based on an unrelated exit code. clause_origin is infer, not rfc_map. The source documentation expressly says None denotes the legal final zero-Psot form, supporting the intended distinction independently of the failing implementation. The precise InvalidTilePart enum remains the review-marked inferred structural-error assumption (also used for wrong tile/SOT and nonfinal zero Psot); no independently quoted standard mandates that enum. Even under that uncertainty, returning successful None for a nonzero impossible length is the demonstrated deviation, not grounds to revise the clause merely to match the code. No production changes or new replay were needed: observation is the supplied host replay, not a local run.

[Standards quote, production lines, and provenance](../../evidence.md#dev-dfa2d383b6)

</details>

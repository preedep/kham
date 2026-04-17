Optimize the performance of: $ARGUMENTS

Follow this strict workflow:

1. **Measure first**: Run `cargo bench -p kham-core -- --save-baseline before` to capture the current baseline. If no benchmark exists for this code path, create one first using criterion.

2. **Profile**: Use `cargo flamegraph` or add timing instrumentation to identify the actual bottleneck. Report what you found — which function, what % of time.

3. **Propose**: Explain 2-3 optimization strategies ranked by expected impact. For each strategy state: what it changes, expected speedup, trade-offs (complexity, memory, readability).

4. **Implement**: Apply only the highest-impact change. ONE change at a time.

5. **Verify**: Run `cargo bench -p kham-core -- --baseline before` and report the exact improvement (% faster, absolute time reduction). Also run `cargo test -p kham-core` to ensure correctness.

6. **If no improvement**: Revert and try the next strategy. Never ship an optimization that doesn't measurably help.

Key constraints for kham-core:
- Must remain `no_std` compatible — no std::fs, std::io in library code
- Zero-copy: prefer `&str` over `String` allocation
- Use `include_bytes!` for embedded data, NOT runtime file I/O
- Consider: pre-compiled binary trie, Aho-Corasick, arena allocation, memchr
- Comment WHY each optimization exists in the code

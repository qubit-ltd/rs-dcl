# Predicate dispatch benchmark verification

## Environment

- Project: `qubit-dcl` 0.13.5, baseline commit `8913f6e1d65f82a51d68250f2e924bf9f7ef0052`.
- Rust: `rustc 1.94.0 (4a4ef493e 2026-03-02)`, LLVM 21.1.8; Cargo 1.94.0.
- Target: `x86_64-unknown-linux-gnu`.
- CPU: Intel Xeon E5-2680 v4 @ 2.40 GHz; 2 sockets, 14 cores/socket, SMT enabled.
- Benchmark profile: Cargo's default `bench` profile (`opt-level = 3`). The project has no explicit `[profile.*]` configuration or LTO setting; Cargo's default bench profile has LTO disabled. No CPU pinning or frequency control was applied.
- Locked benchmark dependencies include Criterion 0.8.2.
- Criterion used its defaults: 3 seconds of warm-up, 100 samples, and an estimated 5 seconds of measurement per case. No custom sampling settings were passed.

## Commands and generated code

The original measurements were compiled and run before adding the opaque-input group:

```text
cargo rustc --locked --release --bench dcl_bench --features parking-lot -- --emit=asm
cargo bench --locked --features parking-lot --bench dcl_bench -- dcl_cost_breakdown
```

The original build emitted assembly at `target/release/deps/dcl_bench-e655b5d303016ec2.s`; that artifact was later overwritten by the updated build and was not preserved separately. The original group constructs all three closures locally and applies `black_box` only to each returned `bool`. The assembly contains indirect calls in Criterion and standard-library code, but the generated closure callbacks were not separable enough to attribute a particular indirect call to `predicate/running/arc_dynamic` rather than to callback dispatch or cleanup. Therefore the original assembly alone did not establish whether the predicate call itself remained indirect.

An input-opaque comparison group was added while retaining all original entries. Its dynamic case passes `dynamic.as_ref()` through `black_box` before invocation; direct and statically typed `Arc` cases use corresponding input barriers. All cases retain the output barrier. Input barriers have their own cost, so differences from the original group cannot be attributed solely to dispatch.

The updated code was compiled using the actual bench profile:

```text
cargo rustc --locked --profile bench --bench dcl_bench --features parking-lot -- --emit=asm
cargo rustc --locked --profile bench --bench dcl_bench --features parking-lot -- --emit=llvm-ir
```

The emitted assembly is `target/release/deps/dcl_bench-e655b5d303016ec2.s`; the LLVM IR is `target/release/deps/dcl_bench-54f93b5a352667c2.ll`. The IR shows callback invocation through loaded function pointers in Criterion's `Bencher::iter`, and the assembly contains indirect calls in those callback paths. The optimized output remains difficult to map unambiguously from each opaque-input benchmark entry to one specific indirect instruction. The generated-code evidence therefore supports using the opaque comparison as a more defensible control, but does not justify claiming a precisely isolated instruction cost.

## Measurements

Original output-only group, `cargo bench --locked --features parking-lot --bench dcl_bench -- dcl_cost_breakdown`:

| State | Direct closure | `Arc` static | `Arc` dynamic |
| --- | ---: | ---: | ---: |
| running | 720.47–769.25 ps (median 740.24 ps) | 1.0389–1.1714 ns (median 1.1036 ns) | 1.7518–1.7683 ns (median 1.7592 ns) |
| shut down | 716.65–733.17 ps (median 724.22 ps) | 895.31–978.12 ps (median 933.85 ps) | 1.7587–1.7687 ns (median 1.7633 ns) |

Updated group from the same command, opaque-input cases:

| State | Direct closure | `Arc` static | `Arc` dynamic |
| --- | ---: | ---: | ---: |
| running | 1.5300–1.6117 ns (median 1.5675 ns) | 1.4637–1.4738 ns (median 1.4683 ns) | 2.2232–2.2426 ns (median 2.2316 ns) |
| shut down | 1.5685–1.7454 ns (median 1.6584 ns) | 1.4814–1.5384 ns (median 1.5070 ns) | 2.8248–3.1382 ns (median 2.9787 ns) |

Criterion reported substantial outliers in several cases (up to 18%) and the CPU was not frequency-pinned. These results are descriptive only; they are not a production performance claim and should not be used to infer the standalone cost of dynamic dispatch.

## Conclusion

The suspected devirtualization is **not confirmed**. The original measurements show a consistent separation between dynamic and static cases, but neither timings nor the available assembly mapping prove which operations account for the difference. The added opaque-input cases provide a clearer comparison and show a similar separation, while their input-barrier cost and remaining generated-code attribution limits prevent a precise dispatch-only conclusion. No production executor or public API change is indicated.

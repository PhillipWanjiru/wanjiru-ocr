# include/

Will hold `wanjiru.h`, the C header of the stable ABI. ADR-001: the C ABI, owned by `wanjiru-ffi`, is the only
binary contract; every other crate is a source-level interface and may change between versions.

`wanjiru-ffi` is implemented in Phase 8 (`crates/wanjiru-ffi/CONTRACT.md`). Its contract already names three of
the functions: `wnj_engine_new`, `wnj_process` and `wnj_result_free`.

**The header and the exported symbols have to match exactly**, and a CI test fails on any difference (the same
contract). That test arrives with the crate. No header is committed before the functions it declares exist, so
there is nothing in this folder that can drift from the code.

Conventions fixed by ADR-001: opaque handles, explicit ownership, `wnj_` function names and `WNJ_` constants,
versioned structs, a panic barrier on every entry point, and failures that cross as `WNJ-E…` codes
(`docs/errors.md`) plus a message. No Rust type crosses the boundary.

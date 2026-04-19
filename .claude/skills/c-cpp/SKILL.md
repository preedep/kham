---
name: c-cpp
description: Write, review, and debug C and C++ code in this project. Use when authoring PostgreSQL C extension shims (shim.c), writing build.rs cc::Build configurations, debugging header include order, fixing macro expansion issues, or reviewing FFI boundary code between Rust and C.
metadata:
  domain: systems
  triggers: shim.c, C extension, C FFI, cc::Build, #include, gcc warning, clang error, macro expansion, PG_FUNCTION_INFO_V1, varatt.h, palloc, pfree, ereport, C ABI, missing magic block, trampoline
  role: specialist
---

# c-cpp — C/C++ Code in kham

Specialist for the C code in this project — primarily `kham-pg/src/shim.c`, the PostgreSQL fmgr bridge that connects Rust parser callbacks to PostgreSQL's function manager.

## Role of C in this project

`kham-pg` is a Rust `cdylib` that also compiles a small C shim (`src/shim.c`).  
The C shim handles PG macro boilerplate and the magic data; Rust handles all logic **and all exported symbols**.

```
PostgreSQL fmgr  ──▶  lib.rs #[no_mangle] trampolines  ──▶  shim.c *_shim() (C)  ──▶  *_impl() (Rust)
                       Pg_magic_func, kham_start, …          PG_GETARG_*, PG_RETURN_*
                       pg_finfo_* (defined inline in Rust)    palloc / pfree / pstrdup
```

**Key rule**: All symbols that PostgreSQL resolves via `dlsym` (parser callbacks, magic func, finfo records) must be defined as `#[no_mangle] pub extern "C"` in `lib.rs`. Rust cdylib linker version scripts hide C symbols by default — there is no reliable linker-flag workaround.

## Trampoline Pattern (current architecture)

C functions are renamed to `*_shim` and called from Rust trampolines:

**shim.c** — C implementations with no PG_MODULE_MAGIC / PG_FUNCTION_INFO_V1:
```c
#include "postgres.h"
#include "fmgr.h"
#include "tsearch/ts_public.h"
#include "utils/palloc.h"

/* Magic data — compiled against target PG headers for correct version fields */
const Pg_magic_struct *kham_pg_magic_impl(void) {
    static const Pg_magic_struct d = PG_MODULE_MAGIC_DATA;
    return &d;
}

extern void *kham_start_impl(const char *text, int len);   // defined in lib.rs
extern int   kham_gettoken_impl(void *state, const char **token, int *tokenlen);
extern void  kham_end_impl(void *state);

Datum kham_start_shim(PG_FUNCTION_ARGS) {
    const char *input = (const char *) PG_GETARG_POINTER(0);
    int len = PG_GETARG_INT32(1);
    void *state = kham_start_impl(input, len);
    if (!state) ereport(ERROR, (errcode(ERRCODE_INTERNAL_ERROR),
                                errmsg("kham: parser init failed")));
    PG_RETURN_POINTER(state);
}
Datum kham_gettoken_shim(PG_FUNCTION_ARGS) { /* ... PG_RETURN_INT32(type) ... */ }
Datum kham_end_shim(PG_FUNCTION_ARGS)      { /* ... PG_RETURN_VOID() ... */ }
Datum kham_lextypes_shim(PG_FUNCTION_ARGS) { /* ... PG_RETURN_POINTER(list) ... */ }
```

**lib.rs** — Rust trampolines that are guaranteed exported:
```rust
type Datum = usize;
type Fcinfo = *mut c_void;

extern "C" {
    fn kham_pg_magic_impl() -> *const c_void;
    fn kham_start_shim(fcinfo: Fcinfo) -> Datum;
    fn kham_gettoken_shim(fcinfo: Fcinfo) -> Datum;
    fn kham_end_shim(fcinfo: Fcinfo) -> Datum;
    fn kham_lextypes_shim(fcinfo: Fcinfo) -> Datum;
}

#[repr(C)]
struct PgFinfoRecord { api_version: c_int }
static FINFO_V1: PgFinfoRecord = PgFinfoRecord { api_version: 1 };

#[no_mangle] pub unsafe extern "C" fn Pg_magic_func() -> *const c_void { kham_pg_magic_impl() }
#[no_mangle] pub unsafe extern "C" fn kham_start(f: Fcinfo)    -> Datum { kham_start_shim(f) }
#[no_mangle] pub unsafe extern "C" fn kham_gettoken(f: Fcinfo) -> Datum { kham_gettoken_shim(f) }
#[no_mangle] pub unsafe extern "C" fn kham_end(f: Fcinfo)      -> Datum { kham_end_shim(f) }
#[no_mangle] pub unsafe extern "C" fn kham_lextypes(f: Fcinfo) -> Datum { kham_lextypes_shim(f) }
#[no_mangle] pub extern "C" fn pg_finfo_kham_start()    -> *const PgFinfoRecord { &FINFO_V1 }
#[no_mangle] pub extern "C" fn pg_finfo_kham_gettoken() -> *const PgFinfoRecord { &FINFO_V1 }
#[no_mangle] pub extern "C" fn pg_finfo_kham_end()      -> *const PgFinfoRecord { &FINFO_V1 }
#[no_mangle] pub extern "C" fn pg_finfo_kham_lextypes() -> *const PgFinfoRecord { &FINFO_V1 }
```

## Why NOT PG_MODULE_MAGIC / PG_FUNCTION_INFO_V1 in C

`PG_MODULE_MAGIC` defines `Pg_magic_func()` and `PG_FUNCTION_INFO_V1(name)` defines `pg_finfo_name()` — both marked `PGDLLEXPORT`. On Linux, Rust's cdylib linker generates an anonymous `--version-script` with `local: *` that hides all C symbols. Approaches that do NOT work:
- `--export-dynamic`: only exports already-global `T` symbols; version script makes them `t` first
- `--undefined=Pg_magic_func`: keeps symbol from GC but doesn't promote from `t` to dynamic table
- `-fvisibility=default` on cc::Build: overridden by Rust's version script
- Second `--version-script`: two anonymous tags conflict (`anonymous version tag cannot be combined with other version tags`)
- `--dynamic-list`: cannot un-hide symbols already made `t` by version script

**Only solution**: define the symbols in Rust with `#[no_mangle]`.

## SQL Return Types (PG17)

All text search parser callbacks must declare `RETURNS internal` in SQL — including `gettoken`:

```sql
CREATE FUNCTION kham_start(internal, int4)     RETURNS internal ...
CREATE FUNCTION kham_gettoken(internal, internal, internal) RETURNS internal ...  -- NOT int4
CREATE FUNCTION kham_end(internal)             RETURNS void ...
CREATE FUNCTION kham_lextypes(internal)        RETURNS internal ...
```

PG17 validates return types when `CREATE TEXT SEARCH PARSER` runs. The C code still uses
`PG_RETURN_INT32(type)` for gettoken — PostgreSQL reads it with `DatumGetInt32` internally
regardless of the declared SQL type.

## Build Integration (cc crate)

```rust
cc::Build::new()
    .file("src/shim.c")
    .include(&includedir_server)   // pg_config --includedir-server
    .include(&includedir)          // pg_config --includedir
    .flag("-Wno-unused-parameter")
    .flag("-Wno-declaration-after-statement")
    .flag("-Wno-missing-field-initializers")
    .compile("kham_pg_shim");
// No extra linker flags needed — PG symbols are all #[no_mangle] Rust functions
```

## Required PostgreSQL Headers

Always include in this order:
```c
#include "postgres.h"           // must be first
#include "fmgr.h"               // PG_GETARG_*, PG_RETURN_*, Datum
#include "tsearch/ts_public.h"  // LexDescr
#include "utils/palloc.h"       // palloc, pfree, pstrdup
```

`varatt.h` (VARDATA_ANY / VARSIZE_ANY_EXHDR) only needed if reading varlena args — not used since `kham_start` receives raw `char*` + `int4`.

## startfunc Argument Convention

`kham_start` receives `(PointerGetDatum(buf), Int32GetDatum(len))` — a raw `char*` and byte count, **not** a varlena `text*`:
```c
const char *input = (const char *) PG_GETARG_POINTER(0);  // NOT PG_GETARG_TEXT_PP
int len = PG_GETARG_INT32(1);
```

## PostgreSQL Macro Reference

```c
PG_GETARG_POINTER(n)   // raw pointer argument
PG_GETARG_INT32(n)     // int32 argument
PG_RETURN_POINTER(p)   // return pointer as Datum
PG_RETURN_INT32(n)     // return int32 as Datum
PG_RETURN_VOID()       // return nothing

palloc(size)           // alloc in current memory context
pstrdup("str")         // palloc-copy of C string
ereport(ERROR, ...)    // longjmp — never returns
```

## LexDescr Array Rules

- Must be `palloc`'d (not stack-allocated)
- Terminated by `{ lexid = 0, alias = NULL, descr = NULL }`
- `alias` and `descr` must be `pstrdup`'d
- `lexid` values must match integers returned by `kham_gettoken_impl`

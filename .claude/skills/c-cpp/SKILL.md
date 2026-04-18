---
name: c-cpp
description: Write, review, and debug C and C++ code in this project. Use when authoring PostgreSQL C extension shims (shim.c), writing build.rs cc::Build configurations, debugging header include order, fixing macro expansion issues, or reviewing FFI boundary code between Rust and C.
metadata:
  domain: systems
  triggers: shim.c, C extension, C FFI, cc::Build, #include, gcc warning, clang error, macro expansion, PG_FUNCTION_INFO_V1, varatt.h, palloc, pfree, ereport, C ABI
  role: specialist
---

# c-cpp — C/C++ Code in kham

Specialist for the C code in this project — primarily `kham-pg/src/shim.c`, the PostgreSQL fmgr bridge that connects Rust parser callbacks to PostgreSQL's function manager.

## Role of C in this project

`kham-pg` is a Rust `cdylib` that also compiles a small C shim (`src/shim.c`).  
The C shim handles all PostgreSQL macro boilerplate; Rust handles all logic.

```
PostgreSQL fmgr  ──▶  shim.c (C)  ──▶  kham_*_impl() (Rust)
                       PG_FUNCTION_INFO_V1
                       PG_GETARG_*, PG_RETURN_*
                       palloc / pfree / pstrdup
                       PG_MODULE_MAGIC
```

Rust `unsafe` is confined to `src/lib.rs` (FFI boundary). `shim.c` is the only C file.

## Build Integration (cc crate)

`build.rs` compiles `shim.c` using the `cc` crate:

```rust
cc::Build::new()
    .file("src/shim.c")
    .include(&includedir_server)   // pg_config --includedir-server
    .include(&includedir)          // pg_config --includedir
    .flag("-Wno-unused-parameter")
    .flag("-Wno-declaration-after-statement")
    .flag("-Wno-missing-field-initializers")
    .compile("kham_pg_shim");
```

The resulting `.a` is linked into the final `libkham_pg.so` by Cargo.

## Required PostgreSQL Headers

Always include in this order:

```c
#include "postgres.h"    // must be first — defines bool, Datum, etc.
#include "varatt.h"      // VARDATA_ANY, VARSIZE_ANY_EXHDR — NOT included by postgres.h
#include "fmgr.h"        // PG_FUNCTION_INFO_V1, PG_GETARG_*, PG_RETURN_*, Datum
#include "tsearch/ts_public.h"  // LexDescr (for lextypes callback)
#include "utils/palloc.h"       // palloc, palloc0, pfree, pstrdup
```

`varatt.h` is the #1 gotcha: `VARDATA_ANY` / `VARSIZE_ANY_EXHDR` are defined there, but `postgres.h` does **not** pull it in automatically. Omitting it produces a clang error:
```
error: call to undeclared function 'VARDATA_ANY'
```

## PostgreSQL Macro Reference

### Module magic (required once per extension)
```c
PG_MODULE_MAGIC;   // declares Pg_magic_func() — version/ABI check at load time
```

### Function registration
```c
PG_FUNCTION_INFO_V1(my_func);   // declares pg_finfo_my_func() — tells PG this is a V1 function
Datum my_func(PG_FUNCTION_ARGS) { ... }
```

### Getting arguments
```c
text *t   = PG_GETARG_TEXT_PP(0);   // varlena text* (detoasted if needed)
void *ptr = PG_GETARG_POINTER(0);   // raw pointer (Datum cast)
int32 n   = PG_GETARG_INT32(0);
```

### Accessing varlena text data
```c
char *data = VARDATA_ANY(t);             // pointer to UTF-8 bytes (not null-terminated)
int   len  = (int) VARSIZE_ANY_EXHDR(t); // byte count, header excluded
```

### Return values
```c
PG_RETURN_POINTER(ptr);   // return *mut c_void as Datum
PG_RETURN_INT32(n);        // return i32 as Datum
PG_RETURN_VOID();          // return nothing (void functions)
```

### Memory management
```c
void *p  = palloc(size);     // allocate in current memory context (freed by PG)
void *p0 = palloc0(size);    // zero-initialised
char *s  = pstrdup("hello"); // palloc-copy of a C string
pfree(p);                    // explicit free (rarely needed — memory context handles it)
```

### Error reporting
```c
ereport(ERROR,
    (errcode(ERRCODE_INTERNAL_ERROR),
     errmsg("kham: %s", "something went wrong")));
// ereport(ERROR, ...) does a longjmp — never returns
```

## Parser Callback Pattern

The text search parser callbacks follow this pattern:

```c
/* startfunc — one call per document */
PG_FUNCTION_INFO_V1(kham_start);
Datum kham_start(PG_FUNCTION_ARGS)
{
    text *input = PG_GETARG_TEXT_PP(0);
    // VARDATA_ANY gives non-null-terminated bytes; VARSIZE_ANY_EXHDR gives length
    void *state = kham_start_impl(VARDATA_ANY(input),
                                  (int) VARSIZE_ANY_EXHDR(input));
    if (state == NULL)
        ereport(ERROR, (errcode(ERRCODE_INTERNAL_ERROR),
                        errmsg("kham: parser initialisation failed")));
    PG_RETURN_POINTER(state);
}

/* gettoken — called in a loop until it returns 0 */
PG_FUNCTION_INFO_V1(kham_gettoken);
Datum kham_gettoken(PG_FUNCTION_ARGS)
{
    void   *state    = PG_GETARG_POINTER(0);
    char  **token    = (char **) PG_GETARG_POINTER(1);   // output: token text
    int    *tokenlen = (int *)   PG_GETARG_POINTER(2);   // output: token length
    int     type     = kham_gettoken_impl(state, (const char **) token, tokenlen);
    PG_RETURN_INT32(type);   // 0 = end of document
}

/* endfunc — free parser state */
PG_FUNCTION_INFO_V1(kham_end);
Datum kham_end(PG_FUNCTION_ARGS)
{
    kham_end_impl(PG_GETARG_POINTER(0));
    PG_RETURN_VOID();
}

/* lextypes — return palloc'd LexDescr array, terminated by { 0, NULL, NULL } */
PG_FUNCTION_INFO_V1(kham_lextypes);
Datum kham_lextypes(PG_FUNCTION_ARGS)
{
    LexDescr *list = (LexDescr *) palloc(N * sizeof(LexDescr));
    list[0].lexid = 1; list[0].alias = pstrdup("thai"); list[0].descr = pstrdup("Thai word");
    /* ... */
    list[N-1].lexid = 0; list[N-1].alias = NULL; list[N-1].descr = NULL; /* sentinel */
    PG_RETURN_POINTER(list);
}
```

## Rust ↔ C ABI Contract

The Rust `*_impl` functions are declared in `shim.c` and defined in `lib.rs`:

```c
// shim.c declarations
extern void *kham_start_impl(const char *text, int len);
extern int   kham_gettoken_impl(void *state, const char **token, int *tokenlen);
extern void  kham_end_impl(void *state);
```

```rust
// lib.rs definitions
#[no_mangle]
pub unsafe extern "C" fn kham_start_impl(text: *const c_char, len: c_int) -> *mut c_void { ... }
```

Rules:
- Rust types must match C types exactly: `c_char`, `c_int`, `c_void` from `std::os::raw`
- All `*_impl` functions must be `#[no_mangle] pub extern "C"`
- Wrap Rust logic in `std::panic::catch_unwind` — panics across FFI boundary are UB
- Return `NULL` / `0` on error; let the C shim call `ereport(ERROR, ...)`

## Common Compiler Warnings to Suppress

These are legitimate in PG extension code; suppress with `-W` flags in `build.rs`:

| Warning flag                         | Why it fires in PG code                         |
|--------------------------------------|-------------------------------------------------|
| `-Wno-unused-parameter`              | `PG_FUNCTION_ARGS` often unused in simple funcs |
| `-Wno-declaration-after-statement`   | PG macros mix declarations and statements        |
| `-Wno-missing-field-initializers`    | PG structs have many fields; partial init is OK  |

## LexDescr Array Rules

- Must be `palloc`'d (not stack-allocated — PG holds the pointer after return)
- Terminated by `{ lexid = 0, alias = NULL, descr = NULL }`
- `alias` and `descr` must be `pstrdup`'d (PG may free/use them later)
- `tokid` values must exactly match the integers returned by `kham_gettoken_impl`

# DEPENDENCIES.md — karis-ky Escrow Dependency Audit Trail

> **Issue:** #250  
> **Review cadence:** Quarterly (next review: 2026-10-24)  
> **Last updated:** 2026-07-24  
> **Lockfile:** `Cargo.lock` (committed; reviewed on every dependency change)

All versions are pinned in `Cargo.lock`. For the dependency update policy, emergency
advisory bump workflow, and lockfile review process see
[`docs/escrow-dependency-policy.md`](docs/escrow-dependency-policy.md).

---

## Dependabot grouping strategy

Dependabot is configured in [`.github/dependabot.yml`](.github/dependabot.yml). To keep
update PRs manageable, minor and patch updates are **grouped per ecosystem**:

| Ecosystem | Group | Update types grouped | Rationale |
|-----------|-------|----------------------|-----------|
| `cargo` | `cargo-minor-patch` | `minor`, `patch` | Batches routine crate bumps into a single PR instead of one PR per crate. |
| `npm` | `npm-minor-patch` | `minor`, `patch` | Batches routine JS tooling bumps into a single PR. |

Rules:

- **Minor and patch updates** for `cargo` and `npm` are collected into one grouped PR
  per ecosystem, reducing PR noise.
- **Major-version updates** are *not* grouped — each major bump arrives as its own
  individual PR so it can be reviewed, tested, and merged in isolation.
- **Auto-merge is not enabled.** Every grouped or individual PR requires manual review
  and must pass CI before merge.
- Grouped PRs are still subject to the lockfile review process described in
  [`docs/escrow-dependency-policy.md`](docs/escrow-dependency-policy.md); review the
  contained version bumps individually within the grouped PR.

---
=======

## Risk legend

| Symbol | Meaning |
|--------|---------|
| ✅ | Actively maintained, no known advisories |
| ⚠️ | Monitor — minor concern noted inline |
| 🔴 | Unmaintained or high-risk — action required |

---

## Direct dependencies

These are the crates declared in `escrow/Cargo.toml`.

| Crate | Version | License | Maintenance | Notes |
|-------|---------|---------|-------------|-------|
| `soroban-sdk` | 25.2.0 | Apache-2.0 | ✅ Stellar Development Foundation | Core contract SDK. Pin to Soroban protocol major. |
| `proptest` | 1.10.0 | MIT OR Apache-2.0 | ✅ Active | Dev-only property-based testing. Test binary only — not in WASM. |


## Transitive dependencies — Stellar / Soroban ecosystem

All pulled in by `soroban-sdk`. Versioned together as a Soroban protocol 25 family.

| Crate | Version | License | Maintenance | Notes |
|-------|---------|---------|-------------|-------|
| `soroban-env-host` | 25.0.1 | Apache-2.0 | ✅ Stellar Development Foundation | Contract execution host; same release train as SDK. |
| `soroban-env-common` | 25.0.1 | Apache-2.0 | ✅ Stellar Development Foundation | Shared ABI types between guest and host. |
| `soroban-env-guest` | 25.0.1 | Apache-2.0 | ✅ Stellar Development Foundation | Guest-side ABI shims compiled into WASM. |
| `soroban-env-macros` | 25.0.1 | Apache-2.0 | ✅ Stellar Development Foundation | Proc-macros for env interface generation. |
| `soroban-builtin-sdk-macros` | 25.0.1 | Apache-2.0 | ✅ Stellar Development Foundation | Macros for built-in contract types. |
| `soroban-sdk-macros` | 25.3.0 | Apache-2.0 | ✅ Stellar Development Foundation | `#[contract]`, `#[contractimpl]`, etc. |
| `soroban-spec` | 25.3.0 | Apache-2.0 | ✅ Stellar Development Foundation | Contract spec generation (XDR-backed). |
| `soroban-spec-rust` | 25.3.0 | Apache-2.0 | ✅ Stellar Development Foundation | Rust binding generation from contract specs. |
| `soroban-ledger-snapshot` | 25.3.0 | Apache-2.0 | ✅ Stellar Development Foundation | Test-only ledger snapshot helpers. |
| `soroban-wasmi` | 0.31.1-soroban.20.0.1 | MIT OR Apache-2.0 | ✅ Stellar fork of wasmi | WASM interpreter used by the host. |
| `stellar-xdr` | 25.0.0 | Apache-2.0 | ✅ Stellar Development Foundation | XDR encode/decode for all Stellar types. |
| `stellar-strkey` | 0.0.13 / 0.0.16 | Apache-2.0 | ✅ Stellar Development Foundation | StrKey encode/decode (G…/S… addresses). Two versions coexist transitively. |


## Transitive dependencies — Cryptography

Used by `soroban-env-host` for signature verification and curve operations inside the Soroban host.
These crates do not execute in the contract guest WASM; they run in the test host environment.

| Crate | Version | License | Maintenance | Notes |
|-------|---------|---------|-------------|-------|
| `ed25519-dalek` | 2.2.0 | MIT OR Apache-2.0 | ✅ dalek-cryptography | Ed25519 sign/verify. Current major (v2). |
| `curve25519-dalek` | 4.1.3 | MIT OR Apache-2.0 | ✅ dalek-cryptography | Underlying curve arithmetic for Ed25519. Current major (v4). |
| `curve25519-dalek-derive` | 0.1.1 | MIT OR Apache-2.0 | ✅ dalek-cryptography | Proc-macro helpers for dalek types. |
| `k256` | 0.13.4 | MIT OR Apache-2.0 | ✅ RustCrypto | secp256k1 (Koblitz curve). Used by host for ECDSA. |
| `p256` | 0.13.2 | MIT OR Apache-2.0 | ✅ RustCrypto | NIST P-256 ECDSA. Used by host. |
| `ecdsa` | 0.16.9 | MIT OR Apache-2.0 | ✅ RustCrypto | Generic ECDSA over `elliptic-curve`. |
| `elliptic-curve` | 0.13.8 | MIT OR Apache-2.0 | ✅ RustCrypto | Core elliptic-curve traits. |
| `sha2` | 0.10.9 | MIT OR Apache-2.0 | ✅ RustCrypto | SHA-256 / SHA-512. |
| `sha3` | 0.10.8 | MIT OR Apache-2.0 | ✅ RustCrypto | SHA-3 / Keccak. |
| `digest` | 0.10.7 | MIT OR Apache-2.0 | ✅ RustCrypto | Digest trait abstraction. |
| `hmac` | 0.12.1 | MIT OR Apache-2.0 | ✅ RustCrypto | HMAC-SHA2. |
| `rfc6979` | 0.4.0 | MIT OR Apache-2.0 | ✅ RustCrypto | Deterministic ECDSA nonce (RFC 6979). |
| `signature` | 2.2.0 | MIT OR Apache-2.0 | ✅ RustCrypto | Generic signature traits. |
| `ed25519` | 2.2.3 | MIT OR Apache-2.0 | ✅ RustCrypto | Ed25519 trait; complements `ed25519-dalek`. |
| `pkcs8` | 0.10.2 | MIT OR Apache-2.0 | ✅ RustCrypto | PKCS#8 key encoding. |
| `der` | 0.7.10 | MIT OR Apache-2.0 | ✅ RustCrypto | ASN.1 DER encoding. |
| `sec1` | 0.7.3 | MIT OR Apache-2.0 | ✅ RustCrypto | SEC1 point encoding for elliptic curves. |
| `spki` | 0.7.3 | MIT OR Apache-2.0 | ✅ RustCrypto | SubjectPublicKeyInfo (X.509). |
| `crypto-bigint` | 0.5.5 | MIT OR Apache-2.0 | ✅ RustCrypto | Constant-time big-integer arithmetic. |
| `subtle` | 2.6.1 | BSD-3-Clause | ✅ dalek-cryptography | Constant-time comparison primitives. |
| `zeroize` | 1.8.2 | MIT OR Apache-2.0 | ✅ RustCrypto | Secure memory zeroing for key material. |
| `zeroize_derive` | 1.4.3 | MIT OR Apache-2.0 | ✅ RustCrypto | Proc-macro derive for `Zeroize`. |
| `fiat-crypto` | 0.2.9 | MIT OR Apache-2.0 | ✅ fiat-crypto project | Formally verified field arithmetic. |
| `ff` | 0.13.1 | MIT OR Apache-2.0 | ✅ ZKCrypto / zcash | Finite-field traits. |
| `group` | 0.13.0 | MIT OR Apache-2.0 | ✅ ZKCrypto / zcash | Group-law traits for elliptic curves. |
| `primeorder` | 0.13.6 | MIT OR Apache-2.0 | ✅ RustCrypto | Prime-order group wrapper. |
| `ark-bls12-381` | 0.4.0 | MIT OR Apache-2.0 | ✅ arkworks | BLS12-381 pairing curve (used by host). |
| `ark-bn254` | 0.4.0 | MIT OR Apache-2.0 | ✅ arkworks | BN254 pairing curve (used by host). |
| `ark-ec` | 0.4.2 | MIT OR Apache-2.0 | ✅ arkworks | Generic elliptic-curve arithmetic. |
| `ark-ff` | 0.4.2 | MIT OR Apache-2.0 | ✅ arkworks | Finite-field arithmetic for arkworks. |
| `ark-ff-asm` | 0.4.2 | MIT OR Apache-2.0 | ✅ arkworks | ASM-accelerated field ops. |
| `ark-ff-macros` | 0.4.2 | MIT OR Apache-2.0 | ✅ arkworks | Field element proc-macros. |
| `ark-poly` | 0.4.2 | MIT OR Apache-2.0 | ✅ arkworks | Polynomial arithmetic. |
| `ark-serialize` | 0.4.2 | MIT OR Apache-2.0 | ✅ arkworks | Canonical serialization for arkworks types. |
| `ark-serialize-derive` | 0.4.2 | MIT OR Apache-2.0 | ✅ arkworks | Derive macro for `CanonicalSerialize`. |
| `ark-std` | 0.4.0 | MIT OR Apache-2.0 | ✅ arkworks | `no_std`-compatible stdlib shims. |
| `const-oid` | 0.9.6 | MIT OR Apache-2.0 | ✅ RustCrypto | Compile-time OID constants. |
| `base16ct` | 0.2.0 | MIT OR Apache-2.0 | ✅ RustCrypto | Constant-time hex encode/decode. |
| `keccak` | 0.1.6 | MIT OR Apache-2.0 | ✅ RustCrypto | Keccak-f permutation (used by sha3). |
| `hex-literal` | 0.4.1 | MIT OR Apache-2.0 | ✅ RustCrypto | `hex!` macro for compile-time byte arrays. |


## Transitive dependencies — Serialization

| Crate | Version | License | Maintenance | Notes |
|-------|---------|---------|-------------|-------|
| `serde` | 1.0.228 | MIT OR Apache-2.0 | ✅ Active | De-facto serialization framework for Rust. |
| `serde_core` | 1.0.228 | MIT OR Apache-2.0 | ✅ Active | `serde` core traits (split crate). |
| `serde_derive` | 1.0.228 | MIT OR Apache-2.0 | ✅ Active | `#[derive(Serialize, Deserialize)]`. |
| `serde_json` | 1.0.149 | MIT OR Apache-2.0 | ✅ Active | JSON support used in spec tooling. |
| `serde_with` | 3.12.0 | MIT OR Apache-2.0 | ✅ Active | Custom serde helpers (used by `stellar-xdr`). |
| `serde_with_macros` | 3.12.0 | MIT OR Apache-2.0 | ✅ Active | Proc-macros for `serde_with`. |
| `schemars` | 0.8.22 | MIT OR Apache-2.0 | ✅ Active | JSON Schema generation (used by spec tooling). |
| `base64` | 0.22.1 | MIT OR Apache-2.0 | ✅ Active | Base64 encode/decode. |
| `base64ct` | 1.8.3 | MIT OR Apache-2.0 | ✅ Active | Constant-time base64 (RustCrypto). |
| `hex` | 0.4.3 | MIT OR Apache-2.0 | ✅ Active | Hex encode/decode with serde support. |
| `data-encoding` | 2.10.0 | MIT | ✅ Active | Multi-alphabet encoding (used by `stellar-strkey`). |
| `escape-bytes` | 0.1.1 | MIT OR Apache-2.0 | ✅ Active | Byte escaping used by `stellar-xdr`. |
| `ethnum` | 1.5.3 | MIT OR Apache-2.0 | ✅ Active | 256-bit integer types (u256/i256) for XDR. |
| `zmij` | 1.0.21 | MIT OR Apache-2.0 | ✅ Active | Integer encoding for `serde_json`. |
| `itoa` | 1.0.17 | MIT OR Apache-2.0 | ✅ Active | Fast integer-to-string formatting. |
| `time` | 0.3.47 | MIT OR Apache-2.0 | ✅ Active | Time types used by `serde_with`. |
| `time-core` | 0.1.8 | MIT OR Apache-2.0 | ✅ Active | Core primitives for the `time` crate. |
| `time-macros` | 0.2.27 | MIT OR Apache-2.0 | ✅ Active | Macros for `time`. |
| `chrono` | 0.4.44 | MIT OR Apache-2.0 | ✅ Active | Date/time types used by `serde_with`. |
| `deranged` | 0.5.8 | MIT OR Apache-2.0 | ✅ Active | Ranged integer types used by `time`. |
| `num-conv` | 0.2.0 | MIT OR Apache-2.0 | ✅ Active | Numeric conversions for `time`. |
| `powerfmt` | 0.2.0 | MIT OR Apache-2.0 | ✅ Active | Formatting helpers for `time`. |

## Transitive dependencies — WASM / WIT

| Crate | Version | License | Maintenance | Notes |
|-------|---------|---------|-------------|-------|
| `wasmparser` | 0.116.1 / 0.244.0 | Apache-2.0 | ✅ Bytecode Alliance | WASM binary parser. Two versions coexist in the transitive graph. |
| `wasmparser-nostd` | 0.100.2 | Apache-2.0 | ✅ Bytecode Alliance | `no_std` WASM parser (used by `soroban-wasmi`). |
| `wasm-encoder` | 0.244.0 | Apache-2.0 | ✅ Bytecode Alliance | WASM binary writer. |
| `wasm-metadata` | 0.244.0 | Apache-2.0 | ✅ Bytecode Alliance | WASM module metadata helpers. |
| `wit-bindgen` | 0.51.0 | Apache-2.0 | ✅ Bytecode Alliance | WIT interface bindings (used by WASI support). |
| `wit-bindgen-core` | 0.51.0 | Apache-2.0 | ✅ Bytecode Alliance | Core WIT binding logic. |
| `wit-bindgen-rust` | 0.51.0 | Apache-2.0 | ✅ Bytecode Alliance | Rust backend for WIT bindings. |
| `wit-bindgen-rust-macro` | 0.51.0 | Apache-2.0 | ✅ Bytecode Alliance | Proc-macro for WIT Rust bindings. |
| `wit-component` | 0.244.0 | Apache-2.0 | ✅ Bytecode Alliance | WASM component model tooling. |
| `wit-parser` | 0.244.0 | Apache-2.0 | ✅ Bytecode Alliance | WIT interface definition parser. |
| `wasm-bindgen` | 0.2.114 | MIT OR Apache-2.0 | ✅ Active | JS/WASM interop (pulled transitively via `getrandom`). |
| `wasm-bindgen-macro` | 0.2.114 | MIT OR Apache-2.0 | ✅ Active | Proc-macro for `wasm-bindgen`. |
| `wasm-bindgen-macro-support` | 0.2.114 | MIT OR Apache-2.0 | ✅ Active | Support library for `wasm-bindgen-macro`. |
| `wasm-bindgen-shared` | 0.2.114 | MIT OR Apache-2.0 | ✅ Active | Shared data structures for `wasm-bindgen`. |
| `wasmi_arena` | 0.4.1 | MIT OR Apache-2.0 | ✅ Active | Arena allocator for `soroban-wasmi`. |
| `wasmi_core` | 0.13.0 | MIT OR Apache-2.0 | ✅ Active | Core WASM execution types for `soroban-wasmi`. |
| `leb128fmt` | 0.1.0 | MIT OR Apache-2.0 | ✅ Active | LEB128 integer encoding for WASM. |
| `wasi` | 0.11.1+wasi-snapshot-preview1 | MIT OR Apache-2.0 | ✅ Active | WASI preview1 bindings. |
| `wasip2` | 1.0.2+wasi-0.2.9 | Apache-2.0 | ✅ Bytecode Alliance | WASI preview2 bindings. |
| `wasip3` | 0.4.0+wasi-0.3.0-rc-2026-01-06 | Apache-2.0 | ✅ Bytecode Alliance | WASI preview3 RC bindings. |

## Transitive dependencies — Randomness

| Crate | Version | License | Maintenance | Notes |
|-------|---------|---------|-------------|-------|
| `rand` | 0.8.5 / 0.9.2 | MIT OR Apache-2.0 | ✅ Active | Two versions coexist: 0.8.5 via `soroban-env-host`; 0.9.2 via `proptest`. |
| `rand_core` | 0.6.4 / 0.9.5 | MIT OR Apache-2.0 | ✅ Active | Two versions mirror the two `rand` majors. |
| `rand_chacha` | 0.3.1 / 0.9.0 | MIT OR Apache-2.0 | ✅ Active | ChaCha20 CSPRNG; versions match `rand` majors. |
| `rand_xorshift` | 0.4.0 | MIT OR Apache-2.0 | ✅ Active | Xorshift RNG (proptest). Dev-only. |
| `getrandom` | 0.2.17 / 0.3.4 / 0.4.2 | MIT OR Apache-2.0 | ✅ Active | Three versions coexist transitively across rand and WASI lineages. |
| `ppv-lite86` | 0.2.21 | MIT OR Apache-2.0 | ✅ Active | SIMD helpers for `rand_chacha`. |
| `fastrand` | 2.3.0 | MIT OR Apache-2.0 | ✅ Active | Fast non-crypto RNG (used by `tempfile`). |


## Transitive dependencies — Proc-macros and code generation

| Crate | Version | License | Maintenance | Notes |
|-------|---------|---------|-------------|-------|
| `proc-macro2` | 1.0.106 | MIT OR Apache-2.0 | ✅ Active | Token-stream manipulation for proc-macros. |
| `quote` | 1.0.45 | MIT OR Apache-2.0 | ✅ Active | Quasi-quoting for proc-macros. |
| `syn` | 1.0.109 / 2.0.117 | MIT OR Apache-2.0 | ✅ Active | Rust syntax tree. Both majors coexist transitively. |
| `prettyplease` | 0.2.37 | MIT OR Apache-2.0 | ✅ Active | Rustfmt-alternative for generated code. |
| `darling` | 0.20.11 | MIT OR Apache-2.0 | ✅ Active | Attribute macro argument parsing. |
| `darling_core` | 0.20.11 | MIT OR Apache-2.0 | ✅ Active | Core logic for `darling`. |
| `darling_macro` | 0.20.11 | MIT OR Apache-2.0 | ✅ Active | Proc-macro entry for `darling`. |
| `derivative` | 2.2.0 | MIT OR Apache-2.0 | ✅ Active | Flexible `#[derive]` customization (used by arkworks). |
| `derive_arbitrary` | 1.3.2 | MIT OR Apache-2.0 | ✅ Active | `#[derive(Arbitrary)]` (used by `arbitrary`). |
| `num-derive` | 0.4.2 | MIT OR Apache-2.0 | ✅ Active | `#[derive(FromPrimitive)]` etc. |
| `heck` | 0.5.0 | MIT OR Apache-2.0 | ✅ Active | Case conversion (snake_case ↔ PascalCase). |
| `bytes-lit` | 0.0.5 | MIT OR Apache-2.0 | ✅ Active | Byte-array literal proc-macro used by Soroban SDK. |
| `macro-string` | 0.1.4 | MIT OR Apache-2.0 | ✅ Active | String literal proc-macro used by Soroban SDK macros. |
| `cfg_eval` | 0.1.2 | MIT OR Apache-2.0 | ✅ Active | `cfg` attribute expansion used by `stellar-xdr`. |
| `paste` | 1.0.15 | MIT OR Apache-2.0 | ✅ Active | Token-paste proc-macro (used by `wasmi_core`). |
| `unicode-ident` | 1.0.24 | MIT OR Apache-2.0 | ✅ Active | Unicode identifier validation for proc-macro2. |
| `unicode-xid` | 0.2.6 | MIT OR Apache-2.0 | ✅ Active | XID Unicode property (used by `wit-parser`). |
| `rustc_version` | 0.4.1 | MIT OR Apache-2.0 | ✅ Active | Read rustc version at build time. |
| `rustversion` | 1.0.22 | MIT OR Apache-2.0 | ✅ Active | `#[rustversion]` conditional compilation. |

## Transitive dependencies — Utilities

| Crate | Version | License | Maintenance | Notes |
|-------|---------|---------|-------------|-------|
| `once_cell` | 1.21.4 | MIT OR Apache-2.0 | ✅ Active | Lazy statics; widely used, stable API. |
| `anyhow` | 1.0.102 | MIT OR Apache-2.0 | ✅ Active | Idiomatic error handling (used by `wit-*`). |
| `thiserror` | 1.0.69 | MIT OR Apache-2.0 | ✅ Active | `#[derive(Error)]` (used by Soroban ledger snapshot). |
| `thiserror-impl` | 1.0.69 | MIT OR Apache-2.0 | ✅ Active | Proc-macro for `thiserror`. |
| `indexmap` | 1.9.3 / 2.13.0 | MIT OR Apache-2.0 | ✅ Active | Ordered hash map. Two versions coexist transitively. |
| `indexmap-nostd` | 0.4.0 | MIT OR Apache-2.0 | ✅ Active | `no_std` variant for `wasmparser-nostd`. |
| `hashbrown` | 0.12.3 / 0.13.2 / 0.15.5 / 0.16.1 | MIT OR Apache-2.0 | ✅ Active | Multiple versions pulled by different major `indexmap`/`hashbrown` consumers. |
| `ahash` | 0.8.12 | MIT OR Apache-2.0 | ✅ Active | Non-cryptographic hash (used by `hashbrown`). |
| `foldhash` | 0.1.5 | MIT OR Apache-2.0 | ✅ Active | Alternative non-crypto hash for newer `hashbrown`. |
| `equivalent` | 1.0.2 | MIT OR Apache-2.0 | ✅ Active | Key-equivalence trait for `indexmap`. |
| `smallvec` | 1.15.1 | MIT OR Apache-2.0 | ✅ Active | Stack-allocated small vectors (used by `soroban-wasmi`). |
| `heapless` | 0.8.0 | MIT OR Apache-2.0 | ✅ Active | Fixed-capacity `no_std` collections (used by `stellar-strkey`). |
| `hash32` | 0.3.1 | MIT OR Apache-2.0 | ✅ Active | 32-bit hash trait used by `heapless`. |
| `stable_deref_trait` | 1.2.1 | MIT OR Apache-2.0 | ✅ Active | Stable dereference semantics. |
| `byteorder` | 1.5.0 | MIT OR Apache-2.0 | ✅ Active | Byte-order conversion (used by `hash32`). |
| `itertools` | 0.10.5 | MIT OR Apache-2.0 | ✅ Active | Iterator adapters (used by ark* / Soroban macros). |
| `either` | 1.15.0 | MIT OR Apache-2.0 | ✅ Active | `Either<L,R>` type (used by `itertools`). |
| `downcast-rs` | 1.2.1 | MIT OR Apache-2.0 | ✅ Active | Dynamic downcasting (used by `wasmi_core`). |
| `libm` | 0.2.16 | MIT OR Apache-2.0 | ✅ Active | `libm` in Rust (used by `wasmi_core`). |
| `spin` | 0.9.8 | MIT | ✅ Active | Spinlock primitives (used by `soroban-wasmi`). |
| `dyn-clone` | 1.0.20 | MIT OR Apache-2.0 | ✅ Active | `dyn Clone` helper (used by `schemars`). |
| `semver` | 1.0.27 | MIT OR Apache-2.0 | ✅ Active | Semantic versioning (used by `wasmparser`). |
| `log` | 0.4.29 | MIT OR Apache-2.0 | ✅ Active | Logging façade (used by `wit-*`). |
| `memchr` | 2.8.0 | MIT OR Apache-2.0 | ✅ Active | Fast byte-search (used by `serde_json`). |
| `fnv` | 1.0.7 | MIT OR Apache-2.0 | ✅ Active | FNV hash (used by `darling` and `rusty-fork`). |
| `strsim` | 0.11.1 | MIT OR Apache-2.0 | ✅ Active | String similarity (used by `darling_core`). |
| `ident_case` | 1.0.1 | MIT OR Apache-2.0 | ✅ Active | Identifier case (used by `darling_core`). |
| `id-arena` | 2.3.0 | MIT OR Apache-2.0 | ✅ Active | Arena with typed IDs (used by `wit-parser`). |
| `autocfg` | 1.5.0 | MIT OR Apache-2.0 | ✅ Active | Build-time cfg detection. |
| `bitflags` | 2.11.0 | MIT OR Apache-2.0 | ✅ Active | Type-safe bitflag structs. |
| `cc` | 1.2.57 | MIT OR Apache-2.0 | ✅ Active | C/C++ compilation support for build scripts. |
| `shlex` | 1.3.0 | MIT OR Apache-2.0 | ✅ Active | Shell-word splitting (used by `cc`). |
| `find-msvc-tools` | 0.1.9 | MIT OR Apache-2.0 | ✅ Active | MSVC tool discovery (used by `cc`; no-op on Linux). |
| `crate-git-revision` | 0.0.6 | MIT OR Apache-2.0 | ✅ Active | Embeds git revision at build time. |
| `cpufeatures` | 0.2.17 | MIT OR Apache-2.0 | ✅ RustCrypto | CPU feature detection for SHA/Keccak. |
| `static_assertions` | 1.1.0 | MIT OR Apache-2.0 | ✅ Active | Compile-time assertions (used by Soroban env). |
| `typenum` | 1.19.0 | MIT OR Apache-2.0 | ✅ Active | Type-level numbers for `generic-array`. |
| `generic-array` | 0.14.9 | MIT | ✅ Active | Fixed-size arrays via typenum (used by RustCrypto). |
| `crypto-common` | 0.1.6 | MIT OR Apache-2.0 | ✅ RustCrypto | Common crypto traits. |
| `block-buffer` | 0.10.4 | MIT OR Apache-2.0 | ✅ RustCrypto | Buffering for digest algorithms. |
| `num-bigint` | 0.4.6 | MIT OR Apache-2.0 | ✅ Active | Big-integer arithmetic (used by ark*). |
| `num-integer` | 0.1.46 | MIT OR Apache-2.0 | ✅ Active | Integer traits. |
| `num-traits` | 0.2.19 | MIT OR Apache-2.0 | ✅ Active | Numeric traits. |
| `r-efi` | 5.3.0 / 6.0.0 | MIT OR Apache-2.0 | ✅ Active | UEFI / EFI types (pulled by `getrandom`; no runtime impact on Linux). |

## Transitive dependencies — Testing / dev only

These crates are only used in test builds; they are not included in the production WASM artifact.

| Crate | Version | License | Maintenance | Notes |
|-------|---------|---------|-------------|-------|
| `arbitrary` | 1.3.2 | MIT OR Apache-2.0 | ✅ Active | Structured fuzzing input. |
| `bit-set` | 0.8.0 | MIT OR Apache-2.0 | ✅ Active | Bit-set data structure (used by `proptest`). |
| `bit-vec` | 0.8.0 | MIT OR Apache-2.0 | ✅ Active | Bit-vector (used by `bit-set`). |
| `regex-syntax` | 0.8.10 | MIT OR Apache-2.0 | ✅ Active | Regex parser used by `proptest` strategy generation. |
| `unarray` | 0.1.4 | MIT OR Apache-2.0 | ✅ Active | Array construction helpers (used by `proptest`). |
| `rusty-fork` | 0.3.1 | MIT OR Apache-2.0 | ✅ Active | Forked test isolation (used by `proptest`). |
| `quick-error` | 1.2.3 | MIT OR Apache-2.0 | ✅ Active | Error macro (used by `rusty-fork`). |
| `wait-timeout` | 0.2.1 | MIT OR Apache-2.0 | ✅ Active | Process timeout (used by `rusty-fork`). |
| `tempfile` | 3.27.0 | MIT OR Apache-2.0 | ✅ Active | Temporary files (used by `proptest`/`rusty-fork`). |
| `ctor` | 0.5.0 | MIT OR Apache-2.0 | ✅ Active | Constructor/destructor attributes (used by `soroban-sdk` test harness). |
| `ctor-proc-macro` | 0.0.6 | MIT OR Apache-2.0 | ✅ Active | Proc-macro for `ctor`. |
| `dtor` | 0.1.1 | MIT OR Apache-2.0 | ✅ Active | Destructor attribute (used by `ctor`). |
| `dtor-proc-macro` | 0.0.6 | MIT OR Apache-2.0 | ✅ Active | Proc-macro for `dtor`. |
| `bumpalo` | 3.20.2 | MIT OR Apache-2.0 | ✅ Active | Bump allocator (used by `wasm-bindgen` in test). |
| `js-sys` | 0.3.91 | MIT OR Apache-2.0 | ✅ Active | JS bindings for test targets (wasm-bindgen). |
| `iana-time-zone` | 0.1.65 | MIT OR Apache-2.0 | ✅ Active | Timezone detection (pulled by `chrono` in tests). |
| `iana-time-zone-haiku` | 0.1.2 | MIT OR Apache-2.0 | ✅ Active | Haiku OS timezone support (no-op on Linux). |


## Transitive dependencies — Platform / OS

| Crate | Version | License | Maintenance | Notes |
|-------|---------|---------|-------------|-------|
| `libc` | 0.2.183 | MIT OR Apache-2.0 | ✅ Active | C FFI bindings; required on Linux by several crates. |
| `rustix` | 1.1.4 | MIT OR Apache-2.0 | ✅ Active | Safe Linux syscall wrappers (used by `tempfile`). |
| `linux-raw-sys` | 0.12.1 | MIT OR Apache-2.0 | ✅ Active | Raw Linux kernel headers for `rustix`. |
| `errno` | 0.3.14 | MIT OR Apache-2.0 | ✅ Active | `errno` accessor (used by `rustix`). |
| `windows-sys` | 0.61.2 | MIT OR Apache-2.0 | ✅ Microsoft | Windows API stubs; no-op on Linux. |
| `windows-core` | 0.62.2 | MIT OR Apache-2.0 | ✅ Microsoft | Windows COM/WinRT stubs. No-op on Linux. |
| `windows-link` | 0.2.1 | MIT OR Apache-2.0 | ✅ Microsoft | Link attribute helpers for Windows API. |
| `windows-result` | 0.4.1 | MIT OR Apache-2.0 | ✅ Microsoft | Windows HRESULT wrapper. |
| `windows-strings` | 0.5.1 | MIT OR Apache-2.0 | ✅ Microsoft | Windows string types (PCWSTR etc.). |
| `windows-implement` | 0.60.2 | MIT OR Apache-2.0 | ✅ Microsoft | COM implement macro. |
| `windows-interface` | 0.59.3 | MIT OR Apache-2.0 | ✅ Microsoft | COM interface macro. |
| `core-foundation-sys` | 0.8.7 | MIT OR Apache-2.0 | ✅ Active | macOS Core Foundation (used by `iana-time-zone`). No-op on Linux. |
| `android_system_properties` | 0.1.5 | MIT OR Apache-2.0 | ✅ Active | Android system properties (used by `iana-time-zone`). No-op on Linux. |

---

## Flagged items

No dependencies are currently flagged as unmaintained or high-risk. The following items are monitored:

| Crate | Concern | Action |
|-------|---------|--------|
| `rand` (dual-version) | Versions 0.8.5 and 0.9.2 coexist due to `soroban-env-host` and `proptest` using different majors | Acceptable; both are current stable releases of their respective majors. Track for eventual SDK upgrade. |
| `hashbrown` (quad-version) | Four versions pulled by different dependency chains | No action needed; all are current releases and the overhead is compile-time only. |
| `syn` (dual-version) | 1.x and 2.x coexist via arkworks / older proc-macro crates | No action needed; syn 1.x is in maintenance mode but not EOL. |
| `getrandom` (triple-version) | 0.2, 0.3, 0.4 coexist across different WASI compatibility lineages | No action needed; resolved by transitive constraints. Monitor for 0.4 becoming dominant. |

---

## Quarterly review checklist

Perform on or before the next review date (listed at the top of this file):

1. Run `cargo audit` — address any advisories with severity ≥ medium before merge.
2. Check `cargo outdated` — note any direct dependencies more than one minor behind current.
3. Review the [Stellar/Soroban release notes](https://github.com/stellar/soroban-tools/releases) for protocol bumps; plan SDK upgrade if a new major protocol version is available.
4. Verify no flagged items above have become unmaintained or received new CVEs.
5. Update the **Last updated** date and the **next review** date at the top of this file.
6. Commit any lockfile-affecting changes on a dedicated dependency branch per [`docs/escrow-dependency-policy.md`](docs/escrow-dependency-policy.md).

---

## How to run a fresh audit

```bash
# Install cargo-audit if not already available
cargo install cargo-audit --locked

# Run advisory check against the RustSec database
cargo audit

# Check for outdated direct dependencies
cargo install cargo-outdated --locked
cargo outdated --depth 1
```

Any advisory output should be triaged before merging to `main`. See
[`docs/escrow-dependency-policy.md`](docs/escrow-dependency-policy.md) for the
emergency bump procedure when a critical CVE is published.

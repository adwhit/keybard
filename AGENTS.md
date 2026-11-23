## General style guide

* Create a `Justfile` and keep it up-to-date with common commands. We should not be running long complex commands via bash over and over again - if they're useful they belong in the Justfile.
* Use `mise.toml` to pin language runtimes. Exception: For Rust, use the system-wide toolchain.
* We expect everything to be linted/formatted/typechecked regularly
* The code repository should be as self-contained as possible. e.g. if we require a Postgres database, there should be a `docker-compose.yml` that spins one up for us.
* Maintain an up-to-date README.md which describes the purpose of the project, how to get it running, and the current status.

## Rust style guide

* use strongly-typed interfaces wherever possible
* prefer fieldless enums over strings
* use enums liberally, they are one of the most powerful Rust primitives
* make impossible states unrepresentable, with enums and newtypes
* avoid creating any `mod.rs` files, instead use the more modern style of `src/my_module.rs` and `src/my_module/submodule.rs`
* avoid `dyn Trait` if possible - prefer to use `<T: Trait>` to take advantage of monomorphisation
* avoid large files - each file should be <1000 LOC
* use the newtype pattern when appropriate. e.g instead of using `Uuid` use `TradeId(Uuid)`
* use the `thiserror` crate to define an `Error` type
* prefer iterators over for-loops - use `iter`, `map`, `filter`, `filter_map` and `collect` to make code clearer
* for async code, use `tokio` and prefer to use async channels to allow different parts of the app to communicate with one-another
* use `cargo fmt` liberally
* use `cargo clippy` to ensure good general style
* use `tracing` for logging. And use it liberally!
* for databses, use the `diesel` library and `diesel-cli` for tooling. `diesel-derive-enum` might also be useful
* prefer integration tests to examples
* do not add dependencies manually, use `cargo add` 
* use the `todo!()` macro liberally for 'missing'/unimplemented code
* do not use `serde_json::json!` or `serde_json::Value` anywhere, use concrete types
* do not make new database migrations, just edit the existing `up.sql` and `down.sql` migrations
* use ripgrep (`rg`) for text search. Assume it is installed.
* the code is still in development and has not been released yet, so *do not* include an support for backwards-compatibility/legacy code. Just make changes directly
* silence warnings with `expect` not `allow`
* prefer ripgrep (`rg`) to grep when searching files
* when possible, use red-green-refactor - write a failing test, add the feature, get it passing, tidy up.

## Frontend style guide

Tools:

* pnpm (use mise.toml to pin version to 'latest')
* vitejs
* typescript on _strict_ settings
* solidjs as our rendering framework
* tailwind css for styling
* zod for API validation
* playwright for e2e testing

Style:

* use `pnpm add` to add dependencies - ensures we get the latest
* Prefer dense page layouts - we do not value lots of padding and rounded edges
* Avoid using classes for the most part. Typescript works best when treated as a pseudo-functional language with minimal mutation
* Make sure the write 

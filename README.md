
# Twine-data

This crate provides means to encode and decode data in the `twine-data` format.

Twine is a relatively simple binary format that is designed to represent complex values that form [DAGs](https://en.wikipedia.org/wiki/Directed_acyclic_graph).
It superficially resembles [CBOR](https://cbor.io) in its data format, but
introduces a notion of sharing on top of it to enable efficient representation
of DAGs (including the serialization of complex in-memory data structures).

A more complete description of twine can be found at https://twine-data.dev .

## Feature flags

- `bumpalo` (default: `false`): introduces a dependency on [bumpalo](https://docs.rs/bumpalo/), which is used by the `value` module to deserialize an entire Twine blob into a Rust AST.

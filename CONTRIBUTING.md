# Contributing

Please keep scientific, numerical, and renderer concerns separate. New coordinate or topology logic belongs below `chart` and `render`; rendering adapters consume prepared primitives and must not leak backend types upward.

Before proposing a change, run the repository verification commands in the CI workflow. Add focused numerical tests for tolerance-sensitive behavior, and avoid snapshot-only image tests. Public API changes should update the architecture inventory and relevant examples.

This project is dual-licensed under MIT or Apache-2.0.
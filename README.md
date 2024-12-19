# Annatomic annotation editor

This is an experimental annotation editor which is intended to be used for the [RIDGES corpus](https://www.linguistik.hu-berlin.de/en/institut-en/professuren-en/korpuslinguistik/research/ridges-projekt).
It is based on [graphANNIS](https://github.com/korpling/graphANNIS).

It is very much **Work in progress** and not usable yet.
We plan to implement the following features first

- import and export corpora in the graphML format used by graphANNIS
- edit meta data on existing documents and (sub)-corpora
- edit the corpus structure
- edit span annotations
- edit the different token layers (RIDGES has diplomatic transcription and two normalization layers)


## Developing annatomic

You need to install Rust to compile the project.
We recommend installing the following Cargo subcommands for developing annis-web:

- [cargo-release](https://crates.io/crates/cargo-release) for creating releases
- [cargo-about](https://crates.io/crates/cargo-about) for re-generating the
  third party license file
- [cargo-llvm-cov](https://crates.io/crates/cargo-llvm-cov) for determining the code coverage
- [cargo-dist](https://crates.io/crates/cargo-dist) for configuring the GitHub actions that create the release binaries.

### Execute tests

You can run the tests with the default `cargo test` command.
To calculate the code coverage, you can use `cargo-llvm-cov`:

```bash
cargo llvm-cov --open --all-features --ignore-filename-regex 'tests?\.rs'
```


### Performing a release

You need to have [`cargo-release`](https://crates.io/crates/cargo-release)
installed to perform a release. Execute the follwing `cargo` command once to
install it.

```bash
cargo install cargo-release cargo-about
```

To perform a release, switch to the main branch and execute:

```bash
cargo release [LEVEL] --execute
```

The [level](https://github.com/crate-ci/cargo-release/blob/HEAD/docs/reference.md#bump-level) should be `patch`, `minor` or `major` depending on the changes made in the release.
Running the release command will also trigger a CI workflow to create release binaries on GitHub.


## Funding

This research was funded by the German Research Foundation (DFG, Deutsche Forschungsgemeinschaft) – SFB 1412, 416591334.

Die Forschungsergebnisse dieser Veröffentlichung wurden gefördert durch die Deutsche Forschungsgemeinschaft (DFG) – SFB 1412, 416591334.


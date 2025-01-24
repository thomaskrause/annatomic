#!/bin/bash

# Stop the script if any command exits with a non-zero return code
set -e

# Run static code checks
cargo fmt --check
cargo clippy

# Execute tests and calculate the code coverage both as lcov and HTML report
cargo llvm-cov --no-cfg-coverage --all-features --ignore-filename-regex 'tests?\.rs' --lcov --output-path annatomic.lcov

# Use diff-cover (https://github.com/Bachmann1234/diff_cover) and output code coverage compared to main branch
diff-cover annatomic.lcov --html-report target/llvm-cov/html/patch.html
echo "HTML report available at $PWD/target/llvm-cov/html/patch.html"
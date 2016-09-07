#!/bin/bash

cargo clean
RUSTFLAGS="-C link-dead-code" cargo test

RUST_TEST_THREADS=1 ./kcov-master/tmp/usr/local/bin/kcov --verify --exclude-pattern=/.cargo target/kcov target/debug/scaproust-*
RUST_TEST_THREADS=1 ./kcov-master/tmp/usr/local/bin/kcov --verify --exclude-pattern=/.cargo target/kcov target/debug/test-*

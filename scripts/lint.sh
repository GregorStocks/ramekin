#!/bin/bash -e

cargo fmt --all

cargo clippy --all-targets --all-features -- -D warnings

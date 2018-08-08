#!/usr/bin/env bash

git pull && cargo build --release && env RUST_LOG=info ./target/release/sentinel

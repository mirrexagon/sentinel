#!/usr/bin/env bash

cargo build --release && env RUST_LOG=info ./target/release/sentinel

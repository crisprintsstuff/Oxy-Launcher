.PHONY: test check release fmt

test:
	cargo test --workspace

fmt:
	cargo fmt --all

check:
	cargo fmt --all --check
	cargo clippy --workspace --all-targets -- -D warnings
	cargo test --workspace

release:
	cargo build --workspace --release

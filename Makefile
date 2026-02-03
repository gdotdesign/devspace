.PHONY: dev
dev:
	cargo build && mv target/debug/devspace ~/.bin/devspace


build:
	cargo build --release

dev:
	cargo build

test:
	cargo test --all -F bumpalo

clean:
	cargo clean

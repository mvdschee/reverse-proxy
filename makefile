dev:
	CONFIG_PATH=./example/local.toml watchexec -q -c -w src --exts rs --restart "cargo run"

scan:
	foxguard --config .foxguard.yml

scan-watch:
	watchexec -q -c -w src -w .foxguard.yml --exts rs,toml,yml -- foxguard --config .foxguard.yml

check:
	cargo clippy --all-targets -- -D warnings
	cargo fmt --check
	foxguard --config .foxguard.yml

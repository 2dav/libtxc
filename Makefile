toolchain: toolchain86 toolchain64

toolchain86:
	rustup target add i686-pc-windows-gnu

toolchain64:
	rustup target add x86_64-pc-windows-gnu

86: toolchain86
	cargo build --release --target i686-pc-windows-gnu

64: toolchain64
	cargo build --release --target x86_64-pc-windows-gnu

doc:
	cargo doc --no-deps --open

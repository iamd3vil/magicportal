default: build-linux

build-linux:
    RUSTFLAGS='-C target-feature=+crt-static' cargo build --release --target x86_64-unknown-linux-gnu

build-macos:
    docker run --rm \
    --volume ${PWD}:/root/src \
    --workdir /root/src \
    joseluisq/rust-linux-darwin-builder:1.76.0 \
    sh -c 'CC=aarch64-apple-darwin22.4-cc CXX=aarch64-apple-darwin22.4-cc++ cargo build --release --target aarch64-apple-darwin'

build-windows:
    cargo zigbuild --target x86_64-pc-windows-gnu --release
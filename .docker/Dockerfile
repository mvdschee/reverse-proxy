# syntax=docker/dockerfile:1.7

# Builder: hardened Alpine + dev toolchain
FROM dhi.io/alpine-base:3.24-alpine3.24-dev AS builder

USER root

# BoringSSL (via pingora) needs: cmake, perl, go, clang, make, musl-dev,
# pkgconf, linux-headers. lld is requested by our linker flag. curl + git
# pull the Rust toolchain.
RUN apk add --no-cache \
    build-base \
    clang \
    clang-dev \
    cmake \
    curl \
    git \
    go \
    linux-headers \
    lld \
    make \
    musl-dev \
    perl \
    pkgconf \
    ca-certificates

ARG RUST_VERSION=1.96.0
ARG TARGETARCH
ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH

# Map docker TARGETARCH -> rust target triple, then install the musl-host
# rustup-init binary directly so we don't rely on rustup.sh's libc sniffing
# (which picks the glibc binary when running under qemu).
RUN <<'EOF'
set -eu
case "${TARGETARCH}" in
  amd64) TRIPLE=x86_64-unknown-linux-musl ;;
  arm64) TRIPLE=aarch64-unknown-linux-musl ;;
  *) echo "unsupported TARGETARCH: ${TARGETARCH}" >&2; exit 1 ;;
esac
echo "${TRIPLE}" > /rust-target
curl --proto '=https' --tlsv1.2 -sSfL \
  "https://static.rust-lang.org/rustup/dist/${TRIPLE}/rustup-init" \
  -o /tmp/rustup-init
chmod +x /tmp/rustup-init
/tmp/rustup-init -y --no-modify-path \
  --default-host "${TRIPLE}" \
  --default-toolchain "${RUST_VERSION}" \
  --profile minimal \
  --target "${TRIPLE}"
EOF

WORKDIR /build

# Override .cargo/config.toml's target-cpu=native so the image is portable.
# Env-set RUSTFLAGS replaces (not merges with) config-file rustflags.
#
# Why -crt-static: musl targets default to crt-static which makes every
# rust binary fully static — including build-script artifacts. Musl's
# fully-static binaries can't dlopen, which breaks bindgen's libclang
# loader. -crt-static makes everything dynamic-link against libmusl. The
# runtime image ships musl, so the resulting binary still runs there.
ENV RUSTFLAGS="-C link-arg=-fuse-ld=lld -C target-feature=-crt-static"

# Prime the dependency cache with an empty main.rs so cargo fetches/builds
# crates without re-running on every source change.
#
# We don't pass --target: host and target triples are identical here
# (alpine musl, both arm64 or both amd64). Skipping --target makes
# RUSTFLAGS apply to build-script artifacts too, which is required for
# bindgen's dlopen path.
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
RUN <<'EOF'
set -eu
mkdir -p src
echo 'fn main() {}' > src/main.rs
cargo build --release --locked
rm -rf src target/release/proxy* target/release/deps/proxy-*
EOF

# Build the real binary, and stage the directory layout + runtime libs
# we'll lay down in the runtime image. The runtime image has no shell-
# as-root and no passwd entry for root, so we can't mkdir there.
#
# libgcc_s.so.1 provides the unwinder symbols (_Unwind_*) the dynamically
# linked binary needs at runtime. The DHI runtime image doesn't ship it.
COPY src ./src
RUN <<'EOF'
set -eu
cargo build --release --locked
strip target/release/proxy
mkdir -p /rootfs/etc/proxy /rootfs/var/lib/proxy/certs /rootfs/usr/lib
cp target/release/proxy /rootfs/proxy
cp -L /usr/lib/libgcc_s.so.1 /rootfs/usr/lib/libgcc_s.so.1
EOF


# Runtime: minimal hardened Alpine. Ships only busybox + musl + ca-certs.
# DHI images run as the 'nonroot' user (uid/gid 65532) by default.
FROM dhi.io/alpine-base:3.24

# Lay down the binary and pre-create mount-point directories with the
# correct ownership. Named volumes inherit this ownership on first use,
# so /var/lib/proxy/certs is writable by the nonroot runtime user.
# /etc/proxy/config.toml is REQUIRED — the binary exits if the file is missing.
COPY --from=builder --chown=65532:65532 /rootfs/proxy /usr/local/bin/proxy
COPY --from=builder --chown=65532:65532 /rootfs/etc/proxy /etc/proxy
COPY --from=builder --chown=65532:65532 /rootfs/var/lib/proxy /var/lib/proxy
COPY --from=builder /rootfs/usr/lib/libgcc_s.so.1 /usr/lib/libgcc_s.so.1

ENV CONFIG_PATH=/etc/proxy/config.toml \
    CERT_DIR=/var/lib/proxy/certs \
    HTTP_PORT=8080 \
    HTTPS_PORT=8443

EXPOSE 8080 8443

USER 65532:65532

ENTRYPOINT ["/usr/local/bin/proxy"]

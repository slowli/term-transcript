# Docker image for the `term-transcript` CLI executable.
# See the CLI crate readme for the usage instructions.

FROM clux/muslrust:stable AS builder
ADD .. ./
ARG FEATURES=portable-pty,tracing
RUN --mount=type=cache,id=cargo-registry,target=/root/.cargo/registry \
  --mount=type=cache,id=artifacts,target=/volume/target \
  cargo build --manifest-path=cli/Cargo.toml --release \
    --no-default-features --features=$FEATURES \
    --target-dir /volume/target && \
  # Move the resulting executable so it doesn't get unmounted together with the cache
  mv /volume/target/x86_64-unknown-linux-musl/release/term-transcript /volume/term-transcript

FROM alpine:3.17
COPY --from=builder /volume/term-transcript /usr/local/bin
# Add OpenBSD version of `nc` so that it supports Unix domain sockets
# as a more secure communication channel with the host compared to TCP sockets.
RUN apk add --no-cache netcat-openbsd
ENTRYPOINT ["term-transcript"]

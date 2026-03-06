# Multi-stage build for ao-recorder
FROM rust:1.87-bookworm AS builder

WORKDIR /build
COPY src/ src/

WORKDIR /build/src
RUN cargo build --release --bin ao-recorder

# Runtime image
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/src/target/release/ao-recorder /usr/local/bin/ao-recorder

RUN useradd -r -s /usr/sbin/nologin aorecorder
USER aorecorder

WORKDIR /data
VOLUME ["/data"]

EXPOSE 3000

ENTRYPOINT ["ao-recorder"]
CMD ["recorder.toml"]

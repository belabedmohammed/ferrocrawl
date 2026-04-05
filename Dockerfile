# Build stage
FROM rust:1.84-bookworm AS builder

WORKDIR /app

RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

# Cache dependencies
COPY Cargo.toml Cargo.lock* ./
RUN mkdir -p src && \
    echo 'fn main() { println!("dummy"); }' > src/main.rs && \
    echo 'fn main() { println!("dummy"); }' > src/local.rs && \
    echo '' > src/lib.rs && \
    cargo build --release --bin local-server 2>/dev/null || true

RUN rm -rf src && \
    find target/release -maxdepth 1 -type f -name "local*" -delete && \
    find target/release -maxdepth 1 -type f -name "ferrocrawl*" -delete

COPY src ./src
RUN cargo build --release --bin local-server

# Runtime stage
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

RUN useradd -r -u 1000 -m ferrocrawl
USER ferrocrawl

COPY --from=builder /app/target/release/local-server /usr/local/bin/local-server

ENV FERROCRAWL_HOST=0.0.0.0
ENV FERROCRAWL_PORT=3400

EXPOSE 3400

CMD ["local-server"]

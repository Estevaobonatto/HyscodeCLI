FROM rust:1.80-slim AS builder

WORKDIR /app

# Cache de dependências
COPY Cargo.toml Cargo.lock ./
COPY crates/hyscode-gateway/Cargo.toml crates/hyscode-gateway/

RUN mkdir -p crates/hyscode-gateway/src && \
    echo 'fn main() {}' > crates/hyscode-gateway/src/main.rs && \
    cargo build --release && \
    rm -rf crates/hyscode-gateway/src

# Build real
COPY . .
RUN cargo build --release -p hyscode-gateway

FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates libssl3 && \
    rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/hyscode-gateway /usr/local/bin/

EXPOSE 3000

ENTRYPOINT ["/usr/local/bin/hyscode-gateway"]

# Layer 1: Planner
FROM lukemathwalker/cargo-chef:latest-rust-1.86-alpine AS planner
WORKDIR /usr/src/app
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# Layer 2: Builder
FROM lukemathwalker/cargo-chef:latest-rust-1.86-alpine AS builder
WORKDIR /usr/src/app

# Install build dependencies
RUN apk add --no-cache pkgconfig openssl-dev musl-dev openssl-libs-static postgresql-dev

# Copy the recipe and build dependencies
COPY --from=planner /usr/src/app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json --target x86_64-unknown-linux-musl

# Copy the source code and build the application
COPY . .
RUN cargo build --release --target x86_64-unknown-linux-musl

# Layer 3: Runtime
FROM alpine:3.19 AS runtime
WORKDIR /app

# Install runtime dependencies
RUN apk add --no-cache ca-certificates

# Copy the binary from builder
COPY --from=builder /usr/src/app/target/x86_64-unknown-linux-musl/release/main /app/main

CMD ["/app/main"]

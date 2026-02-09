# Build stage - Use nightly for edition2024
FROM rustlang/rust:nightly AS builder
WORKDIR /app

# Install dependencies for sqlx offline mode
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy dependency files first for better layer caching
COPY Cargo.toml Cargo.lock ./
COPY deny.toml ./

# Create a dummy main.rs to build dependencies
RUN mkdir src && \
    echo "fn main() {}" > src/main.rs && \
    cargo build --release && \
    rm -rf src

# Copy the actual source code
COPY src ./src
COPY migrations ./migrations
COPY .sqlx ./.sqlx

# Build the application
RUN touch src/main.rs && \
    cargo build --release

# Runtime stage
FROM debian:bookworm-slim
WORKDIR /app

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Create a non-root user
RUN useradd -m -u 1000 appuser && chown -R appuser:appuser /app

# Copy the binary from builder
COPY --from=builder /app/target/release/tero_platform /app/tero_platform
COPY --from=builder /app/migrations /app/migrations

# Copy config files
COPY src/config/*.toml /app/src/config/

USER appuser

EXPOSE 3000

CMD ["/app/tero_platform"]

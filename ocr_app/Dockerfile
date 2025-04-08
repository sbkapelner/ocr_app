FROM rust:1.83.0-slim as builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libfontconfig1-dev \
    build-essential \
    g++ \
    make \
    libclang1 \
    clang \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /usr/src/ocr_app
COPY . .

# Set environment variables to suppress warnings and configure mupdf-sys
ENV CFLAGS="-w" \
    CXXFLAGS="-w" \
    MUPDF_SYS_DISABLE_WARNINGS=1

RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y \
    libssl-dev \
    ca-certificates \
    libfontconfig1 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /usr/src/ocr_app/target/release/web /app/
COPY --from=builder /usr/src/ocr_app/templates /app/templates
COPY --from=builder /usr/src/ocr_app/static /app/static
COPY --from=builder /usr/src/ocr_app/models /app/models

EXPOSE 8080

CMD ["./web"]

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

WORKDIR /
COPY --from=builder /usr/src/ocr_app/target/release/web /
COPY --from=builder /usr/src/ocr_app/templates /templates
COPY --from=builder /usr/src/ocr_app/static /static
COPY --from=builder /usr/src/ocr_app/models /models

EXPOSE 8080

CMD ["./web"]

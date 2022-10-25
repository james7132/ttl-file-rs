FROM rust:latest AS builder

RUN rustup toolchain install nightly && \
  rustup target add x86_64-unknown-linux-musl --toolchain nightly && \
  rustup component add rust-src --toolchain nightly && \
  apt update && \
  apt install -y musl-tools musl-dev upx

WORKDIR /ttl
COPY ./ .
RUN cargo +nightly build -Z build-std=std,panic_abort -Z build-std-features=panic_immediate_abort --target x86_64-unknown-linux-musl --release
RUN upx --best --lzma target/x86_64-unknown-linux-musl/release/ttl-file

################################################################################
## Final image
################################################################################
FROM scratch
COPY --from=builder /ttl/target/x86_64-unknown-linux-musl/release/ttl-file ./
RUN mkdir /ttl
CMD ["/ttl/ttl-file", "/ttl"]

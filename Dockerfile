FROM rust:latest AS builder

RUN rustup toolchain install nightly && \
  rustup target add x86_64-unknown-linux-musl --toolchain nightly && \
  rustup component add rust-src --toolchain nightly && \
  apt update && \
  apt install -y musl-tools musl-dev upx

ENV USER=ttl
ENV UID=10001

RUN adduser \
    --disabled-password \
    --gecos "" \
    --home "/nonexistent" \
    --shell "/sbin/nologin" \
    --no-create-home \
    --uid "${UID}" \
    "${USER}"

WORKDIR /ttl
COPY ./ .
RUN cargo +nightly build -Z build-std=std,panic_abort -Z build-std-features=panic_immediate_abort --target x86_64-unknown-linux-musl --release
RUN upx --best --lzma target/x86_64-unknown-linux-musl/release/ttl-file

################################################################################
## Final image
################################################################################
FROM scratch

# Import from builder.
COPY --from=builder /etc/passwd /etc/passwd
COPY --from=builder /etc/group /etc/group

WORKDIR /ttl
COPY --from=builder /ttl/target/x86_64-unknown-linux-musl/release/ttl-file ./
USER ttl:ttl

CMD ["/ttl/ttl-file"]

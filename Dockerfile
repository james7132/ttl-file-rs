FROM rust:latest AS builder

RUN rustup target add x86_64-unknown-linux-musl
RUN apt update && apt install -y musl-tools musl-dev upx

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
RUN cargo build --target x86_64-unknown-linux-musl --release
RUN upx --best target/x86_64-unknown-linux-musl/release/ttl-file

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

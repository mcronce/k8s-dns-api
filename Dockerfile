FROM rustlang/rust:nightly AS builder

ADD . /repo
WORKDIR /repo

RUN cargo build --release
RUN strip target/release/k8s-dns-api

FROM gcr.io/distroless/cc-debian11
COPY --from=builder /repo/target/release/k8s-dns-api /usr/local/bin/k8s-dns-api
ENTRYPOINT ["/usr/local/bin/k8s-dns-api"]


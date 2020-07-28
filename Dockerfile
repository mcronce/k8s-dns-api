FROM rustlang/rust:nightly AS builder

ADD . /repo
WORKDIR /repo

RUN cargo build --release

FROM centos
COPY --from=builder /repo/target/release/k8s-dns-api /usr/local/bin/k8s-dns-api
ENTRYPOINT ["/usr/local/bin/k8s-dns-api"]


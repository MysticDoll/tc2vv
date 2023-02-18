FROM rust as builder
RUN USER=root cargo new --bin app
WORKDIR /app

COPY ./Cargo.lock ./Cargo.lock
COPY ./Cargo.toml ./Cargo.toml
COPY ./src ./src

RUN cargo install --locked --path .

FROM debian:latest
COPY --from=builder /usr/local/cargo/bin/tc2vv .
RUN apt-get update
RUN apt install -y --no-install-recommends ca-certificates
CMD ["./tc2vv"]

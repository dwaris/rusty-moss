FROM rust:slim as builder
WORKDIR /usr/src/app
COPY . .
RUN cargo build --release

FROM debian:trixie-slim
COPY --from=builder /usr/src/app/target/release/rusty-moss .
CMD ["./rusty-moss"]

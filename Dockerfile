FROM rust:slim as builder
WORKDIR /usr/src/app
COPY . .
RUN cargo build --release

FROM debian:trixie-slim
RUN apt-get update \
	&& apt-get install -y --no-install-recommends ca-certificates \
	&& rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/src/app/target/release/rusty-moss .
EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=5s --start-period=15s --retries=3 CMD ["./rusty-moss", "--healthcheck"]
CMD ["./rusty-moss"]

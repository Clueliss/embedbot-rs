FROM rustlang/rust:nightly-stretch-slim AS builder
RUN apt-get update && apt-get install pkg-config libssl-dev -y
WORKDIR /usr/src/embedbot-rs
COPY ./Cargo.toml ./
COPY ./src ./src
RUN cargo build --release


FROM debian:stretch-slim

ENV DISCORD_TOKEN=YOUR_DISCORD_TOKEN_HERE

RUN apt-get update && apt-get install libssl-dev ca-certificates -y
COPY --from=builder /usr/src/embedbot-rs/target/release/embedbot-rs /usr/local/bin/
RUN chmod +x /usr/local/bin/embedbot-rs

ENTRYPOINT ["/usr/local/bin/embedbot-rs", "--settings-file", "/etc/embedbot.conf"]

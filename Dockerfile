# One container: the page (wasm) and the relay that introduces islands.
FROM rust:1-bookworm AS build
RUN rustup target add wasm32-unknown-unknown \
 && cargo install wasm-bindgen-cli --version 0.2.129 --locked
WORKDIR /src
COPY . .
RUN bash scripts/build-web.sh && cargo build -p secretspace-relay --release

FROM debian:bookworm-slim
COPY --from=build /src/target/release/relay /usr/local/bin/relay
COPY --from=build /src/dist /srv/dist
ENV PORT=8787
EXPOSE 8787
CMD ["relay", "--static", "/srv/dist"]

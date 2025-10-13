list:
    just --list

game *ARGS:
    cargo run -- {{ARGS}}

client *ARGS:
    cargo run -- --connect ws://127.0.0.1:1155 {{ARGS}}

web command *ARGS:
    cargo geng {{command}} --platform web --release -- {{ARGS}}

server := "friendly.nertsal.com"
server_user := "nertsal"

update-server:
    docker run --rm -it -e CARGO_TARGET_DIR=/target -v `pwd`/docker-target:/target -v `pwd`:/src -w /src ghcr.io/geng-engine/cargo-geng cargo geng build --release
    rsync -avz docker-target/geng/ {{server_user}}@{{server}}:friendly/
    ssh {{server_user}}@{{server}} systemctl --user restart friendly

publish-web:
    CONNECT=wss://{{server}} cargo geng build --release --platform web --out-dir target/geng
    butler -- push target/geng nertsal/ded-end:html5

deploy:
    just update-server
    just publish-web

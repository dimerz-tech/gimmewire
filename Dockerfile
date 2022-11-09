FROM rust:1.31

WORKDIR /usr/bin

COPY . .

RUN cargo install --path .

CMD ["gimmewire"]
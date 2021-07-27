#! /bin/bash

function packaging () {
    rm -rdf release/
    mv $target/release/* $target/
    rm -rdf $target/release/
    cp -r sample/ $target/
    cp -r assets/ $target/
}
function build () {
    target="x86_64-unknown-linux-gnu"
    if [[ $1 ]]; then
        if [ "${1}" = "win" ]; then
            rustup target add x86_64-pc-windows-gnu
            target="x86_64-pc-windows-gnu"
        elif [ "${1}" = 'mac' ]; then
            rustup target add x86_64-apple-darwin
            target="x86_64-apple-darwin"
            
        else
            rustup target add x86_64-unknown-linux-gnu
        fi
    fi
    cargo build --release --target $target --target-dir .
    packaging "$target"
}
build "$1"
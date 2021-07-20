#! /bin/bash

function packaging () {
    rm -rdf release/
    mv $target/release/* $target/
    rm -rdf $target/release/
    cp -r sample/ $target/
}
function build () {
    target="x86_64-unknown-linux-gnu"
    if [[ $1 ]]; then
        if [ "${1}" = "win" ]; then
            $target="x86_64-pc-windows-gnu"
        elif [ "${1}" = 'mac' ]; then
            $target="x86_64-apple-darwin"
        else
            $target="x86_64-unknown-linux-gnu"
        fi
    fi
    cargo build --bins --release --target $target --target-dir .
    packaging "$target"
}
build "$1"
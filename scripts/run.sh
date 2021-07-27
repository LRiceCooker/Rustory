#! /bin/bash

function build (){
    if [[ $1 ]]; then 
        ./scripts/build.sh $1 > /dev/null
    else
        ./scripts/build.sh linux > /dev/null
    fi
}

function run (){
    build "$1"
    path="x86_64-unknown-linux-gnu"
    if [ "${1}" = "win" ]; then
        path="x86_64-pc-windows-gnu"
    elif [ "${1}" = "mac" ]; then
        path="x86_64-apple-darwin"
    fi
    ./$path/rustory
    rm -rdf $path
}


run "$1"
#! /bin/bash

function run (){
    cp -r sample/ target/debug
    cp -r assets/ target/debug
    cargo run
}
run
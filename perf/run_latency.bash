#!/bin/bash

PERF_PATH="$( cd "$(dirname "$0")" ; pwd -P )"
BIN_PATH="$( cd "$PERF_PATH/../target/release/examples" ; pwd -P )"


function run_once {
    URL=$1
    MSG_SIZE=$2
    MSG_COUNT=$3
    $BIN_PATH/perf_local_lat $URL $MSG_SIZE $MSG_COUNT & perf_local=$!
    $BIN_PATH/perf_remote_lat $URL $MSG_SIZE $MSG_COUNT
    kill $perf_local 2> /dev/null
    wait $perf_local 2> /dev/null
}

run_once tcp://127.0.0.1:18080 512      50000
run_once tcp://127.0.0.1:18080 1024     10000
run_once tcp://127.0.0.1:18080 8192     10000
run_once tcp://127.0.0.1:18080 102400    2000
run_once tcp://127.0.0.1:18080 524288     500
run_once tcp://127.0.0.1:18080 1048576    100

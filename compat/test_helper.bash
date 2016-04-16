#!/bin/bash

EXAMPLE_PATH="$( cd "$COMPAT_PATH/../target/debug/examples" ; pwd -P )"

NO_COLOR='\033[0m' 
RED_COLOR='\033[0;31m'
GREEN_COLOR='\033[0;32m'

function echo_test_case_succeeded {
    echo -e "$1 ${GREEN_COLOR}SUCCEEDED${NO_COLOR}" 
}

function echo_test_case_failed {
    echo -e "$1 ${RED_COLOR}FAILED !${NO_COLOR}" 
}

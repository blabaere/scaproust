#!/bin/bash

COMPAT_PATH="$( cd "$(dirname "$0")" ; pwd -P )"
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

# TEST CASE 1
# Checks that scaproust can receive a request from nanocat and send a reply
# Arguments : URL
function testcase_reqrep1 {
    URL=$1
    $EXAMPLE_PATH/td-request-reply node0 $URL > /tmp/reqrep_tc_1.log & node0=$!
    nanocat --req --connect $URL --data "DATE" > /dev/null -i2 & ncat=$!
    sleep 0.5 && kill $ncat && kill $node0
    result=`cat /tmp/reqrep_tc_1.log`
    expected=`cat $COMPAT_PATH/reqrep_tc_1_expected.log`
    if [[ $result == $expected ]]; then
        echo_test_case_succeeded "reqrep test case 1 $URL"
    else
        echo_test_case_failed "reqrep test case 1 $URL"
    fi
}

function testcase_reqrep2 {
    URL=$1
    echo_test_case_failed "reqrep test case 2 $URL"
}

function test_reqrep {
    testcase_reqrep1 $1
    testcase_reqrep2 $1
}

test_reqrep "tcp://127.0.0.1:5454"
test_reqrep "ipc:///tmp/reqrep_test.ipc"

#encéphalopulseur...
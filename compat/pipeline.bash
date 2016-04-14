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
# Checks that scaproust can receive a message sent by nanocat
# Arguments : URL
function testcase_pipeline1 {
    URL=$1
    msg="asterohache"
    $EXAMPLE_PATH/td-pipeline node0 $URL > /tmp/pipeline_tc_1.log & node0=$!
    nanocat --push --connect $URL --data "$msg" > /dev/null -i2 & ncat=$!
    sleep 0.5 && kill $ncat && kill $node0
    result=`cat /tmp/pipeline_tc_1.log`
    expected=`cat $COMPAT_PATH/pipeline_tc_1_expected.log`
    if [[ $result == $expected ]]; then
        echo_test_case_succeeded "pipeline test case 1 $URL"
    else
        echo_test_case_failed "pipeline test case 1 $URL"
    fi
}

# TEST CASE 2
# Checks that a message sent by scaproust is received by nanocat
# Arguments : URL
function testcase_pipeline2 {
    URL=$1
    msg="cornofulgur"
    nanocat --pull --bind $URL --ascii > /tmp/pipeline_tc_2.log & ncat=$!
    ./target/debug/examples/td-pipeline node1 $URL "$msg" > /dev/null & node1=$!
    sleep 0.5 && kill $ncat && kill $node1
    result=`cat /tmp/pipeline_tc_2.log`
    expected=`cat $COMPAT_PATH/pipeline_tc_2_expected.log`
    if [[ $result == $expected ]]; then
        echo_test_case_succeeded "pipeline test case 2 $URL"
    else
        echo_test_case_failed "pipeline test case 2 $URL"
    fi
}

function test_pipeline {
    testcase_pipeline1 $1
    testcase_pipeline2 $1
}

test_pipeline "tcp://127.0.0.1:5454"
test_pipeline "ipc:///tmp/pipeline_test.ipc"

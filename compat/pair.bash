#!/bin/bash

COMPAT_PATH="$( cd "$(dirname "$0")" ; pwd -P )"
source "$COMPAT_PATH/test_helper.bash"

# TEST CASE 1
# Checks that two scaproust pairs can exchange messages
# Arguments : URL
function testcase_pair1 {
    URL=$1
    $EXAMPLE_PATH/pair node0 $URL > /tmp/pair_tc_1_node0.log & node0=$!
    $EXAMPLE_PATH/pair node1 $URL > /tmp/pair_tc_1_node1.log & node1=$!
    sleep 3.5 && kill $node1 && kill $node0
    result_node0=`cat /tmp/pair_tc_1_node0.log`
    expected_node0=`cat $COMPAT_PATH/pair_tc_1_node0_expected.log`
    result_node1=`cat /tmp/pair_tc_1_node1.log`
    expected_node1=`cat $COMPAT_PATH/pair_tc_1_node1_expected.log`
    if [[ $result_node0 == $expected_node0 && $result_node1 == $expected_node1 ]]; then
        echo_test_case_succeeded "pair test case 1 ${URL:0:3}"
    else
        echo_test_case_failed "pair test case 1 ${URL:0:3}"
    fi
}

function test_pair {
    testcase_pair1 $1
}

if [[ -f "/tmp/pair_test.ipc" ]]; then
    rm -f "/tmp/pair_test.ipc"
fi

test_pair "tcp://127.0.0.1:5454"
test_pair "ipc:///tmp/pair_test.ipc"

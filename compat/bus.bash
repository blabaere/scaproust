#!/bin/bash

COMPAT_PATH="$( cd "$(dirname "$0")" ; pwd -P )"
source "$COMPAT_PATH/test_helper.bash"

function test_case1 {
    $EXAMPLE_PATH/bus node0 $1 $2 $3 > /tmp/bus_tc1_node0.log & node0=$!
    $EXAMPLE_PATH/bus node1 $2 $3 $4 > /tmp/bus_tc1_node1.log & node1=$!
    $EXAMPLE_PATH/bus node2 $3 $4    > /tmp/bus_tc1_node2.log & node2=$!
    nanocat --bus --bind $4 --connect $1 --ascii --data node3 -d 2 -i 10 > /tmp/bus_tc1_node3.log & node3=$!
    sleep 3.5 && kill $node0 $node1 $node2 $node3
    result=`sort /tmp/bus_tc1_node0.log`
    expected=`sort $COMPAT_PATH/bus_tc1_node0_expected.log`
    if [[ ! $result == $expected ]]; then
        echo_test_case_failed "bus test case 1"
        return 1
    fi
    result=`sort /tmp/bus_tc1_node1.log`
    expected=`sort $COMPAT_PATH/bus_tc1_node1_expected.log`
    if [[ ! $result == $expected ]]; then
        echo_test_case_failed "bus test case 1"
        return 2
    fi
    result=`sort /tmp/bus_tc1_node2.log`
    expected=`sort $COMPAT_PATH/bus_tc1_node2_expected.log`
    if [[ ! $result == $expected ]]; then
        echo_test_case_failed "bus test case 1"
        return 3
    fi
    result=`sort /tmp/bus_tc1_node3.log`
    expected=`sort $COMPAT_PATH/bus_tc1_node3_expected.log`
    if [[ ! $result == $expected ]]; then
        echo_test_case_failed "bus test case 1"
        return 4
    fi
    echo_test_case_succeeded "bus test case 1"
}

function test_suite {
    test_case1 $1 $2 $3 $4
}

URL0="tcp://127.0.0.1:5450"
URL1="tcp://127.0.0.1:5451"
URL2="tcp://127.0.0.1:5452"
URL3="tcp://127.0.0.1:5453"
test_suite $URL0 $URL1 $URL2 $URL3

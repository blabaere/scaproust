#!/bin/bash

COMPAT_PATH="$( cd "$(dirname "$0")" ; pwd -P )"
source "$COMPAT_PATH/test_helper.bash"

# TEST CASE 1
# Checks that scaproust can receive a request from nanocat and send a reply
# Arguments : URL
function testcase_reqrep1 {
    URL=$1
    $EXAMPLE_PATH/reqrep node0 $URL > /tmp/reqrep_tc1_node0.log & node0=$!
    nanocat --req --connect $URL --data "DATE" --ascii -i 2 > /tmp/reqrep_tc1_nanocat.log & ncat=$!
    sleep 0.5 && kill $ncat $node0
    result_node0=`cat /tmp/reqrep_tc1_node0.log`
    expected_node0=`cat $COMPAT_PATH/reqrep_tc1_node0_expected.log`
    result_nanocat=`cat /tmp/reqrep_tc1_nanocat.log`
    expected_nanocat=`cat $COMPAT_PATH/reqrep_tc1_nanocat_expected.log`
    if [[ $result_node0 == $expected_node0 && $result_nanocat == $expected_nanocat ]]; then
        echo_test_case_succeeded "reqrep test case 1 ${URL:0:3}"
    else
        echo_test_case_failed "reqrep test case 1 ${URL:0:3}"
    fi 
}

# TEST CASE 2
# Checks that scaproust can send a request to nanocat and receive a reply
# Arguments : URL
function testcase_reqrep2 {
    URL=$1
    msg="pulvonium"
    nanocat --rep --bind $URL --ascii --data $msg > /tmp/reqrep_tc2_nanocat.log -i 2 & ncat=$!
    ./target/debug/examples/reqrep node1 $URL > /tmp/reqrep_tc2_node1.log & node1=$!
    sleep 0.5 && kill $ncat $node1
    result_node1=`cat /tmp/reqrep_tc2_node1.log`
    expected_node1=`cat $COMPAT_PATH/reqrep_tc2_node1_expected.log`
    result_nanocat=`cat /tmp/reqrep_tc2_nanocat.log`
    expected_nanocat=`cat $COMPAT_PATH/reqrep_tc2_nanocat_expected.log`
    if [[ $result_node1 == $expected_node1 && $result_nanocat == $expected_nanocat ]]; then
        echo_test_case_succeeded "reqrep test case 2 ${URL:0:3}"
    else
        echo_test_case_failed "reqrep test case 2 ${URL:0:3}"
    fi
}

function test_reqrep {
    testcase_reqrep1 $1
    testcase_reqrep2 $1
}

if [[ -f "/tmp/reqrep_test.ipc" ]]; then
    rm -f "/tmp/reqrep_test.ipc"
fi

test_reqrep "tcp://127.0.0.1:5454"
#test_reqrep "ipc:///tmp/reqrep_test.ipc"

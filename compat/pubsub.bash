#!/bin/bash

COMPAT_PATH="$( cd "$(dirname "$0")" ; pwd -P )"
source "$COMPAT_PATH/test_helper.bash"

# TEST CASE 1
# Checks that scaproust can publish messages to nanocat
# Arguments : URL
function testcase_pubsub1 {
    URL=$1
    $EXAMPLE_PATH/pubsub server $URL > /tmp/pubsub_tc1_server.log & server=$!
    nanocat --sub --connect $URL --ascii > /tmp/pubsub_tc1_nanocat1.log & ncat1=$!
    nanocat --sub --connect $URL --ascii > /tmp/pubsub_tc1_nanocat2.log & ncat2=$!
    sleep 3.5 && kill $ncat1 $ncat2 $server
    result_server=`cat /tmp/pubsub_tc1_server.log`
    expected_server=`cat $COMPAT_PATH/pubsub_tc1_server_expected.log`
    result_nanocat1=`cat /tmp/pubsub_tc1_nanocat1.log`
    expected_nanocat1=`cat $COMPAT_PATH/pubsub_tc1_nanocat1_expected.log`
    result_nanocat2=`cat /tmp/pubsub_tc1_nanocat2.log`
    expected_nanocat2=`cat $COMPAT_PATH/pubsub_tc1_nanocat2_expected.log`

    if [[ $result_server == $expected_server && $result_nanocat1 == $expected_nanocat1 && $result_nanocat2 == $expected_nanocat2 ]]; then
        echo_test_case_succeeded "pubsub test case 1 ${URL:0:3}"
    else
        echo_test_case_failed "pubsub test case 1 ${URL:0:3}"
    fi 
}

# TEST CASE 2
# Checks that scaproust can subscribe and receive messages from nanocat
# Arguments : URL
function testcase_pubsub2 {
    URL=$1
    nanocat --pub --bind $URL --data "retrolaser" -d 1 -i 1 > /dev/null & ncat=$!
    $EXAMPLE_PATH/pubsub client $URL "raoul" > /tmp/pubsub_tc2_client1.log & client1=$!
    $EXAMPLE_PATH/pubsub client $URL "simone" > /tmp/pubsub_tc2_client2.log & client2=$!
    sleep 3.5 && kill $client1 $client2 $ncat

    result_client1=`cat /tmp/pubsub_tc2_client1.log`
    expected_client1=`cat $COMPAT_PATH/pubsub_tc2_client1_expected.log`
    result_client2=`cat /tmp/pubsub_tc2_client2.log`
    expected_client2=`cat $COMPAT_PATH/pubsub_tc2_client2_expected.log`

    if [[ $result_client1 == $expected_client1 && $result_client2 == $expected_client2 ]]; then
        echo_test_case_succeeded "pubsub test case 2 ${URL:0:3}"
    else
        echo_test_case_failed "pubsub test case 2 ${URL:0:3}"
    fi 
}

function test_pubsub {
    testcase_pubsub1 $1
    testcase_pubsub2 $1
}

rm -f /tmp/pubsub* > /dev/null

test_pubsub "tcp://127.0.0.1:5454"
test_pubsub "ipc:///tmp/pubsub_test.ipc"

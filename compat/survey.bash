#!/bin/bash

COMPAT_PATH="$( cd "$(dirname "$0")" ; pwd -P )"
source "$COMPAT_PATH/test_helper.bash"

# TEST CASE 1
# Checks that scaproust can send a survey to nanocat and receive votes
# Arguments : URL
function testcase_survey1 {
    URL=$1
    nanocat --resp --connect $URL --data "mammouth" --ascii -i 2 > /tmp/survey_tc1_nanocat1.log & ncat1=$!
    nanocat --resp --connect $URL --data "mammouth" --ascii -i 2 > /tmp/survey_tc1_nanocat2.log & ncat2=$!
    $EXAMPLE_PATH/survey server $URL > /tmp/survey_tc1_server.log & server=$!
    sleep 1.5 && kill $ncat1 $ncat2 $server
    result_server=`cat /tmp/survey_tc1_server.log`
    expected_server=`cat $COMPAT_PATH/survey_tc1_server_expected.log`
    result_nanocat1=`cat /tmp/survey_tc1_nanocat1.log`
    expected_nanocat1=`cat $COMPAT_PATH/survey_tc1_nanocat1_expected.log`
    if [[ $result_server == $expected_server && $result_nanocat1 == $expected_nanocat1 && $result_nanocat2 == $expected_nanocat2 ]]; then
        echo_test_case_succeeded "survey test case 1 ${URL:0:3}"
    else
        echo_test_case_failed "survey test case 1 ${URL:0:3}"
    fi 
}

# TEST CASE 2
# Checks that scaproust can receive a survey from nanocat and send votes
# Arguments : URL
function testcase_survey2 {
    URL=$1
    nanocat --surv --bind $URL --data "clavicogyre" --ascii -i 2 -d 1 > /tmp/survey_tc2_nanocat.log & ncat=$!
    $EXAMPLE_PATH/survey client $URL "mammouth" > /tmp/survey_tc2_client1.log & client1=$!
    $EXAMPLE_PATH/survey client $URL "mammouth" > /tmp/survey_tc2_client2.log & client2=$!
    sleep 1.5 && kill $ncat $client1 $client2
    result_nanocat=`cat /tmp/survey_tc2_nanocat.log`
    expected_nanocat=`cat $COMPAT_PATH/survey_tc2_nanocat_expected.log`
    result_client1=`cat /tmp/survey_tc2_client1.log`
    expected_client1=`cat $COMPAT_PATH/survey_tc2_client1_expected.log`
    result_client2=`cat /tmp/survey_tc2_client2.log`
    expected_client2=`cat $COMPAT_PATH/survey_tc2_client2_expected.log`
    if [[ $result_nanocat == $expected_nanocat && $result_client1 == $expected_client1 && $result_client2 == $expected_client2 ]]; then
        echo_test_case_succeeded "survey test case 2 ${URL:0:3}"
    else
        echo_test_case_failed "survey test case 2 ${URL:0:3}"
    fi 
}

function test_survey {
    testcase_survey1 $1
    testcase_survey2 $1
}

if [[ -f "/tmp/survey_test.ipc" ]]; then
    rm -f "/tmp/survey_test.ipc"
fi

test_survey "tcp://127.0.0.1:5454"
#test_survey "ipc:///tmp/survey_test.ipc"

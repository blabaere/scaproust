#!/bin/bash
# 
# TEST CASES
COMPAT_PATH="$( cd "$(dirname "$0")" ; pwd -P )"
EXAMPLE_PATH="$( cd "$COMPAT_PATH/../target/debug/examples" ; pwd -P )"

# TEST CASE 1
msg="toto raoul simone"
./target/debug/examples/td-pipeline node0 tcp://127.0.0.1:5454 > /tmp/pipeline_tc_1.log & node0=$!
nanocat --push --connect tcp://127.0.0.1:5454 --data "$msg" > /dev/null -i2 & ncat=$!
sleep 0.5 && kill $ncat && kill $node0
result=`cat /tmp/pipeline_tc_1.log`
expected=`cat $COMPAT_PATH/pipeline_tc_1_expected.log`
if [[ $result == $expected ]]; then
    echo "test case 1 SUCCEEDED !" 
else
    echo "test case 1 FAILED !" 
fi

# TEST CASE 2
msg="cornofulgur"
nanocat --pull --bind tcp://127.0.0.1:5454 --ascii > testcase2.log & ncat=$!
./target/debug/examples/td-pipeline node1 tcp://127.0.0.1:5454 "$msg" > /dev/null & node1=$!
sleep 0.5 && kill $ncat && kill $node1

#!/bin/bash
# 
# TEST CASES

# TEST CASE 1
msg="toto raoul simone"
./target/debug/examples/td-pipeline node0 tcp://127.0.0.1:5454 > pipeline_tcp.log & node0=$!
nanocat --push --connect tcp://127.0.0.1:5454 --data "$msg" > /dev/null -i2 & ncat=$!
sleep 0.5 && kill $ncat && kill $node0
result=`cat pipeline_tcp.log`
expected=`cat expected.log`
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

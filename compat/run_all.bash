#!/bin/bash

COMPAT_PATH="$( cd "$(dirname "$0")" ; pwd -P )"

$COMPAT_PATH/pair.bash
$COMPAT_PATH/pipeline.bash
$COMPAT_PATH/pubsub.bash
$COMPAT_PATH/reqrep.bash
$COMPAT_PATH/survey.bash

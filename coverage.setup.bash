#!/bin/bash

wget https://github.com/SimonKagstrom/kcov/archive/master.tar.gz
tar xzf master.tar.gz
rm master.tar.gz
mkdir kcov-master/build
pushd kcov-master/build
cmake ..
make
make install DESTDIR=../tmp
popd

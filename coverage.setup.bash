#!/bin/bash

# sudo apt-get install libdw-dev
# sudo apt-get install elfutils-dev
# sudo apt-get install elfutils
# sudo apt-get install libiberty-dev
# sudo apt-get install libelf-dev
# sudo apt-get install binutils-dev
# sudo apt-get install libssl-dev

wget https://github.com/SimonKagstrom/kcov/archive/master.tar.gz
tar xzf master.tar.gz
rm master.tar.gz
mkdir kcov-master/build
pushd kcov-master/build
cmake ..
make
make install DESTDIR=../tmp
popd

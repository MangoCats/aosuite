#!/bin/bash
#
# https://git.gnupg.org/cgi-bin/gitweb.cgi?p=gnupg.git;a=blob;f=README
# https://stackoverflow.com/questions/14866333/building-linking-libgcrypt-for-mingw

set -e
echo ""
echo "build-w64.sh"
echo ""
set -x

export LD_LIBRARY_PATH=/usr/local/lib:$LD_LIBRARY_PATH

./wipe-w64.sh

pushd npth
autoreconf -vfi
./autogen.sh --build-w64
make -j
sudo make install
popd

pushd libgpg-error
autoreconf -vfi
./autogen.sh --build-w64
make -j
sudo make install
popd

pushd libgcrypt
autoreconf -vfi
./autogen.sh --build-w64
make -j
sudo make install
popd

pushd libksba
autoreconf -vfi
./autogen.sh --build-w64
make -j
sudo make install
popd

pushd libassuan
autoreconf -vfi
./autogen.sh --build-w64
make -j
sudo make install
popd

pushd gnupg
autoreconf -vfi
./autogen.sh --build-w64
make -j
sudo make install
popd

pushd gpgme
autoreconf -vfi
./autogen.sh --build-w64
make -j
sudo make install
popd

set +x
echo "build-w64.sh complete."

#!/bin/bash
#
# https://git.gnupg.org/cgi-bin/gitweb.cgi?p=gnupg.git;a=blob;f=README
# https://gist.github.com/RogueScholar/cdade8709bb392dbe112f2d60d0763fd

set -e
echo ""
echo "build-2004.sh"
echo ""
set -x

./wipe-2004.sh

export LD_LIBRARY_PATH=/usr/local/lib:$LD_LIBRARY_PATH

pushd npth
./autogen.sh --force
./configure --enable-maintainer-mode
make -j
make check
sudo make install
popd

pushd libgpg-error
./autogen.sh --force
./configure --enable-maintainer-mode
make -j
make check
sudo make install
popd

pushd libgcrypt
./autogen.sh --force
./configure --enable-maintainer-mode
make -j
make check
sudo make install
popd

pushd libksba
./autogen.sh --force
./configure --enable-maintainer-mode
make -j
make check
sudo make install
popd

pushd libassuan
./autogen.sh --force
./configure --enable-maintainer-mode
make -j
make check
sudo make install
popd

pushd gnupg
./autogen.sh --force
./configure --enable-maintainer-mode --enable-all-tests --enable-gpg-is-gpg2
make -j
#pushd bin
#ln -s gpg gpg2
#popd
make check
sudo make install
popd

pushd gpgme
./autogen.sh --force
./configure --enable-maintainer-mode
make -j
make check
sudo make install
popd

sudo ldconfig

set +x
echo "build-2004.sh complete."

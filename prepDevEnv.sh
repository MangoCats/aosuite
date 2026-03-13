#! /bin/bash
# MIT License
#
# Copyright (c) 2021 Assign Onward
#
# Permission is hereby granted, free of charge, to any person obtaining a copy
# of this software and associated documentation files (the "Software"), to deal
# in the Software without restriction, including without limitation the rights
# to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
# copies of the Software, and to permit persons to whom the Software is
# furnished to do so, subject to the following conditions:
#
# The above copyright notice and this permission notice shall be included in all
# copies or substantial portions of the Software.
#
# THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
# IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
# FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
# AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
# LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
# OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
# SOFTWARE.
#
######################################################################
#                                                                    #
# Note: imagemagick requires permission to write PDF files:          #
# do a sudo apt-get install -y imagemagick first, then:              #
# edit /etc/ImageMagick-X/policy.xml to modify the PDF line to read: #
#  <policy domain="coder" rights="read | write" pattern="PDF" />     #
#                                                                    #
######################################################################

set -e
set -x
sudo apt-get update -y
sudo apt-get install -y imagemagick
#echo "TODO: maybe auto-edit the policy.xml file here... until then, do it by hand then edit this file to comment out the exit 1 and run again."
#exit 1
sudo apt-get install -y automake make build-essential bison flex gettext texinfo
sudo apt-get install -y transfig fig2dev gnutls-bin rng-tools adns-tools nettle-dev libncurses5-dev
sudo apt-get install -y libsqlite3-dev libldap2-dev libreadline-dev libgmp-dev libbz2-dev zlib1g-dev
sudo apt-get install -y lib64c-dev lib64z1 lib64z1-dev
sudo apt-get install -y qtcreator
sudo apt-get install -y g++ gcc graphviz
# This one to fix syntax highlighting and compile warnings in qt creator in 20.04
sudo apt-get install -y libclang-common-8-dev

# for gpg:
pushd gpg
./wipe.sh
./build.sh
popd

tar -xf gmp-6.1.2.tar.bz2
pushd gmp-6.1.2
./configure --enable-cxx --disable-fft
make
sudo make install
make check
popd
rm -rf gmp-6.1.2

pushd OpenPGP
make
popd
set +x
echo "prepDevEnv.sh complete."

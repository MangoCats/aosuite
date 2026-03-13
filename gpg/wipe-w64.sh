#!/bin/bash
#
# https://git.gnupg.org/cgi-bin/gitweb.cgi?p=gnupg.git;a=blob;f=README
# sudo apt install bison flex texinfo transfig fig2dev libbz2-dev zlib1g-dev qt5-default

set -e
echo ""
echo "wipe-w64.sh"
echo ""
set -x

sudo rm -rf npth
sudo rm -rf libgpg-error
sudo rm -rf libgcrypt
sudo rm -rf libksba
sudo rm -rf libassuan
sudo rm -rf gnupg
sudo rm -rf gpgme
git submodule init
git submodule update
./setVersions-w64.sh

set +x
echo "wipe-w64.sh complete."

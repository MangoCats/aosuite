#!/bin/bash

set -e
echo ""
echo "setVersions-w64.sh"
echo ""
set -x

pushd npth
git checkout npth-1.6
popd

pushd libgpg-error
git checkout libgpg-error-1.47
popd

pushd libgcrypt
git checkout libgcrypt-1.10.2
popd

pushd libksba
git checkout libksba-1.6.3
popd

pushd libassuan
git checkout libassuan-2.5.5
popd

pushd gnupg
git checkout gnupg-2.4.2
popd

pushd gpgme
git checkout gpgme-1.20.0
popd

set +x
echo "setVersions-w64.sh complete."

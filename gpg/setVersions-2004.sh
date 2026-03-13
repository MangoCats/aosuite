#!/bin/bash

set -e
echo ""
echo "setVersions-2004.sh"
echo ""
set -x

pushd npth
git checkout npth-1.6
popd

pushd libgpg-error
git checkout libgpg-error-1.41
popd

pushd libgcrypt
git checkout libgcrypt-1.9.2
popd

pushd libksba
git checkout libksba-1.5.0
popd

pushd libassuan
git checkout libassuan-2.5.4
popd

pushd gnupg
git checkout gnupg-2.2.27
popd

pushd gpgme
git checkout gpgme-1.15.1
popd

set +x
echo "setVersions-2004.sh complete."

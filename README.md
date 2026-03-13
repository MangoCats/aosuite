Assign Onward

State: Pre-alpha, non functional, still designing.

2021: sort of a cleanish sheet restart, trying more of a bottup up approach.

Goal: To demonstrate a distributed Promise Of Correctness blockchain recording system.

Quickstart:

install the required libraries, for Ubuntu 18.04:
sudo apt-get install libbz2-dev libgcrypt11-dev libgpgme11-dev

Build the OpenPGP library from included source folder (taken as-is from GitHub https://github.com/calccrypto/OpenPGP ):
cd OpenPGP
make gpg-compatible

Open aoSuite.pro in Qt Creator, with Qt 5.10 or later, to build and run the applications found in the apps folder.

Documentation: docs/html/index.html is a good place to start reading the whitepapers.

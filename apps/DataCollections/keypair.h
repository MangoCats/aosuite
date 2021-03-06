/* MIT License
 *
 * Copyright (c) 2018 Assign Onward
 *
 * Permission is hereby granted, free of charge, to any person obtaining a copy
 * of this software and associated documentation files (the "Software"), to deal
 * in the Software without restriction, including without limitation the rights
 * to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
 * copies of the Software, and to permit persons to whom the Software is
 * furnished to do so, subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be included in all
 * copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
 * IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 * FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
 * AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
 * LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
 * OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
 * SOFTWARE.
 */
#ifndef KEYPAIR_H
#define KEYPAIR_H

#include "genericcollection.h"
#include "prikey.h"
#include "pubkey.h"
#include <QPointer>
//#define USE_QPOINTERS

/**
 * @brief The KeyPair class - contains a (hopefully matching) public/private key pair
 */
class KeyPair : public GenericCollection
{
    Q_OBJECT
public:
                KeyPair( QObject *p = nullptr )
                  : GenericCollection( AO_KEYPAIR, p ) {}
                KeyPair( const DataItemBA &di, QObject *p = nullptr );
                KeyPair( const KeyPair &k, QObject *p = nullptr )
                  : GenericCollection( AO_KEYPAIR, p ? p : k.parent() ),
                    pubKey( k.pubKey ), priKey( k.priKey )
                    { insert( pubKey );
                      insert( priKey );
                    }
                KeyPair( PriKey *priKp, PubKey *pubKp, QObject *p = nullptr );
          void  operator = ( const DataItemBA &di );
          bool  isValid() { return pubKey->isValid() && priKey->isValid(); } // TODO: ensure they match
        PubKey *getPubKey() { return pubKey; }
        PriKey *getPriKey() { return priKey; }

private:
#ifdef USE_QPOINTERS
  QPointer<PubKey> pubKey;
  QPointer<PriKey> priKey;
#else
  PubKey *pubKey;
  PriKey *priKey;
#endif
};

#endif // KEYPAIR_H

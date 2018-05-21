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
#ifndef SIGNATURE_H
#define SIGNATURE_H

#include "aotime.h"
#include "datavarlenlong.h"
#include "index.h"
#include "sigecdsa.h"
#include "sigrsa3072.h"

/**
 * @brief The Signature class - multi-container for various types of signatures
 *   of course, a signature type must correspond to the PubKey type to validate.
 */
class Signature : public DataVarLenLong
{
    Q_OBJECT
public:
    explicit  Signature( typeCode_t tc = AO_UNDEFINED_DATAITEM, QObject *p = NULL );
              Signature( const QByteArray &di, QObject *p = NULL );
              Signature( const Signature &s, QObject *p = NULL );
        void  operator = ( const Signature &s ) { sigType = s.sigType; sigEcdsa = s.sigEcdsa; sigRsa3072 = s.sigRsa3072; sigTime = s.sigTime; index = s.index; }
        void  operator = ( const QByteArray &di );
  QByteArray  toDataItem( bool cf = false );
  QByteArray  getSig() const;
        void  setSig( const QByteArray &s, typeCode_t tc = AO_UNDEFINED_DATAITEM );
      AOTime  getTime() const { return sigTime; }
        void  setTime( const AOTime &t ) { sigTime = t; }
  typeCode_t  getType() const { return sigType; }
       Index  getIndex() const { return index; }
        void  setIndex( const Index &i ) { index = i; }

private:
  // Every signature has a time of signing
      AOTime  sigTime;
  // Only one of these types of signatures is used at a time
  typeCode_t  sigType;
    SigEcdsa  sigEcdsa;
  SigRsa3072  sigRsa3072;
  // Index, used in Authorizations to match to index numbers in the participant list
       Index  index;
};

#endif // SIGNATURE_H

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
#ifndef GENESISBLOCK_H
#define GENESISBLOCK_H

#include "datavarlenlong.h"
#include "hash.h"
#include "keyvaluepair.h"
#include <QMap>

#define PropertyMap QMap<KeyValueKey_t,DataItem>

/**
 * @brief The GenesisBlock class - the anchorpoint of a blockchain
 */
class GenesisBlock : public DataVarLenLong
{
    Q_OBJECT
public:
      explicit  GenesisBlock( QByteArray di = QByteArray(), QObject *p = NULL );
                GenesisBlock( const GenesisBlock &r )
                  : DataVarLenLong( AO_GENESIS_BLOCK, QByteArray(), r.parent() ),
                    hash( r.hash ), properties( r.properties ) {}
          void  operator = ( const QByteArray &di );
          Hash  getHash()    const { return  hash; }
          void  setHash( const Hash &h ) { hash = h; }
    QByteArray  toDataItem( bool cf = false );
          bool  isValid() { return hash.isValid(); }
      DataItem  getProp( const KeyValueKey_t &key ) const { return ( properties.contains( key ) ) ? properties.value(key) : DataItem(); }

private:
           Hash  hash;        // hash signature (unique ID) of the genesis block
    PropertyMap  properties;  // Collection of properties that describe the chain
};

#endif // GENESISBLOCK_H
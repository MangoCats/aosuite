/* MIT License
 *
 * Copyright (c) 2021 Assign Onward
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
#include "protocolParser.h"

/**
 * @brief ProtocolParser::isValid
 * @return true if the passed protocol is valid
 */
bool  ProtocolParser::isValid()
{ if ( name().size() < 1 )
    { qWarning( "protocol name not found" );
      return false;
    }
  // TODO: more checks as needed
  return true;
}

/**
 * @brief ProtocolParser::name
 * @return name of the protocol, or empty string if name is not found.
 */
Utf8String  ProtocolParser::name()
{ if ( pr->key() == RCD_ProtocolDef_o )
    { BlockValueObject *bvo = qobject_cast<BlockValueObject *>(pr->value());              if ( bvo == nullptr ) { qWarning( "ProtocolParser::name() bad cast bvo 1" ); return ""; }
                                    if ( !bvo->contains( RCD_hash_o ) )                                         { qWarning(     "hash_o not found" );                  return ""; }
      bvo = qobject_cast<BlockValueObject *>(bvo->value( RCD_hash_o ));                   if ( bvo == nullptr ) { qWarning( "ProtocolParser::name() bad cast bvo 2" ); return ""; }
                                    if ( !bvo->contains( RCD_hashInfo_o ) )                                     { qWarning( "hashInfo_o not found" );                  return ""; }
      bvo = qobject_cast<BlockValueObject *>(bvo->value( RCD_hashInfo_o ));               if ( bvo == nullptr ) { qWarning( "ProtocolParser::name() bad cast bvo 3" ); return ""; }
                                                      if ( !bvo->contains( RCD_text_s ) )                       { qWarning(     "text_s not found" );                  return ""; }
      BlockValueString *bvs = qobject_cast<BlockValueString *>(bvo->value( RCD_text_s )); if ( bvs == nullptr ) { qWarning( "ProtocolParser::name() bad cast bvs" );   return ""; }
      return bvs->value();
    }
  qWarning( "Expected ProtocolDef_o as the protocol key" );
  return "";
}

/**
 * @brief ProtocolUser::setProtocol - in the single protocol ecosystem model, the incoming
 *   protocol is the only one that the ProtocolUser will be working with.
 * @param p - protocol to set
 */
void ProtocolUser::setProtocol( BaoSerial p )
{ if ( pp )
    pp->deleteLater();
  pp = new ProtocolParser( p );
  if ( !pp->isValid() ) qWarning( "protocol not valid, not set." ); else
    { protocolSet();
      emit newName( QString::fromUtf8( pp->name() ) );
    }
}

// TODO: instead of coding navigation into the protocol, make it data driven - perhaps by a protocol definition
/**
 * @brief ProtocolActor::protocolSet - a new protocol has been set in the protocol parser
 *   get ths actor's sendable and receivable items.
 */
void  ProtocolActor::protocolSet()
{ if ( pp     == nullptr ) { qWarning( "ProtocolActor::protocolSet() error 1" ); return; }
  if ( pp->pr == nullptr ) { qWarning( "ProtocolActor::protocolSet() error 2" ); return; }
  Utf8String actor = dict.nameFromCode( actTyp );
  QList<RiceyCode> items;
  if ( pp->pr->key() == RCD_ProtocolDef_o )
    { BlockValueObject *bvi = nullptr;
      BlockValueObject *bvo = qobject_cast<BlockValueObject *>(pp->pr->value());
      if ( bvo == nullptr )                          { qWarning( "ProtocolActor::protocolSet() Error 3"  ); return; }
      if ( !bvo->contains( RCD_hashedOb_o ) )        { qWarning( "ProtocolActor::protocolSet() Error 4"  ); return; }
      bvo = qobject_cast<BlockValueObject *>(bvo->value( RCD_hashedOb_o ));
      if ( bvo == nullptr )                          { qWarning( "ProtocolActor::protocolSet() Error 4.5" ); return; }
      if ( !bvo->contains( RCD_ItemsDef_o ) )        { qWarning( "ProtocolActor::protocolSet() Error 5"  ); return; }
      bvi = qobject_cast<BlockValueObject *>(bvo->value( RCD_ItemsDef_o ));
      if ( bvi == nullptr )                          { qWarning( "ProtocolActor::protocolSet() Error 5.5" ); return; }
      if ( !bvo->contains( RCD_ActorsDef_o ) )       { qWarning( "ProtocolActor::protocolSet() Error 6"  ); return; }

      // Populate the actor definition with its sendable and receivable item structure definitions
      bvo = qobject_cast<BlockValueObject *>(bvo->value( RCD_ActorsDef_o ));
      if ( bvo == nullptr )                          { qWarning( "ProtocolActor::protocolSet() Error 6.5" ); return; }
      if ( !bvo->contains( actTyp ) )                { qWarning( "ProtocolActor::protocolSet() actor %s not defined in protocol", actor.data() ); return; }
      bvo = qobject_cast<BlockValueObject *>(bvo->value( actTyp ));
      if ( bvo == nullptr )                          { qWarning( "ProtocolActor::protocolSet() Error 7" ); return; }

      if ( !bvo->contains( RCD_sendableItems_C ) )   { qWarning( "ProtocolActor::protocolSet() Error 8" ); return; }
      BlockValueRiceyCodeArray *bvc = qobject_cast<BlockValueRiceyCodeArray *>(bvo->value().value( RCD_sendableItems_C ));
      if ( bvc == nullptr )                          { qWarning( "ProtocolActor::protocolSet() Error 8.5" ); return; }
      if ( bvc->size() < 1 )                         { qWarning( "ProtocolActor::protocolSet() Warning 9 - no sendable items" ); }
       else
        { if ( sendableItemDefs )
            sendableItemDefs->deleteLater();
          sendableObTypes.clear();
          sendableContents.clear();
          sendableItemDefs = new BlockValueObject( this );
          items = bvc->value(); // populate sendableItemDefs for actTyp
          foreach( RiceyCode item, items )
            { Utf8String itNam = dict.nameFromCode( item );
              if ( !bvi->contains( item ) )
                { qWarning( "%s sendable item %s not found", actor.data(), itNam.data() );
                }
               else
                { BlockValueObject *bviv = qobject_cast<BlockValueObject *>(bvi->value( item ));
                  if ( bviv == nullptr ) qWarning( "ProtocolActor::protocolSet() Error 9.5" ); else
                    { if ( !sendableItemDefs->insert( riceToInt(item), bviv->value() ) )
                        qWarning( "%s sendable item %s failed to insert", actor.data(), itNam.data() );
                    }
                }
            }
        } // if ( bvc->size() < 1 ) else
      connectSendableItems();

      if ( !bvo->contains( RCD_receivableItems_C ) ) { qWarning( "ProtocolActor::protocolSet() Error 10" ); return; }
      bvc = (BlockValueRiceyCodeArray *)bvo->value( RCD_receivableItems_C );
      if ( bvc->size() < 1 )                         { qWarning( "ProtocolActor::protocolSet() Warning 11 - no receivable items" ); }
       else
        { if ( receivableItemDefs )
            receivableItemDefs->deleteLater();
          receivableObTypes.clear();
          receivableContents.clear();
          receivableItemDefs = new BlockValueObject( this );
          items = bvc->value();
          foreach( RiceyCode item, items )
            { Utf8String itNam = dict.nameFromCode( item );
              if ( !bvi->contains( item ) )
                { qWarning( "%s receivable item %s not found", actor.data(), itNam.data() ); }
               else
                { BlockValueObject *bviv = qobject_cast<BlockValueObject *>(bvi->value( item ));
                  if ( bviv == nullptr ) qWarning( "ProtocolActor::protocolSet() Error 12" ); else
                    { if ( !receivableItemDefs->insert( riceToInt(item), bviv->value() ) )
                        qWarning( "%s receivable item %s failed to insert", actor.data(), itNam.data() );
                    }
                }
            }
        } // if ( bvc->size() < 1 ) else
    } // if ( pp->pr->key() == RCD_ProtocolDef_o )
  connectReceivableItems();
  emit newProtocolSet(); // Enables ui to react to the new protocol items, lack of items.
  emit transactionRecord( QString( "Protocol set: %1" ).arg( QString::fromUtf8( pp->name() ) ) ); // TODO: maybe add a few descriptives...
}

/**
 * @brief ProtocolActor::connectSendableItems - navigate through each of the sendableItemDefs
 *   populate the lists of ObTypes and Contents
 *
 */
void  ProtocolActor::connectSendableItems()
{ Utf8String actor = dict.nameFromCode( actTyp );
  QList<RiceyInt> keys = sendableItemDefs->keys();
  foreach ( RiceyInt key, keys )
    { BlockValueObject *si = qobject_cast<BlockValueObject *>(sendableItemDefs->value( key ));
      if ( si == nullptr )                              qWarning( "%s sendable item nullptr", actor.data() ); else
        { Utf8String siName = dict.nameFromCode( si->vKey() );
          if ( !si->contains( RCD_ItemStructure_o ) )   qWarning( "%s sendable %s lacks structure", actor.data(), siName.data() ); else
            { BlockValueObject *itSt = qobject_cast<BlockValueObject *>(si->value( RCD_ItemStructure_o ));
              if ( itSt == nullptr )                    qWarning( "%s sendable ItemStructure %s nullptr", actor.data(), siName.data() ); else
                { QList<RiceyInt> itStKeys = itSt->keys();
                  if ( itStKeys.size() != 1 )           qWarning( "%s sendable ItemStructure %s %lld keys, should be 1"       , actor.data(), siName.data(), itStKeys.size() ); else
                    { if ( key != itStKeys.at(0) )      qWarning( "%s sendable ItemStructure %s key mismatch 0x%llx vs 0x%llx", actor.data(), siName.data(), key, itStKeys.at(0) ); else
                        sendableObTypes.append( key );
                    }
                }
            } // si contains ItemStructure_o
          if ( !si->contains( RCD_ItemContents_O ) )    qWarning( "%s sendable ItemContents %s lacks contents", actor.data(), siName.data() ); else
            { BlockValueObjectArray *icA = qobject_cast<BlockValueObjectArray *>(si->value( RCD_ItemContents_O ));
              if ( icA == nullptr )                     qWarning( "%s sendable ItemContents %s nullptr", actor.data(), siName.data() ); else
                { QList<BlockObjectMap> icObs = icA->value();
                  foreach ( BlockObjectMap icOm, icObs )
                    { if ( icOm.contains( RCD_OpDataLink_C ) )
                        { BlockValueRiceyCodeArray *opDataLink = qobject_cast<BlockValueRiceyCodeArray *>(icOm[RCD_OpDataLink_C]);
                          if ( opDataLink->size() < 2 ) qWarning( "%s sendable ItemContents %s not enough items %d", actor.data(), siName.data(), opDataLink->size() ); else
                            sendableContents.append( riceToInt( opDataLink->at(0) ) );
                        }
                    }
                }
            } // si contains ItemContents_O
        }    // si not nullptr
    }       // foreach key in keys
}

/**
 * @brief ProtocolActor::connectReceivableItems - navigate through each of the receivableItemDefs
 *   populate the lists of ObTypes and Contents
 */
void  ProtocolActor::connectReceivableItems()
{ Utf8String actor = dict.nameFromCode( actTyp );
  QList<RiceyInt> keys = receivableItemDefs->keys();
  foreach ( RiceyInt key, keys )
    { BlockValueObject *ri = qobject_cast<BlockValueObject *>(receivableItemDefs->value( key ));
      if ( ri == nullptr )                              qWarning( "%s receivable item nullptr", actor.data() ); else
        { Utf8String siName = dict.nameFromCode( ri->vKey() );
          if ( !ri->contains( RCD_ItemStructure_o ) )   qWarning( "%s receivable %s lacks structure", actor.data(), siName.data() ); else
           { BlockValueObject *itSt = qobject_cast<BlockValueObject *>(ri->value( RCD_ItemStructure_o ));
             if ( itSt == nullptr )                     qWarning( "%s receivable ItemStructure %s nullptr", actor.data(), siName.data() ); else
               { QList<RiceyInt> itStKeys = itSt->keys();
                 if ( itStKeys.size() != 1 )            qWarning( "%s receivable ItemStructure %s %lld keys, should be 1",        actor.data(), siName.data(), itStKeys.size() ); else
                   { if ( key != itStKeys.at(0) )       qWarning( "%s receivable ItemStructure %s key mismatch 0x%llx vs 0x%llx", actor.data(), siName.data(), key, itStKeys.at(0) ); else
                       receivableObTypes.append( key );
                   }
               }
           } // ri contains ItemStructure_o
          if ( !ri->contains( RCD_ItemContents_O ) )    qWarning( "%s receivable %s lacks contents", actor.data(), siName.data() ); else
            { BlockValueObjectArray *icA = qobject_cast<BlockValueObjectArray *>(ri->value( RCD_ItemContents_O ));
              if ( icA == nullptr )                     qWarning( "%s receivable ItemContents %s nullptr", actor.data(), siName.data() ); else
                { QList<BlockObjectMap> icObs = icA->value();
                  foreach ( BlockObjectMap icOm, icObs )
                    { if ( icOm.contains( RCD_OpDataLink_C ) )
                        { BlockValueRiceyCodeArray *opDataLink = qobject_cast<BlockValueRiceyCodeArray *>(icOm[RCD_OpDataLink_C]);
                          if ( opDataLink->size() < 2 ) qWarning( "%s receivable ItemContents %s not enough items %d", actor.data(), siName.data(), opDataLink->size() ); else
                            { receivableContents.append( riceToInt( opDataLink->at(0) ) );
                              // qWarning( "%s receivable ItemContents %s added 0x%llx", actor.data(), siName.data(), riceToInt( opDataLink->at(0) ) );
                            }
                        }
                    }
                }
            } // ri contains ItemContents_O
        }    // ri not nullptr
    }       // foreach key in keys
}

/**
 * @brief ProtocolActor::get
 * @param bom - map to look for key in
 * @param key - key to get value for
 * @param v - qint64 to store value in, if nothing goes wrong
 * @param r - is this a required parameter?  Controls warning message output
 * @return true if all goes well
 */
bool  ProtocolActor::get( const BlockObjectMap &bom, RiceyInt key, qint64 &v, bool r )
{ if ( !bom.contains( key ) )
    { if ( r )
        { Utf8String kn = dict.nameOrHexFromCode( key );
          qWarning( "ProtocolActor::get( qint64 ) %s not found", kn.data() );
          return false;
        }
      return true; // not present, not required
    }
  if ( (key & RDT_OBTYPEMASK) != RDT_INT64 )
    { Utf8String kn = dict.nameOrHexFromCode( key );
      qWarning( "ProtocolActor::get( qint64 ) %s unexpected key type", kn.data() );
      return false;
    }
  if ( bom.value( key ) == nullptr )
    { Utf8String kn = dict.nameOrHexFromCode( key );
      qWarning( "ProtocolActor::get( qint64 ) %s nullptr", kn.data() );
      return false;
    }
  BlockValueInt64 *bvip = qobject_cast<BlockValueInt64 *>(bom.value( key ));
  if ( bvip == nullptr )
    { Utf8String kn = dict.nameOrHexFromCode( key );
      qWarning( "ProtocolActor::get( qint64 ) %s bvip nullptr", kn.data() );
      return false;
    }
  v = bvip->value();
  return true;
}

bool  ProtocolActor::get( const BlockObjectMap &bom, RiceyInt key, RiceyInt &v, bool r )
{ if ( !bom.contains( key ) )
    { if ( r )
        { Utf8String kn = dict.nameOrHexFromCode( key );
          qWarning( "ProtocolActor::get( RiceyInt ) %s not found", kn.data() );
          return false;
        }
      return true; // not present, not required
    }
  if ( (key & RDT_OBTYPEMASK) != RDT_RICEYINT )
    { Utf8String kn = dict.nameOrHexFromCode( key );
      qWarning( "ProtocolActor::get( RiceyInt ) %s unexpected key type", kn.data() );
      return false;
    }
  if ( bom.value( key ) == nullptr )
    { Utf8String kn = dict.nameOrHexFromCode( key );
      qWarning( "ProtocolActor::get( RiceyInt ) %s nullptr", kn.data() );
      return false;
    }
  BlockValueRiceyInt *rip = qobject_cast<BlockValueRiceyInt *>(bom.value( key ));
  if ( rip == nullptr )
    { Utf8String kn = dict.nameOrHexFromCode( key );
      qWarning( "ProtocolActor::get( RiceyInt ) %s rip nullptr", kn.data() );
      return false;
    }
  v = rip->value();
  return true;
}

bool  ProtocolActor::get( const BlockObjectMap &bom, RiceyInt key, QByteArray &v, bool r )
{ if ( !bom.contains( key ) )
    { if ( r )
        { Utf8String kn = dict.nameOrHexFromCode( key );
          qWarning( "ProtocolActor::get( QByteArray ) %s not found", kn.data() );
          return false;
        }
      return true; // not present, not required
    }
  if ( bom.value( key ) == nullptr )
    { Utf8String kn = dict.nameOrHexFromCode( key );
      qWarning( "ProtocolActor::get( QByteArray ) %s nullptr", kn.data() );
      return false;
    }
  if ( (key & RDT_OBTYPEMASK) == RDT_BYTEARRAY )
    { BlockValueByteArray *ba = qobject_cast<BlockValueByteArray *>(bom.value( key ));
      if ( ba == nullptr )
        { Utf8String kn = dict.nameOrHexFromCode( key );
          qWarning( "ProtocolActor::get( QByteArray ) %s ba nullptr", kn.data() );
          return false;
        }
      v = ba->value();
      return true;
    }
  if ( (key & RDT_OBTYPEMASK) == RDT_STRING )
    { BlockValueString *s = qobject_cast<BlockValueString *>(bom.value( key ));
      if ( s == nullptr )
        { Utf8String kn = dict.nameOrHexFromCode( key );
          qWarning( "ProtocolActor::get( QByteArray ) %s s nullptr", kn.data() );
          return false;
        }
      v = s->value();
      return true;
    }
  if ( (key & RDT_OBTYPEMASK) == RDT_RCODE )
    { BlockValueRiceyCode *rc = qobject_cast<BlockValueRiceyCode *>(bom.value( key ));
      if ( rc == nullptr )
        { Utf8String kn = dict.nameOrHexFromCode( key );
          qWarning( "ProtocolActor::get( QByteArray ) %s rc nullptr", kn.data() );
          return false;
        }
      v = rc->value();
      return true;
    }
  Utf8String kn = dict.nameOrHexFromCode( key );
  qWarning( "ProtocolActor::get( QByteArray ) %s unhandled key type", kn.data() );
  return false;
}

bool  ProtocolActor::get( const BlockObjectMap &bom, RiceyInt key, MP_INT &v, bool r )
{ if ( !bom.contains( key ) )
    { if ( r )
        { Utf8String kn = dict.nameOrHexFromCode( key );
          qWarning( "ProtocolActor::get( MP_INT ) %s not found", kn.data() );
          return false;
        }
      return true; // not present, not required
    }
  if ( (key & RDT_OBTYPEMASK) != RDT_MPZ )
    { Utf8String kn = dict.nameOrHexFromCode( key );
      qWarning( "ProtocolActor::get( MP_INT ) %s unexpected key type", kn.data() );
      return false;
    }
  if ( bom.value( key ) == nullptr )
    { Utf8String kn = dict.nameOrHexFromCode( key );
      qWarning( "ProtocolActor::get( MP_INT ) %s nullptr", kn.data() );
      return false;
    }
  BlockValueMPZ *mpz = qobject_cast<BlockValueMPZ *>(bom.value( key ));
  if ( mpz == nullptr )
    { Utf8String kn = dict.nameOrHexFromCode( key );
      qWarning( "ProtocolActor::get( MP_INT ) %s mpz nullptr", kn.data() );
      return false;
    }
  v = mpz->value();
  return true;
}

bool  ProtocolActor::get( const BlockObjectMap &bom, RiceyInt key, MP_RAT &v, bool r )
{ if ( !bom.contains( key ) )
    { if ( r )
        { Utf8String kn = dict.nameOrHexFromCode( key );
          qWarning( "ProtocolActor::get( MP_RAT ) %s not found", kn.data() );
          return false;
        }
      return true; // not present, not required
    }
  if ( (key & RDT_OBTYPEMASK) != RDT_MPQ )
    { Utf8String kn = dict.nameOrHexFromCode( key );
      qWarning( "ProtocolActor::get( MP_RAT ) %s unexpected key type", kn.data() );
      return false;
    }
  if ( bom.value( key ) == nullptr )
    { Utf8String kn = dict.nameOrHexFromCode( key );
      qWarning( "ProtocolActor::get( MP_RAT ) %s nullptr", kn.data() );
      return false;
    }
  BlockValueMPQ *mpq = qobject_cast<BlockValueMPQ *>(bom.value( key ));
  if ( mpq == nullptr )
    { Utf8String kn = dict.nameOrHexFromCode( key );
      qWarning( "ProtocolActor::get( MP_RAT ) %s mpq nullptr", kn.data() );
      return false;
    }
  v = mpq->value();
  return true;
}

/**
 * @brief ProtocolActor::prepare - one handler for all byte array types, distinguish by key
 * @param bom - map to insert new object in
 * @param key - key type to insert new object under
 * @param ba - the byte array to copy into an object in the bom
 * @param parent - parent for the ValueBase object created (optional)
 */
void  ProtocolActor::prepare( BlockObjectMap &bom, RiceyInt key, const QByteArray &ba, QObject *parent )
{ if ( bom.contains( key ) )
    { Utf8String kn = dict.nameOrHexFromCode( key );
      qWarning( "ProtocolActor::prepare( QByteArray ) map already contains key %s", kn.data() );
      return;
    }
  if ( (key & RDT_OBTYPEMASK) == RDT_BYTEARRAY )
    { bom.insert( key, new BlockValueByteArray( ba, parent ) );
      return;
    }
  if ( (key & RDT_OBTYPEMASK) == RDT_STRING )
    { bom.insert( key, new BlockValueString( ba, parent ) );
      return;
    }
  if ( (key & RDT_OBTYPEMASK) == RDT_RCODE )
    { bom.insert( key, new BlockValueRiceyCode( ba, parent ) );
      return;
    }
  Utf8String kn = dict.nameOrHexFromCode( key );
  qWarning( "ProtocolActor::prepare( QByteArray ) unhandled key type %s", kn.data() );
}

void  ProtocolActor::prepare( BlockObjectMap &bom, RiceyInt key, qint64 i, QObject *parent )
{ if ( bom.contains( key ) )
    { Utf8String kn = dict.nameOrHexFromCode( key );
      qWarning( "ProtocolActor::prepare( qint64 ) map already contains key %s, will not create/insert a new one", kn.data() );
      return;
    }
  if ( (key & RDT_OBTYPEMASK) != RDT_INT64 )
    { Utf8String kn = dict.nameOrHexFromCode( key );
      qWarning( "ProtocolActor::prepare( qint64 ) key type mismatch, will not create BlockValueInt64 for %s", kn.data() );
      return;
    }
  bom.insert( key, new BlockValueInt64( i, parent ) );
}

void  ProtocolActor::prepare( BlockObjectMap &bom, RiceyInt key, RiceyInt ri, QObject *parent )
{ if ( bom.contains( key ) )
    { Utf8String kn = dict.nameOrHexFromCode( key );
      qWarning( "ProtocolActor::prepare( RiceyInt ) map already contains key %s, will not create/insert a new one", kn.data() );
      return;
    }
  if ( (key & RDT_OBTYPEMASK) != RDT_INT64 )
    { Utf8String kn = dict.nameOrHexFromCode( key );
      qWarning( "ProtocolActor::prepare( RiceyInt ) key type mismatch, will not create BlockValueRiceyInt for %s", kn.data() );
      return;
    }
  bom.insert( key, new BlockValueRiceyInt( ri, parent ) );
}

void  ProtocolActor::prepare( BlockObjectMap &bom, RiceyInt key, const MP_INT &mpi, QObject *parent )
{ if ( bom.contains( key ) )
    { Utf8String kn = dict.nameOrHexFromCode( key );
      qWarning( "ProtocolActor::prepare( MP_INT ) map already contains key %s, will not create/insert a new one", kn.data() );
      return;
    }
  if ( (key & RDT_OBTYPEMASK) != RDT_INT64 )
    { Utf8String kn = dict.nameOrHexFromCode( key );
      qWarning( "ProtocolActor::prepare( MP_INT ) key type mismatch, will not create BlockValueMPZ for %s", kn.data() );
      return;
    }
  bom.insert( key, new BlockValueMPZ( mpi, parent ) );
}

void  ProtocolActor::prepare( BlockObjectMap &bom, RiceyInt key, const MP_RAT &mpr, QObject *parent )
{ if ( bom.contains( key ) )
    { Utf8String kn = dict.nameOrHexFromCode( key );
      qWarning( "ProtocolActor::prepare( MP_RAT ) map already contains key %s, will not create/insert a new one", kn.data() );
      return;
    }
  if ( (key & RDT_OBTYPEMASK) != RDT_INT64 )
    { Utf8String kn = dict.nameOrHexFromCode( key );
      qWarning( "ProtocolActor::prepare( MP_RAT ) key type mismatch, will not create BlockValueMPQ for %s", kn.data() );
      return;
    }
  bom.insert( key, new BlockValueMPQ( mpr, parent ) );
}

/**
 * @brief ProtocolActor::compose - organize inputs into an obType packet according to the currently set protocol.
 *   Composed items are for sending, so they only consider sendable items.
 *   TODO: a logical nesting structure that works with the inputs map
 * @param obType - type of packet to make
 * @param inputs - data to fill the packet with
 * @return serialized packet - or empty array if there was a problem.
 */
BaoSerial  ProtocolActor::compose( RiceyInt obType, const BlockObjectMap &inputs )
{ Utf8String obName = dict.nameFromCode( obType );
  BaoSerial err; // empty bao indicates error
  if ( !sendableItemDefs->contains( obType ) )
    { qWarning( "ProtocolActor::compose( 0x%llx ) not found in sendableItemDefs.", obType );
      return err;
    }
  BlockValueObject    itemDef( sendableItemDefs->value( obType )             ->bao() );
  BlockValueObject itemStruct( itemDef          .value( RCD_ItemStructure_o )->bao() );
  BlockValueObject         ob( itemStruct       .value( obType )             ->bao() );
  BlockValueObjectArray itemContents;
          itemContents.setBao( itemDef          .value( RCD_ItemContents_O ) ->bao() );
  for ( qint32 i = 0; i < itemContents.size(); i++ )
    { BlockObjectMap contents = itemContents.at(i);
      if ( !contents.contains( RCD_OpDataLink_C ) )                  qWarning( "unexpected: %s contents[%d] doesn't have an OpDataLink_C", obName.data(), i ); else
        { BlockValueRiceyCodeArray dataLink; dataLink.setBao( contents[RCD_OpDataLink_C]->bao() );
          if ( dataLink.size() < 2 )                                 qWarning( "%s dataLink[%d] size %d", obName.data(), i, dataLink.size() ); else
            { if ( !inputs.contains( riceToInt( dataLink.at(0) ) ) ) qWarning( "%s dataLink[%d] defined item 0x%llx not found in inputs", obName.data(), i, riceToInt( dataLink.at(0) ) ); else
                { if ( !populate( ob, dataLink, inputs[riceToInt(dataLink.at(0))] ) )
                    { qWarning( "problem during populate" );
                      return err;
                    }
                }
            } // dataLink size 2 or higher
        }    // contents contains OpDataLink
    }       // for i = 0 to itemContents.size()-1
  KeyValuePair kvp( obType, &ob );
  return kvp.bao();
}

/**
 * @brief ProtocolActor::populate
 * @param ob       - object to populate value in
 * @param dataLink - description of where to put the value
 * @param v        - value to be populated
 * @return true if successful
 */
bool  ProtocolActor::populate( BlockValueObject &ob, const BlockValueRiceyCodeArray &dataLink, ValueBase *v )
{ if ( dataLink.size() < 2 )
    { qWarning( "dataLink undersized %d", dataLink.size() );
      return false;
    }
  qint32 i = 1;
  ValueBase *ptr = &ob;
  while ( i < dataLink.size() )
    { // TODO: enable navigation to parents as well as children
      // TODO2: navigation into array elements
      BlockValueObject *bvp = qobject_cast<BlockValueObject *>(ptr);
      if ( bvp != nullptr )
        { if ( !bvp->contains( dataLink.at(i) ) ) qWarning( "navigation could not find item %d 0x%llx", i, riceToInt( dataLink.at(i) ) ); else
            ptr = bvp->value( dataLink.at(i) );
        }
      i++;
    }
  if ( ptr == nullptr )
    { qWarning( "populate ptr is nullptr" );
      return false;
    }
  if ( ptr->type() != v->type() )
    { qWarning( "mismatch ptr type %d v type %d", ptr->type(), v->type() );
      return false;
    }
  ptr->setBao( v->bao() );
  return true;
}

/**
 * @brief ProtocolActor::dispose - done with all Value objects, free the memory at next event loop idle point
 * @param bom - map of objects to deleteLater..
 */
void  ProtocolActor::dispose( BlockObjectMap &bom )
{ foreach( ValueBase *vb, bom )
    vb->deleteLater();
  bom.clear();
}



/**
 * @brief ProtocolActor::extract
 * @param bao - packet to extract receivable values from
 * @return map of receivable values, empty if there is a problem
 */
BlockObjectMap ProtocolActor::extract( BaoSerial bao )
{ BlockObjectMap bom;
  if ( bao.size() < 1 )                      { qWarning( "ProtocolActor::extract() received empty bao" );                          return bom; }
  KeyValuePair *kvp = new KeyValuePair( bao, this );
  if ( kvp == nullptr )                      { qWarning( "kvp nullptr" );                                                          return bom; }
  if ( kvp->type() == RDT_NULL )             { qWarning( "kvp type is NULL" );                                 kvp->deleteLater(); return bom; }
  RiceyInt obk = kvp->key();
  Utf8String ks = intToRice( obk ).toHex(); // Key ricey code hex as a string, more readable...
  BlockValueObject *bob = qobject_cast<BlockValueObject *>(kvp->value());
  if ( bob == nullptr )                      { qWarning( "bob nullptr" );                                      kvp->deleteLater(); return bom; }
  if ( receivableItemDefs == nullptr )       { qWarning( "receivableItemDefs nullptr" );                       kvp->deleteLater(); return bom; }
  if ( !receivableObTypes.contains(obk) )    { qWarning( "key %s not found in receivableObTypes", ks.data() ); kvp->deleteLater(); return bom; }
  if ( !receivableItemDefs->contains(obk) )  { qWarning( "rid does not contain %s", ks.data() );               kvp->deleteLater(); return bom; }
  BlockValueObject *ob = qobject_cast<BlockValueObject *>(receivableItemDefs->value(obk));
  if ( ob == nullptr )                       { qWarning( "%s ob nullptr", ks.data() );                         kvp->deleteLater(); return bom; }
  if ( !ob->contains( RCD_ItemContents_O ) ) { qWarning( "ob does not contain ItemContents" );                 kvp->deleteLater(); return bom; }
  BlockValueObjectArray *ic = qobject_cast<BlockValueObjectArray *>(ob->value( RCD_ItemContents_O ));
  if ( ic == nullptr )                       { qWarning( "%s ic nullptr", ks.data() );                         kvp->deleteLater(); return bom; }
  if ( ic->size() < 1 )                        qWarning( "ic is empty?" );
  QList<BlockObjectMap> icm = ic->value();  // Work through ItemContents_O
  foreach( BlockObjectMap ici, icm )
    if ( ici.contains( RCD_OpDataLink_C ) ) // one of the OpDataLink_Cs in ItemContents_O
      { BlockValueRiceyCodeArray *odl = qobject_cast<BlockValueRiceyCodeArray *>(ici.value( RCD_OpDataLink_C ));
        if ( odl == nullptr )                  qWarning( "odl nullptr" ); else
          { if ( odl->size() < 2 )             qWarning( "odl size %d?", odl->size() ); else
              { RiceyInt it = riceToInt( odl->at(0) ); // id of the value being extracted
                ValueBase *vb = extractOne( bob, odl );
                if ( vb == nullptr )           qWarning( "extractOne failed on 0x%llx", it ); else
                  bom.insert( it, vb );
              }
          }
      }
  kvp->deleteLater();
  return bom;
}

/**
 * @brief ProtocolActor::extractOne - pull one specified value out of the object
 * @param bob - object to read into
 * @param odl - describes a route to the value
 * @return value from bob at the end of the route odl, nullptr if there is a problem
 */
ValueBase *ProtocolActor::extractOne( BlockValueObject *bob, BlockValueRiceyCodeArray *odl )
{ if ( bob == nullptr )  { qWarning( "bob nullptr?!?" ); return nullptr; }
  if ( odl == nullptr )  { qWarning( "odl nullptr?!?" ); return nullptr; }
  if ( odl->size() < 2 ) { qWarning( "odl size %d, no path", odl->size() ); return nullptr; }
  qint32 i = 1; // index working down the route
  while ( i < odl->size() )
    { RiceyInt cid = riceToInt( odl->at(i) );
      // TODO: enable navigation to parents as well as children
      // TODO2: navigation into array elements
      if ( !bob->contains( cid ) ) { qWarning( "bob doesn't contain 0x%llx", cid ); return nullptr; }
      if ( i == ( odl->size() - 1 ) )
        return bob->value( cid );
      if ( bob->value( cid )->type() != RDT_OBJECT ) { qWarning( "extractOne only navigates into object trees" ); return nullptr; }
      i++;
    }
  return nullptr;
}

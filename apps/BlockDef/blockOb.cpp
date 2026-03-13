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
#include "blockOb.h"
// #include <QTextCodec> qt5 method
#include <string.h>

// Design Intent:
//
// Block data structures provide an extensible, specifiable format for the
//   communication and storage of data and the common wrappers that accompany
//   it in blockchain operations.  The basic structures resemble .json object
//   formats, including objects, arrays, and individual data elements, but the
//   keys are made to be strongly typed, and specify a larger number of datatypes
//   than standard .json does.  Nonetheless, a block data structure can be expressed
//   in standard .json format and manipulated with common .json tools.
//
// The intended form for operations and communications is designated "bao" 包 - a
//   binary form of the json structures which is both more compact and more specific,
//   there is only one correct bao representation of a given object because:
//   1) duplicate key values are not allowed in a single level of an object container
//   2) key-value elements in an object container are listed in key sort order
//   3) array elements are always listed in the order in which they are saved
//   4) sparse arrays are not allowed, they always start with element 0 (or can be empty)
//   (designated bao because bson implies a certain translation of json which bao is not.)
//
// Further, in a given protocol, only recognized keys (defined in the protocol) are allowed.
//
// Defined data types include:
//   1) MPZ, arbitrary precision integers stored MSB first in BCD, terminated with F for negative and E for positive
//   2) MPQ, arbitrary precision ratio of two integers stored as two MPZ: numerator / denominator
//   5) String, any valid UTF-8 string stored as a rice code length followed by the characters - not including null terminator
//   6) int64, 8 byte signed integers stored LSB first
//   9) RiceyInt, 63 bit unsigned integers stored MSB first in a "Ricey chain" one to 9 bytes long
//   11) Data, any octet stream stored as a rice code length followed by the data
//   12) RCODE, when used as a key - a rice code value which appears in the protocol dictionary
//      note that the following data types use rice code numbers (aka RiceyInts: positive integers from 0 to 2^63-1) to specify lengths in bao code
//   15) Objects, contain 0 or more key-value pairs
//   16-31) arrays of the above, stored as a key plus length specifying 0 or more elements, followed by that number of elements stored as above
//
// State of the code:
//
// Functional, evolving to support functions as needed.

size_t strnlength (const char* s, size_t n)
{ const char *found = (char *)memchr(s, '\0', n);
  return found ? (size_t)(found-s) : n;
}

    Dictionary  dict;        // Single dictionary for the application
BlockValueNull  glob_null;   // Returned when BlockValueObject::valueAt() calls an invalid index

/**
 * @brief ValueBase::removeQuotes
 * @param j - json value to remove quotes from
 * @return j trimmed of whitespace and if " is present at the start and end also remove them.
 */
JsonSerial ValueBase::removeQuotes( const JsonSerial &j )
{ JsonSerial jt = j.trimmed();
  while ( jt.endsWith( '"' ) && jt.startsWith( '"' ) )
    { jt.chop(1);
      jt = jt.mid(1);
      jt = jt.trimmed();
    }
  return jt;
}

/**
 * @brief ValueBase::ensureQuotes
 * @param j
 * @return j trimmed and enclosed in quotes (unless it already was enclosed in quotes)
 */
JsonSerial ValueBase::ensureQuotes( const JsonSerial &j )
{ JsonSerial jt = j.trimmed();
  if ( jt.endsWith( '"' ) && jt.startsWith( '"' ) )
    return jt;
  return "\""+jt+"\"";
}

Utf8String ValueBase::metaText() const
{ QString t;
  t.append( QString( "key %1\n" ).arg( dict.nameFromCode( vKey() ) ) );
  t.append( QString( "idx %1\n" ).arg( idx() ) );
  t.append( QString( "id %1\n"  ).arg( id()  ) );
  if ( isContainer() )    t.append( "isContainer\n" );
  if ( isArray() )        t.append( "isArray\n" );
  if ( isKeyValue() )     t.append( "isKeyValue\n" );
  if ( isKeyValuePair() ) t.append( "isKeyValuePair\n" );
  if ( vbParent )
    t.append( QString( "Parent:\n%1").arg( vbParent->metaText() ) );
  return t.toUtf8();
}

/**
 * @brief ValueBase::newValue
 * @param key - tells the type of value object to return
 * @param parent - optional, parent for the new value object
 * @param vtc - optional, value to copy - when not nullptr, copy the value from this
 * @return pointer to the new value object
 */
ValueBase *ValueBase::newValue( RiceyInt key, ValueBase *vbp, ValueBase *vtc )
{ qint32 t = key & RDT_OBTYPEMASK;
  ValueBase *vbo = nullptr;
  if ( t == RDT_OBJECT          ) vbo = new BlockValueObject( vbp );
  if ( t == RDT_INT64           ) vbo = new BlockValueInt64( vbp );
  if ( t == RDT_RICEYINT        ) vbo = new BlockValueRiceyInt( vbp );
  if ( t == RDT_MPZ             ) vbo = new BlockValueMPZ( vbp );
  if ( t == RDT_MPQ             ) vbo = new BlockValueMPQ( vbp );
  if ( t == RDT_RCODE           ) vbo = new BlockValueRiceyCode( vbp );
  if ( t == RDT_STRING          ) vbo = new BlockValueString( vbp );
  if ( t == RDT_BYTEARRAY       ) vbo = new BlockValueByteArray( vbp );
  if ( t == RDT_OBJECT_ARRAY    ) vbo = new BlockValueObjectArray( vbp );
  if ( t == RDT_INT64_ARRAY     ) vbo = new BlockValueInt64Array( vbp );
  if ( t == RDT_RICEYINT_ARRAY  ) vbo = new BlockValueRiceyIntArray( vbp );
  if ( t == RDT_MPZ_ARRAY       ) vbo = new BlockValueMPZArray( vbp );
  if ( t == RDT_MPQ_ARRAY       ) vbo = new BlockValueMPQArray( vbp );
  if ( t == RDT_RCODE_ARRAY     ) vbo = new BlockValueRiceyCodeArray( vbp );
  if ( t == RDT_STRING_ARRAY    ) vbo = new BlockValueStringArray( vbp );
  if ( t == RDT_BYTEARRAY_ARRAY ) vbo = new BlockValueByteArrayArray( vbp );
  if ( vbo == nullptr )
    qWarning( "unhandled type %d in ValueBase::newValue", t );
   else
    { vbo->setMetaData( key, vbp );
      if ( vtc )
        { if (( vtc->type() & RDT_OBTYPEMASK ) != t )
            qWarning( "type mismatch with %d vtc %d ValueBase::newValue", t, vtc->type() & RDT_OBTYPEMASK );
           else
            vbo->setBao( vtc->bao() );
        }
    }
  return vbo;
}

/**
 * @brief readBao - create a KeyValue from a bao stream
 * @return a KeyValue for the contents of bs, nullptr if there is a problem
 */
KeyValueBase *KeyValueBase::readBao( const BaoSerial &bs, QObject *parent )
{ KeyValueBase *kvb = nullptr;
  if ( bs.size() < 1 )
    { qWarning( "KeyValueBase::readBao() no data passed" );
      return kvb;
    }
  qint32 len = 0;
  bool ok = false;
  RiceyInt k = riceToInt( bs, &len, &ok );
  if ( !ok )
    { qWarning( "KeyValueBase::readBao() problem reading initial key" );
      return kvb;
    }
  if ( !dict.codesContainCode( k ) )
    { qWarning( "KeyValueBase::readBao() key 0x%llx not found in dictionary",k );
      return kvb;
    }
  if ( k & RDT_ARRAY )
    kvb = new KeyValueArray( k, parent );
   else
    kvb = new KeyValuePair( k, parent );
  kvb->setBao( bs );
  return kvb;
}


/**
 * @brief KeyValueBase::setKey
 * @param key - recognized dictonary entry
 * @return number of bytes the key occupied, or -1 if unsuccessful setting key
 */
qint32 KeyValueBase::setKey( const RiceyCode &key )
{ qint32 len = 0;
  bool ok = false;
  RiceyInt k = riceToInt( key, &len, &ok );
  if ( !ok )
    { qWarning( "problem converting key rice code '%s' to int", key.toHex().data() ); return -1; }
  if ( !dict.codesContainCode( k ) )
    { qWarning( "dictionary does not contain key %llu", k ); return -1; }
  setVKey( k );
  return len;
}

/**
 * @brief KeyValuePair::setBao
 * @param b - key followed by a value of type matching the key type
 * @return length of all data read from b to get key and value, or -1 if there was a problem
 */
qint32  KeyValuePair::setBao( const  BaoSerial &b )
{ // qWarning( "KeyValuePair::setBao" );
  qint32 len = setKey( b );
  if ( len < 1 )
    { qWarning( "problem reading key" ); setKey( RCD_null_z ); return -1; }
  qint32 i = len;
  ValueBase *vbo = baoValueByKey( key(), b.mid(i), &len, this );
  if ( vbo == nullptr )
    { qWarning( "KeyValuePair::setBao() problem reading value" ); setKey( RCD_null_z ); return -1; }
  if ( m_value )
    m_value->deleteLater();
  m_value = vbo;
  m_value->setMetaData( key(), this );
  return i+len;
}

/**
 * @brief BlockValueInt64::setBao
 * @param b - byte array which starts with 8 bytes of LittleEndian encoded integer
 * @return number of bytes converted from the BaoSerial stream, -1 if there was a problem
 */
qint32 BlockValueInt64::setBao( const BaoSerial &b )
{ if ( b.size() < 8 )
    { qWarning( "undersized array pased to BlockValueInt64::setBao" ); return -1; }
  QDataStream s(b);
  s.setByteOrder(QDataStream::LittleEndian);
  s >> m_value;
  return 8;
}


/**
 * @brief BlockValueMPZ::toStr
 * @param in - signed integer to convert
 * @return base 10 string, with negative sign prepended when needed
 */
Utf8String  BlockValueMPZ::toStr( const MP_INT &in )
{ size_t sz = mpz_sizeinbase( &in, 10 );
  if ( sz > MAX_MPINT_LEN )
    { qWarning( "oversized MP_INT %d digits", (int)sz ); return Utf8String( "" ); }
  char str[sz+2];
  mpz_get_str( str, 10, &in );
  return Utf8String( str );
}

  /**
 * @brief BlockValueMPZ::toBCD
 * @param in - signed integer to convert
 * @return BCD string, terminated with an xE/ED if positive or xF/FD if negative, EC/FC if there is a problem
 */
 BaoSerial  BlockValueMPZ::toBCD( const MP_INT &in )
{  BaoSerial bcd;
  qint32 sz = mpz_sizeinbase( &in, 10 );
  if ( sz > MAX_MPINT_LEN )
    { qWarning( "oversized MP_INT %d digits", (int)sz ); return Utf8String( "" ); }
  char str[sz+2];
  mpz_get_str( str, 10, &in );
  int len = strnlength( str, MAX_MPINT_LEN );
  if ( len < 1 )
    { bcd.append( (quint8)0xEC ); // empty result, different from 0 which would be 0x0E
      qWarning( "empty string returned from mpz_get_str" );
      return bcd;
    }
  int i = 0;
  bool neg = false;
  if ( str[0] == '-' )
    { neg = true;
      i = 1;
      if ( len == 1 )
        { bcd.append( (quint8)0xFC ); // negative sign only result, different from -0 which would be 0x0F
          qWarning( "lonely negative sign returned from mpz_get_str" );
          return bcd;
        }
    }
  while ( i < len )
    { if ( (i+1) < len )
        { if (( str[i] < '0' ) || ( str[i] > '9' ) || ( str[i+1] < '0' ) || ( str[i+1] > '9' ))
            { qWarning( "unexpected character in mpz_get_str output" );
              bcd.append( neg ? (quint8)0xFC : (quint8)0xEC );
              return bcd;
            }
          bcd.append( ((str[i]-'0') << 4) + (str[i+1]-'0') );
          i += 2;
        }
       else
        { if (( str[i] < '0' ) || ( str[i] > '9' ))
            { qWarning( "unexpected character in mpz_get_str output" );
              bcd.append( neg ? (quint8)0xFC : (quint8)0xEC );
              return bcd;
            }
          bcd.append( ((str[i]-'0') << 4) + ( neg ? 0xF : 0xE ) );
          return bcd;
        }
    }
  bcd.append( ( neg ? 0xFD : 0xED ) );
  return bcd;
}

/**
 * @brief BlockValueMPZ::fromBCD
 * @param bcd - octet stream that starts with a BCD coded integer, as above in toBCD()
 * @param in - (initialized) MP_INT to set the
 * @return number of bytes read from bcd, or -1 if an error was encountered
 */
qint32 BlockValueMPZ::fromBCD( const  BaoSerial &bcd, mpz_t in )
{ if ( bcd.size() < 1 )
    { qWarning( "fromBCD was passed an empty array" );
      return 0;
    }
  quint8 secondNybble;
  quint8 firstNybble = (bcd.at(0) & 0xF0) >> 4;
  if ( firstNybble > 9 )
    { qWarning( "fromBCD was passed a non-bcd first nybble %x", firstNybble );
      return 0;
    }
  Utf8String iStr; iStr.reserve( 32 );
  qint32 i = 0;
  while ( i < MAX_MPINT_LEN )
    {  firstNybble = (bcd.at(i) & 0xF0) >> 4;
      secondNybble = (bcd.at(i) & 0x0F);
      i++;
      if ( firstNybble < 10 )
        iStr.append( firstNybble + '0' );
       else
        { if (( firstNybble == 15 ) && ( secondNybble == 13 ))
            { iStr.prepend( "-" );
              mpz_set_str( in, iStr.data(), 10 );
              return i;
            }
           else if (( firstNybble == 14 ) && ( secondNybble == 13 ))
            { mpz_set_str( in, iStr.data(), 10 );
              return i;
            }
           else
            { qWarning( "Invalid first nybble passed to fromBCD %x", firstNybble );
              return -1;
            }
        }
      if ( secondNybble < 10 )
        iStr.append( secondNybble + '0' );
       else
        { if ( secondNybble == 15 )
            { iStr.prepend( "-" );
              mpz_set_str( in, iStr.data(), 10 );
              return i;
            }
           else if ( secondNybble == 14 )
            { mpz_set_str( in, iStr.data(), 10 );
              return i;
            }
           else
            { qWarning( "Invalid first nybble passed to fromBCD %x", firstNybble );
              return -1;
            }
        }
    } // while ( i < MAX_MPINT_LEN )
  qWarning( "fromBCD did not encounter a terminator within %d bytes", i );
  return -1;
}

/**
 * @brief BlockValueMPQ::toStr
 * @param in - signed integer to convert
 * @return base 10 strings separated with / with negative sign prepended when needed
 */
Utf8String  BlockValueMPQ::toStr( const MP_RAT &in )
{ MP_INT n;
  mpz_init( &n );
  mpq_get_num( &n, &in );
  Utf8String s = BlockValueMPZ::toStr( n ) + "/";
  mpq_get_den( &n, &in );
  s.append( BlockValueMPZ::toStr( n ) );
  mpz_clear( &n );
  return s;
}

/**
 * @brief BlockValueMPQ::bao
 * @return numerator followed by denominator both in sign terminated BCD
 */
BaoSerial  BlockValueMPQ::bao() const
{ MP_INT n;
  mpz_init( &n );
  mpq_get_num( &n, &m_value );
  BaoSerial b = BlockValueMPZ::toBCD( n );
  mpq_get_den( &n, &m_value );
  b.append( BlockValueMPZ::toBCD( n ) );
  mpz_clear( &n );
  return b;
}

/**
 * @brief BlockValueMPQ::setBao
 * @param b - bao stream which should have two sign terminated BCD numbers
 * @return number of bytes read in total, or -1 if there was a problem
 */
qint32  BlockValueMPQ::setBao( const BaoSerial &b )
{ MP_INT n;
  mpz_init( &n );
  qint32 len = BlockValueMPZ::fromBCD( b, &n );
  if ( len < 1 )
    { mpz_clear( &n );
      qWarning( "problem reading BCD numerator in BlockValueMPQ::setBao()" );
      return -1;
    }
  MP_INT d;
  mpz_init( &d );
  qint32 led = BlockValueMPZ::fromBCD( b.mid(len), &d );
  if ( led < 1 )
    { mpz_clear( &n );
      mpz_clear( &d );
      qWarning( "problem reading BCD denominator in BlockValueMPQ::setBao()" );
      return -1;
    }
  mpq_set_num( &m_value, &n );
  mpq_set_den( &m_value, &d );
  mpz_clear( &n );
  mpz_clear( &d );
  return len + led;
}

/**
 * @brief BlockValueRiceyInt::setBao
 * @param b - ricey code in octets
 * @return number of bytes converted from the BaoSerial stream, -1 if there was a problem
 */
qint32  BlockValueRiceyInt::setBao( const BaoSerial &b )
{ qint32 len = 0;
  bool ok = false;
  qint64 v = riceToInt( b, &len, &ok );
  if ( !ok )
    { qWarning( "riceToInt conversion problem." ); return -1; }
  set( v );
  return len;
}

/**
 * @brief BlockValueRiceyCode::setBao
 * @param b - ricey code in octets
 * @return number of bytes converted from the BaoSerial stream, -1 if there was a problem
 */
qint32  BlockValueRiceyCode::setBao( const BaoSerial &b )
{ qint32 len = 0;
  bool ok = false;
  riceToInt( b, &len, &ok );
  if ( !ok )
    { qWarning( "riceToInt conversion problem." ); return -1; }
  set( b.mid(0,len) );
  return len;
}

/**
 * @brief BlockValueByteArray::bao
 * @return the bao representation of this byte array
 */
BaoSerial  BlockValueByteArray::bao() const
{ RiceyCode b = intToRice( value().size() );
  b.append( value() );
  return b;
}


/**
 * @brief BlockValueByteArray::setBao
 * @param b - Rice Code length plus byte array data
 * @return number of bytes converted from the BaoSerial stream, -1 if there was a problem
 */
qint32  BlockValueByteArray::setBao( const  BaoSerial &b )
{ if ( b.size() < 1 )
    { qWarning( "undersized array passed to BlockValueByteArray::setBao" ); return -1; }
  qint32 sz;
  bool ok;
  qint64 length = (qint64)riceToInt( b, &sz, &ok );
  if ( !ok || ( b.size() < (sz + length) ) || (length > MAX_LENGTH) || ( sz < 0 ) || ( length < 0 ) )
    { qWarning( "riceConversion %d size %d length %lld problem in BlockValueByteArray::setBao",ok,sz,length );
      return -1;
    }
  if ( length == 0 )
    set( QByteArray() );
   else
    set( b.mid(sz,length) );
  return length+sz;
}

/**
 * @brief BlockValueString::utf8CharSize - tell the size of the first UTF-8 character in the string
 * @param s - byte array to read from, starting at [0]
 * @return size of first UTF-8 char - 0 if invalid or empty
 */
qint32 BlockValueString::utf8CharSize( const char *s, qint32 sz )
{ if ( sz < 1 )
    return 0;  // Empty string character has size 0
  if (( s[0] & 0x80 ) == 0)
    return 1;  // Valid single character
  if (( s[0] & 0xE0 ) == 0xC0 )
    { if ( sz < 2 )
        return 0;
      if (( s[1] & 0xC0 ) == 0x80 )
        return 2;  // Valid double character
      return 0;
    }
  if (( s[0] & 0xF0 ) == 0xE0 )
    { if ( sz < 3 )
        return 0;
      if ( (( s[1] & 0xC0 ) == 0x80 ) &&
           (( s[2] & 0xC0 ) == 0x80 ) )
        return 3;  // Valid triple character
      return 0;
    }
  if (( s[0] & 0xF8 ) == 0xF0 )
    { if ( sz < 4 )
        return 0;
      if ( (( s[1] & 0xC0 ) == 0x80 ) &&
           (( s[2] & 0xC0 ) == 0x80 ) &&
           (( s[3] & 0xC0 ) == 0x80 ) )
        return 4;  // Valid quad character
    }
  return 0;
}

/**
 * @brief BlockValueString::vaildUtf8
 * @param s - string to evaluate
 * @return true if s is a valid and complete string of UTF-8 characters
 */
bool BlockValueString::validUtf8( const Utf8String &s )
{ const char *sp = s.data();
  qint32 ssz = s.size();
  qint32 len = 0;
  while ( ssz > len )
    { qint32 csz = utf8CharSize( &(sp[len]), ssz - len );
      if ( csz < 1 )
        return false;
      len += csz;
    }
  return true;
}

/**
 * @brief BlockValueString::bao
 * @return rice code length (in bytes, not characters) followed by UTF-8 encoded string
 */
BaoSerial  BlockValueString::bao() const
{ RiceyCode b = intToRice( value().size() );
  b.append( value() );
  return b;
}

/**
 * @brief BlockValueString::setBao
 * @param b - rice coded length in bytes followed by UTF-8 encoded string
 * @return number of bytes converted from the BaoSerial stream, -1 if there was a problem
 */
qint32  BlockValueString::setBao( const  BaoSerial &b )
{ if ( b.size() < 1 )
    { qWarning( "empty array passed to BlockValueString::setBao" ); return -1; }
  qint32 sz;
  bool ok;
  qint64 length = (qint64)riceToInt( b, &sz, &ok );
  if ( !ok || ( b.size() < (sz + length) ) || (length > MAX_LENGTH) )
    { qWarning( "riceConversion %d size %d problem in BlockValueString::setBao",ok,sz ); return -1; }
  if ( length == 0 )  // empty string is a valid construct
    { set( Utf8String() );
      return sz;
    }
  QByteArray string = b.mid(sz, length);
  if ( !validUtf8( string ) )
    { qWarning( "BlockValueString::setBao() invalid UTF8 %s", b.mid(sz, length).toHex().data() ); return -1; }
  set( string );
  return sz+length;
}

/**
 * @brief baoDefaultValue - used to prevent bao parse errors when a value is missing
 * @param keyType - key type to make the value for
 * @return a default bao value for the type
 */
BaoSerial ValueBase::baoDefaultValue( qint8 keyType )
{ QByteArray b;
  QDataStream s(b);
  s.setByteOrder(QDataStream::LittleEndian);
  switch ( keyType )
    { case RDT_OBJECT:    s << intToRice( 0 );               break;
      case RDT_INT64:     s << (qint64)0;                    break;
      case RDT_RICEYINT:  s << (RiceyInt)0;                  break;
      case RDT_MPZ:       s << (quint8)0x0E;                 break;
      case RDT_MPQ:       s << (quint8)0x0E << (quint8)0x1E; break;
      case RDT_RCODE:     s << intToRice( RCD_null_z );      break;
      case RDT_STRING:    s << intToRice( 0 );               break;
      case RDT_BYTEARRAY: s << intToRice( 0 );               break;
      default:
      if (( keyType & RDT_ARRAY ) == RDT_ARRAY )
        { s << intToRice( 0 ); }
       else
        qWarning( "unrecognized keyType in ValueBase::baoDefaultValue" );
    }
  return b;
}

/**
 * @brief KeyValuePair::bao
 * @return the bao representation of a single key-value pair
 */
BaoSerial  KeyValuePair::bao() const
{ BaoSerial b = keyCode();
  if ( !dict.codesContainCode(vKey()) )
    { qWarning( "unknown key 0x%s", keyHexd() );
      return BaoSerial();
    }
  if ( m_value )
    b.append( m_value->bao() );
   else
    { qWarning( "null value with 0x%s key", keyHexd() );
      b.append( baoDefaultValue( type() & RDT_TYPEMASK ) );  // Mask off the array bit too
    }
  return b;
}

/**
 * @brief BlockValueArray::bao
 * @return bao representation of the array value, including size and each element
 *   key must be prepended before this can be sensibly decoded
 */
BaoSerial ValueBaseArray::bao() const
{  BaoSerial b;
  quint64 elementCount = 0; // Fitering out empty values to prevent nullptr reference
  for ( qint32 i = 0; i < m_values.size(); i++ )
    if ( m_values.at(i) != nullptr )
      elementCount++;
     else
      qWarning( "missing value in array" );
  b.append( intToRice( elementCount ) );
  for ( qint32 i = 0; i < m_values.size(); i++ )
    { if ( m_values.at(i) != nullptr )
        b.append( m_values.at(i)->bao() );
    }
  return b;
}

/**
 * @brief KeyValueArray::bao
 * @return The entire array as a key plus list of values (matching the type of the key)
 */
BaoSerial KeyValueArray::bao() const
{  BaoSerial b = keyCode();
  if ( !dict.codesContainCode(b) )
    { qWarning( "unknown key 0x%s", keyHexd() );
      return BaoSerial();
    }
  if ( m_val == nullptr )
    { qWarning( "no BlockValueArray allocated" );
      return BaoSerial();
    }
  b.append( m_val->bao() );
  return b;
}

qint32  ValueBaseArray::setBao( const BaoSerial &b )
{ if ( b.size() < 1 ) { qWarning( "empty BaoSerial" ); return -1; }
  qint32 len = 0;
  bool ok = false;
  qint32 i = len;
  quint64 elementCount = riceToInt( b.mid(i), &len, &ok );
  if ( !ok )          { qWarning( "bad ricey code element count" ); return -1; }
  i += len;
  clear();
  while ( elementCount > 0 )
    { if ( b.size() <= i ) { qWarning( "value data missing" ); return -1; }
      ValueBase *vbo = newValue( type() & RDT_TYPEMASK, this );
      if ( vbo == nullptr ) { qWarning( "BlockValueArray::setBao() problem making new value for type: %d", type() ); return -1; }
      len = vbo->setBao( b.mid(i) );
      if ( len < 1 )        { delete vbo; qWarning( "BlockValueArray::setBao() problem reading element in setBao %s", b.mid(i).toHex().data() ); return -1; }
      if ( !append( vbo ) ) { delete vbo; qWarning( "BlockValueArray::setBao() problem appending element during append()" ); return -1; }
      i += len;
      elementCount--;
    }
  return i;
}

/**
 * @brief KeyValueArray::setBao
 * @param b - bao code for a KeyValueArray to be set into this object
 * @return number of bytes converted from the BaoSerial stream, -1 if there was a problem
 */
qint32  KeyValueArray::setBao( const BaoSerial &b )
{ if ( b.size() < 1 )                { qWarning( "empty BaoSerial" ); return -1; }
  qint32 len = 0;
  bool ok = false;
  RiceyInt k = riceToInt( b, &len, &ok );
  if ( !ok )                         { qWarning( "bad ricey code key" ); return -1; }
  if ( !dict.codesContainCode( k ) ) { qWarning( "unrecognized key" ); return -1; }
  if ( b.size() <= len )             { qWarning( "no element count after key" ); return -1; }
  qint32 i = len;
  quint64 elementCount = riceToInt( b.mid(i), &len, &ok );
  if ( !ok )                         { qWarning( "bad ricey code element count" ); return -1; }
  i += len;
  clear();
  if ( !setKey( k ) ) // Now we also know our data type
    qWarning( "problem setting key in KeyValueArray::setBao()" );
  RiceyInt t = k & RDT_TYPEMASK;
  while ( elementCount > 0 )
    { if ( b.size() <= i )  { qWarning( "value data missing" ); return -1; }
      ValueBase *vbo = newValue( t, this );
      if ( vbo == nullptr ) { qWarning( "BlockValueArray::setBao() problem making new value for type: %d", type() ); return -1; }
      len = vbo->setBao( b.mid(i) );
      if ( len < 1 )        { delete vbo; qWarning( "problem reading element" ); return -1; }
      if ( !append( vbo ) ) { delete vbo; qWarning( "problem appending element" ); return -1; }
      vbo->vbParent = this;
      vbo->setVKey( t );
      i += len;
      elementCount--;
    }
  return i;
}

ValueBase *ValueBaseArray::nextChild( ValueBase *v )
{ if ( m_values.size() > 0 )
    if ( m_values.contains( v ) )
      if ( (m_values.indexOf( v ) + 1) < m_values.size() )
        return m_values.at( m_values.indexOf( v ) + 1 );
  if ( vbParent )
    return vbParent->nextChild( this );
  return nullptr;
}

ValueBase *ValueBaseArray::prevChild( ValueBase *v )
{ if ( m_values.size() > 0 )
    if ( m_values.contains( v ) )
      if ( (m_values.indexOf( v ) ) > 0 )
        return m_values.at( m_values.indexOf( v ) - 1 );
  return this;
}

bool  ValueBaseArray::append( ValueBase *value )
{ if ( value == nullptr )
    { qWarning( "ValueBaseArray will not append nullptr" );
      return false;
    }
  if ( value->type() != ( type() & RDT_TYPEMASK ) )
    { qWarning( "type mismatch in ValueBaseArray::append(ValueBase) %d %d",(int)value->type(),(int)type() );
      return false;
    }
  // Setting metadata for new array element
  value->vbParent = this;
  value->setIdx( "i"+Utf8String::number( size() ) );
  value->setVKey( vKey() & RDT_TYPEMASK );
  // qWarning( "appending to %s to 0x%llx", m_idx.data(), m_key );
  m_values.append( value );
  return true;
}

ValueBase *ValueBaseArray::removeAt( qint32 i )
{ if (( i < 0 ) || ( i > size() ))
    return nullptr;
  ValueBase *v = m_values.at(i);
  m_values.remove(i);
  for ( qint32 j = i; j < size(); j++ ) // readjust m_idx of the later elements in the array
    at(j)->setIdx( "i"+Utf8String::number( j ) );
  return v;
}

bool  ValueBaseArray::insertAt( ValueBase *v, qint32 i )
{ if ( i == size() )
    return append(v);
  if ( v == nullptr )
    { qWarning( "ValueBaseArray will not insert nullptr" );
      return false;
    }
  if ( v->type() != ( type() & RDT_TYPEMASK ) )
    { qWarning( "type mismatch in ValueBaseArray::insertAt() %d %d",(int)v->type(),(int)type() );
      return false;
    }
  if (( i >= size() ) || ( i < 0 ))
    { qWarning( "index out of range in ValueBaseArray::insertAt() %d %d",(int)size(),(int)i );
      return false;
    }
  // Setting metadata for new array element
  v->vbParent = this;
  m_values.insert(i,v);
  for ( qint32 j = i; j < size(); j++ ) // readjust m_idx of the later elements in the array too
    at(j)->setIdx( "i"+Utf8String::number( j ) );
  v->setVKey( vKey() );
  return true;
}


/**
 * @brief KeyValueArray::append
 * @param value - pointer to a generic value, must be type matched to the array
 * @return true if successful
 */
bool  KeyValueArray::append( ValueBase *value )
{ if ( value == nullptr )
    { qWarning( "will not KeyValueArray::append null" );
      return false;
    }
  if ((value->type() & RDT_TYPEMASK) != (type() & RDT_TYPEMASK))
    { qWarning( "type mismatch in KeyValueArray::append() %d %d",(int)value->type(),(int)type() );
      return false;
    }
  if ( !typeMatch( value->type() & RDT_TYPEMASK ) )
    { qWarning( "problem in typeMatch in KeyValueArray::append() %d %d",(int)value->type(),(int)type() );
      return false;
    }
  value->vbParent = this;
  value->setIdx( "i"+Utf8String::number( size() ) );
  value->setVKey( vKey() );
  m_val->append( value );
  return true;
}

/**
 * @brief KeyValueArray::typeMatch
 * @param t - type to check
 * @return true if the KeyValueArray type stores the passed type
 */
bool  KeyValueArray::typeMatch( quint8 t )
{ if ( m_val )
    return ((m_val->type() & RDT_TYPEMASK) == (t & RDT_TYPEMASK));
  switch ( t )
    { case RDT_MPZ:       m_val = new BlockValueMPZArray      ( this ); return true;
      case RDT_MPQ:       m_val = new BlockValueMPQArray      ( this ); return true;
      case RDT_STRING:    m_val = new BlockValueStringArray   ( this ); return true;
      case RDT_INT64:     m_val = new BlockValueInt64Array    ( this ); return true;
      case RDT_RICEYINT:  m_val = new BlockValueRiceyIntArray ( this ); return true;
      case RDT_BYTEARRAY: m_val = new BlockValueByteArrayArray( this ); return true;
      case RDT_RCODE:     m_val = new BlockValueRiceyCodeArray( this ); return true;
      case RDT_OBJECT:    m_val = new BlockValueObjectArray   ( this ); return true;
    }
  qWarning( "unrecognized type %d", t );
  return false;
}

ValueBase *BlockValueObject::nextChild( ValueBase *v )
{ QList<RiceyInt> omk = m_obMap.keys();
  for ( qint32 i = 0; i < omk.size() - 1; i++ )
    { if ( v == m_obMap[omk.at(i)] )
        return m_obMap[omk.at(i+1)];
    }
  if ( vbParent )
    if ( !vbParent->isKeyValuePair() )
      return vbParent->nextChild( this );
  return nullptr;
}

ValueBase *BlockValueObject::prevChild( ValueBase *v )
{ QList<RiceyInt> omk = m_obMap.keys();
  for ( qint32 i = 1; i < omk.size(); i++ )
    { if ( v == m_obMap[omk.at(i)] )
        return m_obMap[omk.at(i-1)];
    }
  return this;
}

/**
 * @brief BlockValueObject::bao
 * @return The entire object as a list of key-values
 */
BaoSerial BlockValueObject::bao() const
{ QList<RiceyInt> keys = m_obMap.keys();
  BaoSerial b = intToRice( m_obMap.size() );
  foreach ( RiceyInt key, keys )
    { b.append( intToRice(key) );
      b.append( ( m_obMap[key] != nullptr ) ? m_obMap[key]->bao() :
                                              baoDefaultValue( m_obMap[key]->type() ) );
    }
  return b;
}

/**
 * @brief BlockValueObject::setBao
 * @param b - bao code for a BlockValueObject to be set into this object
 * @return number of bytes converted from the BaoSerial stream, -1 if there was a problem
 */
qint32  BlockValueObject::setBao( const BaoSerial &b )
{ if ( b.size() < 1 )
    { qWarning( "empty BaoSerial" ); return -1; }
  qint32 len = 0;
  bool ok = false;
  qint64 obCount = riceToInt( b, &len, &ok );
  if ( !ok )
    { qWarning( "bad count rice code" ); return -1; }
  clear();
  if ( obCount == 0 ) // TODO: check for exceeding max ob count
    return len; // empty object, we're done
  qint32 i = len;
  while ( obCount > 0 )
    { if ( b.size() <= i )
        { qWarning( "object key data missing" ); return -1; }
      RiceyInt k = riceToInt( b.mid(i), &len, &ok );
      if ( !ok )
        { qWarning( "bad key rice code" ); return -1; }
      i += len;
      if ( b.size() <= i )
        { qWarning( "object key data missing" ); return -1; }
      ValueBase *vbo = baoValueByKey( k, b.mid(i), &len, this );
      if ( vbo == nullptr )
        { qWarning( "BlockValueObject::setBao() problem reading bao value" ); return -1; }
      if ( !insert( k, vbo ) )
        { delete vbo; qWarning( "problem inserting value" ); return -1; }
      i += len;
      obCount--;
    }
  return i;
}

bool  BlockValueObjectArray::operator==(const QList<BlockObjectMap>& l) const
{ if ( size() != l.size() )
    return false;
  for ( qint32 i = 0; i < size(); i++ )
    { BlockObjectMap mt = at(i);
      BlockObjectMap ml = l.at(i);
      if ( mt.size() != ml.size() )
        return false;
      QList<RiceyInt> keys = mt.keys();
      foreach ( RiceyInt key, keys )
        { if ( !ml.contains( key ) )
            return false;
          if ( !( *mt[key] == *ml[key] ) )
            return false;
        }
    }
  return true;
}


/**
 * @brief ValueBase::baoValueByKey
 * @param k - key as a RiceyInt which tells the type of the value to be read
 * @param b - bytestream ready to read a value from
 * @param l - when not null, return length of stream read here, -1 if there is a problem
 * @param vbp - parent for the value object
 * @return length of stream read to get the value, or -1 if there was a problem
 */
ValueBase *ValueBase::baoValueByKey( RiceyInt k, const  BaoSerial &b, qint32 *l, ValueBase *vbp ) const
{ if ( l != nullptr )
    *l = -1;
  ValueBase *vbo = newValue( k, vbp );
  if ( vbo == nullptr )
    { qWarning( "problem making new value for key %llu", k ); return nullptr; }
  qint32 len = vbo->setBao( b );
  if ( len < 1 )
    { delete vbo; qWarning( "ValueBase::baoValueByKey() problem reading value" ); return nullptr; }
  if ( l != nullptr )
    *l = len;
  return vbo;
}

/**
 * @brief BlockValueObject::insert
 * @param k - key
 * @param v - value
 * @return true if insertion was successful
 */
bool  BlockValueObject::insert( RiceyInt k, ValueBase *v )
{ if ( v == nullptr )
    { qWarning( "will not insert null value" ); return false; }
  if (( k & RDT_OBTYPEMASK ) != v->type() )
    { qWarning( "will not insert mismatched type and value" ); return false; }
  if ( !dict.codesContainCode( k ) )
    { qWarning( "BlockValueObject::insert(RiceyInt,ValueBase) key %llu not recognized, will not insert.", k ); return false; }
  if ( m_obMap.contains( k ) )
    { qWarning( "type collision, insertion blocked." ); return false; }
  v->setMetaData( k, this );
  m_obMap.insert( k, v );
  return true;
}

bool  BlockValueObject::insert( KeyValueArray *kva )
{ if ( kva == nullptr )                       { qWarning( "will not insert nullptr kva" ); return false; }
  // if ( kva->value() == nullptr )              { qWarning( "will not insert nullptr kva->value()" ); return false; }
  RiceyInt k = kva->key();
  if (( k & RDT_OBTYPEMASK ) != kva->type() ) { qWarning( "will not insert mismatched type and value" ); return false; }
  if ( !dict.codesContainCode( k ) )          { qWarning( "BlockValueObject::insert(KeyValueArray) key %llu not recognized, will not insert.", kva->key() ); return false; }
  if ( m_obMap.contains( k ) )                { qWarning( "type collision, insertion blocked." ); return false; }
  ValueBaseArray *vba = (ValueBaseArray *)newValue( k, this, kva->value() );
  m_obMap.insert( k, vba );
  return true;
}

/**
 * @brief ValueBase::setMetaData - for non-array elements
 * @param k - key
 * @param vbp - parent
 */
void  ValueBase::setMetaData( RiceyInt k, ValueBase *vbp )
{ vbParent = vbp;
  setIdx( "k" + intToRice( k ).toHex() );
  setVKey( k );
}

/**
 * @brief BlockValueObject::remove
 * @param k - key of child to remove
 * @return removed object, nullptr if problem
 */
ValueBase *BlockValueObject::remove( RiceyInt k )
{ if ( !contains(k) ) { qWarning( "BlockValueObject::remove(0x%llx) key not found", k ); return nullptr; }
  ValueBase *vb = m_obMap[k];
  m_obMap.remove(k);
  if ( vb == nullptr )
    qWarning( "BlockValueObject::remove(0x%llx) key returned nullptr", k );
  return vb;
}

/**
 * @brief BlockValueObject::insert
 * @param vl - list of items to insert
 * @return number of successfully inserted objects
 */
qint32  BlockValueObject::insert( const BlockObjectMap &vl )
{ qint32 count = 0;
  QList<RiceyInt> keys = vl.keys();
  ValueBase *vb;
  foreach( RiceyInt k, keys )
    { vb = newValue( k, this, vl[k] );
      if ( insert( k, vb ) )
        count++;
       else
        { vb->deleteLater();
          qWarning( "insert Failed" );
        }
    }
  return count;
}

/**
 * @brief BlockValueArray::operator == for all array types
 * @param v - array value to compare
 * @return true if the two are the same size, type, and value in each element
 */
bool  ValueBaseArray::operator==( const ValueBaseArray &v ) const
{ if ( v.size() != size() ) return false;
  if ( v.type() != type() ) return false;
  for ( qint32 i = 0; i < size(); i++ )
    if ( !( *at(i) == *v.at(i) ) ) return false;
  return true;
}

/**
 * @brief BlockValueObject::operator ==
 * @param v - BlockObjectMap to compare
 * @return true if equal
 */
bool BlockValueObject::operator==( const BlockObjectMap &v ) const
{ if ( v.size() != m_obMap.size() )
    return false;
  if ( v.size() == 0 )
    return true;
  QList<RiceyInt> keys = v.keys();
  foreach ( RiceyInt k, keys )
    { if ( !m_obMap.contains( k ) )
        return false;
      ValueBase *vv = v.value(k);
      ValueBase *vt = value(k);
      if ( vv->type() != vt->type() )
        { qWarning( "BlockValueObject::operator== type mismatch %d %d",vv->type(),vt->type() ); return false; }
      switch ( k & RDT_OBTYPEMASK )
        { case RDT_OBJECT:    if ( !(*(qobject_cast<BlockValueObject    *>(vt))          == *(qobject_cast<BlockValueObject    *>(vv))          ) ) return false; break;
          case RDT_INT64:     if (  ( (qobject_cast<BlockValueInt64     *>(vt))->value() !=  (qobject_cast<BlockValueInt64     *>(vv))->value() ) ) return false; break;
          case RDT_RICEYINT:  if (  ( (qobject_cast<BlockValueRiceyInt  *>(vt))->value() !=  (qobject_cast<BlockValueRiceyInt  *>(vv))->value() ) ) return false; break;
          case RDT_MPZ:       if ( !(*(qobject_cast<BlockValueMPZ       *>(vt))          ==  (qobject_cast<BlockValueMPZ       *>(vv))->value() ) ) return false; break;
          case RDT_MPQ:       if ( !(*(qobject_cast<BlockValueMPQ       *>(vt))          ==  (qobject_cast<BlockValueMPQ       *>(vv))->value() ) ) return false; break;
          case RDT_RCODE:     if (  ( (qobject_cast<BlockValueRiceyCode *>(vt))->value() !=  (qobject_cast<BlockValueRiceyCode *>(vv))->value() ) ) return false; break;
          case RDT_STRING:    if (  ( (qobject_cast<BlockValueString    *>(vt))->value() !=  (qobject_cast<BlockValueString    *>(vv))->value() ) ) return false; break;
          case RDT_BYTEARRAY: if (  ( (qobject_cast<BlockValueByteArray *>(vt))->value() !=  (qobject_cast<BlockValueByteArray *>(vv))->value() ) ) return false; break;
          case RDT_OBJECT_ARRAY:
          case RDT_INT64_ARRAY:
          case RDT_RICEYINT_ARRAY:
          case RDT_MPZ_ARRAY:
          case RDT_MPQ_ARRAY:
          case RDT_RCODE_ARRAY:
          case RDT_STRING_ARRAY:
          case RDT_BYTEARRAY_ARRAY: if ( !(*(qobject_cast<ValueBaseArray *>(vt)) == *(qobject_cast<ValueBaseArray *>(vv)) ) ) return false; break;
          default: Utf8String n = dict.nameFromCode( k ); qWarning( "unhandled type %s", n.data() ); return false;
        }
    }
  return true;
}


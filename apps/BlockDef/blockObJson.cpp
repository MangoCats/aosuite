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
#include <QJsonDocument>
#include <QJsonObject>
#include <QJsonValue>
#include <QVariant>

// Design Intent:
//
// The json() and setJson() functions of block object family members are intended as
//   a bi-directional serialization/deserialization pathway to/from a somewhat human
//   readable form for the block data.  Although json is not the primary form for
//   block data, it should be possible to serialize a block as json, edit it, then
//   deserialize the edited json data back into a block object.  Operations like
//   hash and digital signatures require the bao representation of the data, so
//   they cannot be calculated directly on json representations without first
//   translating to bao.
//
// State of the code:
//
// Although the Qt Json library functions are convenient for parsing of structures
//   like arrays, they are struggling with accuracy of larger int64 numbers due to
//   internal representation of such numbers as double floats.  Accordingly, integer
//   values are stored as strings, with quotes, to use the QString .toLongLong
//   conversions which do not lose precision.
//


/**
 * @brief readJson - create a KeyValue from a json document - this function mostly decides
 *   if the document is a key value pair or key value array then passes the parse work to
 *   the newly created object.
 * @return a KeyValue for the contents of js, nullptr if there is a problem
 */
KeyValueBase *KeyValueBase::readJson( const JsonSerial &js, QObject *parent )
{ KeyValueBase *kvb = nullptr;
  if ( js.size() < 5 )         { qWarning( "KeyValueBase::readJson() no doc passed" ); return kvb; }
  QJsonDocument jd = QJsonDocument::fromJson( js );
  if ( !jd.isObject() )        { qWarning( "KeyValueBase::readJson() doc is not a json object" ); return kvb; }
  JsonSerial jt = js.trimmed();
  if ( !jt.startsWith( '{' ) ) { qWarning( "KeyValueBase::readJson() expected json stream to start with {, it didn't" ); return kvb; }
  if ( !jt.endsWith  ( '}' ) ) { qWarning( "KeyValueBase::readJson() expected json stream to end with }, it didn't" ); return kvb; }
  jt.chop( 1 );
  jt = jt.mid(1);
  QJsonObject jo = jd.object();
  QStringList keys = jo.keys();
  if ( keys.size() != 1 )      { qWarning( "KeyValueBase::readJson() expected 1 key, not %lld", keys.size() ); return kvb; }
  Utf8String key = keys.at(0).toUtf8();
  if ( !dict.codesContainName( key ) ) { qWarning( "KeyValueBase::readJson() unknown key %s", key.data() ); return kvb; }
  RiceyInt k = dict.codeFromCodeName( key );
  if ( k & RDT_ARRAY )
    kvb = new KeyValueArray( k, parent );
   else
    kvb = new KeyValuePair( k, parent );
  kvb->setJson( jt );
  return kvb;
}

bool  KeyValuePair::setJson( const JsonSerial &j )
{ QJsonDocument jd = QJsonDocument::fromJson( "{"+j+"}" );
  if ( !jd.isObject() )
    { qWarning( "KeyValueArray::document is not a JsonObject '%s'",j.data() ); return false; }
  QJsonObject jo = jd.object();
  if ( jo.keys().size() != 1 )
    { qWarning( "object has %lld keys (should be 1)", jo.keys().size() ); return false; }

  QVector<QString> keys = jo.keys();
  Utf8String obKey = keys.at(0).toUtf8();
  if ( !dict.codesContainName( obKey ) )
    { qWarning( "dictionary does not contain a %s element", obKey.data() ); return false; }
  setKey( dict.codeFromCodeName( obKey ) );
  QJsonValue jv = jo.value( obKey );

  if ( !jv.isObject() )
    { qWarning( "value is not an object" ); return false; }
  clear();
  set( newValue( key(), this ) );
  if ( m_value == nullptr )
    { qWarning( "unexpected problem allocating BlockValue" );
      return false;
    }
  QJsonDocument jda = QJsonDocument( jv.toObject() );
  return m_value->setJson( jda.toJson() );
}

/**
 * @brief BlockValueInt64::setJson
 * @param j - byte array which should contain a UTF8 encoded base 10 integer
 * @return true if conversion was successful
 */
bool BlockValueInt64::setJson( const JsonSerial &j )
{ if ( j.size() < 1 )
    { qWarning( "empty json" ); return false; }
  bool ok;
  qint64 v = removeQuotes(j).toLongLong(&ok);
  if ( ok )
    m_value = v;
   else
    qWarning( "problem converting '%s' to int64", j.data() );
  return ok;
}

/**
 * @brief BlockValueRiceyInt::setJson
 * @param j - byte array which should contain a UTF8 encoded base 10 unsigned integer
 * @return true if conversion was successful
 */
bool BlockValueRiceyInt::setJson( const JsonSerial &j )
{ if ( j.size() < 1 )
    { qWarning( "empty json" ); return false; }
  bool ok;
  qint64 v = removeQuotes(j).toLongLong(&ok);
  if ( ok )
    { if ( v >= 0 )
        m_value = v;
       else
        { ok = false;
          qWarning( "RiceyInt cannot be negative" );
        }
    }
   else
    qWarning( "problem converting '%s' to RiceyInt", j.data() );
  return ok;
}

/**
 * @brief BlockValueMPZ::setJson
 * @param j - json representation of the big integer
 * @return true if successful
 */
bool BlockValueMPZ::setJson( const JsonSerial &j )
{ JsonSerial t = j.trimmed();
  if ( t.size() < 3 )
    { qWarning( "empty array passed to BlockValueMPZ::setJson" ); return false; }
  if ( t.at(0) != '"' )
    { qWarning( "BlockValueMPZ::setJson expected to start with \"" ); return false; }
  t = t.mid(1);
  if ( t.at(t.size()-1) != '"' )
    { qWarning( "BlockValueMPZ::setJsonexpected to end with \"" ); return false; }
  t.chop(1);
  t.append( (quint8)0 );

  int ok = mpz_set_str( &m_value, t.data(), 10 );
  if ( ok != 0 )
    qWarning( "problem in converting json to MPZ MP_INT %s %s",j.data(),t.data() );
  return ( ok == 0 );
}

/**
 * @brief BlockValueMPQ::setJson
 * @param j - json representation of two big integers
 * @return true if successful
 */
bool BlockValueMPQ::setJson( const JsonSerial &j )
{ JsonSerial t = j.trimmed();
  if ( t.size() < 3 )
    { qWarning( "empty array passed to BlockValueMPQ::setJson" ); return false; }
  if ( t.at(0) != '"' )
    { qWarning( "BlockValueMPQ::setJson expected to start with \"" ); return false; }
  t = t.mid(1);
  if ( t.at(t.size()-1) != '"' )
    { qWarning( "BlockValueMPQ::setJsonexpected to end with \"" ); return false; }
  t.chop(1);
  t.append( (quint8)0 );
  if ( ! (t.count( '/' ) == 1) )
    { qWarning( "BlockValueMPQ::setJsonexpected to contain one and only /" ); return false; }
  qint32 si = t.indexOf( '/' );
  if (( si <= 0 ) || ( si >= (t.size() - 1) ))
    { qWarning( "BlockValueMPQ::setJsonexpected to include numbers on either side of the /" ); return false; }
  JsonSerial ns = t.mid( 0,si ); ns.append((quint8)0);
  JsonSerial ds = t.mid(si+1);   ds.append((quint8)0);
  // qWarning( "ns %s ds %s", ns.data(), ds.data() );
  MP_INT n;
  MP_INT d;
  int ok;
  ok = mpz_init_set_str( &n, ns.data(), 10 );
  if ( ok != 0 )
    { qWarning( "problem in converting numerator json to MPZ MP_INT %s",ns.data() ); return false; }
  ok = mpz_init_set_str( &d, ds.data(), 10 );
  if ( ok != 0 )
    { qWarning( "problem in converting denominator json to MPZ MP_INT %s",ds.data() ); return false; }
  mpq_set_num( &m_value, &n );
  mpq_set_den( &m_value, &d );
  mpz_clear( &n );
  mpz_clear( &d );
  return true;
}

/**
 * @brief BlockValueRiceyCode::setJson
 * @param j - ricey code in ASCII hexadecimal
 * @return true if conversion was successful
 */
bool  BlockValueRiceyCode::setJson( const JsonSerial &j )
{ JsonSerial jt = removeQuotes( j );
  if ( dict.codesContainName( jt ) )
    { m_value = dict.riceyFromCodeName( jt );
      return true;
    }
  qWarning( "invalid code name in json %s", jt.data() );
  return false;
}

/**
 * @brief BlockValueByteArray::setJson
 * @param j - ASCII hexadecimal encoded list of octets
 * @return - true always, conversion can't fail, invalid hex is skipped
 */
bool  BlockValueByteArray::setJson( const JsonSerial &j )
{ // TODO: validate hex, or do we care?
  if ( j.size() == 0 )
    m_value = QByteArray();
   else
    m_value = j.fromHex( j );
  return true;
}

/**
 * @brief BlockValueString::setJson
 * @param j - json escaped UTF-8 string in quotes
 * @return - true if conversion succeeds
 */
bool  BlockValueString::setJson( const JsonSerial &j )
{ JsonSerial d = "{ \"v\": ";
  d.append( j );
  d.append( " }" );
  QJsonDocument jd = QJsonDocument::fromJson(d);
  if ( !jd.isObject() )
    { qWarning( "BlockValueString::document is not a JsonObject '%s'",d.data() ); return false; }
  QJsonObject o = jd.object();
  if ( !o.contains("v") )
    { qWarning( "object does not contain the v element" ); return false; }
  QJsonValue v = o.value("v");
  if ( !v.isString() )
    { qWarning( "value is not a string" ); return false; }
  m_value = v.toString().toUtf8();
  return true;
}

/**
 * @brief BlockValueString::json - m_value is assumed to be valid UTF-8
 * @return m_value UTF-8 string encoded as a json value field (escaped, in quotes)
 */
JsonSerial  BlockValueString::json() const
{ QJsonValue v( QString::fromUtf8(m_value) );
  QJsonObject o;
  o.insert( "k", v );
  QJsonDocument d(o);
  JsonSerial js = d.toJson(QJsonDocument::Compact);
  if (( !js.contains( ':' ) ) || ( !js.contains( '"' ) ))
    return "\"\"";
  qint32 ci = js.indexOf( ':' );
  qint32 fqi = js.indexOf( '"', ci );
  qint32 lqi = js.lastIndexOf( '"' );
  if (( fqi >= lqi ) || ( fqi < 0 ) || ( lqi < 0 ))
    return "\"\"";
  return js.mid( fqi, lqi-fqi+1 );
  // TODO: refactor to expose this encoding to the string array
}

/**
 * @brief KeyValuePair::json
 * @return the json representation of a single key-value pair
 */
JsonSerial  KeyValuePair::json() const
{ JsonSerial j = "{ \"";
  if ( !dict.codesContainCode(key()) )
    { qWarning( "unknown key 0x%s", keyHexd() );
      return "{<!-- unknown key 0x"+keyHex()+" -->}";
    }
  j.append( dict.nameFromCode(key()) );
  j.append( "\": " );
  if ( m_value )
    j.append( m_value->json() );
   else
    { qWarning( "null value with 0x%s key", keyHexd() );
      j.append( "0 <!-- null value -->" );
    }
  j.append( " }\n" );
  return j;
}

JsonSerial  ValueBaseArray::json() const
{ JsonSerial j = " [ ";
  bool wroteOne = false;
  for ( qint32 i = 0; i < m_values.size(); i++ )
    { ValueBase *vp = m_values.at(i);
      if ( vp != nullptr )
        { j.append( vp->json() + " , \n" );
          wroteOne = true;
        }
    }
  if ( wroteOne )
    j = j.mid( 0, j.size() - 3 );
  j.append( "] \n" );
  return j;
}

/**
 * @brief KeyValueArray::json
 * @return json array object with the key and the whole array
 */
JsonSerial  KeyValueArray::json() const
{ JsonSerial j = "{ \"";
  if ( !dict.codesContainCode(key()) )
    return "{<!-- unknown key -->}";
  j.append( dict.nameFromCode(key()) );
  j.append( "\": [ " );
  bool wroteOne = false;
  for ( qint32 i = 0; i < size(); i++ )
    { ValueBase *vp = at(i);
      if ( vp != nullptr )
        { j.append( vp->json() + " , \n" );
          wroteOne = true;
        }
    }
  if ( wroteOne )
    j = j.mid( 0, j.size() - 3 );
  j.append( "] }\n" );
  return j;
}

bool  ValueBaseArray::setJson( const JsonSerial &j )
{ QJsonDocument jd = QJsonDocument::fromJson( "{ \"ob\": "+j+" }" );
  if ( !jd.isObject() )
    { qWarning( "BlockValueArray::document is not a JsonObject '%s'",j.data() ); return false; }
  QJsonObject jo = jd.object();
  if ( jo.keys().size() != 1 )
    { qWarning( "object has %lld keys (should be 1)", jo.keys().size() ); return false; }

  QVector<QString> keys = jo.keys();
  Utf8String obKey = keys.at(0).toUtf8();
  if ( obKey != "ob" )
    { qWarning( "failed to recover array object" ); return false; }
  QJsonValue jv = jo.value( "ob" );

  if ( !jv.isArray() )
    { qWarning( "value is not an array" ); return false; }
  clear();
  QJsonArray ja = jv.toArray();
  if ( ja.size() < 1 )  // Empty?
    return true;        // We're done.
  if ( type() == RDT_OBJECT_ARRAY )
    { for ( qint32 i = 0; i < ja.size(); i++ )
        { QJsonValue jv = ja.at(i);
          if ( !jv.isObject() )
            qWarning( "array element is not object in KeyValueArray::setJson() type OBJECT_ARRAY" );
           else
            { QJsonObject jo = jv.toObject();
              QJsonDocument ed( jo );
              BlockValueObject *bvo = new BlockValueObject( this, ed.toJson() );
              append( bvo ); bvo->deleteLater();
            }
        }
    }
   else
    { QVariantList vl = ja.toVariantList();
      Utf8String vt;
      foreach ( QVariant v, vl )
        { ValueBase *vbo = nullptr;
          switch ( type() )
            { case RDT_RCODE_ARRAY:
                vt = v.toString().toUtf8();
                if ( dict.codesContainName( vt ) )
                  vbo = new BlockValueRiceyCode( dict.codeFromCodeName( vt ), this );
                 else
                  qWarning( "dictionary does not contain %s", vt.data() );
                break;
              case RDT_INT64_ARRAY:     vbo = new BlockValueInt64   ( (qint64)v.toString().toLongLong(), this ); break;
              case RDT_RICEYINT_ARRAY:  vbo = new BlockValueRiceyInt( (qint64)v.toString().toLongLong(), this ); break;
              case RDT_STRING_ARRAY:    vbo = new BlockValueString( v.toString().toUtf8(), this ); break; // TODO: encode using the same escapes as in the BlockString.json() function
              case RDT_BYTEARRAY_ARRAY: vbo = new BlockValueByteArray( QByteArray::fromHex( v.toString().toUtf8() ), this ); break;
              case RDT_MPZ_ARRAY:       vbo = new BlockValueMPZ( v.toString().toUtf8(), this );    break;
              case RDT_MPQ_ARRAY:       vbo = new BlockValueMPQ( v.toString().toUtf8(), this );    break;
              default:
                qWarning( "unhandled type in KeyValueArray::setJson()" );
                return false;
            } // switch ( type() )
          if ( vbo )
            { append( vbo );
              vbo->deleteLater();
            }
           else
            qWarning( "unexpected nullptr in vbo" );
        } // foreach ( QVariant v, vl )
    }
  return true;
}

bool  KeyValueArray::setJson( const JsonSerial &j )
{ QJsonDocument jd = QJsonDocument::fromJson( "{"+j+"}" );
  if ( !jd.isObject() )
    { qWarning( "KeyValueArray::document is not a JsonObject '%s'",j.data() ); return false; }
  QJsonObject jo = jd.object();
  if ( jo.keys().size() != 1 )
    { qWarning( "object has %lld keys (should be 1)", jo.keys().size() ); return false; }

  QVector<QString> keys = jo.keys();
  Utf8String obKey = keys.at(0).toUtf8();
  if ( !dict.codesContainName( obKey ) )
    { qWarning( "dictionary does not contain a %s element", obKey.data() ); return false; }
  setKey( dict.codeFromCodeName( obKey ) );
  QJsonValue jv = jo.value( obKey );

  if ( !jv.isArray() )
    { qWarning( "value is not an array" ); return false; }
  clear();
  typeMatch( key() & RDT_TYPEMASK );
  if ( m_val == nullptr )
    { qWarning( "unexpected problem allocating BlockValueArray" );
      return false;
    }
  QJsonDocument jda = QJsonDocument( jv.toArray() );
  return m_val->setJson( jda.toJson() );
}

/**
 * @brief BlockValueObject::json
 * @return object as a standard json string
 */
JsonSerial BlockValueObject::json() const
{ JsonSerial j = " {";
  QList<RiceyInt> keys = m_obMap.keys();
  bool wroteOne = false;
  foreach ( RiceyInt key, keys )
    { ValueBase *vp = m_obMap[key];
      if ( vp != nullptr )
        { if ( !dict.codesContainCode(key) )
            { j.append( " \""+intToRice(key).toHex()+"\" <!-- unknown key --> : " ); qWarning( "json Object unknown key" ); } // TODO: type extend key with _X
           else
            j.append( " \""+dict.nameFromCode(key)+"\": " );
          if ( vp != nullptr )
            j.append( vp->json() + " ,\n" );
           else
            { j.append( "NULL ,\n" ); qWarning( "json Object conversion encountered NULL" ); }
          wroteOne = true;
        }
    }
  if ( wroteOne )
    j = j.mid( 0, j.size() - 2 );
  j.append( "\n }\n" );
  return j;
}

/**
 * @brief BlockValueObject::setJson
 * @param j - json sub-string just containing the key-value pairs in the object
 * @return true if successful
 */
bool  BlockValueObject::setJson( const JsonSerial &j )
{ QJsonDocument jd = QJsonDocument::fromJson(j);
  if ( !jd.isObject() )
    { if ( !jd.isArray() )
        { qWarning( "BlockValueObject::document is not a JsonObject or JsonArray '%s'",j.data() ); return false; }
      qWarning( "TODO: process array '%s'",j.data() );
      return false;
    }
  QJsonObject jo   = jd.object();
  QVector<QString> keys = jo.keys();
  // clear();
  QList<RiceyInt> kl = m_obMap.keys();
  foreach( RiceyInt k, kl )
    { if ( m_obMap[k] != nullptr )
        m_obMap[k]->deleteLater();
      m_obMap.remove(k);
    }
  if ( keys.size() < 1 ) // Empty object?
    return true;         // yes, we're done.
  foreach ( QString kStr, keys )
    { if ( !dict.codesContainName( kStr.toUtf8() ) )
        { qWarning( "unknown key %s in object", kStr.toUtf8().data() ); }
       else
        { RiceyInt k = dict.codeFromCodeName( kStr.toUtf8() );
          ValueBase *vbo = jsonValueByKey( k, jo.value( kStr ), this );
          if ( vbo == nullptr )
            { qWarning( "problem reading json value %s from %s", kStr.toUtf8().data(), j.data() ); return false; }
          if ( !insert( k, vbo ) )
            { delete vbo; qWarning( "problem inserting value" ); return false; }
        }
    }
  return true;
}

#define JDT_OBJECT 1
#define JDT_DOUBLE 2
#define JDT_STRING 3
#define JDT_ARRAY  4
/**
 * @brief ValueBase::jsonValueByKey
 * @param k - key to help determine/verify value type
 * @param jv - single json value read from larger document
 * @param parent - object parent for the ValueBase object created
 * @return pointer to a ValueBase object with the value of the JsonValue, nullptr if there is a problem
 */
ValueBase *ValueBase::jsonValueByKey( RiceyInt k, const QJsonValue &jv, ValueBase *vbp ) const
{ qint32 typ = k & RDT_OBTYPEMASK;
  qint32 jdt = 0;
  bool typeMatch = false;
  switch ( typ )
    { case RDT_OBJECT:    typeMatch = jv.isObject(); jdt = JDT_OBJECT; break;
      case RDT_INT64:
      case RDT_RICEYINT:
      case RDT_MPZ:
      case RDT_MPQ:
      case RDT_RCODE:
      case RDT_STRING:
      case RDT_BYTEARRAY: typeMatch = jv.isString(); jdt = JDT_STRING; break;
      case RDT_OBJECT_ARRAY:
      case RDT_INT64_ARRAY:
      case RDT_RICEYINT_ARRAY:
      case RDT_MPZ_ARRAY:
      case RDT_MPQ_ARRAY:
      case RDT_RCODE_ARRAY:
      case RDT_STRING_ARRAY:
      case RDT_BYTEARRAY_ARRAY: typeMatch = jv.isArray(); jdt = JDT_ARRAY; break;
      default: qWarning( "typefault" ); return nullptr;
    }
  if ( !typeMatch )
    { Utf8String name = dict.nameFromCode(k);
      qWarning( "%s value does not match type %d", name.data(), typ );
      return nullptr;
    }
  ValueBase *vbo;
  QJsonDocument jd;
  // QString str;
  switch ( jdt )
    { case JDT_OBJECT:
        vbo = newValue( k, vbp );
        jd.setObject( jv.toObject() );
        vbo->setJson( jd.toJson() );
        return vbo;

      case JDT_STRING:
        vbo = newValue( k, vbp );
        if (( typ == RDT_STRING ) || ( typ == RDT_MPZ ) || ( typ == RDT_MPQ ) || ( typ == RDT_INT64 ) || ( typ == RDT_RICEYINT ))
          vbo->setJson( "\""+jv.toString().toUtf8()+"\"" );
         else
          vbo->setJson(      jv.toString().toUtf8()      );
        return vbo;

      case JDT_ARRAY:
        vbo = newValue( k, vbp );
        jd.setArray( jv.toArray() );
        if ( !vbo->setJson( jd.toJson() ) )
          { vbo->deleteLater();
            qWarning( "problem setting array from json" );
            return nullptr;
          }
        return vbo;
    }
  qWarning( "unrecognized json value type" );
  return nullptr;
}

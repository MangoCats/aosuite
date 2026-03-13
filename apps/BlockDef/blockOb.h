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
#ifndef BLOCKOB_H
#define BLOCKOB_H

#include <QObject>
#include <QMap>
#include <QPointer>
#include <QDataStream>
#include <QIODevice>
#include <QColor>
#include "dictionary.h"

#include <stdio.h>
#include <stdarg.h>
#include <obstack.h>
#include <gmp.h>

#define MAX_LENGTH 1073741824  // Nothing bigger than 1GB

/***********************************************************************
 * Philosophical thought of the day: we don't need key value pairs.    *
 * Everything could be stored in key value arrays, and things that     *
 * "want" to be key value pairs are simply stored in arrays of size 1. *
 * This would still translate to-from .json cleanly.  Every structure  *
 * element would be forced to carry an array size byte, but this is    *
 * partly compensated by reducing the array bit out of the type code,  *
 * thus enabling twice as many single byte codes for a given type.     *
 * Primary downside I see at the moment is that it would feel strange  *
 * to people "thinking in json" who would have an instinct to use key  *
 * value pairs.  Protocol definitions might also become more clumsy    *
 * with array index references everywhere, but an assumed default of   *
 * index 1 might overcome that.  .dot representations might be similar *
 * with assumed array size of 1 when not explicitly shown.  Code would *
 * drop all the KeyValuePair vs KeyValueArray handling, and could also *
 * have inputs for KeyValuePairs which get implemented as arrays of    *
 * size 1.  Current code could support a trial implementation of this  *
 * by specifying protocols which only use arrays.  If it's not too     *
 * weird, the implementation could shift behind the scenes, though a   *
 * dictionary change would be required to take advantage of the extra  *
 * bit made available by the defaulting to only array types.           *
 *                                                                     *
 * Conclusion after experiment:                                        *
 * While it would be nice to get rid of the KeyValuePair vs            *
 * KeyValueArray code bifurcations, it's not a good tradeoff for the   *
 * assurance provided by KeyValuePair that there will be a singular    *
 * value present just by the fact that the key refers to a pair        *
 * instead of an array.  Also, the "work of values" would be preserved *
 * in a KeyValueArray only implementation even though the values would *
 * not be handled directly from a key, they are still handled as       *
 * members of the arrays and most of the required value functions      *
 * remain necessary in a KeyValueArray only implementation.  Finally,  *
 * in addition to being cumbersome to always check array sizes before  *
 * accessing the array, it is highly likely that some size checks      *
 * would be skipped in actual practice, leading to potential crashes   *
 * and exploit vulnerabilities.                                        *
 *                                                                     *
 * Bottom Line:                                                        *
 * KeyValuePairs are staying in the code, and the protocol definition  *
 * format will be using them.                                          *
 * *********************************************************************/

/**
 * @brief The ValueBase class - base class for all value types found in block objects
 */
class ValueBase : public QObject
{ Q_OBJECT
public:
              enum  Mode { make, build, idle, selected };
          explicit  ValueBase( QObject *p = nullptr ) : QObject( p ) { m_sel = false; m_key = RCD_null_z; }
virtual            ~ValueBase() {}
static   ValueBase *newValue( RiceyInt k, ValueBase *vbp = nullptr, ValueBase *vtc = nullptr );
static  JsonSerial  removeQuotes( const JsonSerial &j );
static  JsonSerial  ensureQuotes( const JsonSerial &j );
static   DotSerial  clusterWrap( Mode m, ValueBase *vb, const DotSerial &k );
static   DotSerial  lineColor( Mode m, qint32 d );
static   DotSerial  bgColor( Mode m, qint32 d );
static   DotSerial  wheelColor( const QColor &c, qreal hDep, qreal sDep, qreal lDep, qreal phase, qint32 d );
static   DotSerial  dotName( RiceyInt k );
static   DotSerial  dotArrayName( RiceyInt k, qint32 sz );
static   DotSerial  dotEmptyNode( qint32 i = 0 );
              bool  sel()   const { return m_sel; }
              void  setSel()      { m_sel = true; }
              void  clearSel()    { m_sel = false; }
            qint32  depth() const { if ( vbParent == nullptr ) return 0; return vbParent->depth()+1; }
virtual Utf8String  id()    const { if ( vbParent == nullptr ) return "_"; return vbParent->id()+idx(); }
virtual Utf8String  idx()   const { return m_idx; }
              void  setIdx( const Utf8String &s ) { m_idx = s; }
          RiceyInt  vKey()  const { return m_key; }
              void  setVKey( RiceyInt k ) { m_key = k; }
        Utf8String  metaText()    const;
virtual       void  setMetaData( RiceyInt, ValueBase * );
static  QByteArray  baoDefaultValue( qint8 );
virtual  ValueBase *firstChild()  const      { return nullptr; }
virtual  ValueBase *prevChild( ValueBase * ) { return nullptr; }
virtual  ValueBase *nextChild( ValueBase * ) { return nullptr; }
virtual       bool  isContainer() const = 0;
virtual       bool  isArray()     const { return false; }
virtual       bool  isKeyValue()  const { return false; }
virtual       bool  isKeyValuePair() const { return false; }
virtual       void  clear()             = 0;
virtual     quint8  type()        const = 0;
virtual     qint32  size()        const = 0;
virtual  BaoSerial  bao()         const = 0;
virtual JsonSerial  json()        const = 0;
virtual  DotSerial  dot(Mode m) const { (void)m; return json(); }
virtual     qint32  setBao ( const  BaoSerial & ) = 0;
virtual       bool  setJson( const JsonSerial & ) = 0;
         ValueBase *baoValueByKey( RiceyInt, const  BaoSerial &, qint32 *l = nullptr, ValueBase *vbp = nullptr ) const;
         ValueBase *jsonValueByKey   ( RiceyInt, const QJsonValue &, ValueBase * ) const;
virtual       bool  operator==( const ValueBase &v ) const { if ( v.type() != type() ) return false;  return v.bao() == bao(); };

QPointer<ValueBase> vbParent;
        Utf8String  m_idx;
              bool  m_sel;
          RiceyInt  m_key; // Copy of information at time of insertion
};

/**
 * @brief The BlockValueNull class - an empty value
 *
 * Container's Keys are type z
 */
class BlockValueNull : public ValueBase
{ Q_OBJECT
public:
     explicit  BlockValueNull( QObject *parent = nullptr ) : ValueBase( parent ) {}
              ~BlockValueNull() {}
 virtual bool  isContainer() const { return false; }
 virtual bool  isArray()     const { return false; }
         void  clear()             {}
       quint8  type()        const { return RDT_NULL; }
       qint32  size()        const { return 0; }
    BaoSerial  bao()         const { return BaoSerial(); }
   JsonSerial  json()        const { QString s = "NULL"; return s.toUtf8(); }
       qint32  setBao ( const  BaoSerial &b ) { (void)b; return -1;    }  // cannot set a value to a BlockValueNull
         bool  setJson( const JsonSerial &j ) { (void)j; return false; }
};

extern BlockValueNull  glob_null;   // Returned when BlockValueObject::valueAt() calls an invalid index
typedef  QMap<RiceyInt, ValueBase *> BlockObjectMap;
typedef QVarLengthArray<ValueBase *> ValueArray;

/**
 * @brief The ValueBaseArray class - a one dimensional array of zero or more ValueBase objects
 */
class ValueBaseArray : public ValueBase
{ Q_OBJECT
public:
          explicit  ValueBaseArray( QObject *parent = nullptr ) : ValueBase( parent ) {}
                    ValueBaseArray( const ValueBaseArray &v, QObject *parent = nullptr ) : ValueBase( parent ) { m_values = v.m_values; }
                   ~ValueBaseArray() {}
    ValueBaseArray &operator= ( const ValueBaseArray &v ) { clear(); m_values = v.m_values; return *this; }
virtual       bool  isContainer() const { return true; }
virtual       bool  isArray()     const { return true; }
         ValueBase *firstChild()  const { if ( m_values.size() > 0 ) return m_values.at(0); return nullptr; }
virtual  ValueBase *nextChild( ValueBase * );
virtual  ValueBase *prevChild( ValueBase * );
              void  clear() { m_values.clear(); }
            qint32  size()        const { return m_values.size(); }
         BaoSerial  bao()         const;
        JsonSerial  json()        const;
         DotSerial  dot(Mode)     const;
            qint32  setBao ( const  BaoSerial & );
              bool  setJson( const JsonSerial & );
              bool  append( ValueBase * );
              bool  insertAt( ValueBase *, qint32 );
         ValueBase *removeAt( qint32 );
         ValueBase *at( qint32 n ) const { if (( n >= 0 ) && ( n < size() )) return m_values.at(n); else return nullptr; }
              bool  operator==( const ValueBaseArray & ) const;

public:
        ValueArray  m_values; // Values in this array
};

/**
 * @brief The KeyValueBase class - common base for objects that contain a key and zero or more values
 */
class KeyValueBase : public ValueBase
{ Q_OBJECT
public:
           explicit  KeyValueBase( const RiceyInt &k, QObject *parent = nullptr ) : ValueBase( parent )  { setKey( k ); }
                    ~KeyValueBase() {}
static KeyValueBase *readBao(  const  BaoSerial &, QObject *parent = nullptr );
static KeyValueBase *readJson( const JsonSerial &, QObject *parent = nullptr );
     virtual   bool  isContainer() const { return true; }
     virtual   bool  isArray()     const =0;
     virtual   bool  isKeyValue()  const { return true; }
     virtual   void  clear()        {}
     virtual quint8  type()    const { return (vKey() & RDT_OBTYPEMASK); }
             qint32  setKey( const RiceyCode &r );
               bool  setKey( const RiceyInt &k ) { return( setKey( intToRice( k ) ) > 0 ); }
     virtual   void  set( ValueBase *vp ) = 0;
         Utf8String  keyHex()  const { return keyCode().toHex(); }
               char *keyHexd() const { return keyHex().data();   }
          RiceyCode  keyCode() const { return intToRice( vKey() ); }
           RiceyInt  key()     const { return vKey(); }
  virtual DotSerial  dot(Mode) const = 0;
  virtual      void  setValueKey(RiceyInt) = 0;
};

/**
 * @brief The KeyValuePair class - a single key and a single value (of any type)
 */
class KeyValuePair : public KeyValueBase
{ Q_OBJECT
public:
          explicit  KeyValuePair( const RiceyInt  &k,                QObject *p = nullptr ) : KeyValueBase( k,            p ) {}
                    KeyValuePair( const RiceyInt  &k, ValueBase *vp, QObject *p = nullptr ) : KeyValueBase( k,            p ) { set(vp); }
                    KeyValuePair( const BaoSerial &b,                QObject *p = nullptr ) : KeyValueBase( riceToInt(b), p ) { setBao(b); }
                   ~KeyValuePair() { if ( m_value ) m_value->deleteLater(); }
virtual       bool  isArray()     const { return false; }
virtual       bool  isKeyValuePair() const { return true; }
virtual  ValueBase *firstChild()  const { return m_value; }
virtual  ValueBase *nextChild( ValueBase *v ) { if ( m_value ) return m_value->nextChild(v); return nullptr; }
virtual  ValueBase *prevChild( ValueBase *v ) { if ( m_value ) return m_value->prevChild(v); return nullptr; }
virtual     qint32  size()    const { return 1; }
virtual       void  set( ValueBase *vp ) { if ( vp->type() != type() ) qWarning("kvp type mismatch"); else { if ( m_value ) m_value->deleteLater(); m_value = vp; } }
         ValueBase *value()   const { return m_value; }
virtual  BaoSerial  bao()     const;
virtual JsonSerial  json()    const;
virtual     qint32  setBao ( const BaoSerial & );
virtual       bool  setJson( const JsonSerial &j );
virtual  DotSerial  dot(Mode) const;
              void  setValueKey(RiceyInt k) { setVKey( k ); }

public:
  QPointer<ValueBase> m_value; // Value of this KeyValuePair
};

/**
 * @brief The KeyValueArray class - zero or more values of the same type stored under a single key
 *   order of items in the array is important, conserved, and determined by the creator of the array.
 *   Equivalent to a JsonObject with a single key: array value inside.
 */
class KeyValueArray : public KeyValueBase
{ Q_OBJECT
public:
          explicit  KeyValueArray( RiceyInt  k,           QObject *p = nullptr ) : KeyValueBase(            k, p ) { m_val = nullptr; }
          explicit  KeyValueArray( RiceyInt  k, ValueBaseArray *bva, QObject *p = nullptr ) : KeyValueBase( k, p ) { m_val = bva;     }
                    KeyValueArray( const BaoSerial &b,    QObject *p = nullptr ) : KeyValueBase( riceToInt(b), p ) { setBao(b); }
                   ~KeyValueArray() { clear(); }
virtual       bool  isArray()      const { return true; }
         ValueBase *firstChild()   const { if ( m_val ) if ( m_val->size() > 0 ) return m_val->at(0); return nullptr; }
virtual       void  clear()              { if ( m_val ) { m_val->clear(); m_val->deleteLater(); } m_val = nullptr; }
virtual     qint32  size()         const { if ( m_val ) return m_val->size(); return 0; }
              bool  append( ValueBase *value );
virtual       void  set( ValueBase *vp ) { if ((vp->type() & RDT_TYPEMASK) != (type() & RDT_TYPEMASK)) qWarning("kva type mismatch"); else { clear(); append( vp ); } }
         ValueBase *at( qint32 n ) const { if ( m_val ) return m_val->at(n); return nullptr; }
    ValueBaseArray *value()        const { return m_val; }
              bool  typeMatch( quint8 );
virtual  BaoSerial  bao()     const;
virtual JsonSerial  json()    const;
virtual     qint32  setBao ( const  BaoSerial &b );
virtual       bool  setJson( const JsonSerial &j );
virtual  DotSerial  dot(Mode) const;
              void  setValueKey(RiceyInt k) { if ( m_val == nullptr ) return;
                                              qint32 sz = m_val->m_values.size();
                                              for ( qint32 i = 0; i < sz; i++ )
                                                m_val->m_values.at(i)->setVKey( k );
                                            }
public:
  QPointer<ValueBaseArray> m_val;
};

/**
 * @brief The BlockValueInt64 class - single signed 64 bit integer
 *
 * Container's Keys are type i/I
 */
class BlockValueInt64 : public ValueBase
{ Q_OBJECT
public:
      explicit  BlockValueInt64( QObject *parent = nullptr ) : ValueBase( parent ) { set( 0 ); }
                BlockValueInt64( const qint64 &v, QObject *parent = nullptr ) : ValueBase( parent ) { set( v ); }
               ~BlockValueInt64() {}
virtual   bool  isContainer() const { return false; }
virtual   bool  isArray()     const { return false; }
          void  clear()             { m_value = 0; }
        quint8  type()        const { return RDT_INT64; }
virtual qint32  size()        const { return 1; }
     BaoSerial  bao()         const {  BaoSerial b; QDataStream s(&b,QIODevice::WriteOnly); s.setByteOrder(QDataStream::LittleEndian); s << m_value; return b; }
    JsonSerial  json()        const { return ensureQuotes( QByteArray::number( m_value ) ); }
        qint32  setBao ( const  BaoSerial &b );
          bool  setJson( const JsonSerial &j );
        qint64  value()       const { return m_value; }
          void  set       ( const qint64 &v )                 { m_value = v; }
          bool  operator==( const qint64 &v )           const { return v         == value(); }
          bool  operator==( const BlockValueInt64 &v  ) const { return v.value() == value(); }

        qint64  m_value;
};

class BlockValueInt64Array : public ValueBaseArray
{ Q_OBJECT
public:
      explicit  BlockValueInt64Array( QObject *parent = nullptr ) : ValueBaseArray( parent ) {}
                BlockValueInt64Array( const QList<qint64> &v, QObject *parent = nullptr ) : ValueBaseArray( parent ) { set(v); }
               ~BlockValueInt64Array() {}
        quint8  type()           const { return RDT_INT64_ARRAY; }
          bool  append(    qint64  v ) { return append( new BlockValueInt64(v, this) ); }
          bool  append( ValueBase *v ) { return ValueBaseArray::append( v ); }
        qint64  at( qint32 n )   const { if (( n >= 0 ) && ( n < size() )) return ((BlockValueInt64 *)m_values[n])->value(); qWarning( "array index %d out of bounds %d",n,size() ); return 0; }
  QList<qint64> value()          const { QList<qint64> vl; qint32 n = 0; while ( n < size() ) vl.append(at(n++)); return vl; }
          void  set( const QList<qint64> &vl ) { clear(); foreach( qint64 v, vl ) { if ( !append( v ) ) qWarning( "append Failed" ); } }
          bool  operator==(const QList<qint64>& l) const { if (l.size() != size()) return false; for (qint32 i=0;i<size();i++) if (l.at(i)!=at(i)) return false; return true; }
};

/**
 * @brief The BlockValueRiceyInt class - single unsigned 63 bit integer
 *
 * Container's Keys are type v/V - codes are not required to appear in the dictionary
 */
class BlockValueRiceyInt : public ValueBase
{ Q_OBJECT
public:
      explicit  BlockValueRiceyInt( QObject *parent = nullptr ) : ValueBase( parent ) { set( 0 ); }
                BlockValueRiceyInt( const RiceyInt  &v, QObject *parent = nullptr ) : ValueBase( parent ) { set( v ); }
                BlockValueRiceyInt( const RiceyCode &v, QObject *parent = nullptr ) : ValueBase( parent ) { set( v ); }
               ~BlockValueRiceyInt() {}
virtual   bool  isContainer() const { return false; }
virtual   bool  isArray()     const { return false; }
          void  clear()             { m_value = 0; }
        quint8  type()        const { return RDT_RICEYINT; }
virtual qint32  size()        const { return 1; }
     BaoSerial  bao()         const { return intToRice( m_value ); }
    JsonSerial  json()        const { return ensureQuotes( QByteArray::number( m_value ) ); }
        qint32  setBao ( const  BaoSerial &b );
          bool  setJson( const JsonSerial &j );
      RiceyInt  value()       const { return m_value; }
     RiceyCode  valueCode()   const { return intToRice( m_value ); }
          void  set       ( const RiceyInt &v )                  { m_value = v; }
          void  set       ( const RiceyCode &v )                 { set( riceToInt( v ) ); }
          bool  operator==( const RiceyInt &v )            const { return v         == value(); }
          bool  operator==( const BlockValueRiceyInt &v  ) const { return v.value() == value(); }

        qint64  m_value;
};

class BlockValueRiceyIntArray : public ValueBaseArray
{ Q_OBJECT
public:
      explicit  BlockValueRiceyIntArray( QObject *parent = nullptr ) : ValueBaseArray( parent ) {}
                BlockValueRiceyIntArray( const QList<RiceyInt> &v, QObject *parent = nullptr ) : ValueBaseArray( parent ) { set(v); }
               ~BlockValueRiceyIntArray() {}
        quint8  type()           const { return RDT_RICEYINT_ARRAY; }
          bool  append(  RiceyInt  v ) { return append( new BlockValueRiceyInt(v, this) ); }
          bool  append( ValueBase *v ) { return ValueBaseArray::append( v ); }
      RiceyInt  at( qint32 n )   const { if (( n >= 0 ) && ( n < size() )) return ((BlockValueRiceyInt *)m_values[n])->value(); qWarning( "array index %d out of bounds %d",n,size() ); return 0; }
QList<RiceyInt> value()          const { QList<RiceyInt> vl; qint32 n = 0; while ( n < size() ) vl.append(at(n++)); return vl; }
          void  set( const QList<RiceyInt> &vl ) { clear(); foreach( RiceyInt v, vl ) { if ( !append( v ) ) qWarning( "append Failed" ); } }
          bool  operator==(const QList<RiceyInt>& l) const { if (l.size() != size()) return false; for (qint32 i=0;i<size();i++) if (l.at(i)!=at(i)) return false; return true; }
};

/**
 * @brief The BlockValueRiceyCode class - a single Ricey Code, 1 to 9 octets in length
 *
 * Container's Keys are type c/C - codes are required to appear in the dictionary
 */
class BlockValueRiceyCode : public ValueBase
{ Q_OBJECT
public:
      explicit  BlockValueRiceyCode( QObject *parent = nullptr ) : ValueBase( parent ) {}
                BlockValueRiceyCode( const RiceyCode &v, QObject *parent = nullptr ) : ValueBase( parent ) { set(v); }
                BlockValueRiceyCode( const RiceyInt &r , QObject *parent = nullptr ) : ValueBase( parent ) { set(r); }
               ~BlockValueRiceyCode() {}
virtual   bool  isContainer() const { return false; }
virtual   bool  isArray()     const { return false; }
          void  clear()             { m_value = 0; }
        quint8  type()        const { return RDT_RCODE; }
virtual qint32  size()        const { return 1; }
     BaoSerial  bao()         const { return m_value; }
    JsonSerial  json()        const { return "\""+dict.nameFromCode(m_value)+"\""; }
        qint32  setBao ( const  BaoSerial & );
          bool  setJson( const JsonSerial & );
     RiceyCode  value()       const { return m_value; }
      RiceyInt  valueInt()    const { return riceToInt( m_value ); }
          void  set( const RiceyCode &v ) { if ( !validRicey( v ) ) qWarning("invalid ricey code"); else m_value = v; } // to detach or not to detach...
          void  set( const RiceyInt &r )  { set( intToRice( r ) ); }
          bool  operator==( const RiceyInt  &v ) const { return v == valueInt(); }
          bool  operator==( const RiceyCode &v ) const { return v == value(); }
          bool  operator==( const BlockValueRiceyCode &v  ) const { return v.value() == value(); }

     RiceyCode  m_value;
};

class BlockValueRiceyCodeArray : public ValueBaseArray
{ Q_OBJECT
public:
         explicit  BlockValueRiceyCodeArray( QObject *parent = nullptr ) : ValueBaseArray( parent ) {}
                   BlockValueRiceyCodeArray( QList<RiceyInt>  v, QObject *parent = nullptr ) : ValueBaseArray( parent ) { set(v); }
                   BlockValueRiceyCodeArray( QList<RiceyCode> v, QObject *parent = nullptr ) : ValueBaseArray( parent ) { set(v); }
                  ~BlockValueRiceyCodeArray() {}
           quint8  type()            const { return RDT_RCODE_ARRAY; }
             bool  append( RiceyInt   v )  { return append( new BlockValueRiceyCode(v, this) ); }
             bool  append( RiceyCode  v )  { return append( new BlockValueRiceyCode(v, this) ); }
             bool  append( ValueBase *v )  { return ValueBaseArray::append( v ); }
        RiceyCode  at( qint32 n )    const { if (( n >= 0 ) && ( n < size() )) return ((BlockValueRiceyCode *)m_values[n])->value(); qWarning( "array index %d out of bounds %d",n,size() ); return RiceyCode(); }
         RiceyInt  intAt( qint32 n ) const { if (( n >= 0 ) && ( n < size() )) return riceToInt( at(n) );                            qWarning( "array index %d out of bounds %d",n,size() ); return -1; }
  QList<RiceyCode> value()           const { QList<RiceyCode> vl; qint32 n = 0; while ( n < size() ) vl.append(   at(n++)); return vl; }
  QList<RiceyInt > valueInt()        const { QList<RiceyInt > vl; qint32 n = 0; while ( n < size() ) vl.append(intAt(n++)); return vl; }
             void  set( const QList<RiceyCode> &vl ) { clear(); foreach( RiceyCode v, vl ) { if ( !append( v ) ) qWarning( "append Failed" ); } }
             void  set( const QList<RiceyInt>  &vl ) { clear(); foreach( RiceyInt  v, vl ) { if ( !append( v ) ) qWarning( "append Failed" ); } }
             bool  operator==(const QList<RiceyCode>& l) const { if (l.size() != size()) return false; for (qint32 i=0;i<size();i++) if (l.at(i)!=   at(i)) return false; return true; }
             bool  operator==(const QList<RiceyInt> & l) const { if (l.size() != size()) return false; for (qint32 i=0;i<size();i++) if (l.at(i)!=intAt(i)) return false; return true; }
};

/**
 * @brief The BlockValueString class - a UTF-8 encoded string
 *
 * Container's Keys are type s/S
 */
class BlockValueString : public ValueBase
{ Q_OBJECT
public:
      explicit  BlockValueString( QObject *parent = nullptr ) : ValueBase( parent ) {}
                BlockValueString( const Utf8String &v, QObject *parent = nullptr ) : ValueBase( parent ) { set(v); }
               ~BlockValueString() {}
 static qint32  utf8CharSize( const char *, qint32 );
   static bool  validUtf8( const Utf8String & );
virtual   bool  isContainer() const { return false; }
virtual   bool  isArray()     const { return false; }
          void  clear()             { m_value.clear(); }
        quint8  type()        const { return RDT_STRING; }
virtual qint32  size()        const { return 1; }
     BaoSerial  bao()         const;
    JsonSerial  json()        const;
        qint32  setBao ( const  BaoSerial &b );
          bool  setJson( const JsonSerial &j );
          void  set    ( const Utf8String &v ) { if ( m_value.size() > MAX_LENGTH ) qWarning("MAX_LENGTH exceeded"); else m_value = v; }
    Utf8String  value()       const { return m_value; }
          bool  operator==( const Utf8String &v ) const { return v == value(); }
          bool  operator==( const BlockValueString &v  ) const { return v.value() == value(); }

    Utf8String  m_value;  // Strings are encoded as UTF8 in a QByteArray
};

class BlockValueStringArray : public ValueBaseArray
{ Q_OBJECT
public:
          explicit  BlockValueStringArray( QObject *parent = nullptr ) : ValueBaseArray( parent ) {}
                    BlockValueStringArray( const QList<Utf8String> &v, QObject *parent = nullptr ) : ValueBaseArray( parent ) { set(v); }
                   ~BlockValueStringArray() {}
            quint8  type() const { return RDT_STRING_ARRAY; }
              bool  append( Utf8String v ) { return append( new BlockValueString(v, this) ); }
              bool  append( ValueBase *v ) { return ValueBaseArray::append( v ); }
        Utf8String  at( qint32 n ) const { if (( n >= 0 ) && ( n < size() )) return ((BlockValueString *)m_values[n])->value(); qWarning( "array index %d out of bounds %d",n,size() ); return Utf8String(); }
  QList<Utf8String> value() const { QList<Utf8String> vl; qint32 n = 0; while ( n < size() ) vl.append(at(n++)); return vl; }
              void  set( const QList<Utf8String> &vl ) { clear(); foreach( Utf8String v, vl ) { if ( !append( v ) ) qWarning( "append Failed" ); } }
              bool  operator==(const QList<Utf8String>& l) const { if (l.size() != size()) return false; for (qint32 i=0;i<size();i++) if (l.at(i)!=at(i)) return false; return true; }
};

/**
 * @brief The BlockValueByteArray class - an array of octets
 *
 * Container's Keys are type b/B
 */
class BlockValueByteArray : public ValueBase
{ Q_OBJECT
public:
          explicit  BlockValueByteArray( QObject *parent = nullptr ) : ValueBase( parent ) {}
                    BlockValueByteArray( const QByteArray &v, QObject *parent = nullptr ) : ValueBase( parent ) { set(v); }
                   ~BlockValueByteArray() {}
    virtual   bool  isContainer() const { return false; }
    virtual   bool  isArray()     const { return false; }
              void  clear()             { m_value.clear(); }
            quint8  type()        const { return RDT_BYTEARRAY; }
    virtual qint32  size()        const { return 1; }
         BaoSerial  bao()         const;
        JsonSerial  json()        const { return "\""+m_value.toHex()+"\""; }
virtual  DotSerial  dot(Mode m)   const;
            qint32  setBao ( const  BaoSerial & );
              bool  setJson( const JsonSerial & );
        QByteArray  value()       const { return m_value; }
              void  set( const QByteArray &v ) { if ( m_value.size() > MAX_LENGTH ) qWarning("MAX_LENGTH exceeded"); else m_value = v; }
              bool  operator==( const QByteArray &v ) const { return v == value(); }
              bool  operator==( const BlockValueByteArray &v  ) const { return v.value() == value(); }

        QByteArray  m_value;
};

class BlockValueByteArrayArray : public ValueBaseArray
{ Q_OBJECT
public:
          explicit  BlockValueByteArrayArray( QObject *parent = nullptr ) : ValueBaseArray( parent ) {}
                    BlockValueByteArrayArray( const QList<QByteArray> &v, QObject *parent = nullptr ) : ValueBaseArray( parent ) { set(v); }
                   ~BlockValueByteArrayArray() {}
            quint8  type() const { return RDT_BYTEARRAY_ARRAY; }
              bool  append( QByteArray v ) { return append( new BlockValueByteArray(v, this) ); }
              bool  append( ValueBase *v ) { return ValueBaseArray::append( v ); }
        QByteArray  at( qint32 n ) const { if (( n >= 0 ) && ( n < size() )) return ((BlockValueByteArray *)m_values[n])->value(); qWarning( "array index %d out of bounds %d",n,size() ); return QByteArray(); }
  QList<QByteArray> value() const { QList<QByteArray> vl; qint32 n = 0; while ( n < size() ) vl.append(at(n++)); return vl; }
              void  set( const QList<QByteArray> &vl ) { clear(); foreach( QByteArray v, vl ) { if ( !append( v ) ) qWarning( "append Failed" ); } }
              bool  operator==(const QList<QByteArray>& l) const { if (l.size() != size()) return false; for (qint32 i=0;i<size();i++) if (l.at(i)!=at(i)) return false; return true; }
};

/**
 * @brief The BlockValueMPZ class - an arbitrary precision integer
 *
 * Container's Keys are type n/N
 */
class BlockValueMPZ : public ValueBase
{ Q_OBJECT
public:
          explicit  BlockValueMPZ( QObject *parent = nullptr ) : ValueBase( parent ) { mpz_init( &m_value ); }
                    BlockValueMPZ( const     MP_INT &v, QObject *parent = nullptr ) : ValueBase( parent ) { mpz_init( &m_value ); set(v); }
                    BlockValueMPZ( const Utf8String &s, QObject *parent = nullptr ) : ValueBase( parent ) { mpz_init( &m_value ); set(s); }
                   ~BlockValueMPZ() { clear(); }
virtual       bool  isContainer() const { return false; }
virtual       bool  isArray()     const { return false; }
              void  clear()             { mpz_clear( &m_value ); }
            quint8  type()        const { return RDT_MPZ; }
    virtual qint32  size()        const { return 1; }
 static Utf8String  toStr( const MP_INT & );
 static  BaoSerial  toBCD( const MP_INT & );
 static     qint32  fromBCD( const  BaoSerial &, mpz_t );
         BaoSerial  bao()         const { return toBCD( value() ); }
        JsonSerial  json()        const { return ensureQuotes( toStr( value() ) ); }
            qint32  setBao ( const  BaoSerial &b ) { return fromBCD( b, &m_value ); }
              bool  setJson( const JsonSerial &j );
            MP_INT  value()       const { return m_value; }
              void  set( const MP_INT &v ) { mpz_set( &m_value, &v ); }
              void  set( const Utf8String &v ) { setJson( ensureQuotes( v ) ); }
              bool  operator==( const MP_INT &v ) const { return ( mpz_cmp( &m_value, &v ) == 0 ); }
              bool  operator==( const BlockValueMPZ &v  ) const { MP_INT v1 = v.value(); return ( mpz_cmp( &m_value, &v1 ) == 0 ); }

            MP_INT  m_value;
};

class BlockValueMPZArray : public ValueBaseArray
{ Q_OBJECT
public:
      explicit  BlockValueMPZArray( QObject *parent = nullptr ) : ValueBaseArray( parent ) {}
               ~BlockValueMPZArray() {}
        quint8  type() const { return RDT_MPZ_ARRAY; }
          bool  append( MP_INT v ) { return append( new BlockValueMPZ(v, this) ); }
          bool  append( ValueBase *v ) { return ValueBaseArray::append( v ); }
        MP_INT  at( qint32 n ) const { if (( n >= 0 ) && ( n < size() )) return ((BlockValueMPZ *)m_values[n])->value(); MP_INT tv = temp; qWarning( "array index %d out of bounds %d",n,size() ); mpz_set_si( &tv, 0 ); return tv; }
  QList<MP_INT> value() const { QList<MP_INT> vl; qint32 n = 0; while ( n < size() ) vl.append(at(n++)); return vl; }
          void  set( const QList<MP_INT> &vl ) { clear(); foreach( MP_INT v, vl ) { if ( !append( v ) ) qWarning( "append Failed" ); } }
          bool  operator==(const QList<MP_INT>& l) const { if (l.size() != size()) return false; MP_INT tv = temp; for (qint32 i=0;i<size();i++) { tv = at(i); if ( mpz_cmp( &l.at(i), &tv ) != 0 ) return false; } return true; }
        MP_INT  temp;
};

/**
 * @brief The BlockValueMPQ class - a fraction of two arbitrary precision integers
 *
 * Container's Keys are type r/R
 */
class BlockValueMPQ : public ValueBase
{ Q_OBJECT
public:
      explicit  BlockValueMPQ( QObject *parent = nullptr ) : ValueBase( parent ) { mpq_init( &m_value ); }
                BlockValueMPQ( const     MP_RAT &v, QObject *parent = nullptr ) : ValueBase( parent ) { mpq_init( &m_value ); set(v); }
                BlockValueMPQ( const Utf8String &s, QObject *parent = nullptr ) : ValueBase( parent ) { mpq_init( &m_value ); set(s); }
               ~BlockValueMPQ() { clear(); }
virtual   bool  isContainer() const { return false; }
virtual   bool  isArray()     const { return false; }
          void  clear()             { mpq_clear( &m_value ); }
        quint8  type()        const { return RDT_MPQ; }
virtual qint32  size()        const { return 1; }
static Utf8String  toStr( const MP_RAT & );
     BaoSerial  bao()         const;
    JsonSerial  json()        const { return ensureQuotes( toStr( value() ) ); }
        qint32  setBao ( const  BaoSerial &b );
          bool  setJson( const JsonSerial &j );
        MP_RAT  value()       const { return m_value; }
          void  set( const MP_RAT &v ) { mpq_set( &m_value, &v ); }
          void  set( const Utf8String &v ) { setJson( ensureQuotes( v ) ); }
          bool  operator==( const MP_RAT &v ) const { return ( mpq_cmp( &m_value, &v ) == 0 ); }
          bool  operator==( const BlockValueMPQ &v  ) const { MP_RAT v1 = v.value(); return ( mpq_cmp( &m_value, &v1 ) == 0 ); }

        MP_RAT  m_value;
};

class BlockValueMPQArray : public ValueBaseArray
{ Q_OBJECT
public:
      explicit  BlockValueMPQArray( QObject *parent = nullptr ) : ValueBaseArray( parent ) {}
               ~BlockValueMPQArray() {}
        quint8  type() const { return RDT_MPQ_ARRAY; }
          bool  append( MP_RAT v ) { return append( new BlockValueMPQ(v, this) ); }
          bool  append( ValueBase *v ) { return ValueBaseArray::append( v ); }
        MP_RAT  at( qint32 n ) const { if (( n >= 0 ) && ( n < size() )) return ((BlockValueMPQ *)m_values[n])->value(); MP_RAT tv = temp; qWarning( "array index %d out of bounds %d",n,size() ); mpq_set_si( &tv, 1, 1 ); return tv; }
  QList<MP_RAT> value() const { QList<MP_RAT> vl; qint32 n = 0; while ( n < size() ) vl.append(at(n++)); return vl; }
          void  set( const QList<MP_RAT> &vl ) { clear(); foreach( MP_RAT v, vl ) { if ( !append( v ) ) qWarning( "append Failed" ); } }
          bool  operator==(const QList<MP_RAT>& l) const { if (l.size() != size()) return false;
                                                           MP_RAT tv;
                                                           for (qint32 i=0;i<size();i++)
                                                             { tv = at(i);
                                                               if ( mpq_cmp( &l.at(i), &tv ) != 0 ) return false;
                                                             }
                                                           return true;
                                                         }
        MP_RAT  temp;

};

/**
 * @brief The BlockValueObject class - zero or more KeyValue
 *   all with unique keys, types may be mixed.
 *
 * Container's Keys are type o/O
 */
class BlockValueObject : public ValueBase
{ Q_OBJECT
public:
         explicit  BlockValueObject( QObject *parent = nullptr )                          : ValueBase( parent ) {}
                   BlockValueObject( const BlockObjectMap &v, QObject *parent = nullptr ) : ValueBase( parent ) { set(v);     }
                   BlockValueObject( const      BaoSerial &b, QObject *parent = nullptr ) : ValueBase( parent ) { setBao(b);  }
                   BlockValueObject( QObject *parent, const JsonSerial &j )               : ValueBase( parent ) { setJson(j); }
                  ~BlockValueObject() { clear(); }
virtual      bool  isContainer() const { return true; }
virtual      bool  isArray()     const { return false; }
virtual ValueBase *firstChild()  const { if ( m_obMap.size() > 0 ) return m_obMap.first(); return nullptr; }
virtual ValueBase *nextChild( ValueBase * );
virtual ValueBase *prevChild( ValueBase * );
           quint8  type()        const { return RDT_OBJECT; }
        BaoSerial  bao()         const;
       JsonSerial  json()        const;
           qint32  setBao ( const  BaoSerial &b );
             bool  setJson( const JsonSerial &j );
        DotSerial  dot(Mode)     const;
             void  clear()             { QList<RiceyInt> keys = m_obMap.keys(); foreach( RiceyInt k, keys ) { if ( m_obMap[k] != nullptr ) m_obMap[k]->deleteLater(); m_obMap.remove(k); } }
           qint32  size()        const { return m_obMap.size(); }
   QList<RiceyInt> keys()        const { return m_obMap.keys(); }
             bool  contains( RiceyInt  k ) const { return m_obMap.contains( k ); }
             bool  contains( RiceyCode c ) const { return contains( riceToInt( c ) ); }
        ValueBase *remove( RiceyInt k );
           qint32  insert    ( const BlockObjectMap & );
             bool  insert    ( RiceyInt, ValueBase * );
             bool  insert    ( KeyValueArray * );
             bool  insert    ( RiceyInt k, const BlockObjectMap &v ) { BlockValueObject    *vp = new BlockValueObject   ( v, this ); bool ok = insert( k, vp ); if ( !ok ) vp->deleteLater(); return ok; }
             bool  insert    ( RiceyInt k,               qint64  v ) { BlockValueInt64     *vp = new BlockValueInt64    ( v, this ); bool ok = insert( k, vp ); if ( !ok ) vp->deleteLater(); return ok; }
             bool  insert    ( RiceyInt k,         const MP_INT &v ) { BlockValueMPZ       *vp = new BlockValueMPZ      ( v, this ); bool ok = insert( k, vp ); if ( !ok ) vp->deleteLater(); return ok; }
             bool  insert    ( RiceyInt k,         const MP_RAT &v ) { BlockValueMPQ       *vp = new BlockValueMPQ      ( v, this ); bool ok = insert( k, vp ); if ( !ok ) vp->deleteLater(); return ok; }
             bool  insert    ( RiceyInt k,             RiceyInt  v ) { BlockValueRiceyCode *vp = new BlockValueRiceyCode( v, this ); bool ok = insert( k, vp ); if ( !ok ) vp->deleteLater(); return ok; }
             bool  insert    ( RiceyInt k, const QByteArray &v ) { BlockValueString    *vs;
                                                                   BlockValueByteArray *vb;
                                                                   BlockValueRiceyCode *vr; bool ok;
                                                                   switch( k & RDT_TYPEMASK )
                                                                     { case RDT_STRING:    vs = new BlockValueString   ( v, this ); ok = insert( k, vs ); if ( !ok ) vs->deleteLater(); return ok;
                                                                       case RDT_BYTEARRAY: vb = new BlockValueByteArray( v, this ); ok = insert( k, vb ); if ( !ok ) vb->deleteLater(); return ok;
                                                                       case RDT_RCODE:     vr = new BlockValueRiceyCode( v, this ); ok = insert( k, vr ); if ( !ok ) vr->deleteLater(); return ok;
                                                                       default: return false;
                                                                 }   }
        ValueBase *value( const RiceyInt  &k ) const { if ( !contains(k) ) return &glob_null; return m_obMap.value(k); }
        ValueBase *value( const RiceyCode &r ) const { return value(riceToInt(r)); }
   BlockObjectMap  value() const { return m_obMap; }
           qint32  set       ( const BlockObjectMap  &vl ) { clear(); return insert( vl ); }
             bool  operator==( const BlockObjectMap   &  ) const;
             bool  operator==( const BlockValueObject &v ) const { return ( v == m_obMap ); }

   BlockObjectMap  m_obMap;
};

class BlockValueObjectArray : public ValueBaseArray
{ Q_OBJECT
public:
             explicit  BlockValueObjectArray( QObject *parent = nullptr ) : ValueBaseArray( parent ) {}
                      ~BlockValueObjectArray() {}
//BlockValueObjectArray &operator= ( const QList<BlockObjectMap> &v ) { set(v); return *this; }
//BlockValueObjectArray &operator+= ( const BlockObjectMap &v )    { append(v); return *this; }
               quint8  type() const { return RDT_OBJECT_ARRAY; }
                 bool  append( const BlockObjectMap &v ) { return append( new BlockValueObject(v, this) ); }
                 bool  append( ValueBase *v ) { return ValueBaseArray::append( v ); }
       BlockObjectMap  at( qint32 n ) const { if (( n >= 0 ) && ( n < size() )) return ((BlockValueObject *)m_values.at(n))->value(); qWarning( "array index %d out of bounds %d",n,size() ); return BlockObjectMap(); }
 QList<BlockObjectMap> value() const { QList<BlockObjectMap> vl; qint32 n = 0; while ( n < size() ) vl.append(at(n++)); return vl; }
                 void  set( const QList<BlockObjectMap> &v ) { clear(); for ( qint32 i = 0; i < v.size(); i++ ) { BlockObjectMap m = v.at(i); if ( !append( new BlockValueObject(m,this) ) ) qWarning( "append failed" ); } }
                 bool  operator==(const QList<BlockObjectMap>& l) const;
                 bool  operator==(const BlockValueObjectArray& o) const { return *this == o.value(); }
};

#endif // BLOCKOB_H

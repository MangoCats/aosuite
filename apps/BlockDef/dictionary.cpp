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
#include "dictionary.h"
#include <QFile>
#include <QByteArray>
#include <QJsonDocument>
#include <QJsonObject>

Dictionary::Dictionary()
{ Q_INIT_RESOURCE(blockDef);
  read();
}

void Dictionary::reread()
{ clear();
  read();
}

void Dictionary::clear()
{ while ( !codes.isEmpty() )
    codes.removeLast();
  while ( !types.isEmpty() )
    types.removeLast();
  ciByNum.clear();
  ciByName.clear();
  ciByRicey.clear();
}

void Dictionary::read()
{ QFile file( ":/data/dictionary.json" );
  if ( !file.open( QIODevice::ReadOnly ) )
    qWarning( "Cannot open dictionary.json resource for reading." );
   else
    interpret( file.readAll() );
}

/**
 * @brief Dictionary::interpret
 * @param ba - json library description
 */
void  Dictionary::interpret( const JsonSerial &js )
{ qint32 sz;
  QJsonDocument jd = QJsonDocument::fromJson( js );
  if ( !jd.isObject() )
    qWarning( "dictionary.json does not contain a json object." );
   else
    { QJsonObject jo = jd.object();
      if ( !jo.contains( "riceyTypes_O" ) )
        qWarning( "dictionary.json does not contain a riceyTypes_O array." );
       else if ( !jo["riceyTypes_O"].isArray() )
        qWarning( "riceyTypes_O is not an array." );
       else
        { types = jo["riceyTypes_O"].toArray();
          sz = types.size();
          for ( int i = 0; i < sz; i++ )
            { if ( !types.at(i).isObject() )
                qWarning( "riceyTypes_O at %d is not an object.", i );
               else
                {} // TODO: index the types for referencing
            } // for ( int i = 0; i < sz; i++ )
          qDebug( "interpreted %d elements in riceyTypes_O", sz );
        }

      if ( !jo.contains( "riceyCodes_O" ) )
        qWarning( "dictionary.json does not contain a riceyCodes_O array." );
       else if ( !jo["riceyCodes_O"].isArray() )
        qWarning( "riceyCodes_O is not an array." );
       else
        { codes = jo["riceyCodes_O"].toArray();
          sz = codes.size();
          for ( int i = 0; i < sz; i++ )
            { if ( !codes.at(i).isObject() )
                qWarning( "riceyCodes_O at %d is not an object.", i );
               else
                { QJsonObject co = codes.at(i).toObject();
                  if ( !co.contains("name_s") )
                    qWarning( "riceyCodes_O at %d has no name_s element.", i );
                   else
                    { ciByName.insert( co["name_s"].toString().toUtf8(), i );
                       diNames.insert( i, co["name_s"].toString().toUtf8() );
                    }
                  if ( !co.contains("type_b") )
                    qWarning( "riceyCodes_O at %d has no type_b element.", i );
                   else
                    { QByteArray ricey = QByteArray::fromHex( co["type_b"].toString().toUtf8() );
                      ciByRicey.insert( ricey, i );
                      ciByNum.insert( riceToInt(ricey), i );
                      // TODO: type checking vs the riceyTypes_O table info
                    }
                }
            } // for ( int i = 0; i < sz; i++ )
          qDebug( "interpreted %d elements in riceyCodes_O", sz );
        }
    }
}

/**
 * @brief Dictionary::nameOrHexFromCode
 * @param i - RiceyInt to convert
 * @return name, or ricey hex if not in dict
 */
Utf8String  Dictionary::nameOrHexFromCode( RiceyInt i )
{ return nameOrHexFromCode( intToRice( i ) ); }

Utf8String  Dictionary::nameOrHexFromCode( RiceyCode c )
{ Utf8String n = nameFromCode( c );
  if ( n.size() > 0 )
    return n;
  return c.toHex();
}

/**
 * @brief Dictionary::nameFromCode
 * @param c - ricey code to look up
 * @return corresponding name, or empty string if not found
 */
Utf8String  Dictionary::nameFromCode( RiceyCode c )
{ if ( !codesContainCode( c ) )
    return Utf8String();
  QJsonValue jv = codes.at(ciByRicey[c]);
  if ( !jv.isObject() )
    return Utf8String();
  QJsonObject jo = jv.toObject();
  if ( !jo.contains("name_s" ) )
    return Utf8String();
  return jo["name_s"].toString().toUtf8();
}

/**
 * @brief Dictionary::nameFromCode
 * @param c - ricey code interpreted to an integer
 * @return corresponding name, or empty string if not found
 */
Utf8String  Dictionary::nameFromCode( RiceyInt i )
{ if ( !codesContainCode( i ) )
    return Utf8String();
  QJsonValue jv = codes.at(ciByNum[i]);
  if ( !jv.isObject() )
    return Utf8String();
  QJsonObject jo = jv.toObject();
  if ( !jo.contains("name_s") )
    return Utf8String();
  return jo["name_s"].toString().toUtf8();
}

/**
 * @brief Dictionary::riceyFromCodeName
 * @param n - name to lookup
 * @return corresponding code, or empty code if not found
 */
RiceyCode  Dictionary::riceyFromCodeName( Utf8String n )
{ if ( !codesContainName(n) )
    return RiceyCode();
  QJsonValue jv = codes.at(ciByName[n]);
  if ( !jv.isObject() )
    return RiceyCode();
  QJsonObject jo = jv.toObject();
  if ( !jo.contains("type_b") )
    return RiceyCode();
  return RiceyCode::fromHex( jo["type_b"].toString().toUtf8() );
}

/**
 * @brief Dictionary::codeFromCodeName
 * @param n - name to lookup
 * @return integer value of corresponding code, -1 if not found
 */
RiceyInt  Dictionary::codeFromCodeName( Utf8String n )
{ if ( !codesContainName(n) )
    return -1;
  QJsonValue jv = codes.at(ciByName[n]);
  if ( !jv.isObject() )
    return -1;
  QJsonObject jo = jv.toObject();
  if ( !jo.contains("type_b") )
    return -1;
  return riceToInt( RiceyCode::fromHex( jo["type_b"].toString().toUtf8() ) );
}

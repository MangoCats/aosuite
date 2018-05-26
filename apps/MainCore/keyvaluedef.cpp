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
#include "keyvaluedef.h"
#include <QFile>
#include <QJsonArray>
#include <QJsonDocument>
#include <QTextStream>

void  KeyValueDef::fromJsonObject( const QJsonObject &jo )
{ if ( jo.contains( "key"  ) ) key  = jo.value( "key"  ).toInt();
  if ( jo.contains( "desc" ) ) desc = jo.value( "desc" ).toString();
  if ( jo.contains( "type" ) ) tn   = jo.value( "type" ).toString();
  if ( jo.contains( "pdef" ) ) pdef = jo.value( "pdef" ).toString();
}

QString KeyValueDef::toDefine()
{ return QString( "#define %1 %2 // %3 %4" )
           .arg( pdef ).arg( key ).arg(tn).arg(desc);
}

///////////////////////////////////////////////////////////////////////////////

void KeyValueDefinitions::fromFile( const QString &filename )
{ kvdList.clear();
  QFile file( filename );
  if ( !file.open( QIODevice::ReadOnly ) )
    { // TODO: log error
      return;
    }
  QJsonDocument doc = QJsonDocument::fromJson( file.readAll() );
  if ( !doc.isArray() )
    { // TODO: log error
      return;
    }
  QJsonArray ja = doc.array();
  foreach( const QJsonValue &jv, ja )
    kvdList.append( KeyValueDef( jv.toObject() ) );
}

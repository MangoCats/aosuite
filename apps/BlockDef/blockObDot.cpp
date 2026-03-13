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

// Design Intent:
//
// The dot() functions of block object family members are intended to create
//   a read-only graphviz presentation of the block structure, showing object
//   nesting, key labels and data value summaries.  Larger data objects may be
//   abbreviated to convey the structure of larger blocks rather than the
//   absolute complete contents.
//
// When protocols start specifying operations, the intent is for the .dot graphs
//    to include arrows and symbols to illustrate the operands, operations and
//    where the results of the operations appear in the structure.
//

#include "blockOb.h"

/* Calculation edge drawing syntax:
 node_145 -> hashCalc [constraint=false];
 node_138 -> hashCalc [constraint=false ltail=cluster_76];
 node_141 -> hashCalc [constraint=false];
 hashCalc -> node_143;

 or drawn on the right side:

 node_145 -> hashCalc [constraint=false];
 node_138 -> hashCalc [constraint=false ltail=cluster_76];
 node_141 -> hashCalc [constraint=false];
 node_143 -> hashCalc [dir=back];
 */

qint32 dex = 0;

DotSerial ValueBase::dotEmptyNode( qint32 i )
{ return DotSerial(i,' ')+"node_m"+DotSerial::number( dex++ )+" [ label=\"\"; shape=plaintext; ];\n"; }

DotSerial ValueBase::dotName( RiceyInt k )
{ if ( dict.codesContainCode( k ) )
    return dict.nameFromCode(k);
   else
    return intToRice( k ).toHex();
}

DotSerial ValueBase::dotArrayName( RiceyInt k, qint32 sz )
{ return dotName(k) + ( "["+DotSerial::number( sz )+"]" ); }

DotSerial ValueBase::clusterWrap( Mode m, ValueBase *vb, const DotSerial &kn )
{ DotSerial dot;
       Mode sm  = vb ? (vb->sel() ? selected : m ) : m;
       bool nm  = vb ? (vb->sel() ? true : false ) : false;
     qint32 d   = vb ? vb->depth() : 0;
  DotSerial uid = vb ? vb->id()    : "_z"+DotSerial::number( dex++ );
  DotSerial v   = vb ? vb->dot(m)  : "";
  DotSerial lab = ensureQuotes( /* uid+":" +DotSerial::number(d)+":"+ */ removeQuotes( kn ) );
  qint32 i = 2; // indent spaces per depth level
  dot.append( DotSerial(d*i,' ')+"subgraph cluster"+uid+" {\n" );
  dot.append( DotSerial(2+d*i,' ')+"label = "+lab+";\n" );
  dot.append( DotSerial(2+d*i,' ')+lineColor( sm, d )+"\n" );
  dot.append( DotSerial(2+d*i,' ')+  bgColor( sm, d )+"\n" );
  dot.append( DotSerial(2+d*i,' ')+"margin = 4;\n\n" );
  if ( v.size() > 0 )
    { if ( !v.trimmed().startsWith( "subgraph" ) && !v.trimmed().startsWith( "node" ) )
        { dot.append( DotSerial(4+d*i,' ')+"node"+uid+" [ label="+v+"; "+( nm ? "color=crimson" : lineColor(m,d) )+" ];\n" );
        }
       else
        { dot.append( v );
          if ( !v.trimmed().endsWith( ";" ) )
            dot.append( DotSerial(3+d*i,' ')+";" );
          dot.append( "\n" );
        }
    }
   else
    dot.append( dotEmptyNode(4+d*i) );
  dot.append( DotSerial(d*i,' ')+"}\n" );
  return dot;
}

DotSerial ValueBase::lineColor( Mode m, qint32 depth )
{ switch ( m )
    { case make:     return "color=\""+wheelColor( QColor( "darkgreen"  ), 0.10, 0.15, 0.15, 1.6, depth)+"\";";
      case build:    return "color=\""+wheelColor( QColor( "darkblue"   ), 0.05, 0.15, 0.1 , 1.6, depth)+"\";";
      case idle:     return "color=\""+wheelColor( QColor( "grey"       ), 0.05, 0.0 , 0.1 , 1.6, depth)+"\";";
      case selected: return "color=\""+wheelColor( QColor( "crimson"    ), 0.05, 0.02, 0.1 , 1.6, depth)+"\";";
    }
  return "";
}

DotSerial ValueBase::bgColor( Mode m, qint32 depth )
{ switch ( m )
    { case make:     return "bgcolor=\""+wheelColor( QColor( "mintcream" ), 0.05, 0.12, 0.07, 0.0, depth)+"\";";
      case build:    return "bgcolor=\""+wheelColor( QColor( "#D4E6F8"   ), 0.04, 0.10, 0.06, 0.0, depth)+"\";";
      case idle:     return "bgcolor=\""+wheelColor( QColor( "gainsboro" ), 0.05, 0.05, 0.1 , 0.0, depth)+"\";";
      case selected: return "bgcolor=\""+wheelColor( QColor( "#FFD8D8"   ), 0.02, 0.05, 0.02, 0.0, depth)+"\";";
    }
  return "";
}

DotSerial ValueBase::wheelColor( const QColor &c, qreal hDep, qreal sDep, qreal lDep, qreal ph, qint32 depth )
{ if ( hDep > 0.5 ) hDep = 0.5;
  if ( hDep < 0.0 ) hDep = 0.0;
  if ( sDep > 0.5 ) sDep = 0.5;
  if ( sDep < 0.0 ) sDep = 0.0;
  if ( lDep > 0.5 ) lDep = 0.5;
  if ( lDep < 0.0 ) lDep = 0.0;
  qreal nHue = c.hslHueF();
  qreal nSat = c.hslSaturationF(); if (nSat < sDep) nSat = sDep; if ( (nSat + sDep) > 1.0 ) nSat = 1.0 - sDep;
  qreal nLig = c.lightnessF();     if (nLig < lDep) nLig = lDep; if ( (nLig + lDep) > 1.0 ) nLig = 1.0 - lDep;
  nHue += hDep * cos( ((qreal)depth)* 70.0*6.28318530718/360.0 + ph );
  nSat += sDep * cos( ((qreal)depth)* 25.0*6.28318530718/360.0 + ph );
  nLig += lDep * sin( ((qreal)depth)*130.0*6.28318530718/360.0 + ph );
  while ( nHue > 1.0 ) { nHue -= 1.0; } if ( nHue < 0.0 ) nHue = 0.0;
  if ( nSat > 1.0 ) nSat = 1.0;
  if ( nSat < 0.0 ) nSat = 0.0;
  if ( nLig > 1.0 ) nLig = 1.0;
  if ( nLig < 0.0 ) nLig = 0.0;
  QColor ct; ct.setHslF( nHue, nSat, nLig );
  DotSerial cs;
  cs.append( (quint8)ct.red()   );
  cs.append( (quint8)ct.green() );
  cs.append( (quint8)ct.blue()  );
  return "#"+cs.toHex();
}

DotSerial KeyValuePair::dot(Mode m) const
{ return clusterWrap( m, value(),      dotName( key() )         ); }

DotSerial KeyValueArray::dot(Mode m) const
{ return clusterWrap( m, value(), dotArrayName( key(), size() ) ); }

DotSerial  ValueBaseArray::dot(Mode m) const
{ DotSerial d;
  if ( size() == 0 )
    d.append( dotEmptyNode(depth()*2+4) );
   else for ( qint32 i = 0; i < size(); i++ )
    d.append( clusterWrap( m, at(i), "["+DotSerial::number(i)+"]" ) );
  return d;
}

DotSerial BlockValueObject::dot(Mode m) const
{ DotSerial d;
  QList<RiceyInt>keys = value().keys();
  if ( keys.size() == 0 )
    d.append( dotEmptyNode(depth()*2+4) );
   else foreach ( RiceyInt i, keys )
    { if (( value(i)->type() & RDT_ARRAY ) == 0 )
        d.append( clusterWrap( m, value(i),     dotName( i )                   ) );
       else
        d.append( clusterWrap( m, value(i),dotArrayName( i, value(i)->size() ) ) );
    }
  return d;
}

/**
 * @brief BlockValueByteArray::dot
 * @param m - mode, unused here
 * @return hex representation of byte array, maxed out at 20 bytes shown, show size when > 20
 */
DotSerial  BlockValueByteArray::dot(Mode m) const
{ (void)m;
  DotSerial s = removeQuotes( json() );
  qint32 sz = s.size()/2; // sz = Number of bytes represented
  if ( sz <= 12 )
    return ensureQuotes( s );
  if ( sz <= 20 )
    { qint32 m = 2*(sz/2);
      return  ensureQuotes( s.mid( 0,m )+"\n"+s.mid(m) );
    }
  return ensureQuotes( s.mid( 0,20 )+
         QString( "...\n%1 bytes total\n..." ).arg( sz ).toUtf8()+
                       s.mid( s.size()-20 ) );
}


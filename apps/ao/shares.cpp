// Assign Onward
//
#include "shares.h"

Shares::Shares( __int128 val, QObject *parent )
  : QObject(parent), n(val) {}

Shares::Shares( const QByteArray &ba, QObject *p ) : QObject(p)
{ if ( ba.size() < 18 )
    { n = 0;
      // TODO: log an exception
      return;
    }
  union _128_as_8
    { __int128 i;
      unsigned char d[16];
    } u;
  unsigned char chk = ba.at(0);
  // if (( chk & AO_CODE_MASK ) != AO_SHARES_CODE )
  //   TODO: log a warning
  chk ^= u.d[ 0] = ba.at( 1);
  chk ^= u.d[ 1] = ba.at( 2);
  chk ^= u.d[ 2] = ba.at( 3);
  chk ^= u.d[ 3] = ba.at( 4);
  chk ^= u.d[ 4] = ba.at( 5);
  chk ^= u.d[ 5] = ba.at( 6);
  chk ^= u.d[ 6] = ba.at( 7);
  chk ^= u.d[ 7] = ba.at( 8);
  chk ^= u.d[ 8] = ba.at( 9);
  chk ^= u.d[ 9] = ba.at(10);
  chk ^= u.d[10] = ba.at(11);
  chk ^= u.d[11] = ba.at(12);
  chk ^= u.d[12] = ba.at(13);
  chk ^= u.d[13] = ba.at(14);
  chk ^= u.d[14] = ba.at(15);
  chk ^= u.d[15] = ba.at(16);
  // if ( chk != ba.at(17) )
  //   TODO: log a warning
  n = u.i;
}

void Shares::operator = ( const QByteArray &ba )
{ Shares temp( ba );
  n = temp.n;
  return;
}

QByteArray Shares::toByteArray( unsigned char code )
{ QByteArray ba;
  union _128_as_8
    { __int128 i;
      unsigned char d[16];
    } u;
  // if (( code & AO_CODE_MASK ) != AO_SHARES_CODE )
  //   TODO: log a warning
  u.i = n;
  ba.append( code );
  ba.append( u.d[ 0] ); code ^= u.d[ 0];
  ba.append( u.d[ 1] ); code ^= u.d[ 1];
  ba.append( u.d[ 2] ); code ^= u.d[ 2];
  ba.append( u.d[ 3] ); code ^= u.d[ 3];
  ba.append( u.d[ 4] ); code ^= u.d[ 4];
  ba.append( u.d[ 5] ); code ^= u.d[ 5];
  ba.append( u.d[ 6] ); code ^= u.d[ 6];
  ba.append( u.d[ 7] ); code ^= u.d[ 7];
  ba.append( u.d[ 8] ); code ^= u.d[ 8];
  ba.append( u.d[ 9] ); code ^= u.d[ 9];
  ba.append( u.d[10] ); code ^= u.d[10];
  ba.append( u.d[11] ); code ^= u.d[11];
  ba.append( u.d[12] ); code ^= u.d[12];
  ba.append( u.d[13] ); code ^= u.d[13];
  ba.append( u.d[14] ); code ^= u.d[14];
  ba.append( u.d[15] ); code ^= u.d[15];
  ba.append( code );
  return ba;
}
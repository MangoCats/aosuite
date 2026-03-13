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
#include <QtWidgets>
#include "tests.h"
#include "ui_tests.h"
#include "blockOb.h"
#include <unistd.h>

// TODO: test intentional failures, error checking, length reporting, trailing input data, etc.

Tests::Tests( QWidget *cw ) :
    QScrollArea(cw),
    ui(new Ui::Tests)
{ ui->setupUi(this);
  if ( cw )
    { new QVBoxLayout( cw );
      cw->layout()->addWidget( this );
    }
}

Tests::~Tests()
{ delete ui; }

void Tests::liveDelay( int t )
{ QTimer timer;
  timer.setSingleShot( true );
  timer.start( t );
  QCoreApplication::processEvents( QEventLoop::AllEvents, t );
  while ( timer.isActive() )
    { usleep( 5000 ); // Get 5ms of "real sleep"
      QCoreApplication::processEvents( QEventLoop::AllEvents, 50 );
    }
}

void  Tests::on_start_clicked()
{ ui->report->clear();
  bool pass = true;
  qint32 count = 0;
  qint32 tc = 0;
  QString msg;
  pass &= testGmp        ( msg, tc ); ui->report->append( msg ); count += tc; tc = 0; liveDelay(4);
  pass &= testDict       ( msg, tc ); ui->report->append( msg ); count += tc; tc = 0; liveDelay(4);
  pass &= testInt64      ( msg, tc ); ui->report->append( msg ); count += tc; tc = 0; liveDelay(4);
  pass &= testRiceyInt   ( msg, tc ); ui->report->append( msg ); count += tc; tc = 0; liveDelay(4);
  pass &= testRicey      ( msg, tc ); ui->report->append( msg ); count += tc; tc = 0; liveDelay(4);
  pass &= testMPZ        ( msg, tc ); ui->report->append( msg ); count += tc; tc = 0; liveDelay(4);
  pass &= testMPQ        ( msg, tc ); ui->report->append( msg ); count += tc; tc = 0; liveDelay(4);
  pass &= testString     ( msg, tc ); ui->report->append( msg ); count += tc; tc = 0; liveDelay(4);
  pass &= testByteArray  ( msg, tc ); ui->report->append( msg ); count += tc; tc = 0; liveDelay(4);
  pass &= testObject     ( msg, tc ); ui->report->append( msg ); count += tc; tc = 0; liveDelay(4);
  pass &= testInt64A     ( msg, tc ); ui->report->append( msg ); count += tc; tc = 0; liveDelay(4);
  pass &= testMPZA       ( msg, tc ); ui->report->append( msg ); count += tc; tc = 0; liveDelay(4);
  pass &= testMPQA       ( msg, tc ); ui->report->append( msg ); count += tc; tc = 0; liveDelay(4);
  pass &= testRiceyIntA  ( msg, tc ); ui->report->append( msg ); count += tc; tc = 0; liveDelay(4);
  pass &= testRiceyA     ( msg, tc ); ui->report->append( msg ); count += tc; tc = 0; liveDelay(4);
  pass &= testStringA    ( msg, tc ); ui->report->append( msg ); count += tc; tc = 0; liveDelay(4);
  pass &= testByteArrayA ( msg, tc ); ui->report->append( msg ); count += tc; tc = 0; liveDelay(4);
  pass &= testObjectA    ( msg, tc ); ui->report->append( msg ); count += tc; tc = 0; liveDelay(4);

  if ( pass )
    ui->report->append( QString( "\nPassed all %1 tests" ).arg( count ) );
   else
    ui->report->append( "FAILED one or more tests" );
}

bool Tests::testGmp( QString &msg, qint32 &tc )
{ bool pass = true;
  msg = "GMP Test: ";
  char str[128];

  MP_INT i1,i2,i3;
  mpz_init_set_str( &i2, "2", 10 );
  mpz_init_set_si( &i3, -3 );

  mpz_init_set_si( &i1, 0 );
  if ( mpz_get_si( &i1 ) == 0 ) tc++; else
    { msg.append( "FAIL set/get 0 test.\n" ); pass = false; }
  mpz_get_str( str, 10, &i1 );
  if ( strcmp( str, "0" ) == 0 ) tc++; else
    { msg.append( "FAIL set/get 0 str test.\n" ); pass = false; }

  mpz_set_si( &i1, 1 );
  if ( mpz_get_si( &i1 ) == 1 ) tc++; else
    { msg.append( "FAIL set/get 1 test.\n" ); pass = false; }
  mpz_get_str( str, 10, &i1 );
  if ( strcmp( str, "1" ) == 0 ) tc++; else
    { msg.append( "FAIL set/get 1 str test.\n" ); pass = false; }

  if ( mpz_get_si( &i2 ) == 2 ) tc++; else
    { msg.append( "FAIL set/get 2 test.\n" ); pass = false; }

  if ( mpz_get_si( &i3 ) == -3 ) tc++; else
    { msg.append( "FAIL set/get -3 test.\n" ); pass = false; }
  mpz_get_str( str, 10, &i3 );
  if ( strcmp( str, "-3" ) == 0 ) tc++; else
    { msg.append( "FAIL set/get -3 str test.\n" ); pass = false; }

  mpz_set_str( &i3, "4", 10 );
  if ( mpz_get_si( &i3 ) == 4 ) tc++; else
    { msg.append( "FAIL set/get 4 test.\n" ); pass = false; }

  mpz_set_str( &i3, "-5", 10 );
  if ( mpz_get_si( &i3 ) == -5 ) tc++; else
    { msg.append( "FAIL set/get -5 test.\n" ); pass = false; }

  mpz_mul_ui( &i1, &i2, 2 );
  if ( mpz_get_si( &i1 ) == 4 ) tc++; else
    { msg.append( "FAIL multiply 4 test.\n" ); pass = false; }

  mpz_mul_ui( &i2, &i1, 4 );
  if ( mpz_get_si( &i2 ) == 16 ) tc++; else
    { msg.append( "FAIL multiply 16 test.\n" ); pass = false; }
  mpz_get_str( str, 10, &i2 );
  if ( strcmp( str, "16" ) == 0 ) tc++; else
    { msg.append( "FAIL set/get 16 str test.\n" ); pass = false; }

  mpz_mul_ui( &i1, &i2, 16 );
  if ( mpz_get_si( &i1 ) == 256 ) tc++; else
    { msg.append( "FAIL multiply 256 test.\n" ); pass = false; }

  mpz_mul_ui( &i2, &i1, 256 );
  if ( mpz_get_si( &i2 ) == 65536 ) tc++; else
    { msg.append( "FAIL multiply 65536 test.\n" ); pass = false; }
  mpz_get_str( str, 10, &i2 );
  if ( strcmp( str, "65536" ) == 0 ) tc++; else
    { msg.append( "FAIL set/get 65536 str test.\n" ); pass = false; }

  mpz_sqrtrem ( &i1, &i3, &i2 );
  if ( mpz_get_si( &i1 ) == 256 ) tc++; else
    { msg.append( "FAIL sqrt 256 test.\n" ); pass = false; }
  if ( mpz_get_si( &i3 ) == 0 ) tc++; else
    { msg.append( "FAIL sqrt remainder 0 test.\n" ); pass = false; }

  mpz_init_set_si( &i3, 65536 );
  if ( mpz_cmp( &i1, &i2 ) < 0 ) tc++; else
    { msg.append( "FAIL compare less test.\n" ); pass = false; }
  if ( mpz_cmp( &i3, &i1 ) > 0 ) tc++; else
    { msg.append( "FAIL compare more test.\n" ); pass = false; }
  if ( mpz_cmp( &i3, &i2 ) == 0 ) tc++; else
    { msg.append( "FAIL compare equal test.\n" ); pass = false; }

  mpz_set_str( &i1, "111111111111111111111111111111111111111111111", 10 );
  mpz_set_str( &i2, "222222222222222222222222222222222222222222222", 10 );
  mpz_add( &i3, &i1, &i2 );
  mpz_set_str( &i1, "333333333333333333333333333333333333333333333", 10 );
  if ( mpz_cmp( &i3, &i1 ) == 0 ) tc++; else
    { msg.append( "FAIL compare bignum test.\n" ); pass = false; }
  mpz_get_str( str, 10, &i1 );
  if ( strcmp( str, "333333333333333333333333333333333333333333333" ) == 0 ) tc++; else
    { msg.append( "FAIL set/get 333333333333333333333333333333333333333333333 str test.\n" ); pass = false; }

   BaoSerial bcd;

  mpz_set_str( &i1, "0", 10 );
  bcd = BlockValueMPZ::toBCD( i1 );
  if ( bcd.toHex() == "0e" ) tc++; else
    { msg.append( QString( "FAIL BCD conversion: 0 -> %1\n" ).arg( QString::fromUtf8( bcd.toHex()) ) ); pass = false; }
  BlockValueMPZ::fromBCD( bcd, &i2 );
  if ( mpz_cmp( &i1, &i2 ) == 0 ) tc++; else
    { msg.append( QString( "FAIL to/from BCD test %1.\n" ).arg( QString::fromUtf8(bcd.toHex().data()) ) ); pass = false; }

  mpz_set_str( &i1, "1", 10 );
  bcd = BlockValueMPZ::toBCD( i1 );
  if ( bcd.toHex() == "1e" ) tc++; else
    { msg.append( QString( "FAIL BCD conversion: 1 -> %1\n" ).arg( QString::fromUtf8( bcd.toHex()) ) ); pass = false; }
  BlockValueMPZ::fromBCD( bcd, &i2 );
  if ( mpz_cmp( &i1, &i2 ) == 0 ) tc++; else
    { msg.append( QString( "FAIL to/from BCD test %1.\n" ).arg( QString::fromUtf8(bcd.toHex().data()) ) ); pass = false; }

  mpz_set_str( &i1, "-2", 10 );
  bcd = BlockValueMPZ::toBCD( i1 );
  if ( bcd.toHex() == "2f" ) tc++; else
    { msg.append( QString( "-2 -> %1\n" ).arg( QString::fromUtf8( bcd.toHex()) ) ); pass = false; }
  BlockValueMPZ::fromBCD( bcd, &i2 );
  if ( mpz_cmp( &i1, &i2 ) == 0 ) tc++; else
    { msg.append( QString( "FAIL to/from BCD test %1.\n" ).arg( QString::fromUtf8(bcd.toHex().data()) ) ); pass = false; }

  mpz_set_str( &i1, "33", 10 );
  bcd = BlockValueMPZ::toBCD( i1 );
  if ( bcd.toHex() == "33ed" ) tc++; else
    { msg.append( QString( "33 -> %1\n" ).arg( QString::fromUtf8( bcd.toHex()) ) ); pass = false; }
  BlockValueMPZ::fromBCD( bcd, &i2 );
  if ( mpz_cmp( &i1, &i2 ) == 0 ) tc++; else
    { msg.append( QString( "FAIL to/from BCD test %1.\n" ).arg( QString::fromUtf8(bcd.toHex().data()) ) ); pass = false; }

  mpz_set_str( &i1, "-44", 10 );
  bcd = BlockValueMPZ::toBCD( i1 );
  if ( bcd.toHex() == "44fd" ) tc++; else
    { msg.append( QString( "-44 -> %1\n" ).arg( QString::fromUtf8( bcd.toHex()) ) ); pass = false; }
  BlockValueMPZ::fromBCD( bcd, &i2 );
  if ( mpz_cmp( &i1, &i2 ) == 0 ) tc++; else
    { msg.append( QString( "FAIL to/from BCD test %1.\n" ).arg( QString::fromUtf8(bcd.toHex().data()) ) ); pass = false; }

  mpz_set_str( &i1, "555", 10 );
  bcd = BlockValueMPZ::toBCD( i1 );
  if ( bcd.toHex() == "555e" ) tc++; else
    { msg.append( QString( "555 -> %1\n" ).arg( QString::fromUtf8( bcd.toHex()) ) ); pass = false; }
  BlockValueMPZ::fromBCD( bcd, &i2 );
  if ( mpz_cmp( &i1, &i2 ) == 0 ) tc++; else
    { msg.append( QString( "FAIL to/from BCD test %1.\n" ).arg( QString::fromUtf8(bcd.toHex().data()) ) ); pass = false; }

  mpz_set_str( &i1, "-666", 10 );
  bcd = BlockValueMPZ::toBCD( i1 );
  if ( bcd.toHex() == "666f" ) tc++; else
    { msg.append( QString( "-666 -> %1\n" ).arg( QString::fromUtf8( bcd.toHex()) ) ); pass = false; }
  BlockValueMPZ::fromBCD( bcd, &i2 );
  if ( mpz_cmp( &i1, &i2 ) == 0 ) tc++; else
    { msg.append( QString( "FAIL to/from BCD test %1.\n" ).arg( QString::fromUtf8(bcd.toHex().data()) ) ); pass = false; }

  mpz_set_str( &i1, "7777", 10 );
  bcd = BlockValueMPZ::toBCD( i1 );
  if ( bcd.toHex() == "7777ed" ) tc++; else
    { msg.append( QString( "7777 -> %1\n" ).arg( QString::fromUtf8( bcd.toHex()) ) ); pass = false; }
  BlockValueMPZ::fromBCD( bcd, &i2 );
  if ( mpz_cmp( &i1, &i2 ) == 0 ) tc++; else
    { msg.append( QString( "FAIL to/from BCD test %1.\n" ).arg( QString::fromUtf8(bcd.toHex().data()) ) ); pass = false; }

  mpz_set_str( &i1, "-8888", 10 );
  bcd = BlockValueMPZ::toBCD( i1 );
  if ( bcd.toHex() == "8888fd" ) tc++; else
    { msg.append( QString( "-8888 -> %1\n" ).arg( QString::fromUtf8( bcd.toHex()) ) ); pass = false; }
  BlockValueMPZ::fromBCD( bcd, &i2 );
  if ( mpz_cmp( &i1, &i2 ) == 0 ) tc++; else
    { msg.append( QString( "FAIL to/from BCD test %1.\n" ).arg( QString::fromUtf8(bcd.toHex().data()) ) ); pass = false; }

  mpz_set_str( &i1, "99999", 10 );
  bcd = BlockValueMPZ::toBCD( i1 );
  if ( bcd.toHex() == "99999e" ) tc++; else
    { msg.append( QString( "99999 -> %1\n" ).arg( QString::fromUtf8( bcd.toHex()) ) ); pass = false; }
  BlockValueMPZ::fromBCD( bcd, &i2 );
  if ( mpz_cmp( &i1, &i2 ) == 0 ) tc++; else
    { msg.append( QString( "FAIL to/from BCD test %1.\n" ).arg( QString::fromUtf8(bcd.toHex().data()) ) ); pass = false; }

  mpz_set_str( &i1, "-123456789", 10 );
  bcd = BlockValueMPZ::toBCD( i1 );
  if ( bcd.toHex() == "123456789f" ) tc++; else
    { msg.append( QString( "-123456789 -> %1\n" ).arg( QString::fromUtf8( bcd.toHex()) ) ); pass = false; }
  BlockValueMPZ::fromBCD( bcd, &i2 );
  if ( mpz_cmp( &i1, &i2 ) == 0 ) tc++; else
    { msg.append( QString( "FAIL to/from BCD test %1.\n" ).arg( QString::fromUtf8(bcd.toHex().data()) ) ); pass = false; }
  mpz_get_str( str, 10, &i1 );
  if ( strcmp( str, "-123456789" ) == 0 ) tc++; else
    { msg.append( "FAIL set/get -123456789 str test.\n" ); pass = false; }

  mpz_set_str( &i1, "9876543210", 10 );
  bcd = BlockValueMPZ::toBCD( i1 );
  if ( bcd.toHex() == "9876543210ed" ) tc++; else
    { msg.append( QString( "9876543210 -> %1\n" ).arg( QString::fromUtf8( bcd.toHex()) ) ); pass = false; }
  BlockValueMPZ::fromBCD( bcd, &i2 );
  if ( mpz_cmp( &i1, &i2 ) == 0 ) tc++; else
    { msg.append( QString( "FAIL to/from BCD test %1.\n" ).arg( QString::fromUtf8(bcd.toHex().data()) ) ); pass = false; }

  mpz_set_str( &i1, "987654321001234567899876543210012345678998765432100123456789", 10 );
  bcd = BlockValueMPZ::toBCD( i1 );
  if ( bcd.toHex() == "987654321001234567899876543210012345678998765432100123456789ed" ) tc++; else
    { msg.append( QString( "987654321001234567899876543210012345678998765432100123456789 -> %1\n" ).arg( QString::fromUtf8( bcd.toHex()) ) ); pass = false; }
  BlockValueMPZ::fromBCD( bcd, &i2 );
  if ( mpz_cmp( &i1, &i2 ) == 0 ) tc++; else
    { msg.append( QString( "FAIL to/from BCD test %1.\n" ).arg( QString::fromUtf8(bcd.toHex().data()) ) ); pass = false; }
  mpz_get_str( str, 10, &i1 );
  if ( strcmp( str, "987654321001234567899876543210012345678998765432100123456789" ) == 0 ) tc++; else
    { msg.append( "FAIL set/get 987654321001234567899876543210012345678998765432100123456789 str test.\n" ); pass = false; }

  mpz_clear( &i1 );
  mpz_clear( &i2 );
  mpz_clear( &i3 );

  if ( pass )
    msg.append( QString("Pass %1 functional tests.").arg(tc) );
  return true;
}

bool Tests::testDict( QString &msg, qint32 &tc )
{ bool pass = true;
  msg = "Dictionary Test: ";
  QList<RiceyInt> rList = dict.ciByNum.keys();
  foreach ( RiceyInt i, rList )
    { Utf8String name = dict.nameFromCode(i);
        RiceyInt j    = dict.codeFromCodeName( name );
        RiceyInt k    = dict.codeFromCodeName( name );
       RiceyCode code = dict.riceyFromCodeName( name );
      Utf8String cnme = dict.nameFromCode( code );
            bool cc1  = dict.codesContainName( name );
            bool cc2  = dict.codesContainCode( i );
            bool cc3  = dict.codesContainCode( code );
            bool tp   = true;
      if (( j != i ) || ( k != i ) || !cc1 || !cc2 || !cc3 ||
          ( riceToInt(code) != i ) || ( cnme != name ) ||
          ( intToRice(i) != code ))
        pass = tp = false;
      if ( tp )
        tc++;
       else
        msg.append( QString( "FAIL i %1 j %2 k %3 cc1 %4 cc2 %5 cc3 %6 code %7 name %8 cnme %9 rti %10 itr %11\n" )
                               .arg(i).arg(j).arg(k).arg(cc1).arg(cc2).arg(cc3)
                               .arg(QString::fromUtf8(code.toHex()),
                                    QString::fromUtf8(name),
                                    QString::fromUtf8(cnme))
                               .arg(riceToInt(code))
                               .arg(QString::fromUtf8(intToRice(i).toHex()))
                            );
    }
  if ( pass )
    msg.append( QString("Pass self consistency %1 entries.").arg(tc) );

  return pass;
}


bool Tests::testInt64( QString &msg, qint32 &tc )
{ bool pass = true;
  msg = "int64 Test: ";
  BlockValueInt64 v(this);
  if ( v.type() == RDT_INT64 ) tc++; else
    { msg.append( "FAIL type() test.\n" ); pass = false; }

  qint64 tv;
  tv =                    0LL; pass &= testInt64( v, tv, tc, msg );
  tv =                    1LL; pass &= testInt64( v, tv, tc, msg );
  tv =                   -1LL; pass &= testInt64( v, tv, tc, msg );
  tv =                70000LL; pass &= testInt64( v, tv, tc, msg );
  tv =               -70000LL; pass &= testInt64( v, tv, tc, msg );
  tv =           5123456789LL; pass &= testInt64( v, tv, tc, msg );
  tv =          -5123456789LL; pass &= testInt64( v, tv, tc, msg );
  tv =     9007199254740992LL; pass &= testInt64( v, tv, tc, msg );
  tv =    -9007199254740992LL; pass &= testInt64( v, tv, tc, msg );
  tv =    12345678901234567LL; pass &= testInt64( v, tv, tc, msg );
  tv =   -12345678901234567LL; pass &= testInt64( v, tv, tc, msg );
  tv =  9223372036854775807LL; pass &= testInt64( v, tv, tc, msg );
  tv = -9223372036854775807LL; pass &= testInt64( v, tv, tc, msg );
  for ( qint64 i = 9223372036854775000LL; i < 9223372036854775807LL; i++ )
    { tv = i;                pass &= testInt64( v, tv, tc, msg ); }
  for ( qint64 i = -9223372036854775807LL; i > -9223372036854775000LL; i-- )
    { tv = i;                pass &= testInt64( v, tv, tc, msg ); }

  if ( pass )
    msg.append( QString("Pass %1 tests.").arg(tc) );

  return pass;
}

bool Tests::testInt64( BlockValueInt64 &v, qint64 tv, qint32 &tc, QString &msg )
{ bool pass = true;
  v.set( tv );
  if ( v.value() == tv ) tc++; else
    { msg.append( QString( "FAIL value set/get test %1\n" ).arg(tv) ); pass = false; }

   BaoSerial b = v.bao();
  v.set( tv + 1 );
  if  ( v.value() != tv ) tc++; else
    { msg.append( QString( "FAIL inequality test %1 %2\n" ).arg(tv).arg( v.value() ) ); pass = false; }
  v.setBao( b );
  if ( v.value() == tv ) tc++; else
    { msg.append( QString( "FAIL bao encode/decode test %1 %2\n" ).arg(tv).arg( QString::fromUtf8(b.toHex()) ) ); pass = false; }
  if ( b == v.bao() ) tc++; else
    { msg.append( QString( "FAIL bao repeat test %1 %2\n" )
            .arg( QString::fromUtf8(b.toHex()),
                  QString::fromUtf8(v.bao().toHex()) ) ); pass = false; }

  JsonSerial j = v.json();
  v.set( tv - 1 );
  if  ( v.value() != tv ) tc++; else
    { msg.append( QString( "FAIL inequality test %1 %2\n" ).arg(tv).arg( v.value() ) ); pass = false; }
  bool ok = v.setJson( j );
  if ( ok && ( v.value() == tv )) tc++; else
    { msg.append( QString( "FAIL json encode/decode test %1 %2 %3\n" ).arg(tv).arg(ok).arg( QString::fromUtf8(j) ) ); pass = false; }

  return pass;
}

bool Tests::testRicey( QString &msg, qint32 &tc )
{ bool pass = true;
  msg = "ricey Test: ";
  BlockValueRiceyCode v(this);
  if ( v.type() == RDT_RCODE ) tc++; else
    { msg.append( "FAIL type() test.\n" ); pass = false; }

  RiceyInt tv;
  tv = RCD_ob_o         ; pass &= testRicey( v, tv, tc, msg );
  tv = RCD_int64_i      ; pass &= testRicey( v, tv, tc, msg );
  tv = RCD_code_s       ; pass &= testRicey( v, tv, tc, msg );
  tv = RCD_chainBlock_o ; pass &= testRicey( v, tv, tc, msg );
  tv = RCD_SHA256_c     ; pass &= testRicey( v, tv, tc, msg );
  tv = RCD_itemRelAddr_C; pass &= testRicey( v, tv, tc, msg );
  tv = RCD_RangeBounds_O; pass &= testRicey( v, tv, tc, msg );
  tv = RCD_Identity_o   ; pass &= testRicey( v, tv, tc, msg );
  tv = RCD_NineCode_z   ; pass &= testRicey( v, tv, tc, msg );
  tv = RCD_MaxiCode_z   ; pass &= testRicey( v, tv, tc, msg );

  if ( pass )
    msg.append( QString("Pass %1 tests.").arg(tc) );

  return pass;
}

bool Tests::testRicey( BlockValueRiceyCode &v, RiceyInt tv, qint32 &tc, QString &msg )
{ bool pass = true;
  v.set( tv );
  if ( v.valueInt() == tv ) tc++; else
    { msg.append( QString( "FAIL value set/get test %1\n" ).arg(tv) ); pass = false; }

  RiceyCode rc = v.value();
  if ( rc == intToRice( tv ) ) tc++; else
    { msg.append( QString( "FAIL code get test %1\n" ).arg(tv) ); pass = false; }

  rc = intToRice( tv );
  v.set( tv + 1 );
  if  ( v.valueInt() != tv ) tc++; else
    { msg.append( QString( "FAIL inequality test %1 %2\n" ).arg(tv).arg( v.valueInt() ) ); pass = false; }
  v.set( rc );
  if ( v.valueInt() == tv ) tc++; else
    { msg.append( QString( "FAIL code set test %1\n" ).arg(tv) ); pass = false; }

   BaoSerial b = v.bao();
  v.set( tv + 1 );
  v.setBao( b );
  if ( v.valueInt() == tv ) tc++; else
    { msg.append( QString( "FAIL bao encode/decode test %1 %2\n" ).arg(tv).arg( QString::fromUtf8(b.toHex()) ) ); pass = false; }
  if ( b == v.bao() ) tc++; else
    { msg.append( QString( "FAIL bao repeat test %1 %2\n" )
            .arg( QString::fromUtf8(b.toHex()),
                  QString::fromUtf8(v.bao().toHex()) ) ); pass = false; }

  JsonSerial j = v.json();
  v.set( tv + 1 );
  bool ok = v.setJson( j );
  if ( ok && ( v.valueInt() == tv )) tc++; else
    { msg.append( QString( "FAIL json encode/decode test %1 %2 %3\n" ).arg(tv).arg(ok).arg( QString::fromUtf8(b.toHex()) ) ); pass = false; }

  return pass;
}

bool Tests::testRiceyInt( QString &msg, qint32 &tc )
{ bool pass = true;
  msg = "riceyInt Test: ";
  BlockValueRiceyInt v(this);
  if ( v.type() == RDT_RICEYINT ) tc++; else
    { msg.append( "FAIL type() test.\n" ); pass = false; }

  RiceyInt tv;
  tv = 0                ; pass &= testRiceyInt( v, tv, tc, msg );
  tv = 1                ; pass &= testRiceyInt( v, tv, tc, msg );
  tv = 2                ; pass &= testRiceyInt( v, tv, tc, msg );
  tv = 126              ; pass &= testRiceyInt( v, tv, tc, msg );
  tv = 127              ; pass &= testRiceyInt( v, tv, tc, msg );
  tv = 128              ; pass &= testRiceyInt( v, tv, tc, msg );
  tv = 129              ; pass &= testRiceyInt( v, tv, tc, msg );
  tv = 65535            ; pass &= testRiceyInt( v, tv, tc, msg );
  tv = 65536            ; pass &= testRiceyInt( v, tv, tc, msg );
  tv = 549755813887LL   ; pass &= testRiceyInt( v, tv, tc, msg );
  tv = 549755813888LL   ; pass &= testRiceyInt( v, tv, tc, msg );
  tv = RCD_NineCode_z   ; pass &= testRiceyInt( v, tv, tc, msg );
  tv = RCD_MaxiCode_z   ; pass &= testRiceyInt( v, tv, tc, msg );

  if ( pass )
    msg.append( QString("Pass %1 tests.").arg(tc) );

  return pass;
}

bool Tests::testRiceyInt( BlockValueRiceyInt &v, RiceyInt tv, qint32 &tc, QString &msg )
{ bool pass = true;
  v.set( tv );
  if ( v.value() == tv ) tc++; else
    { msg.append( QString( "FAIL value set/get test %1\n" ).arg(tv) ); pass = false; }

  v.set( tv + 1 );
  if  ( v.value() != tv ) tc++; else
    { msg.append( QString( "FAIL inequality test %1 %2\n" ).arg(tv).arg( v.value() ) ); pass = false; }
  v.set( intToRice( tv ) );
  if ( v.value() == tv ) tc++; else
    { msg.append( QString( "FAIL code set test %1\n" ).arg(tv) ); pass = false; }

  BaoSerial b = v.bao();
  v.set( tv + 1 );
  v.setBao( b );
  if ( v.value() == tv ) tc++; else
    { msg.append( QString( "FAIL bao encode/decode test %1 %2\n" ).arg(tv).arg( QString::fromUtf8(b.toHex()) ) ); pass = false; }
  if ( b == v.bao() ) tc++; else
    { msg.append( QString( "FAIL bao repeat test %1 %2\n" )
            .arg( QString::fromUtf8(b.toHex()),
                  QString::fromUtf8(v.bao().toHex()) ) ); pass = false; }

  JsonSerial j = v.json();
  v.set( tv + 1 );
  bool ok = v.setJson( j );
  if ( ok && ( v.value() == tv )) tc++; else
    { msg.append( QString( "FAIL json encode/decode test %1 %2 %3\n" ).arg(tv).arg(ok).arg( QString::fromUtf8(b.toHex()) ) ); pass = false; }

  return pass;
}

bool Tests::testMPZ( QString &msg, qint32 &tc )
{ bool pass = true;
  msg = "MPZ Test: ";
  BlockValueMPZ v(this);
  if ( v.type() == RDT_MPZ ) tc++; else
    { msg.append( "FAIL type() test.\n" ); pass = false; }

  MP_INT tv;
  mpz_init_set_str( &tv, "0", 10 );  pass &= testMPZ( v, tv, tc, msg );
  mpz_set_str( &tv,  "12345", 10 );  pass &= testMPZ( v, tv, tc, msg );
  mpz_set_str( &tv, "-54321", 10 );  pass &= testMPZ( v, tv, tc, msg );
  mpz_set_str( &tv, "123456789009876543211234567890098765432112345678900987654321", 10 );
                                     pass &= testMPZ( v, tv, tc, msg );
  mpz_set_str( &tv, "-123456789009876543211234567890098765432112345678900987654321", 10 );
                                     pass &= testMPZ( v, tv, tc, msg );
  mpz_set_str( &tv, "1234567890098765432112345678900987654321123456789009876543210", 10 );
                                     pass &= testMPZ( v, tv, tc, msg );
  mpz_set_str( &tv, "-1234567890098765432112345678900987654321123456789009876543210", 10 );
                                     pass &= testMPZ( v, tv, tc, msg );

  if ( pass )
    msg.append( QString("Pass %1 tests.").arg(tc) );

  return pass;
}

bool Tests::testMPZ( BlockValueMPZ &v, const MP_INT &tv, qint32 &tc, QString &msg )
{ bool pass = true;
  v.set( tv );
  MP_INT v1 = v.value();
  if ( mpz_cmp( &v1, &tv ) == 0 ) tc++; else
    { msg.append( QString( "FAIL value set/get test %1\n" ).arg(QString::fromUtf8(BlockValueMPZ::toStr(tv))) ); pass = false; }
  if ( v == tv ) tc++; else
    { msg.append( QString( "FAIL value set/get == operator test %1\n" ).arg(QString::fromUtf8(BlockValueMPZ::toStr(tv))) ); pass = false; }

  MP_INT v2;
  mpz_init_set_si( &v2, -5551212 );
  v.set( v2 );
  v1 = v.value();
  if ( mpz_cmp( &v1, &tv ) != 0 ) tc++; else
    { msg.append( QString( "FAIL value set/get other test\n" ).arg(QString::fromUtf8(BlockValueMPZ::toStr(tv))) ); pass = false; }
  if ( !( v == tv ) ) tc++; else
    { msg.append( QString( "FAIL value set/get == operator other test %1\n" ).arg(QString::fromUtf8(BlockValueMPZ::toStr(tv))) ); pass = false; }

  v.set( tv );
   BaoSerial b = v.bao();
  v.set( v2 );
  v.setBao( b );
  if ( v == tv ) tc++; else
    { msg.append( QString( "FAIL bao encode/decode test %1 %2\n" ).arg(QString::fromUtf8(BlockValueMPZ::toStr(tv))).arg( QString::fromUtf8(b.toHex()) ) ); pass = false; }
  if ( b == v.bao() ) tc++; else
    { msg.append( QString( "FAIL bao repeat test %1 %2\n" )
            .arg( QString::fromUtf8(b.toHex()),
                  QString::fromUtf8(v.bao().toHex()) ) ); pass = false; }

  v.set( tv );
  JsonSerial j = v.json();
  v.set( v2 );
  bool ok = v.setJson( j );
  if ( ok && ( v == tv )) tc++; else
    { msg.append( QString( "FAIL json encode/decode test %1 %2 %3\n" )
                     .arg(QString::fromUtf8(BlockValueMPZ::toStr(tv)))
                     .arg(ok).arg( QString::fromUtf8(j) ) ); pass = false; }

  return pass;
}

bool Tests::testMPQ( QString &msg, qint32 &tc )
{ bool pass = true;
  msg = "MPQ Test: ";
  BlockValueMPQ v(this);
  if ( v.type() == RDT_MPQ ) tc++; else
    { msg.append( "FAIL type() test.\n" ); pass = false; }

  MP_RAT tv;
  mpq_init( &tv );
  mpq_set_si( &tv,  1,  2 ); mpq_canonicalize( &tv ); pass &= testMPQ( v, tv, tc, msg );
  mpq_set_si( &tv,  2,  1 ); mpq_canonicalize( &tv ); pass &= testMPQ( v, tv, tc, msg );
  mpq_set_si( &tv,  0,  1 ); mpq_canonicalize( &tv ); pass &= testMPQ( v, tv, tc, msg );
  mpq_set_si( &tv,  1,  1 ); mpq_canonicalize( &tv ); pass &= testMPQ( v, tv, tc, msg );
  mpq_set_si( &tv, -1,  1 ); mpq_canonicalize( &tv ); pass &= testMPQ( v, tv, tc, msg );
  mpq_set_si( &tv, -1,  2 ); mpq_canonicalize( &tv ); pass &= testMPQ( v, tv, tc, msg );
  mpq_set_si( &tv, -2,  2 ); mpq_canonicalize( &tv ); pass &= testMPQ( v, tv, tc, msg );
  mpq_set_si( &tv,  1, -2 ); mpq_canonicalize( &tv ); pass &= testMPQ( v, tv, tc, msg );
  mpq_set_si( &tv,  6, -2 ); mpq_canonicalize( &tv ); pass &= testMPQ( v, tv, tc, msg );
  mpq_set_si( &tv, -2, -4 ); mpq_canonicalize( &tv ); pass &= testMPQ( v, tv, tc, msg );
  mpq_set_si( &tv,  2,  4 ); mpq_canonicalize( &tv ); pass &= testMPQ( v, tv, tc, msg );
  mpq_set_si( &tv,  4,  2 ); mpq_canonicalize( &tv ); pass &= testMPQ( v, tv, tc, msg );
  mpq_set_si( &tv, 12, 10 ); mpq_canonicalize( &tv ); pass &= testMPQ( v, tv, tc, msg );
  mpq_set_si( &tv, 10, 12 ); mpq_canonicalize( &tv ); pass &= testMPQ( v, tv, tc, msg );
  MP_INT v1,v2;
  mpz_init( &v1 );
  mpz_init( &v2 );
  mpz_set_str( &v1, "12345", 10 );
  mpz_set_str( &v2, "54321", 10 );
  mpq_set_num( &tv, &v1 );
  mpq_set_den( &tv, &v2 );
  mpq_canonicalize( &tv );
  pass &= testMPQ( v, tv, tc, msg );
  mpz_set_str( &v1, "12345678900987654321", 10 );
  mpz_set_str( &v2, "01234567899876543210", 10 );
  mpq_set_num( &tv, &v1 );
  mpq_set_den( &tv, &v2 );
  mpq_canonicalize( &tv );
  pass &= testMPQ( v, tv, tc, msg );
  mpz_set_str( &v1, "-123456789009876543211234567890098765432101234567899876543210", 10 );
  mpz_set_str( &v2, "012345678998765432100123456789987654321001234567899876543210", 10 );
  mpq_set_num( &tv, &v1 );
  mpq_set_den( &tv, &v2 );
  mpq_canonicalize( &tv );
  pass &= testMPQ( v, tv, tc, msg );

  mpz_clear( &v1 );
  mpz_clear( &v2 );
  mpq_clear( &tv );
  if ( pass )
    msg.append( QString("Pass %1 tests.").arg(tc) );

  return pass;
}

bool Tests::testMPQ( BlockValueMPQ &v, const MP_RAT &tv, qint32 &tc, QString &msg )
{ bool pass = true;
  v.set( tv );
  MP_RAT v1 = v.value();
  if ( mpq_cmp( &v1, &tv ) == 0 ) tc++; else
    { msg.append( QString( "FAIL value set/get test %1\n" ).arg(QString::fromUtf8(BlockValueMPQ::toStr(tv))) ); pass = false; }
  if ( v == tv ) tc++; else
    { msg.append( QString( "FAIL value set/get == operator test %1\n" ).arg(QString::fromUtf8(BlockValueMPQ::toStr(tv))) ); pass = false; }

  MP_RAT v2;
  mpq_init( &v2 );
  mpq_set_si( &v2, -5551212, 987654321 );
  v.set( v2 );
  v1 = v.value();
  if ( mpq_cmp( &v1, &tv ) != 0 ) tc++; else
    { msg.append( QString( "FAIL value set/get other test\n" ).arg(QString::fromUtf8(BlockValueMPQ::toStr(tv))) ); pass = false; }
  if ( !( v == tv ) ) tc++; else
    { msg.append( QString( "FAIL value set/get == operator other test %1\n" ).arg(QString::fromUtf8(BlockValueMPQ::toStr(tv))) ); pass = false; }

  v.set( tv );
   BaoSerial b = v.bao();
  v.set( v2 );
  v.setBao( b );
  if ( v == tv ) tc++; else
    { msg.append( QString( "FAIL bao encode/decode test %1 %2\n" ).arg(QString::fromUtf8(BlockValueMPQ::toStr(tv))).arg( QString::fromUtf8(b.toHex()) ) ); pass = false; }
  if ( b == v.bao() ) tc++; else
    { msg.append( QString( "FAIL bao repeat test %1 %2\n" )
            .arg( QString::fromUtf8(b.toHex()),
                  QString::fromUtf8(v.bao().toHex()) ) ); pass = false; }

  v.set( tv );
  JsonSerial j = v.json();
  v.set( v2 );
  bool ok = v.setJson( j );
  if ( ok && ( v == tv )) tc++; else
    { msg.append( QString( "FAIL json encode/decode test %1 %2 %3\n" )
                     .arg(QString::fromUtf8(BlockValueMPQ::toStr(tv)))
                     .arg(ok).arg( QString::fromUtf8(j) ) ); pass = false; }

  return pass;
}

bool Tests::testString( QString &msg, qint32 &tc )
{ bool pass = true;
  msg = "string Test: ";
  BlockValueString v(this);
  if ( v.type() == RDT_STRING ) tc++; else
    { msg.append( "FAIL type() test.\n" ); pass = false; }

  Utf8String tv;
  tv = "";                                         pass &= testString( v, tv, tc, msg );
  tv = "X";                                        pass &= testString( v, tv, tc, msg );
  tv = "1";                                        pass &= testString( v, tv, tc, msg );
  tv = "Stringy McStringface";                     pass &= testString( v, tv, tc, msg );
  tv = "Ⓐ~!@#$%^&*()`[]{}\\|;'\":<>?/.,_+-=\r\n"; pass &= testString( v, tv, tc, msg );
  for ( int i = 0; i < 1024*768; i++ )
    tv.append( "Ⓐ~!@#$%^&*()`[]{}\\|;'\":<>?/.,_+-=abcdefghijklmnopqrstuvwxyz01234567890" );
                                                   pass &= testString( v, tv, tc, msg );
  // TODO: international characters, more wingdings, etc.

  if ( pass )
    msg.append( QString("Pass %1 tests.").arg(tc) );

  return pass;
}

bool Tests::testString( BlockValueString &v, const Utf8String &tv, qint32 &tc, QString &msg )
{ bool pass = true;
  v.set( tv );
  if ( v.value() == tv ) tc++; else
    { msg.append( QString( "FAIL value set/get test %1\n" ).arg(QString::fromUtf8(tv)) ); pass = false; }

   BaoSerial b = v.bao();
  v.set( "?" );
  if  ( v.value() != tv ) tc++; else
    { msg.append( QString( "FAIL inequality test %1 %2\n" ).arg( QString::fromUtf8(tv), QString::fromUtf8( v.value() ) ) ); pass = false; }
  v.setBao( b );
  if ( v.value() == tv ) tc++; else
    { msg.append( QString( "FAIL bao encode/decode test %1 %2\n" ).arg(QString::fromUtf8(tv), QString::fromUtf8(b.toHex()) ) ); pass = false; }
  if ( b == v.bao() ) tc++; else
    { msg.append( QString( "FAIL bao repeat test %1 %2\n" )
            .arg( QString::fromUtf8(b.toHex()),
                  QString::fromUtf8(v.bao().toHex()) ) ); pass = false; }

  JsonSerial j = v.json();
  v.set( "x" );
  if  ( v.value() != tv ) tc++; else
    { msg.append( QString( "FAIL inequality test %1 %2\n" ).arg( QString::fromUtf8(tv), QString::fromUtf8( v.value() ) ) ); pass = false; }
  bool ok = v.setJson( j );
  if ( ok && ( v.value() == tv )) tc++; else
    { msg.append( QString( "FAIL json encode/decode test %1 %2 %3\n" ).arg(QString::fromUtf8(tv)).arg(ok).arg( QString::fromUtf8(j) ) ); pass = false; }

  return pass;
}

bool Tests::testByteArray( QString &msg, qint32 &tc )
{ bool pass = true;
  msg = "byteArray Test: ";
  BlockValueByteArray v(this);
  if ( v.type() == RDT_BYTEARRAY ) tc++; else
    { msg.append( "FAIL type() test.\n" ); pass = false; }

  QByteArray tv;
  tv = "";                                         pass &= testByteArray( v, tv, tc, msg );
  tv = "X";                                        pass &= testByteArray( v, tv, tc, msg );
  tv = "1";                                        pass &= testByteArray( v, tv, tc, msg );
  tv = "Stringy McStringface";                     pass &= testByteArray( v, tv, tc, msg );
  tv = "Ⓐ~!@#$%^&*()`[]{}\\|;'\":<>?/.,_+-=\r\n"; pass &= testByteArray( v, tv, tc, msg );
  for ( int i = 0; i < 1024*768; i++ )
    tv.append( "Ⓐ~!@#$%^&*()`[]{}\\|;'\":<>?/.,_+-=abcdefghijklmnopqrstuvwxyz01234567890" );
                                                   pass &= testByteArray( v, tv, tc, msg );
  tv.clear();
  for ( int i = 0; i < 256; i++ )
    tv.append( (quint8)i );
                                                   pass &= testByteArray( v, tv, tc, msg );

  if ( pass )
    msg.append( QString("Pass %1 tests.").arg(tc) );

  return pass;
}

bool Tests::testByteArray( BlockValueByteArray &v, const QByteArray &tv, qint32 &tc, QString &msg )
{ bool pass = true;
  v.set( tv );
  if ( v.value() == tv ) tc++; else
    { msg.append( QString( "FAIL value set/get test %1\n" ).arg(QString::fromUtf8(tv)) ); pass = false; }

   BaoSerial b = v.bao();
  v.set( "?" );
  if  ( v.value() != tv ) tc++; else
    { msg.append( QString( "FAIL inequality test %1 %2\n" ).arg( QString::fromUtf8(tv.toHex()) ).arg( QString::fromUtf8( v.value().toHex() ) ) ); pass = false; }
  v.setBao( b );
  if ( v.value() == tv ) tc++; else
    { msg.append( QString( "FAIL bao encode/decode test %1 %2\n" ).arg(QString::fromUtf8(tv)).arg( QString::fromUtf8(b.toHex()) ) ); pass = false; }
  if ( b == v.bao() ) tc++; else
    { msg.append( QString( "FAIL bao repeat test %1 %2\n" )
            .arg( QString::fromUtf8(b.toHex()) )
            .arg( QString::fromUtf8(v.bao().toHex()) ) ); pass = false; }

  JsonSerial j = v.json();
  v.set( "x" );
  if  ( v.value() != tv ) tc++; else
    { msg.append( QString( "FAIL inequality test %1 %2\n" ).arg( QString::fromUtf8(tv.toHex()) ).arg( QString::fromUtf8( v.value().toHex() ) ) ); pass = false; }
  bool ok = v.setJson( j );
  if ( ok && ( v.value() == tv )) tc++; else
    { msg.append( QString( "FAIL json encode/decode test %1 %2 %3\n" ).arg(QString::fromUtf8(tv)).arg(ok).arg( QString::fromUtf8(j) ) ); pass = false; }

  return pass;
}

bool Tests::testObjectA( QString &msg, qint32 &tc )
{ bool pass = true;
  msg = "object array Test: ";
  BlockValueObjectArray v(this);
  if ( v.type() == RDT_OBJECT_ARRAY ) tc++; else
    { msg.append( "FAIL type() test.\n" ); pass = false; }

        BlockObjectMap  to;
  QList<BlockObjectMap> tv;
  pass &= testObjectA( v, tv, tc, msg ); // Empty Array test
  to.insert( RCD_int64_i, new BlockValueInt64    (        -456789, this) );
  to.insert( RCD_type_c , new BlockValueRiceyCode( RCD_MaxiCode_z, this) );
  to.insert( RCD_text_s , new BlockValueString   (     "Stringy!", this) );
  to.insert( RCD_data_b , new BlockValueByteArray(    "123456789", this) );
  tv.append( to ); pass &= testObjectA( v, tv, tc, msg ); // Array with one varied object
  tv.append( to ); pass &= testObjectA( v, tv, tc, msg ); // Array with two varied but identical objects
  tv.append( to ); pass &= testObjectA( v, tv, tc, msg ); // Array with thee varied but identical objects
  tv.clear();

  if ( pass )
    msg.append( QString("Pass %1 tests.").arg(tc) );

  return pass;
}

bool Tests::testObjectA( BlockValueObjectArray &v, const QList<BlockObjectMap> &tv, qint32 &tc, QString &msg )
{ bool pass = true;
  v.set( tv );
  if ( v == tv ) tc++; else
    { msg.append( QString( "FAIL value set/get test %1 %2\n" ).arg(tv.size()).arg(v.size()) ); pass = false; }

  BlockObjectMap  dob;
  dob.insert( RCD_text_s, new BlockValueString( "Dummy Object", this) );

   BaoSerial b = v.bao();
  v.clear();
  if ( v.append( dob ) ) tc++; else
    { pass = false; msg.append( "FAIL during append()\n" ); }
  if  ( !( v.value() == tv ) ) tc++; else
    { msg.append( QString( "FAIL inequality test\n" ) ); pass = false; }
  v.setBao( b );
  if ( v == tv ) tc++; else
    { msg.append( QString( "FAIL bao encode/decode test %1 %2\n" ).arg(tv.size()).arg( QString::fromUtf8(b.toHex()) ) ); pass = false; }
  if ( b == v.bao() ) tc++; else
    { msg.append( QString( "FAIL bao repeat test %1 %2\n" )
            .arg( QString::fromUtf8(b.toHex()) )
            .arg( QString::fromUtf8(v.bao().toHex()) ) ); pass = false; }
  JsonSerial j = v.json();
  if ( v.append( dob ) ) tc++; else
    { pass = false; msg.append( "FAIL during append()\n" ); }
  if  ( !( v.value() == tv ) ) tc++; else
    { msg.append( QString( "FAIL inequality test\n" ) ); pass = false; }
  bool ok = v.setJson( j );
  if ( ok && ( v == tv )) tc++; else
    { msg.append( QString( "FAIL json encode/decode test %1 %2 %3\n" ).arg(tv.size()).arg(ok).arg( QString::fromUtf8(j) ) ); pass = false; }
  return pass;
}


bool Tests::testInt64A( QString &msg, qint32 &tc )
{ bool pass = true;
  msg = "int64 array Test: ";
  BlockValueInt64Array v(this);
  if ( v.type() == RDT_INT64_ARRAY ) tc++; else
    { msg.append( "FAIL type() test.\n" ); pass = false; }

  QList<qint64> tv;
                                   pass &= testInt64A( v, tv, tc, msg ); // Empty Array test
  tv.append(                    0LL ); pass &= testInt64A( v, tv, tc, msg );
  tv.append(                    1LL ); pass &= testInt64A( v, tv, tc, msg );
  tv.append(                   -1LL ); pass &= testInt64A( v, tv, tc, msg );
  tv.append(                70000LL ); pass &= testInt64A( v, tv, tc, msg );
  tv.append(               -70000LL ); pass &= testInt64A( v, tv, tc, msg );
  tv.append(           5123456789LL ); pass &= testInt64A( v, tv, tc, msg );
  tv.append(          -5123456789LL ); pass &= testInt64A( v, tv, tc, msg );
  tv.append(  9223372036854775807LL ); pass &= testInt64A( v, tv, tc, msg );
  tv.append( -9223372036854775807LL ); pass &= testInt64A( v, tv, tc, msg );

  for ( qint64 i = 9223372036854775000LL; i <= 9223372036854775806LL; i++ )
    { tv.append( i ); tv.append( -i ); }
                                   pass &= testInt64A( v, tv, tc, msg );

  tv.clear();
  for ( qint64 i = 1; i < 1100; i++ )
    tv.append(0);
                            pass &= testInt64A( v, tv, tc, msg );
  tv.clear();
  for ( qint64 i = 1; i < 1101; i++ )
    tv.append(i*438957);
                            pass &= testInt64A( v, tv, tc, msg );
  tv.clear();
  for ( qint64 i = 1; i < 1102; i++ )
    tv.append(-i*234683);
                            pass &= testInt64A( v, tv, tc, msg );

  if ( pass )
    msg.append( QString("Pass %1 tests.").arg(tc) );

  return pass;
}

bool Tests::testInt64A( BlockValueInt64Array &v, const QList<qint64> &tv, qint32 &tc, QString &msg )
{ bool pass = true;
  v.set( tv );
  if ( v == tv ) tc++; else
    { msg.append( QString( "FAIL value set/get test %1\n" ).arg(tv.size()) ); pass = false; }

   BaoSerial b = v.bao();
  v.clear();
  if ( v.append( (qint64)123 ) ) tc++; else
    { pass = false; msg.append( "FAIL during append()\n" ); }
  if  ( !( v.value() == tv ) ) tc++; else
    { msg.append( QString( "FAIL inequality test\n" ) ); pass = false; }
  v.setBao( b );
  if ( v == tv ) tc++; else
    { msg.append( QString( "FAIL bao encode/decode test %1 %2\n" ).arg(tv.size()).arg( QString::fromUtf8(b.toHex()) ) ); pass = false; }
  if ( b == v.bao() ) tc++; else
    { msg.append( QString( "FAIL bao repeat test %1 %2\n" )
            .arg( QString::fromUtf8(b.toHex()) )
            .arg( QString::fromUtf8(v.bao().toHex()) ) ); pass = false; }

  JsonSerial j = v.json();
  v.clear();
  if ( v.append( (qint64)321 ) ) tc++; else
    { pass = false; msg.append( "FAIL during append()\n" ); }
  if  ( !( v.value() == tv ) ) tc++; else
    { msg.append( QString( "FAIL inequality test\n" ) ); pass = false; }
  bool ok = v.setJson( j );
  if ( ok && ( v == tv )) tc++; else
    { msg.append( QString( "FAIL json encode/decode test %1 %2 %3\n" ).arg(tv.size()).arg(ok).arg( QString::fromUtf8(j) ) ); pass = false;
      if ( v.size() != tv.size() )
        msg.append( QString( "    size mismatch original %1 read from json %2\n" ).arg( tv.size() ).arg( v.size() ) );
       else
        { for ( qint32 i = 0; i < v.size() ; i++ )
            if ( v.at(i) != tv.at(i) )
              msg.append( QString( "     value mismatch at %1, original %2 read from json %3\n" ).arg(i).arg(tv.at(i)).arg(v.at(i)) );
        }
    }
  return pass;
}

bool Tests::testMPZA( QString &msg, qint32 &tc )
{ bool pass = true;
  msg = "MPZ array Test: ";
  BlockValueMPZArray v(this);
  if ( v.type() == RDT_MPZ_ARRAY ) tc++; else
    { msg.append( "FAIL type() test.\n" ); pass = false; }

  QList<MP_INT> tv;
                                                                    pass &= testMPZA( v, tv, tc, msg ); // Empty Array test
  MP_INT v1; mpz_init_set_str( &v1, "12345", 10 ); tv.append( v1 ); pass &= testMPZA( v, tv, tc, msg );
  MP_INT v2; mpz_init_set_str( &v2, "67890", 10 ); tv.append( v2 ); pass &= testMPZA( v, tv, tc, msg );
  MP_INT v3; mpz_init_set_str( &v3, "-12345", 10 ); tv.append( v3 ); pass &= testMPZA( v, tv, tc, msg );
  MP_INT v4; mpz_init_set_str( &v4, "-67890", 10 ); tv.append( v4 ); pass &= testMPZA( v, tv, tc, msg );
  MP_INT v5; mpz_init_set_str( &v5, "123456", 10 ); tv.append( v5 ); pass &= testMPZA( v, tv, tc, msg );
  MP_INT v6; mpz_init_set_str( &v6, "-123456", 10 ); tv.append( v6 ); pass &= testMPZA( v, tv, tc, msg );
  MP_INT v7; mpz_init_set_str( &v7, "123456123456123456123456123456123456123456", 10 ); tv.append( v7 ); pass &= testMPZA( v, tv, tc, msg );
  MP_INT v8; mpz_init_set_str( &v8, "-123456123456123456123456123456123456123456", 10 ); tv.append( v8 ); pass &= testMPZA( v, tv, tc, msg );
  MP_INT v9; mpz_init_set_str( &v9, "1234561234561234561234561234561234561234567", 10 ); tv.append( v9 ); pass &= testMPZA( v, tv, tc, msg );
  MP_INT va; mpz_init_set_str( &va, "-1234561234561234561234561234561234561234567", 10 ); tv.append( va ); pass &= testMPZA( v, tv, tc, msg );

  foreach ( MP_INT v, tv )
    mpz_clear( &v );
  if ( pass )
    msg.append( QString("Pass %1 tests.").arg(tc) );

  return pass;
}

bool Tests::testMPZA( BlockValueMPZArray &v, const QList<MP_INT> &tv, qint32 &tc, QString &msg )
{ bool pass = true;
  v.set( tv );
  if ( v == tv ) tc++; else
    { msg.append( QString( "FAIL value set/get test %1\n" ).arg(tv.size()) ); pass = false; }

  MP_INT other;
  mpz_init_set_str( &other, "5551212", 10 );
   BaoSerial b = v.bao();
  v.clear();
  if ( v.append( other ) ) tc++; else
    { pass = false; msg.append( "FAIL during append()\n" ); }
  if  ( !( v == tv ) ) tc++; else
    { msg.append( QString( "FAIL inequality test\n" ) ); pass = false; }
  v.setBao( b );
  if ( v == tv ) tc++; else
    { msg.append( QString( "FAIL bao encode/decode test %1 %2\n" ).arg(tv.size()).arg( QString::fromUtf8(b.toHex()) ) ); pass = false; }
  if ( b == v.bao() ) tc++; else
    { msg.append( QString( "FAIL bao repeat test %1 %2\n" )
            .arg( QString::fromUtf8(b.toHex()) )
            .arg( QString::fromUtf8(v.bao().toHex()) ) ); pass = false; }

  JsonSerial j = v.json();
  v.clear();
  if ( v.append( other ) ) tc++; else
    { pass = false; msg.append( "FAIL during append()\n" ); }
  if  ( !( v == tv ) ) tc++; else
    { msg.append( QString( "FAIL inequality test\n" ) ); pass = false; }
  bool ok = v.setJson( j );
  if ( ok && ( v == tv )) tc++; else
    { msg.append( QString( "FAIL json encode/decode test %1 %2 %3\n" ).arg(tv.size()).arg(ok).arg( QString::fromUtf8(j) ) ); pass = false;
      if ( v.size() != tv.size() )
        msg.append( QString( "    size mismatch original %1 read from json %2\n" ).arg( tv.size() ).arg( v.size() ) );
       else
        { MP_INT v1;
          MP_INT v2;
          for ( qint32 i = 0; i < v.size() ; i++ )
            { v1 = v.at(i);
              v2 = tv.at(i);
              if ( mpz_cmp( &v1, &v2 ) != 0 )
                msg.append( QString( "     value mismatch at %1\n" ).arg(i) );
            }
        }
    }
  mpz_clear( &other );
  return pass;
}

bool Tests::testMPQA( QString &msg, qint32 &tc )
{ bool pass = true;
  msg = "MPQ array Test: ";
  BlockValueMPQArray v(this);
  if ( v.type() == RDT_MPQ_ARRAY ) tc++; else
    { msg.append( "FAIL type() test.\n" ); pass = false; }

  QList<MP_RAT> tv;
  pass &= testMPQA( v, tv, tc, msg ); // Empty Array test
  MP_RAT v1; mpq_init( &v1 ); mpq_set_str( &v1,                                          "12345", 10 ); tv.append( v1 ); pass &= testMPQA( v, tv, tc, msg );
  MP_RAT v2; mpq_init( &v2 ); mpq_set_str( &v2,                                    "77777/67890", 10 ); tv.append( v2 ); pass &= testMPQA( v, tv, tc, msg );
  MP_RAT v3; mpq_init( &v3 ); mpq_set_str( &v3,                                   "-12345/77777", 10 ); tv.append( v3 ); pass &= testMPQA( v, tv, tc, msg );
  MP_RAT v4; mpq_init( &v4 ); mpq_set_str( &v4,                                       "-67890/1", 10 ); tv.append( v4 ); pass &= testMPQA( v, tv, tc, msg );
  MP_RAT v5; mpq_init( &v5 ); mpq_set_str( &v5,                                       "1/123456", 10 ); tv.append( v5 ); pass &= testMPQA( v, tv, tc, msg );
  MP_RAT v6; mpq_init( &v6 ); mpq_set_str( &v6,                              "-123456/123456789", 10 ); tv.append( v6 ); pass &= testMPQA( v, tv, tc, msg );
  MP_RAT v7; mpq_init( &v7 ); mpq_set_str( &v7,   "123456123456123456123456123456123456123456/1", 10 ); tv.append( v7 ); pass &= testMPQA( v, tv, tc, msg );
  MP_RAT v8; mpq_init( &v8 ); mpq_set_str( &v8,  "-123456123456123456123456123456123456123456/1", 10 ); tv.append( v8 ); pass &= testMPQA( v, tv, tc, msg );
  MP_RAT v9; mpq_init( &v9 ); mpq_set_str( &v9,  "1/1234561234561234561234561234561234561234567", 10 ); tv.append( v9 ); pass &= testMPQA( v, tv, tc, msg );
  MP_RAT va; mpq_init( &va ); mpq_set_str( &va, "-1/1234561234561234561234561234561234561234567", 10 ); tv.append( va ); pass &= testMPQA( v, tv, tc, msg );

  foreach ( MP_RAT v, tv )
    mpq_clear( &v );

  if ( pass )
    msg.append( QString("Pass %1 tests.").arg(tc) );

  return pass;
}

bool Tests::testMPQA( BlockValueMPQArray &v, const QList<MP_RAT> &tv, qint32 &tc, QString &msg )
{ bool pass = true;
  v.set( tv );
  if ( v == tv ) tc++; else
    { msg.append( QString( "FAIL value set/get test %1\n" ).arg(tv.size()) ); pass = false; }

  MP_INT oNum,oDen;
  MP_RAT other;
  mpz_init_set_str( &oNum, "5551212", 10 );
  mpz_init_set_str( &oDen, "2125551212", 10 );
  mpq_init( &other );
  mpq_set_num( &other, &oNum );
  mpq_set_den( &other, &oDen );
   BaoSerial b = v.bao();
  v.clear();
  if ( v.append( other ) ) tc++; else
    { pass = false; msg.append( "FAIL during append()\n" ); }
  if  ( !( v == tv ) ) tc++; else
    { msg.append( QString( "FAIL inequality test\n" ) ); pass = false; }
  v.setBao( b );
  if ( v == tv ) tc++; else
    { msg.append( QString( "FAIL bao encode/decode test %1 %2\n" ).arg(tv.size()).arg( QString::fromUtf8(b.toHex()) ) ); pass = false; }
  if ( b == v.bao() ) tc++; else
    { msg.append( QString( "FAIL bao repeat test %1 %2\n" )
            .arg( QString::fromUtf8(b.toHex()) )
            .arg( QString::fromUtf8(v.bao().toHex()) ) ); pass = false; }

  JsonSerial j = v.json();
  v.clear();
  if ( v.append( other ) ) tc++; else
    { pass = false; msg.append( "FAIL during append()\n" ); }
  if  ( !( v == tv ) ) tc++; else
    { msg.append( QString( "FAIL inequality test\n" ) ); pass = false; }
  bool ok = v.setJson( j ); // qWarning( "json %s", j.data() );
  if ( ok && ( v == tv )) tc++; else
    { msg.append( QString( "FAIL json encode/decode test %1 %2 %3\n" ).arg(tv.size()).arg(ok).arg( QString::fromUtf8(j) ) ); pass = false;
      if ( v.size() != tv.size() )
        msg.append( QString( "    size mismatch original %1 read from json %2\n" ).arg( tv.size() ).arg( v.size() ) );
       else
        { MP_RAT v1;
          MP_RAT v2;
          for ( qint32 i = 0; i < v.size() ; i++ )
            { v1 = v.at(i);
              v2 = tv.at(i);
              if ( mpq_cmp( &v1, &v2 ) != 0 )
                msg.append( QString( "     value mismatch at %1\n" ).arg(i) );
            }
        }
    }
  mpq_clear( &other );
  mpz_clear( &oNum );
  mpz_clear( &oDen );
  return pass;
}

bool Tests::testRiceyA( QString &msg, qint32 &tc )
{ bool pass = true;
  msg = "ricey array Test: ";
  BlockValueRiceyCodeArray v(this);
  if ( v.type() == RDT_RCODE_ARRAY ) tc++; else
    { msg.append( "FAIL type() test.\n" ); pass = false; }

  QList<RiceyInt> tv;
                                    pass &= testRiceyA( v, tv, tc, msg ); // Empty Array test
  tv.append( RCD_ob_o          ); pass &= testRiceyA( v, tv, tc, msg );
  tv.append( RCD_int64_i       ); pass &= testRiceyA( v, tv, tc, msg );
  tv.append( RCD_code_s        ); pass &= testRiceyA( v, tv, tc, msg );
  tv.append( RCD_chainBlock_o  ); pass &= testRiceyA( v, tv, tc, msg );
  tv.append( RCD_SHA256_c      ); pass &= testRiceyA( v, tv, tc, msg );
  tv.append( RCD_itemRelAddr_C ); pass &= testRiceyA( v, tv, tc, msg );
  tv.append( RCD_RangeBounds_O ); pass &= testRiceyA( v, tv, tc, msg );
  tv.append( RCD_Identity_o    ); pass &= testRiceyA( v, tv, tc, msg );
  tv.append( RCD_NineCode_z    ); pass &= testRiceyA( v, tv, tc, msg );
  tv.append( RCD_MaxiCode_z    ); pass &= testRiceyA( v, tv, tc, msg );
  tv.clear();
  for ( qint32 i = 1; i < 1100; i++ )
    tv.append(RCD_ob_o);
                                   pass &= testRiceyA( v, tv, tc, msg );
  tv.clear();
  for ( qint32 i = 1; i < 1101; i++ )
    tv.append(RCD_MaxiCode_z);
                                   pass &= testRiceyA( v, tv, tc, msg );

  if ( pass )
    msg.append( QString("Pass %1 tests.").arg(tc) );

  return pass;
}

bool Tests::testRiceyA( BlockValueRiceyCodeArray &v, const QList<RiceyInt> &tv, qint32 &tc, QString &msg )
{ bool pass = true;
  v.set( tv );
  if ( v == tv ) tc++; else
    { msg.append( QString( "FAIL value set/get test %1\n" ).arg(tv.size()) ); pass = false; }

   BaoSerial b = v.bao();
  if ( v.append( (RiceyInt)123 ) ) tc++; else
    { pass = false; msg.append( "FAIL during append()\n" ); }
  if  ( !( v.valueInt() == tv ) ) tc++; else
    { msg.append( QString( "FAIL inequality test\n" ) ); pass = false; }
  v.setBao( b );
  if ( v == tv ) tc++; else
    { msg.append( QString( "FAIL bao encode/decode test %1 %2\n" ).arg(tv.size()).arg( QString::fromUtf8(b.toHex()) ) ); pass = false; }
  if ( b == v.bao() ) tc++; else
    { msg.append( QString( "FAIL bao repeat test %1 %2\n" )
            .arg( QString::fromUtf8(b.toHex()) )
            .arg( QString::fromUtf8(v.bao().toHex()) ) ); pass = false; }

  JsonSerial j = v.json();
  if ( v.append( (RiceyInt)321 ) ) tc++; else
    { pass = false; msg.append( "FAIL during append()\n" ); }
  if  ( !( v.valueInt() == tv ) ) tc++; else
    { msg.append( QString( "FAIL inequality test\n" ) ); pass = false; }
  bool ok = v.setJson( j );
  if ( ok && ( v == tv )) tc++; else
    { msg.append( QString( "FAIL json encode/decode test %1 %2 %3\n" ).arg(tv.size()).arg(ok).arg( QString::fromUtf8(j) ) ); pass = false; }
  return pass;
}

bool Tests::testRiceyA( BlockValueRiceyCodeArray &v, const QList<RiceyCode> &tv, qint32 &tc, QString &msg )
{ bool pass = true;
  v.set( tv );
  if ( v == tv ) tc++; else
    { msg.append( QString( "FAIL value set/get test %1\n" ).arg(tv.size()) ); pass = false; }

   BaoSerial b = v.bao();
  v.clear();
  if ( v.append( (RiceyInt)123 ) ) tc++; else
    { pass = false; msg.append( "FAIL during append()\n" ); }
  if  ( !( v.value() == tv ) ) tc++; else
    { msg.append( QString( "FAIL inequality test\n" ) ); pass = false; }
  v.setBao( b );
  if ( v == tv ) tc++; else
    { msg.append( QString( "FAIL bao encode/decode test %1 %2\n" ).arg(tv.size()).arg( QString::fromUtf8(b.toHex()) ) ); pass = false; }
  if ( b == v.bao() ) tc++; else
    { msg.append( QString( "FAIL bao repeat test %1 %2\n" )
            .arg( QString::fromUtf8(b.toHex()) )
            .arg( QString::fromUtf8(v.bao().toHex()) ) ); pass = false; }

  JsonSerial j = v.json();
  v.clear();
  if ( v.append( (RiceyInt)321 ) ) tc++; else
    { pass = false; msg.append( "FAIL during append()\n" ); }
  if  ( !( v.value() == tv ) ) tc++; else
    { msg.append( QString( "FAIL inequality test\n" ) ); pass = false; }
  bool ok = v.setJson( j );
  if ( ok && ( v == tv )) tc++; else
    { msg.append( QString( "FAIL json encode/decode test %1 %2 %3\n" ).arg(tv.size()).arg(ok).arg( QString::fromUtf8(j) ) ); pass = false; }
  return pass;
}

bool Tests::testRiceyIntA( QString &msg, qint32 &tc )
{ bool pass = true;
  msg = "riceyInt array Test: ";
  BlockValueRiceyIntArray v(this);
  if ( v.type() == RDT_RICEYINT_ARRAY ) tc++; else
    { msg.append( "FAIL type() test.\n" ); pass = false; }

  QList<RiceyInt> tv;
                                  pass &= testRiceyIntA( v, tv, tc, msg ); // Empty Array test
  tv.append( 0                 ); pass &= testRiceyIntA( v, tv, tc, msg );
  tv.append( 1                 ); pass &= testRiceyIntA( v, tv, tc, msg );
  tv.append( 2                 ); pass &= testRiceyIntA( v, tv, tc, msg );
  tv.append( 126               ); pass &= testRiceyIntA( v, tv, tc, msg );
  tv.append( 127               ); pass &= testRiceyIntA( v, tv, tc, msg );
  tv.append( 128               ); pass &= testRiceyIntA( v, tv, tc, msg );
  tv.append( 129               ); pass &= testRiceyIntA( v, tv, tc, msg );
  tv.append( 1048575           ); pass &= testRiceyIntA( v, tv, tc, msg );
  tv.append( 1048576           ); pass &= testRiceyIntA( v, tv, tc, msg );
  tv.append( 34359738367LL     ); pass &= testRiceyIntA( v, tv, tc, msg );
  tv.append( 34359738368LL     ); pass &= testRiceyIntA( v, tv, tc, msg );
  tv.append( RCD_NineCode_z    ); pass &= testRiceyIntA( v, tv, tc, msg );
  tv.append( RCD_MaxiCode_z    ); pass &= testRiceyIntA( v, tv, tc, msg );
  tv.clear();
  for ( qint32 i = 1; i < 1100; i++ )
    tv.append(RCD_ob_o);
                                   pass &= testRiceyIntA( v, tv, tc, msg );
  tv.clear();
  for ( qint32 i = 1; i < 1101; i++ )
    tv.append(RCD_MaxiCode_z);
                                   pass &= testRiceyIntA( v, tv, tc, msg );

  if ( pass )
    msg.append( QString("Pass %1 tests.").arg(tc) );

  return pass;
}

bool Tests::testRiceyIntA( BlockValueRiceyIntArray &v, const QList<RiceyInt> &tv, qint32 &tc, QString &msg )
{ bool pass = true;
  v.set( tv );
  if ( v == tv ) tc++; else
    { msg.append( QString( "FAIL value set/get test %1\n" ).arg(tv.size()) ); pass = false; }

   BaoSerial b = v.bao();
  if ( v.append( (RiceyInt)123 ) ) tc++; else
    { pass = false; msg.append( "FAIL during append()\n" ); }
  if  ( !( v.value() == tv ) ) tc++; else
    { msg.append( QString( "FAIL inequality test\n" ) ); pass = false; }
  v.setBao( b );
  if ( v == tv ) tc++; else
    { msg.append( QString( "FAIL bao encode/decode test %1 %2\n" ).arg(tv.size()).arg( QString::fromUtf8(b.toHex()) ) ); pass = false; }
  if ( b == v.bao() ) tc++; else
    { msg.append( QString( "FAIL bao repeat test %1 %2\n" )
            .arg( QString::fromUtf8(b.toHex()) )
            .arg( QString::fromUtf8(v.bao().toHex()) ) ); pass = false; }

  JsonSerial j = v.json();
  if ( v.append( (RiceyInt)321 ) ) tc++; else
    { pass = false; msg.append( "FAIL during append()\n" ); }
  if  ( !( v.value() == tv ) ) tc++; else
    { msg.append( QString( "FAIL inequality test\n" ) ); pass = false; }
  bool ok = v.setJson( j );
  if ( ok && ( v == tv )) tc++; else
    { msg.append( QString( "FAIL json encode/decode test %1 %2 %3\n" ).arg(tv.size()).arg(ok).arg( QString::fromUtf8(j) ) ); pass = false; }
  return pass;
}

bool Tests::testStringA( QString &msg, qint32 &tc )
{ bool pass = true;
  msg = "string array Test: ";
  BlockValueStringArray v(this);
  if ( v.type() == RDT_STRING_ARRAY ) tc++; else
    { msg.append( "FAIL type() test.\n" ); pass = false; }

  QList<Utf8String> tv;
                                                           pass &= testStringA( v, tv, tc, msg ); // Empty Array test
  tv.append( ""                                         ); pass &= testStringA( v, tv, tc, msg );
  tv.append( "1"                                        ); pass &= testStringA( v, tv, tc, msg );
  tv.append( "A"                                        ); pass &= testStringA( v, tv, tc, msg );
  tv.append( "Stringy McStringface"                     ); pass &= testStringA( v, tv, tc, msg );
  tv.append( "Ⓐ~!@#$%^&*()`[]{}\\|;'\":<>?/.,_+-=\r\n" ); pass &= testStringA( v, tv, tc, msg );
  Utf8String lStr;
  for ( qint32 i = 0; i < 1000; i++ )
    lStr.append( "Ⓐ~!@#$%^&*()`[]{}\\|;'\":<>?/.,_+-=\r\n" );
  tv.append( lStr                                       ); pass &= testStringA( v, tv, tc, msg );

  if ( pass )
    msg.append( QString("Pass %1 tests.").arg(tc) );

  return pass;
}

bool Tests::testStringA( BlockValueStringArray &v, const QList<Utf8String> &tv, qint32 &tc, QString &msg )
{ bool pass = true;
  v.set( tv );
  if ( v == tv ) tc++; else
    { msg.append( QString( "FAIL value set/get test %1\n" ).arg(tv.size()) ); pass = false; }

   BaoSerial b = v.bao();
  v.clear();
  if ( v.append( "?" ) ) tc++; else
    { pass = false; msg.append( "FAIL during append()\n" ); }
  if  ( !( v.value() == tv ) ) tc++; else
    { msg.append( QString( "FAIL inequality test\n" ) ); pass = false; }
  v.setBao( b );
  if ( v == tv ) tc++; else
    { msg.append( QString( "FAIL bao encode/decode test %1 %2\n" ).arg(tv.size()).arg( QString::fromUtf8(b.toHex()) ) ); pass = false; }
  if ( b == v.bao() ) tc++; else
    { msg.append( QString( "FAIL bao repeat test %1 %2\n" )
            .arg( QString::fromUtf8(b.toHex()) )
            .arg( QString::fromUtf8(v.bao().toHex()) ) ); pass = false; }

  JsonSerial j = v.json();
  v.clear();
  if ( v.append( "??" ) ) tc++; else
    { pass = false; msg.append( "FAIL during append()\n" ); }
  if  ( !( v.value() == tv ) ) tc++; else
    { msg.append( QString( "FAIL inequality test\n" ) ); pass = false; }
  bool ok = v.setJson( j );
  if ( ok && ( v == tv )) tc++; else
    { msg.append( QString( "FAIL json encode/decode test %1 %2 %3\n" ).arg(tv.size()).arg(ok).arg( QString::fromUtf8(j) ) ); pass = false;
      if ( v.size() != tv.size() )
        msg.append( QString( "    size mismatch original %1 read from json %2\n" ).arg( tv.size() ).arg( v.size() ) );
       else
        { for ( qint32 i = 0; i < v.size() ; i++ )
            if ( v.at(i) != tv.at(i) )
              msg.append( QString( "     value mismatch at %1, original %2 read from json %3\n" ).arg(i).arg(QString::fromUtf8(tv.at(i))).arg(QString::fromUtf8(v.at(i))) );
        }
    }
  return pass;
}

bool Tests::testByteArrayA( QString &msg, qint32 &tc )
{ bool pass = true;
  msg = "byte array array Test: ";
  BlockValueByteArrayArray v(this);
  if ( v.type() == RDT_BYTEARRAY_ARRAY ) tc++; else
    { msg.append( "FAIL type() test.\n" ); pass = false; }

  QList<QByteArray> tv;
                                                           pass &= testByteArrayA( v, tv, tc, msg ); // Empty Array test
  tv.append( ""                                         ); pass &= testByteArrayA( v, tv, tc, msg );
  tv.append( "1"                                        ); pass &= testByteArrayA( v, tv, tc, msg );
  tv.append( "A"                                        ); pass &= testByteArrayA( v, tv, tc, msg );
  tv.append( "Stringy McStringface"                     ); pass &= testByteArrayA( v, tv, tc, msg );
  tv.append( "Ⓐ~!@#$%^&*()`[]{}\\|;'\":<>?/.,_+-=\r\n" ); pass &= testByteArrayA( v, tv, tc, msg );
  QByteArray lStr;
  for ( qint32 i = 0; i < 1000; i++ )
    lStr.append( "Ⓐ~!@#$%^&*()`[]{}\\|;'\":<>?/.,_+-=\r\n" );
  tv.append( lStr                                       ); pass &= testByteArrayA( v, tv, tc, msg );
  lStr.clear();
  for ( qint32 i = 0; i < 256; i++ )
    lStr.append( (quint8)i );
  tv.append( lStr                                       ); pass &= testByteArrayA( v, tv, tc, msg );

  if ( pass )
    msg.append( QString("Pass %1 tests.").arg(tc) );

  return pass;
}

bool Tests::testByteArrayA( BlockValueByteArrayArray &v, const QList<QByteArray> &tv, qint32 &tc, QString &msg )
{ bool pass = true;
  v.set( tv );
  if ( v == tv ) tc++; else
    { msg.append( QString( "FAIL value set/get test %1\n" ).arg(tv.size()) ); pass = false; }

   BaoSerial b = v.bao();
  v.clear();
  if ( v.append( "?" ) ) tc++; else
    { pass = false; msg.append( "FAIL during append()\n" ); }
  if  ( !( v.value() == tv ) ) tc++; else
    { msg.append( QString( "FAIL inequality test\n" ) ); pass = false; }
  v.setBao( b );
  if ( v == tv ) tc++; else
    { msg.append( QString( "FAIL bao encode/decode test %1 %2\n" ).arg(tv.size()).arg( QString::fromUtf8(b.toHex()) ) ); pass = false; }
  if ( b == v.bao() ) tc++; else
    { msg.append( QString( "FAIL bao repeat test %1 %2\n" )
            .arg( QString::fromUtf8(b.toHex()) )
            .arg( QString::fromUtf8(v.bao().toHex()) ) ); pass = false; }

  JsonSerial j = v.json();
  v.clear();
  if ( v.append( "??" ) ) tc++; else
    { pass = false; msg.append( "FAIL during append()\n" ); }
  if  ( !( v.value() == tv ) ) tc++; else
    { msg.append( QString( "FAIL inequality test\n" ) ); pass = false; }
  bool ok = v.setJson( j );
  if ( ok && ( v == tv )) tc++; else
    { msg.append( QString( "FAIL json encode/decode test %1 %2 %3\n" ).arg(tv.size()).arg(ok).arg( QString::fromUtf8(j) ) ); pass = false;
      if ( v.size() != tv.size() )
        msg.append( QString( "    size mismatch original %1 read from json %2\n" ).arg( tv.size() ).arg( v.size() ) );
       else
        { for ( qint32 i = 0; i < v.size() ; i++ )
            if ( v.at(i) != tv.at(i) )
              msg.append( QString( "     value mismatch at %1, original %2 read from json %3\n" ).arg(i).arg(QString::fromUtf8(tv.at(i))).arg(QString::fromUtf8(v.at(i))) );
        }
    }
  return pass;
}

bool  Tests::testObject( QString &msg, qint32 &tc )
{ bool pass = true;
  msg = "object Test: ";
  BlockValueObject v(this);
  if ( v.type() == RDT_OBJECT ) tc++; else
    { msg.append( "FAIL type() test.\n" ); pass = false; }

  BlockObjectMap tv;
  pass &= testObject( v, tv, tc, msg ); // Empty Object test
  MP_INT v1,v2;
  mpz_init_set_str( &v1, "12233344445555556666667777777888888888999999999", 10 );
  mpz_init_set_str( &v2, "98877766665555544444433333333222222221111111110", 10 );
  MP_RAT vr;
  mpq_init( &vr );
  mpq_set_num( &vr, &v1 );
  mpq_set_den( &vr, &v2 );

  tv.insert( RCD_int64_i, new BlockValueInt64    (        -456789, this) );  pass &= testObject( v, tv, tc, msg );
  tv.insert( RCD_mpz_n  , new BlockValueMPZ      (             v1, this) );  pass &= testObject( v, tv, tc, msg );
  tv.insert( RCD_mpq_r  , new BlockValueMPQ      (             vr, this) );  pass &= testObject( v, tv, tc, msg );
  tv.insert( RCD_type_c , new BlockValueRiceyCode( RCD_MaxiCode_z, this) );  pass &= testObject( v, tv, tc, msg );
  tv.insert( RCD_text_s , new BlockValueString   (     "Stringy!", this) );  pass &= testObject( v, tv, tc, msg );
  tv.insert( RCD_data_b , new BlockValueByteArray(    "123456789", this) );  pass &= testObject( v, tv, tc, msg );

  BlockObjectMap tvo = tv; // Nesting test
  tv.insert( RCD_hashedOb_o  , new BlockValueObject( tvo, this ) );      pass &= testObject( v, tv, tc, msg );
  tvo = tv;
  tv.insert( RCD_hash_o      , new BlockValueObject( tvo, this ) );      pass &= testObject( v, tv, tc, msg );
  tvo = tv;
  tv.insert( RCD_chainBlock_o, new BlockValueObject( tvo, this ) );      pass &= testObject( v, tv, tc, msg );
  tvo = tv;
  tv.insert( RCD_hashedOb_o  , new BlockValueObject( tvo, this ) );      pass &= testObject( v, tv, tc, msg );
  QList<qint64> ta64 = { 0,1,-1,70000,-70000,-5123456789, 5123456789 };
  tv.insert( RCD_int64_I     , new BlockValueInt64Array( ta64, this ) ); pass &= testObject( v, tv, tc, msg );
  // emit newDot( v.dot() );
  QList<RiceyInt> tari = { RCD_mpz_n, RCD_type_C, RCD_data_b, RCD_MaxiCode_z, RCD_RangeBounds_O };
  tv.insert( RCD_type_C      , new BlockValueRiceyCodeArray( tari, this ) );     pass &= testObject( v, tv, tc, msg );
  QList<Utf8String> tau8 = { "A", "1", "Stringy" };
  tv.insert( RCD_text_S      , new BlockValueStringArray( tau8, this ) );       pass &= testObject( v, tv, tc, msg );
  tv.insert( RCD_data_B      , new BlockValueByteArrayArray( tau8, this ) ); pass &= testObject( v, tv, tc, msg );

  mpz_clear( &v1 );
  mpz_clear( &v2 );
  mpq_clear( &vr );
  if ( pass )
    msg.append( QString("Pass %1 tests.").arg(tc) );

  return pass;
}

bool  Tests::testObject( BlockValueObject &v, const BlockObjectMap &tv, qint32 &tc, QString &msg )
{ bool pass = true;
  qint32 sz = v.set(tv);
  // msg.append( QString( "object json %1" ).arg( QString::fromUtf8( v.json() ) ) );

  if ( sz == tv.size() ) tc++; else
    { msg.append( QString( "FAIL value set test %1 vs %2\n" ).arg(tv.size()).arg(sz) ); pass = false; }
  if ( v == tv ) tc++; else
    { msg.append( QString( "FAIL value set/get test %1\n" ).arg(tv.size()) ); pass = false; }

   BaoSerial b = v.bao();
  v.clear();
  if ( v.insert( RCD_text_s, "?" ) ) tc++; else
    { pass = false; msg.append( "FAIL during insert()\n" ); }
  if  ( !( v.value() == tv ) ) tc++; else
    { msg.append( QString( "FAIL inequality test\n" ) ); pass = false; }
  if ( v.setBao( b ) > 0 ) tc++; else
    { msg.append( QString( "FAIL during setBao\n" ) ); pass = false; }
  if ( v == tv ) tc++; else
    { msg.append( QString( "FAIL bao encode/decode test %1 %2\n" ).arg(tv.size()).arg( QString::fromUtf8(b.toHex()) ) ); pass = false; }
  if ( b == v.bao() ) tc++; else
    { msg.append( QString( "FAIL bao repeat test %1 %2\n" )
            .arg( QString::fromUtf8(b.toHex()) )
            .arg( QString::fromUtf8(v.bao().toHex()) ) ); pass = false; }

  JsonSerial j = v.json();
  v.clear();
  if ( v.insert( RCD_text_s, "??" ) ) tc++; else
    { pass = false; msg.append( "FAIL during append()\n" ); }
  if  ( !( v.value() == tv ) ) tc++; else
    { msg.append( QString( "FAIL inequality test\n" ) ); pass = false; }
  bool ok = v.setJson( j );
  if ( ok && ( v == tv )) tc++; else
    { msg.append( QString( "FAIL json encode/decode test tv sz:%1 v sz:%2 ok:%3 %4\n" )
                     .arg(tv.size()).arg(v.size()).arg(ok).arg( QString::fromUtf8(j) ) ); pass = false;
      if ( v.size() != tv.size() )
        msg.append( QString( "    size mismatch %1 %2\n" ).arg( tv.size() ).arg( v.size() ) );
       else
        { QList<RiceyInt> keys = tv.keys();
          foreach ( RiceyInt k, keys )
            { if ( !( v.value(k) != tv.value(k) ) )
                msg.append( QString( "     value mismatch at %1, original %2 read from json %3\n" )
                               .arg(QString::fromUtf8(dict.nameFromCode(k)))
                               .arg(QString::fromUtf8(tv.value(k)->json()))
                               .arg(QString::fromUtf8( v.value(k)->json())) );
               else
                msg.append( QString( "     value match at %1, original %2 read from json %3\n" )
                               .arg(QString::fromUtf8(dict.nameFromCode(k)))
                               .arg(tv.value(k)->type())
                               .arg( v.value(k)->type()) );
            }
        }
    }

  return pass;
}

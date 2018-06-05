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
#include "testform.h"

TestForm::TestForm( QWidget *cw, MainWinCommon *mw ) :
    QScrollArea(cw),
    ui(new Ui::TestForm)
{ (void)mw;
  ui->setupUi(this);
  new QVBoxLayout( cw );
  cw->layout()->addWidget( this );

  /*
    if ( mw )
      { connect( mw, SIGNAL(restoringConfig()), SLOT(restoreConfig()));
        connect( mw, SIGNAL(   savingConfig()), SLOT(   saveConfig()));
      }
  */
}

TestForm::~TestForm()
{ delete ui;
}

void TestForm::on_test_clicked()
{ testGenesisBlock();
}

#include "keypair.h"
void TestForm::on_generateKey_clicked()
{ KeyPair kp;
  kp.makeNewPair( AO_ECDSA_PRI_KEY );
}

#include "aotime.h"
#include "aocoins.h"
#include "data16.h"
#include "genericcollection.h"
#include "shares.h"
void  TestForm::testGenesisBlock()
{ GenericCollection gb( GB_GENESIS_BLOCK );
  __int128_t tv;
  gb.add( GB_PROTOCOL    , new Data16       (      1, GB_PROTOCOL    , &gb ) );
  gb.add( GB_PROTOCOL_REV, new Data16       (      2, GB_PROTOCOL_REV, &gb ) );
  gb.add( GB_TEXT_SYMBOL , new DataVarLength( "tSmb", GB_TEXT_SYMBOL , &gb ) );
  gb.add( GB_DESCRIPTION , new DataVarLength( "Test description string of reasonably long length, exceeding 128 bytes so as to trigger some multi-byte length code action.", GB_DESCRIPTION, &gb ) );
//  gb.add( GB_ICON           , DataByteArray( ) ) // TODO: file reader
//  gb.add( GB_IMAGE          , DataByteArray( ) ) // TODO: file reader
  tv = 1; tv = tv << 86;
  gb.add( GB_STARTING_SHARES, new Shares( tv, GB_STARTING_SHARES, &gb ) );
  tv = 1; tv = tv << 64; tv = tv * 600.1;
  gb.add( GB_MIN_BLOCK_INT  , new AOTime( tv, GB_MIN_BLOCK_INT  , &gb ) );
  tv = 1; tv = tv << (33 + 64);
  gb.add( GB_N_COINS_TOTAL  , new AOCoins( tv, GB_N_COINS_TOTAL, &gb ) );
  tv = 1; tv = tv << (-30 + 64);
  gb.add( GB_RECORDING_TAX  , new AOCoins( tv, GB_RECORDING_TAX, &gb ) );
  gb.testHashVerify();
}

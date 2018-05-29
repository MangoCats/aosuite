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
#include "genesisblock.h"
#include "genesisForm.h"
#include "keyvaluepair.h"
#include <QFileDialog>

GenesisForm::GenesisForm( QWidget *cw, MainWinCommon *mw ) :
    QScrollArea(cw),
    ui(new Ui::GenesisForm)
{ ui->setupUi(this);
  new QVBoxLayout( cw );
  cw->layout()->addWidget( this );
  if ( mw )
    { connect( mw, SIGNAL(restoringConfig()), SLOT(restoreConfig()));
      connect( mw, SIGNAL(   savingConfig()), SLOT(   saveConfig()));
    }
}

GenesisForm::~GenesisForm()
{ delete ui; }

void  GenesisForm::restoreConfig()
{ QSettings s;
  if ( s.contains( "description"    ) ) ui->description   ->setText        ( s.value( "description"    ).toString() );
  if ( s.contains( "protocol"       ) ) ui->protocol      ->setCurrentIndex( s.value( "protocol"       ).toInt()    );
  if ( s.contains( "symbol"         ) ) ui->symbol        ->setText        ( s.value( "symbol"         ).toString() );
  if ( s.contains( "icon"           ) ) ui->icon          ->setText        ( s.value( "icon"           ).toString() );
  if ( s.contains( "image"          ) ) ui->image         ->setText        ( s.value( "image"          ).toString() );
  if ( s.contains( "startingShares" ) ) ui->startingShares->setValue       ( s.value( "startingShares" ).toInt()    );
  if ( s.contains( "totalCoins"     ) ) ui->totalCoins    ->setValue       ( s.value( "totalCoins"     ).toInt()    );
  if ( s.contains( "recordingTax"   ) ) ui->recordingTax  ->setValue       ( s.value( "recordingTax"   ).toInt()    );
  if ( s.contains( "minBlockTime"   ) ) ui->minBlockTime  ->setValue       ( s.value( "minBlockTime"   ).toDouble() );
}

void  GenesisForm::saveConfig()
{ QSettings s;
  s.setValue( "description"   , ui->description   ->toPlainText()  );
  s.setValue( "protocol"      , ui->protocol      ->currentIndex() );
  s.setValue( "symbol"        , ui->symbol        ->text()         );
  s.setValue( "icon"          , ui->icon          ->text()         );
  s.setValue( "image"         , ui->image         ->text()         );
  s.setValue( "startingShares", ui->startingShares->value()        );
  s.setValue( "totalCoins"    , ui->totalCoins    ->value()        );
  s.setValue( "recordingTax"  , ui->recordingTax  ->value()        );
  s.setValue( "minBlockTime"  , ui->minBlockTime  ->value()        );
}

void  GenesisForm::on_chooseIcon_clicked()
{ QString name = QFileDialog::getOpenFileName( this, "Icon file" );
  if ( name.size() > 0 )
    ui->icon->setText( name );
}

void  GenesisForm::on_chooseImage_clicked()
{ QString name = QFileDialog::getOpenFileName( this, "Image file" );
  if ( name.size() > 0 )
    ui->image->setText( name );
}

void  GenesisForm::on_importGenesisBlock_clicked()
{

}

#include "aocoins.h"
#include "aotime.h"
#include "bytearraylong.h"
#include "index.h"
#include "note.h"
#include "sharesout.h"
#include "shortnote.h"

/**
 * @brief GenesisForm::on_publishGenesisBlock_clicked - saving to file at the moment
 *   when we have a recorder to talk to, can communicate it directly to the recorder.
 */
void  GenesisForm::on_publishGenesisBlock_clicked()
{ QString name = QFileDialog::getSaveFileName( this, "save Genesis Block to file:" );
  if ( name.size() < 1 )
    return;
  GenesisBlock gb;
  __int128_t tv;
  gb.add( AOK_PROTOCOL       ,     Index( ui->protocol   ->currentIndex()         ) );
  gb.add( AOK_PROTOCOL_REV   ,     Index( ui->protocolRev->value()                ) );
  gb.add( AOK_TEXT_SYMBOL    , ShortNote( ui->symbol     ->text().toUtf8()        ) );
  gb.add( AOK_DESCRIPTION    ,      Note( ui->description->toPlainText().toUtf8() ) );
//  gb.add( AOK_ICON           , ByteArrayLong( ) ) // TODO: file reader
//  gb.add( AOK_IMAGE          , ByteArrayLong( ) ) // TODO: file reader
  tv = 1; tv = tv << ui->startingShares->value();
  gb.add( AOK_STARTING_SHARES, SharesOut( tv ) );
  tv = 1; tv = tv << 64; tv = tv * ui->minBlockTime->value();
  gb.add( AOK_MIN_BLOCK_INT  , AOTime( tv, AO_TIME_DIFF ) );
  tv = 1; tv = tv << (ui->totalCoins->value() + 64);
  gb.add( AOK_N_COINS_TOTAL  , AOCoins( tv ) );
  tv = 1; tv = tv << (ui->recordingTax->value() + 64);
  gb.add( AOK_RECORDING_TAX  , AOCoins( tv ) );
}

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
#include "readerClient.h"

ReaderClient::ReaderClient(QWidget *cw) :
    QGroupBox(cw),
    ui(new Ui::ReaderClient)
{ ui->setupUi(this);
  if ( cw )
    { new QVBoxLayout( cw );
      cw->layout()->addWidget( this );
      cw->layout()->setContentsMargins( 0,0,0,0 );
    }
  pa = new ProtocolActor( RCD_actorReaderClient_o, this );
  connect( pa           , SIGNAL( newName(QString)           ), ui->rcProtocol, SLOT( setText(QString)  ) );
  connect( pa           , SIGNAL( transactionRecord(QString) ), ui->rcLog     , SLOT( append(QString)   ) );
  connect( pa           , SIGNAL( newProtocolSet()           )                , SLOT( newProtocolSet()  ) );
  connect( ui->rcRequest, SIGNAL( clicked()                  )                , SLOT( sendReadRequest() ) );
}

ReaderClient::~ReaderClient()
{ delete ui; }

/**
 * @brief ReaderClient::newProtocolSet
 */
void ReaderClient::newProtocolSet()
{ ui->rcRequest          ->setVisible( pa->   sendableObTypes.contains( (RiceyInt)RCD_readRequest_o  ) );
  ui->rcDataHandleGroup  ->setVisible( pa->  sendableContents.contains( (RiceyInt)RCD_recordId_i     ) );
  ui->rcIdGroup          ->setVisible( pa->  sendableContents.contains( (RiceyInt)RCD_userId_b       ) );
  ui->rcBlockchainIdGroup->setVisible( pa->  sendableContents.contains( (RiceyInt)RCD_blockchainId_b ) );
  ui->rcDataGroup        ->setVisible( pa->receivableContents.contains( (RiceyInt)RCD_recordText_s   ) );
  // qWarning( "ReaderClient::newProtocolSet()" );
}

/**
 * @brief ReaderClient::sendReadRequest - catches signal from the ui button
 */
void ReaderClient::sendReadRequest()
{ if ( pa->pp == nullptr )
    { emit pa->transactionRecord("protocol not defined.");
      return;
    }
  // Compose bao based on protocol definition and ui contents and send it.
  // Unneeded components will be ignored by pa->compose
  BlockObjectMap inputs;
  pa->prepare( inputs, RCD_userId_b      , ui->rcId          ->text().toUtf8()    , this );
  pa->prepare( inputs, RCD_recordId_i    , ui->rcDataHandle  ->text().toLongLong(), this );
  pa->prepare( inputs, RCD_blockchainId_b, ui->rcBlockchainId->text().toUtf8()    , this );
  // more to come
  BaoSerial bao = pa->compose( RCD_readRequest_o, inputs );
  if ( bao.size() > 0 )
    { emit pa->transactionRecord( QString("sendReadRequest(%1)").arg(bao.toHex()) );
      emit sendRequest( bao );
    }
   else
    qWarning( "problem when composing readRequest" );
  pa->dispose( inputs );
}

/**
 * @brief ReaderClient::receiveResponse
 * @param resp - from server
 */
void ReaderClient::receiveResponse( QByteArray resp )
{ if ( resp.size() < 1 )
    { qWarning( "ReaderClient::receiveResponse() received empty bao" );
      return;
    }
  RiceyInt respTyp = riceToInt( resp );
  BlockObjectMap bom = pa->extract( resp );
  Utf8String recordText;
  bool valid = true;
  switch ( respTyp )
    { case RCD_readResponse_o:
        if ( !bom.contains( RCD_recordText_s ) )
          qWarning( "response did not contain a recordText" );
         else
          { if ( bom.value(RCD_recordText_s) == nullptr )
              qWarning( "bom recordText nullptr" );
             else
              { BlockValueString *bvs = qobject_cast<BlockValueString *>(bom.value(RCD_recordText_s));
                if ( bvs != nullptr )
                  recordText = bvs->value();
              } // recordText not null
          }    // recordText present
        break;

      default:
        Utf8String ns = dict.nameOrHexFromCode( respTyp );
        qWarning( "ReaderClient::receiveResponse() unrecognized type %s", ns.data() );
        valid = false;
    }
  QString msg = QString("receiveResponse(%1)%2" ).arg( QString::fromUtf8(resp.toHex()) ).arg( valid ? "" : " invalid." );
  emit pa->transactionRecord( msg );
  if ( resp.size() > 0 )
    ui->rcData->append( recordText );
}

void ReaderClient::handleReceived( qint64 h )
{ ui->rcDataHandle->setText( QString::number(h) ); }

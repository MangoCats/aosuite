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
#include "writerClient.h"

WriterClient::WriterClient(QWidget *cw) :
    QGroupBox(cw),
    ui(new Ui::WriterClient)
{ ui->setupUi(this);
  if ( cw )
    { new QVBoxLayout( cw );
      cw->layout()->addWidget( this );
      cw->layout()->setContentsMargins( 0,0,0,0 );
    }
  pa = new ProtocolActor( RCD_actorWriterClient_o, this );
  connect( pa        , SIGNAL( newName(QString)           ), ui->wcProtocol, SLOT( setText(QString)   ) );
  connect( pa        , SIGNAL( transactionRecord(QString) ), ui->wcLog     , SLOT( append(QString)    ) );
  connect( pa        , SIGNAL( newProtocolSet()           )                , SLOT( newProtocolSet()   ) );
  connect( ui->wcSend, SIGNAL( clicked()                  )                , SLOT( sendWriteRequest() ) );
}

WriterClient::~WriterClient()
{ delete ui; }

/**
 * @brief WriterClient::newProtocolSet
 */
void WriterClient::newProtocolSet()
{ ui->wcSend             ->setVisible( pa->sendableObTypes .contains( (RiceyInt)RCD_writeRequest_o ) );
  ui->wcIdGroup          ->setVisible( pa->sendableContents.contains( (RiceyInt)RCD_userId_b       ) );
  ui->wcDataGroup        ->setVisible( pa->sendableContents.contains( (RiceyInt)RCD_recordText_s   ) );
  ui->wcBlockchainIdGroup->setVisible( pa->sendableContents.contains( (RiceyInt)RCD_blockchainId_b ) );
  // qWarning( "WriterClient::newProtocolSet()" );
}

/**
 * @brief WriterClient::sendWriteRequest - catches signal from the ui button
 */
void WriterClient::sendWriteRequest()
{ emit pa->transactionRecord("sendWriteRequest()");
  if ( pa->pp == nullptr )
    { emit pa->transactionRecord("protocol not defined.");
      return;
    }
  // Compose bao based on protocol definition and ui contents and send it.
  // Unneeded components will be ignored by pa->compose
  BlockObjectMap inputs;
  BlockValueByteArray  *userId = new BlockValueByteArray( ui->wcId          ->text().toUtf8(), this );
  BlockValueString *recordText = new BlockValueString   ( ui->wcData ->toPlainText().toUtf8(), this );
  BlockValueByteArray *chainId = new BlockValueByteArray( ui->wcBlockchainId->text().toUtf8(), this );
  inputs.insert( RCD_userId_b      , userId     );
  inputs.insert( RCD_recordText_s  , recordText );
  inputs.insert( RCD_blockchainId_b, chainId    );
  // more to come
  BaoSerial bao = pa->compose( RCD_writeRequest_o, inputs ); bao.detach();
  if ( bao.size() > 0 )
    emit sendRequest( bao );
   else
    qWarning( "problem when composing writeRequest" );
  userId    ->deleteLater();
  recordText->deleteLater();
  chainId   ->deleteLater();
}

/**
 * @brief WriterClient::receiveResponse
 * @param resp - from server
 */
void WriterClient::receiveResponse( QByteArray resp )
{ RiceyInt respTyp = riceToInt( resp );
  BlockObjectMap bom = pa->extract( resp );
  qint64 recordHandle = -1;
  switch ( respTyp )
    { case RCD_writeResponse_o:
        if ( !bom.contains( RCD_recordId_i ) )
          qWarning( "response did not contain a recordId" );
         else
          { if ( bom.value(RCD_recordId_i) == nullptr )
              qWarning( "bom recordId nullptr" );
             else
              { BlockValueInt64 *bvi = qobject_cast<BlockValueInt64 *>(bom.value(RCD_recordId_i));
                if ( bvi == nullptr ) qWarning( "void WriterClient::receiveResponse() bvi nullptr" ); else
                  recordHandle = bvi->value();
              } // recordHandle not null
          }    // recordHandle present
        break;

      default:
        qWarning( "unrecognized response type %llx", respTyp );
    }
  QString msg = QString("receiveResponse(%1) %2 %3")
          .arg( QString::fromUtf8(resp.toHex()) ).arg( recordHandle )
          .arg( QDateTime::fromMSecsSinceEpoch(recordHandle/1000).toString() );
  emit pa->transactionRecord( msg );
}


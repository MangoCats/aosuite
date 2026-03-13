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
{ if ( pa->pp == nullptr ) { emit pa->transactionRecord("protocol not defined."); return; }
  // Compose bao based on protocol definition and ui contents and send it.
  // Components not required in the currently selected protocol will be ignored by pa->compose
  BlockObjectMap inputs;
  pa->prepare( inputs, RCD_userId_b      , ui->wcId          ->text().toUtf8(), this );
  pa->prepare( inputs, RCD_recordText_s  , ui->wcData ->toPlainText().toUtf8(), this );
  pa->prepare( inputs, RCD_blockchainId_b, ui->wcBlockchainId->text().toUtf8(), this );
  // more to come
  BaoSerial bao = pa->compose( RCD_writeRequest_o, inputs ); bao.detach();
  if ( bao.size() > 0 )
    { emit pa->transactionRecord( QString("sendWriteRequest(%1)").arg(bao.toHex()) );
      emit sendRequest( bao );
    }
   else
    qWarning( "problem when composing writeRequest" );
  pa->dispose( inputs );
}

/**
 * @brief WriterClient::receiveResponse
 * @param resp - from server
 */
void WriterClient::receiveResponse( QByteArray resp )
{ BlockObjectMap bom;
  if ( resp.size() < 1 )
    { qWarning( "WriterClient::receiveResponse() received empty bao" );
      return;
    }
  qint64 recordHandle = -1;
  qint32 sz = 0;
    bool ok = false;
  RiceyInt respTyp = riceToInt( resp, &sz, &ok );
  if (!ok)
    { qWarning( "response type not a valid rice code." ); }
   else
    { bom = pa->extract( resp ); }
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
        Utf8String ns = dict.nameOrHexFromCode( respTyp );
        qWarning( "WriterClient::receiveResponse() unrecognized type %s", ns.data() );
    }
  QString msg = QString("receiveResponse(%1)").arg( QString::fromUtf8(resp.toHex()) );
  if ( recordHandle > 0 ) // TODO: a more generalized validity check
    msg.append( QString( " %1 %2" ).arg( recordHandle ).arg( QDateTime::fromMSecsSinceEpoch(recordHandle/1000).toString() ) );
   else
    msg.append( " invalid." );
  if ( recordHandle > 0 )
    emit handleReceived( recordHandle );
  emit pa->transactionRecord( msg );
}


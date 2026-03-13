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
#include "readerServer.h"

ReaderServer::ReaderServer(QWidget *cw) :
    QGroupBox(cw),
    ui(new Ui::ReaderServer)
{ ui->setupUi(this);
  if ( cw )
    { new QVBoxLayout( cw );
      cw->layout()->addWidget( this );
      cw->layout()->setContentsMargins( 0,0,0,0 );
    }
  pa = new ProtocolActor( RCD_actorReaderServer_o, this );
  connect( pa, SIGNAL( newName(QString)           ), ui->rsProtocol, SLOT( setText(QString) ) );
  connect( pa, SIGNAL( transactionRecord(QString) ), ui->rsLog     , SLOT( append(QString)  ) );
  connect( pa, SIGNAL( newProtocolSet()           )                , SLOT( newProtocolSet() ) );

}

ReaderServer::~ReaderServer()
{ delete ui; }

/**
 * @brief ReaderServer::newProtocolSet
 */
void ReaderServer::newProtocolSet()
{ ui->rsIdGroup->setVisible( pa->sendableContents.contains( (RiceyInt)RCD_serverId_b ) );
}

/**
 * @brief ReaderServer::receiveRequest
 * @param req - request from reader client
 */
void ReaderServer::receiveRequest( QByteArray req )
{ emit pa->transactionRecord( QString("receiveRequest(%1)").arg( QString::fromUtf8( req.toHex() ) ) );
  RiceyInt reqTyp = riceToInt( req );
  BlockObjectMap bom = pa->extract( req );
  BaoSerial resp;
  qint64 tm = -1;
  QByteArray cid = "0";
  QByteArray uid = "";
  switch ( reqTyp )
    { case RCD_readRequest_o:
        if ( pa->get( bom, RCD_recordId_i, tm ) )            // Must get this.
          { pa->get( bom, RCD_blockchainId_b, cid, false );  // default to "0" if not present, TODO: make required when it is so in the protocol
            pa->get( bom, RCD_userId_b      , uid, false );  // default to "" if not present , TODO: make required when it is so in the protocol
            Utf8String recordText = readRecord( tm, cid );
            if ( recordText.size() > 0 )
              resp = buildResponse( recordText );
             else
              qWarning( "readRecord had a problem." );
          } // if recordId present
        break;

      default:
        Utf8String ns = dict.nameOrHexFromCode( reqTyp );
        qWarning( "ReaderServer::receiveRequest() unrecognized type %s", ns.data() );
    }
  emit sendResponse( resp );
  pa->dispose( bom );
}

#include <QFile>
/**
 * @brief ReaderServer::readRecord
 * @param tm - time of record to read
 * @param cid = chain id of record to read
 * @return data read from record, or empty if there was a problem
 */
Utf8String  ReaderServer::readRecord( qint64 tm, const Utf8String &cid )
{ QString fn = QString( "/var/aos/%1/%2" ).arg( QString::fromUtf8( cid ) ).arg( tm );
  QFile file(fn);
    if ( !file.open( QIODevice::ReadOnly ) )
      { qWarning( "could not open file %s", fn.toUtf8().data() );
        return Utf8String();
      }
  QByteArray ba = file.readAll();
  return ba;
}

/**
 * @brief ReaderServer::buildResponse
 * @param dataRecord - data that was read from the file
 * @return response packet
 */
BaoSerial  ReaderServer::buildResponse( const Utf8String &dataRecord )
{ BlockObjectMap inputs;
  BlockValueString *dr = new BlockValueString( dataRecord, this );
  inputs.insert( RCD_recordText_s, dr );
  return pa->compose( RCD_readResponse_o, inputs );
}

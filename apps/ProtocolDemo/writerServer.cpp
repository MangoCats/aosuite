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
#include "writerServer.h"

WriterServer::WriterServer(QWidget *cw) :
    QGroupBox(cw),
    ui(new Ui::WriterServer)
{ ui->setupUi(this);
  if ( cw )
    { new QVBoxLayout( cw );
      cw->layout()->addWidget( this );
      cw->layout()->setContentsMargins( 0,0,0,0 );
    }
  index = 0;
  pa = new ProtocolActor( RCD_actorWriterServer_o, this );
  connect( pa, SIGNAL( newName(QString)           ), ui->wsProtocol, SLOT( setText(QString) ) );
  connect( pa, SIGNAL( transactionRecord(QString) ), ui->wsLog     , SLOT( append(QString)  ) );
  connect( pa, SIGNAL( newProtocolSet()           )                , SLOT( newProtocolSet() ) );
}

WriterServer::~WriterServer()
{ delete ui; }

/**
 * @brief WriterServer::newProtocolSet
 */
void WriterServer::newProtocolSet()
{ ui->wsIdGroup->setVisible( pa->sendableContents.contains( (RiceyInt)RCD_serverId_b ) );
}

/**
 * @brief WriterServer::receiveRequest - process it and generate a response
 * @param req - request from writer client
 */
void WriterServer::receiveRequest( QByteArray req )
{ emit pa->transactionRecord( QString("receiveRequest(%1)").arg( QString::fromUtf8( req.toHex() ) ) );
  RiceyInt reqTyp = riceToInt( req );
  BlockObjectMap bom = pa->extract( req );
  BaoSerial resp;
  qint64 recordHandle = -1;
  switch ( reqTyp )
    { case RCD_writeRequest_o:
        if ( !bom.contains( RCD_recordText_s ) )
          qWarning( "will not write without recordText" );
         else
          { if ( bom.value(RCD_recordText_s) == nullptr )
              qWarning( "bom recordText nullptr" );
             else
              { QByteArray rt;
                BlockValueString *rtp = qobject_cast<BlockValueString *>(bom.value(RCD_recordText_s));
                if ( rtp == nullptr ) qWarning( "rtp is nullptr" ); else
                  {  rt = rtp->value();
                    rt.detach();
                  }
                QByteArray cid = "0";
                if ( bom.contains( RCD_blockchainId_b ) )
                  { if ( bom.value(RCD_blockchainId_b) == nullptr )
                      qWarning( "bom blockchainId nullptr" );
                     else
                      { BlockValueByteArray *cip = qobject_cast<BlockValueByteArray *>(bom.value(RCD_blockchainId_b));
                        if ( cip == nullptr ) qWarning( "cip is nullptr" ); else
                          { cid = cip->value();
                            cid.detach();
                          }
                      }
                  }
                QByteArray uid = "";
                if ( bom.contains( RCD_userId_b ) )
                  { if ( bom.value(RCD_userId_b) == nullptr )
                      qWarning( "bom userId nullptr" );
                     else
                      { BlockValueByteArray *uip = qobject_cast<BlockValueByteArray *>(bom.value(RCD_userId_b));
                        if ( uip == nullptr ) qWarning( "uip is nullptr" ); else
                          { uid = uip->value();
                            uid.detach();
                          }
                      }
                  }
                recordHandle = writeRecord( rt, cid );
                if ( recordHandle > 0 )
                  resp = buildResponse( recordHandle );
                 else
                  qWarning( "writeRecord had a problem." );
                  // empty resp for the error state
              } // recordText not null
          }    // recordText present
        break;

      default:
        qWarning( "unrecognized request type %llx", reqTyp );
    }
  emit sendResponse( resp );
}

#include <QFile>
/**
 * @brief WriterServer::writeRecord
 * @param rt - record text, to write
 * @param cid - chainId, to write text in
 * @return handle (time) of the record written, or -1 if there was a problem
 */
qint64  WriterServer::writeRecord( const QByteArray &rt, const QByteArray &cid )
{ qint64 tm = QDateTime::currentMSecsSinceEpoch() * 1000 + index;
  if ( ++index >= 1000 )
    index = 0;
  // TODO: fill the huge security hole (file access) before using this exposed to the wild
  QString fn = QString( "/var/aos/%1/%2" ).arg( QString::fromUtf8( cid ) ).arg( tm );
  QFile file(fn);
  if ( !file.open( QIODevice::WriteOnly ) )
    { qWarning( "could not open file %s", fn.toUtf8().data() );
      return -1;
    }
  file.write( rt );
  return tm;
}

/**
 * @brief WriterServer::buildResponse
 * @param recordHandle - time record was written
 * @return response packet
 */
BaoSerial  WriterServer::buildResponse( qint64 recordHandle )
{ BlockObjectMap inputs;
  BlockValueInt64 *rh = new BlockValueInt64( recordHandle, this );
  inputs.insert( RCD_recordId_i, rh );
  return pa->compose( RCD_writeResponse_o, inputs );
}

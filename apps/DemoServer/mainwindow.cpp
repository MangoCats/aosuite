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
#include "mainwindow.h"
#include "ui_mainwindow.h"
#include "amqpInterface.h"
#include <QDir>

MainWindow::MainWindow(QWidget *parent)
    : QMainWindow(parent)
    , ui(new Ui::MainWindow)
{ ui->setupUi(this);
  initReadFile();
  rs = new ReaderServer( ui->readerFrame );
  ws = new WriterServer( ui->writerFrame );
  ai = amqpQuickStart( this );
  AmqpInterface *ai = amqpQuickStart( this );
  connect( ai  , SIGNAL(logMessage(QString)), ws->ui->wsLog, SLOT(insertPlainText(QString))       );
  connect( this, SIGNAL(setProtocol(BaoSerial)), ws->pa    , SLOT(setProtocol(BaoSerial))         );
  connect( this, SIGNAL(setProtocol(BaoSerial)), rs->pa    , SLOT(setProtocol(BaoSerial))         );
  connect( rs  , SIGNAL(sendResponse(BaoSerial))           , SLOT(sendResponse(BaoSerial))        );
  connect( ws  , SIGNAL(sendResponse(BaoSerial))           , SLOT(sendResponse(BaoSerial))        );
  connect( ai  , SIGNAL(reqReceivedMessage(QByteArray))    , SLOT(reqReceivedMessage(QByteArray)) );
}

MainWindow::~MainWindow()
{ delete ui; }

/**
 * @brief MainWindow::sendResponse
 * @param b - response to send
 * // TODO: interpret the topic out of the packet
 */
void MainWindow::sendResponse(BaoSerial b)
{ ai->respSendMessage( b, "" ); }

/**
 * @brief MainWindow::reqReceivedMessage
 * @param ba - message received from the interface
 * // TODO: maybe a little better destination routing?
 */
void MainWindow::reqReceivedMessage(QByteArray ba)
{ if ( ba.size() < 1 )
    { qWarning( "MainWindow::reqReceivedMessage() received empty message" );
      return;
    }
  rs->receiveRequest(ba);
  ws->receiveRequest(ba);
}

void MainWindow::closeEvent(QCloseEvent *event)
{ QSettings settings;
  settings.setValue( "geometry", saveGeometry() );
  settings.setValue( "state"   , saveState()    );
  QMainWindow::closeEvent(event);
}

/**
 * @brief ProtocolDemo::initReadFile - readFile is a comboBox which specifies
 *   a resource file to select a protocol from when Set is clicked.
 */
void  MainWindow::initReadFile()
{ ui->readFile->clear();
  QDir d(":/protocols");
  QFileInfoList fil = d.entryInfoList();
  foreach( QFileInfo fi, fil )
    { QString fn = fi.fileName();
      if ( fn.endsWith( ".bao" ) )
        ui->readFile->addItem( fn.mid( 0, fn.size()-4) );
    }
}

/**
 * @brief ProtocolDemo::on_set_clicked - protocol set has clicked by user
 */
void  MainWindow::on_set_clicked()
{ QString readFile;
  readFile = ":/protocols/"+ui->readFile->currentText()+".bao";
  if ( readFile.size() < 1 )
    return;
  QFile file(readFile);
  if ( !file.open(QIODevice::ReadOnly) )
    { qWarning( "could not open %s for reading", readFile.toUtf8().data() );
      return;
    }
   QFileInfo fi = QFileInfo( file );
  Utf8String fn = fi.fileName().toUtf8();
   BaoSerial bs = file.readAll();
  if ( bs.size() > 0 )
    emit setProtocol( bs );
   else
    qWarning( "read no data from %s", fn.data() );
}

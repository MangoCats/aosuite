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
#ifndef READERSERVER_H
#define READERSERVER_H

#include <QGroupBox>
#include "ui_readerServer.h"
#include "protocolParser.h"

namespace Ui {
class ReaderServer;
}

class ReaderServer : public QGroupBox
{ Q_OBJECT
public:
    explicit  ReaderServer(QWidget *parent = nullptr);
             ~ReaderServer();
  Utf8String  readRecord( qint64, const Utf8String & );
   BaoSerial  buildResponse( const Utf8String &dataRecord );
signals:
        void  sendResponse( BaoSerial );

public slots:
        void  receiveRequest( BaoSerial );
        void  newProtocolSet();

public:
  QPointer<ProtocolActor> pa;
        Ui::ReaderServer *ui;
};

#endif // READERSERVER_H

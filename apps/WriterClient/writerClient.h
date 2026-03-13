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
#ifndef WRITERCLIENT_H
#define WRITERCLIENT_H

#include <QGroupBox>
#include "ui_writerClient.h"
#include "protocolParser.h"

namespace Ui {
class WriterClient;
}

class WriterClient : public QGroupBox
{ Q_OBJECT
public:
    explicit WriterClient(QWidget *parent = nullptr);
            ~WriterClient();

signals:
        void sendRequest( BaoSerial ); // sends request to the server
        void handleReceived( qint64 );

public slots:
        void sendWriteRequest();       // catches signal from the ui button
        void receiveResponse( BaoSerial );
        void newProtocolSet();

public:
  QPointer<ProtocolActor> pa;
        Ui::WriterClient *ui;
};

#endif // WRITERCLIENT_H

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
#ifndef AMQPINTERFACE_H
#define AMQPINTERFACE_H

#include <QObject>

#include <QtCore>
#include <QAbstractSocket>
#include "qamqpclient.h"
#include "qamqpexchange.h"
#include "qamqpqueue.h"

// Connection State Definitions
#define RI_NOT_STARTED       0
#define RI_STARTED           1
#define RI_CLIENT_CONNECTED  2
#define RI_EXCHANGE_DECLARED 3
#define RI_QUEUE_DECLARED    4
#define RI_QUEUE_BOUND       5

class AmqpInterface;

AmqpInterface *amqpQuickStart( QObject *p = nullptr );

class AmqpInterface : public QObject
{
    Q_OBJECT
public:
    enum Exchange : quint8 { REQ, RESP };
    Q_ENUM(Exchange)

public:
    AmqpInterface( QObject *parent = nullptr );
    void prep();
    void setToDefaultValues();
    void readSettings();
    void writeSettings();
    void disconnect();
    void shutDown();

signals:
    void  reqReceivedMessage( QByteArray );
    void respReceivedMessage( QByteArray );
    void logMessage( QString );
    void connected();
    void disconnected();
    void  reqReadySig();
    void respReadySig();
    void messageToRemote(QByteArray, Exchange, QString);

public slots:
    void start();
    void retry();
    void connectButton();
    void clientConnected();
    void  reqExchangeDeclared();
    void respExchangeDeclared();
    void  reqQueueDeclared();
    void respQueueDeclared();
    void  reqQueueBound();
    void respQueueBound();
    void  reqSendMessage( QByteArray message, QString route = "" );
    void respSendMessage( QByteArray message, QString route = "" );
    void  reqMessageReceived();
    void respMessageReceived();
    void disconnectAndRestart();
    void clientDisconnected();
    void error(QAMQP::Error error);
    void socketError(QAbstractSocket::SocketError error);
    void messageFromRemote(QByteArray message, Exchange exchange, const QString& route);

private:
    void  reqPublishMessage( QByteArray message, QString route );
    void respPublishMessage( QByteArray message, QString route );

public:
               QAmqpClient    m_client;
      QPointer<QAmqpQueue   >  amqpReqQueue;
      QPointer<QAmqpQueue   > amqpRespQueue;
      QPointer<QAmqpExchange>  reqExchange;
      QPointer<QAmqpExchange> respExchange;
                     QString   reqExchangeName;
                     QString  respExchangeName;
                     QString   reqQueueName;
                     QString  respQueueName;
                        bool   reqQueueName_set;
                        bool  respQueueName_set;
                         int   reqReady;
                         int  respReady;
                     QString   reqConsumerTag;
                     QString  respConsumerTag;
                        bool   reqConsumerTag_set;
                        bool  respConsumerTag_set;
                     QString   reqBindingKey;
                     QString  respBindingKey;
                         int   reqStats_packetCount;
                         int  respStats_packetCount;
                         int   reqStats_byteCount;
                         int  respStats_byteCount;
                         int   reqStats_lastPacketSize;
                         int  respStats_lastPacketSize;
  QAmqpMessage::PropertyHash  sendProperties;
                        bool  objectConnections;
                        bool  startPending;
                        bool  connectButtonPressed;
                        // properties
                         int  reqConnectionState;
                         int  respConnectionState;
                         int  startDelay;
                         int  heartbeatDelay;
                        bool  shuttingDown;
                        bool  shutdownComplete;
                        bool  restartOnDisconnect;
                         int  restartOnDisconnect_time;
                     QString  username;
                     QString  password;
                     QString  vhost;
                     QString  address;
                     quint16  port;
                        bool  echo;
                        bool  echoHex;
                  QByteArray  appId;
};

#endif // AMQPINTERFACE_H

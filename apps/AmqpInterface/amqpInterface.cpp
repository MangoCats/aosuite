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
#include "amqpInterface.h"

/**
 * @brief amqpQuickStart
 * @param parent - object to become parent of the AmqpInterface
 * @return ri - pointer to a new AmqpInterface
 */
AmqpInterface *amqpQuickStart( QObject *parent )
{ AmqpInterface *ai = new AmqpInterface( parent );
  ai->prep();
  ai->startPending = true;
  emit ai->logMessage( "  AmqpInterface will start() after "+QString::number( ai->startDelay ) + " milliseconds" );
  QTimer::singleShot( ai->startDelay, ai, SLOT(start()) );
  return ai;
}

/**
 * @brief AmqpInterface::AmqpInterface - creates the AmqpInterface object,
 *   waits for prep() and start() to be called later.
 * @param p - parent object
 */
AmqpInterface::AmqpInterface( QObject *p ) : QObject( p )
{
  qRegisterMetaType<Exchange>();
  objectConnections    = false;
  startPending         = false;
  connectButtonPressed = false;
  setToDefaultValues();
  readSettings();
}

void AmqpInterface::setToDefaultValues()
{  reqConnectionState      = RI_NOT_STARTED;
  respConnectionState      = RI_NOT_STARTED;
   reqReady                = 0;
  respReady                = 0;
   reqExchangeName         = "Requests";
  respExchangeName         = "Responses";
   reqQueueName            = "requests";
  respQueueName            = "responses";
   reqQueueName_set        = false;
  respQueueName_set        = false;
   reqConsumerTag          = "reqConsumerTag";
  respConsumerTag          = "respConsumerTag";
   reqConsumerTag_set      = false;
  respConsumerTag_set      = false;
   reqBindingKey           = "#";
  respBindingKey           = "#";
   reqStats_packetCount    = 0;
  respStats_packetCount    = 0;
   reqStats_byteCount      = 0;
  respStats_byteCount      = 0;
   reqStats_lastPacketSize = 0;
  respStats_lastPacketSize = 0;
  startDelay               = 1000;  // milliseconds
  shuttingDown             = false;
  shutdownComplete         = false;
  restartOnDisconnect      = true;
  restartOnDisconnect_time = 5000;
  heartbeatDelay           = 10000;
  username                 = "guest";
  password                 = "guest";
  vhost                    = "/";
  address                  = "localhost";
  port                     = 5672;
  echo                     = false;
  echoHex                  = false;
  appId                    = "me";
}

void AmqpInterface::readSettings()
{ QSettings settings;

   reqConnectionState      = settings.value(  "reqConnectionState"      ).toInt();
  respConnectionState      = settings.value( "respConnectionState"      ).toInt();
   reqReady                = settings.value(  "reqReady"                ).toInt();
  respReady                = settings.value( "respReady"                ).toInt();
   reqExchangeName         = settings.value(  "reqExchangeName"         ).toString();
  respExchangeName         = settings.value( "respExchangeName"         ).toString();
   reqQueueName            = settings.value(  "reqQueueName"            ).toString();
  respQueueName            = settings.value( "respQueueName"            ).toString();
   reqQueueName_set        = settings.value(  "reqQueueName_set"        ).toBool();
  respQueueName_set        = settings.value( "respQueueName_set"        ).toBool();
   reqConsumerTag          = settings.value(  "reqConsumerTag"          ).toString();
  respConsumerTag          = settings.value( "respConsumerTag"          ).toString();
   reqConsumerTag_set      = settings.value(  "reqConsumerTag_set"      ).toBool();
  respConsumerTag_set      = settings.value( "respConsumerTag_set"      ).toBool();
   reqBindingKey           = settings.value(  "reqBindingKey"           ).toString();
  respBindingKey           = settings.value( "respBindingKey"           ).toString();
   reqStats_packetCount    = settings.value(  "reqStats_packetCount"    ).toInt();
  respStats_packetCount    = settings.value( "respStats_packetCount"    ).toInt();
   reqStats_byteCount      = settings.value(  "reqStats_byteCount"      ).toInt();
  respStats_byteCount      = settings.value( "respStats_byteCount"      ).toInt();
   reqStats_lastPacketSize = settings.value(  "reqStats_lastPacketSize" ).toInt();
  respStats_lastPacketSize = settings.value( "respStats_lastPacketSize" ).toInt();
  startDelay               = settings.value( "startDelay"               ).toInt();
  shuttingDown             = settings.value( "shuttingDown"             ).toBool();
  shutdownComplete         = settings.value( "shutdownComplete"         ).toBool();
  restartOnDisconnect      = settings.value( "restartOnDisconnect"      ).toBool();
  restartOnDisconnect_time = settings.value( "restartOnDisconnect_time" ).toInt();
  heartbeatDelay           = settings.value( "heartbeatDelay"           ).toInt();
  username                 = settings.value( "username"                 ).toString();
  password                 = settings.value( "password"                 ).toString();
  vhost                    = settings.value( "vhost"                    ).toString();
  address                  = settings.value( "address"                  ).toString();
  port                     = settings.value( "port"                     ).toUInt();
  echo                     = settings.value( "echo"                     ).toBool();
  echoHex                  = settings.value( "echoHex"                  ).toBool();
  appId                    = settings.value( "appId"                    ).toByteArray();
}

void AmqpInterface::writeSettings()
{ QSettings settings;

  settings.setValue(  "reqConnectionState",       reqConnectionState      );
  settings.setValue( "respConnectionState",      respConnectionState      );
  settings.setValue(  "reqReady",                 reqReady                );
  settings.setValue( "respReady",                respReady                );
  settings.setValue(  "reqExchangeName",          reqExchangeName         );
  settings.setValue( "respExchangeName",         respExchangeName         );
  settings.setValue(  "reqQueueName",             reqQueueName            );
  settings.setValue( "respQueueName",            respQueueName            );
  settings.setValue(  "reqQueueName_set",         reqQueueName_set        );
  settings.setValue( "respQueueName_set",        respQueueName_set        );
  settings.setValue(  "reqConsumerTag",           reqConsumerTag          );
  settings.setValue( "respConsumerTag",          respConsumerTag          );
  settings.setValue(  "reqConsumerTag_set",       reqConsumerTag_set      );
  settings.setValue( "respConsumerTag_set",      respConsumerTag_set      );
  settings.setValue(  "reqBindingKey",            reqBindingKey           );
  settings.setValue( "respBindingKey",           respBindingKey           );
  settings.setValue(  "reqStats_packetCount",     reqStats_packetCount    );
  settings.setValue( "respStats_packetCount",    respStats_packetCount    );
  settings.setValue(  "reqStats_byteCount",       reqStats_byteCount      );
  settings.setValue( "respStats_byteCount",      respStats_byteCount      );
  settings.setValue(  "reqStats_lastPacketSize",  reqStats_lastPacketSize );
  settings.setValue( "respStats_lastPacketSize", respStats_lastPacketSize );
  settings.setValue( "startDelay",               startDelay               );
  settings.setValue( "shuttingDown",             shuttingDown             );
  settings.setValue( "shutdownComplete",         shutdownComplete         );
  settings.setValue( "restartOnDisconnect",      restartOnDisconnect      );
  settings.setValue( "restartOnDisconnect_time", restartOnDisconnect_time );
  settings.setValue( "heartbeatDelay",           heartbeatDelay           );
  settings.setValue( "username",                 username                 );
  settings.setValue( "password",                 password                 );
  settings.setValue( "vhost",                    vhost                    );
  settings.setValue( "address",                  address                  );
  settings.setValue( "port",                     port                     );
  settings.setValue( "echo",                     echo                     );
  settings.setValue( "echoHex",                  echoHex                  );
  settings.setValue( "appId",                    appId                    );
}

void AmqpInterface::retry()
{ emit logMessage( "AmqpInterface::retry()" );
  if ((  reqConnectionState < RI_QUEUE_BOUND ) ||
      ( respConnectionState < RI_QUEUE_BOUND ))
    { disconnectAndRestart();
      return;
    }
  emit logMessage( "Connections are good, end of retries." );
}

void AmqpInterface::disconnectAndRestart()
{ emit logMessage( "AmqpInterface::disconnectAndRestart()");
  if ( m_client.isConnected() )
    { disconnect();
      return;
    }
   else
    { emit logMessage( "  not connected, will not disconnectFromHost()" );
      clientDisconnected();
    }
}

void AmqpInterface::disconnect()
{ emit logMessage( "  disconnectFromHost()" );
  m_client.disconnectFromHost(); // should continue in clientDisconnected() on disconnected() signal from m_client
}

void AmqpInterface::clientDisconnected()
{ emit disconnected();
  emit logMessage( "AmqpInterface::clientDisconnected()");
   reqConnectionState = RI_STARTED;
  respConnectionState = RI_STARTED;

  if ( shuttingDown )
    { emit logMessage( "shutdown complete." );
      startPending = false;
    }
   else if ( restartOnDisconnect )
    { if ( startPending )
        { emit logMessage( "  start already pending, will not start another" );
        }
       else
        { startPending = true;
          prep();
          int time = restartOnDisconnect_time;
          if ( connectButtonPressed )
            { time = 100;
              connectButtonPressed = false;
            }
          emit logMessage( "  will start() again in "+QString::number( time ) + " milliseconds" );
          QTimer::singleShot( time, this, SLOT(start()) );
        }
    }
   else
    emit logMessage( "restartOnDisconnect == false, not attempting restart." );
}

void AmqpInterface::shutDown()
{ emit logMessage( "shutDown()" );
  shuttingDown     = true;
  shutdownComplete = false;
  disconnect();
}

void AmqpInterface::error(QAMQP::Error error)
{ QString msg = QString( "AmqpInterface::error( %1 ) %2" ).arg( error ).arg( m_client.errorString() );
  emit logMessage( msg );
  if ((  reqConnectionState < RI_QUEUE_BOUND ) ||
      ( respConnectionState < RI_QUEUE_BOUND ))
    disconnectAndRestart();
  // qApp->quit();
}

void AmqpInterface::socketError(QAbstractSocket::SocketError error)
{ QString msg = QString( "AmqpInterface::socketError( %1 ) %2" ).arg( error ).arg( m_client.errorString() );
  emit logMessage( msg );
  if ((  reqConnectionState < RI_QUEUE_BOUND ) ||
      ( respConnectionState < RI_QUEUE_BOUND ))
    disconnectAndRestart();
}

////////////////////////////////////////////////////////////////////////////////////////////

/**
 * @brief AmqpInterface::prep - first step after the AmqpInterface object is created and
 *   initialized with optional form/widget pointers.  Also called during a disconnectAndRestart().
 */
void AmqpInterface::prep()
{ emit logMessage( "AmqpInterface::prep()" );
             reqReady = 0;
            respReady = 0;
   reqConnectionState = RI_NOT_STARTED;
  respConnectionState = RI_NOT_STARTED;
  if (  amqpReqQueue )  amqpReqQueue->deleteLater();
  if ( amqpRespQueue ) amqpRespQueue->deleteLater();
  if (   reqExchange )   reqExchange->deleteLater();
  if (  respExchange )  respExchange->deleteLater();
  if ( !objectConnections )
    { objectConnections = true;
      // These are the ins and outs
      // connect( this    , SIGNAL(receivedMessage(QByteArray) )         , pb, SLOT(newMessageReceived(QByteArray)), Qt::QueuedConnection );
      // connect( pb      , SIGNAL(messageToPublish(QByteArray,QString))     , SLOT(propSendMessage(QByteArray,QString)       ));
      connect(&m_client, SIGNAL(connected())                              , SLOT(clientConnected()                         ));
      connect(&m_client, SIGNAL(disconnected())                           , SLOT(clientDisconnected()                      ));
      connect(&m_client, SIGNAL(error(QAMQP::Error))                      , SLOT(error(QAMQP::Error)                       ));
      connect(&m_client, SIGNAL(socketError(QAbstractSocket::SocketError)), SLOT(socketError(QAbstractSocket::SocketError) ));
      emit logMessage( "AmqpInterface::prep() objectConnections made." );
    }
  if ( heartbeatDelay > 0 )
    { emit logMessage( "  setHeartbeatDelay("+QString::number(heartbeatDelay)+")" );
      m_client.setHeartbeatDelay( heartbeatDelay );
    }
  emit logMessage( "  setUsername( "+username+" )" );  m_client.setUsername( username );
  emit logMessage( "  setPassword( "+password+" )" );  m_client.setPassword( password );
  emit logMessage( "  setVirtualHost( "+vhost+" )" );  m_client.setVirtualHost( vhost );

  m_client.setAutoReconnect( true );
}

////////////////////////////////////////////////////////////////////////////////////////////

/**
 * @brief AmqpInterface::start - try to connect to host
 *   supports all exchanges and queues together.
 */
void AmqpInterface::start()
{ emit logMessage( "AmqpInterface::start()" );
  startPending = false;
  if ((  reqConnectionState > RI_NOT_STARTED ) ||
      ( respConnectionState > RI_NOT_STARTED ))
    { emit logMessage( "Already started, not going to start again." );
      return;
    }
   reqConnectionState = RI_STARTED;
  respConnectionState = RI_STARTED;

  if ( m_client.isConnected() )
    { logMessage( "  already connected, skipping ahead." );
      clientConnected();
      return;
    }

  emit logMessage( "  connectToHost( "+address+":"+QString::number( port ) +"/"+ vhost + " )" );
  m_client.connectToHost( QHostAddress( address ), (quint16)port );
}

/**
 * @brief AmqpInterface::connectButton - start, or retry, depending on current state.
 */
void AmqpInterface::connectButton()
{ emit logMessage( "AmqpInterface::connectButton()" );
  if ((  reqConnectionState > RI_NOT_STARTED ) ||
      ( respConnectionState > RI_NOT_STARTED ))
    { restartOnDisconnect  = true;
      connectButtonPressed = true;  // cuts short the reconnect delay
      retry();
    }
   else
    start();
}

///////////////////////////////////////////////////////////////////////////////////////////////

/**
 * @brief AmqpInterface::clientConnected - the server has responded,
 *   try to create exchanges.
 */
void AmqpInterface::clientConnected()
{ emit logMessage( "AmqpInterface::clientConnected()" );
   reqConnectionState = RI_CLIENT_CONNECTED;
  respConnectionState = RI_CLIENT_CONNECTED;

  emit logMessage( "  createExchange( "+reqExchangeName+ " )" );
  reqExchange = m_client.createExchange( reqExchangeName );
  connect( reqExchange, SIGNAL(declared()), this, SLOT( reqExchangeDeclared()));
  reqExchange->declare(QAmqpExchange::Topic);

  emit logMessage( "  createExchange( "+respExchangeName+ " )" );
  respExchange = m_client.createExchange( respExchangeName );
  connect(respExchange, SIGNAL(declared()), this, SLOT(respExchangeDeclared()));
  respExchange->declare(QAmqpExchange::Topic);
}

///////////////////////////////////////////////////////////////////////////////////////////////

void AmqpInterface::reqExchangeDeclared()
{ emit logMessage( "AmqpInterface::reqExchangeDeclared()" );
  reqConnectionState = RI_EXCHANGE_DECLARED;
  amqpReqQueue = m_client.createQueue();
  if ( amqpReqQueue == nullptr )
    { emit logMessage( "AmqpInterface client createQueue() failed for reqQueue." );
      return;
    }
  connect(amqpReqQueue, SIGNAL(declared()),        SLOT(reqQueueDeclared())  );
  connect(amqpReqQueue, SIGNAL(bound()),           SLOT(reqQueueBound())     );
  connect(amqpReqQueue, SIGNAL(messageReceived()), SLOT(reqMessageReceived()));
  if ( reqQueueName_set ) // checkbox control
    { emit logMessage( "  set req queue name '"+reqQueueName+"'" );
      amqpReqQueue->setName( reqQueueName );
    }
  emit logMessage( "  req declare(QAmqpQueue::Exclusive)" );
  amqpReqQueue->declare(QAmqpQueue::Exclusive);
}

void AmqpInterface::respExchangeDeclared()
{ emit logMessage( "AmqpInterface::respExchangeDeclared()" );
  respConnectionState = RI_EXCHANGE_DECLARED;
  amqpRespQueue = m_client.createQueue();
  if ( amqpRespQueue == nullptr )
    { emit logMessage( "AmqpInterface client createQueue() failed for respQueue." );
      return;
    }
  connect(amqpRespQueue, SIGNAL(declared()),        SLOT(respQueueDeclared())  );
  connect(amqpRespQueue, SIGNAL(bound()),           SLOT(respQueueBound())     );
  connect(amqpRespQueue, SIGNAL(messageReceived()), SLOT(respMessageReceived()));
  if ( respQueueName_set ) // checkbox control
    { emit logMessage( "  set resp queue name '"+respQueueName+"'" );
      amqpRespQueue->setName( respQueueName );
    }
  emit logMessage( "  resp declare(QAmqpQueue::Exclusive)" );
  amqpRespQueue->declare(QAmqpQueue::Exclusive);
}

///////////////////////////////////////////////////////////////////////////////////////////////

void AmqpInterface::reqQueueDeclared()
{ emit logMessage( "AmqpInterface::reqQueueDeclared()" );
  reqConnectionState = RI_QUEUE_DECLARED;
  if (!amqpReqQueue)
    { emit logMessage( "AmqpInterface::reqQueueDeclared() amqpReqQueue NULL" );
      return;
    }

  if ( reqConsumerTag_set ) // checkbox control
    { emit logMessage( "  set req consumer tag '"+reqConsumerTag+"'" );
      amqpReqQueue->setConsumerTag( reqConsumerTag );
    }

  amqpReqQueue->bind( reqExchangeName, reqBindingKey );
  emit logMessage( "  waiting for req queue to bind( "+reqExchangeName+" )" );
}

void AmqpInterface::respQueueDeclared()
{ emit logMessage( "AmqpInterface::respQueueDeclared()" );
  respConnectionState = RI_QUEUE_DECLARED;
  if (!amqpRespQueue)
    { emit logMessage( "AmqpInterface::respQueueDeclared() amqpRespQueue NULL" );
      return;
    }

  if ( reqConsumerTag_set ) // checkbox control
    { emit logMessage( "  set resp consumer tag '"+reqConsumerTag+"'" );
      amqpRespQueue->setConsumerTag( reqConsumerTag );
    }

  amqpRespQueue->bind( respExchangeName, reqBindingKey );
  emit logMessage( "  waiting for resp queue to bind( "+respExchangeName+" )" );
}

///////////////////////////////////////////////////////////////////////////////////////////////

void AmqpInterface::reqQueueBound()
{ emit logMessage( "AmqpInterface::reqQueueBound()" );
  reqConnectionState = RI_QUEUE_BOUND;
  emit connected();

  if (!amqpReqQueue)
    { emit logMessage( "AmqpInterface::reqQueueBound() amqpReqQueue NULL" );
      return;
    }
  emit logMessage( "  waiting for request messages" );
  reqReady = 1;
  amqpReqQueue->consume(QAmqpQueue::coNoAck | QAmqpQueue::coNoLocal);
  emit reqReadySig();
}

void AmqpInterface::respQueueBound()
{ emit logMessage( "AmqpInterface::respQueueBound()" );
  respConnectionState = RI_QUEUE_BOUND;
  emit connected();

  if (!amqpRespQueue)
    { emit logMessage( "AmqpInterface::respQueueBound() amqpRespQueue NULL" );
      return;
    }
  emit logMessage( "  waiting for response messages" );
  respReady = 1;
  amqpRespQueue->consume(QAmqpQueue::coNoAck | QAmqpQueue::coNoLocal);
  emit respReadySig();
}

///////////////////////////////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////////////////////

void AmqpInterface::reqSendMessage( QByteArray message, QString route )
{ reqPublishMessage(message, route);
  emit messageToRemote(message, Exchange::REQ, route);
}

void AmqpInterface::respSendMessage( QByteArray message, QString route )
{ respPublishMessage(message, route);
  emit messageToRemote(message, Exchange::RESP, route);
}

void AmqpInterface::reqMessageReceived()
{ if (!amqpReqQueue)
    { emit logMessage( "AmqpInterface::reqMessageReceived() amqpReqQueue NULL" );
      return;
    }

  if ( amqpReqQueue->size() < 0 )
    { emit logMessage( "AmqpInterface::reqMessageReceived() amqpReqQueue empty" );
      return;
    }

  QAmqpMessage qmsg  = amqpReqQueue->dequeue();
  QByteArray message = qmsg.payload();
  emit reqReceivedMessage( message );
  qint32 s = message.size();
  reqStats_packetCount    = reqStats_packetCount + 1;
  reqStats_byteCount      = reqStats_byteCount   + s;
  reqStats_lastPacketSize = s;

  if ( echo )
    { QString lmsg = QString( "[req] {%1}:%2:%3")
                        .arg( qmsg.routingKey() )
                        .arg( message.size() )
                        .arg( QString( echoHex ? message.toHex() : "" ) );
      emit logMessage( lmsg );
    }
}

void AmqpInterface::respMessageReceived()
{ if (!amqpRespQueue)
    { emit logMessage( "AmqpInterface::respMessageReceived() amqpRespQueue NULL" );
      return;
    }

  if ( amqpRespQueue->size() < 0 )
    { emit logMessage( "AmqpInterface::respMessageReceived() amqpRespQueue empty" );
      return;
    }

  QAmqpMessage qmsg  = amqpRespQueue->dequeue();
  QByteArray message = qmsg.payload();
  emit respReceivedMessage( message );
  qint32 s = message.size();
  respStats_packetCount    = respStats_packetCount + 1;
  respStats_byteCount      = respStats_byteCount   + s;
  respStats_lastPacketSize = s;

  if ( echo )
    { QString lmsg = QString( "[resp] {%1}:%2:%3")
                        .arg( qmsg.routingKey() )
                        .arg( message.size() )
                        .arg( QString( echoHex ? message.toHex() : "" ) );
      emit logMessage( lmsg );
    }
}

void AmqpInterface::messageFromRemote(QByteArray message, Exchange exchange, const QString& route)
{ switch (exchange)
    { case Exchange::REQ:
        reqPublishMessage(message, route);
        break;
      case Exchange::RESP:
        respPublishMessage(message, route);
        break;
      default:
        break;
    }
}

void AmqpInterface::reqPublishMessage( QByteArray message, QString route )
{
  if ( reqReady < 1 )
     { emit logMessage ( "[!req] "+QString( message.toHex() ) );
       return;
     }
   if ( !reqExchange )
     { emit logMessage ( "[x-req] "+QString( message.toHex() ) );
       return;
     }

   reqExchange->publish( message, route, "application/octet-stream", sendProperties );
}

void AmqpInterface::respPublishMessage( QByteArray message, QString route )
{
  if ( respReady < 1 )
     { emit logMessage ( "[!resp] "+QString( message.toHex() ) );
       return;
     }
   if ( !respExchange )
     { emit logMessage ( "[x-resp] "+QString( message.toHex() ) );
       return;
     }

   respExchange->publish( message, route, "application/octet-stream", sendProperties );
}

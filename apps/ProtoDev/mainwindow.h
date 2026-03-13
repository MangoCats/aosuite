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
#ifndef MAINWINDOW_H
#define MAINWINDOW_H

#include <QMainWindow>
#include <QJsonObject>
#include "riceyCodes.h"
#include "protocolDemo.h"
#include "blockTool.h"

QT_BEGIN_NAMESPACE
namespace Ui { class MainWindow; }
QT_END_NAMESPACE

class ProtocolDemo;

class MainWindow : public QMainWindow
{
    Q_OBJECT

public:
             MainWindow(QWidget *parent = nullptr);
            ~MainWindow();
       void  closeEvent(QCloseEvent *);
       void  restoreConfig();
       void  saveConfig();
       void  init();
       void  firstPass();
       void  showResults();
       void  translateRicey();
       void  translateNotes();
       void  translateToJson();
 QJsonValue  riceyLineToJson( QString );
 QJsonValue  notesLineToJson( QString );
    QString  codesLineToHtml( QString );
    QString  notesLineToHtml( QString, int );
       bool  rulesCheck( QString, QString );

public slots:
       void  on_save_clicked();
       void  on_update_clicked();
       void  on_reformat_clicked();
       void  on_readJson_clicked();
       void  on_write_clicked();
       void  on_importJson_clicked();

public:
                QString  t; // translation
                QString  v; // violations
            QJsonObject  jo;
                 qint32  maxNameLength;
                 qint32  maxNumLength;
                 qint32  maxCombLength;
        QVector<QString> riceyList,notesList;
         QList<RiceyInt> riceyInts;
 QHash<QString, QString> riceyCodes;
    QHash<qint32, QChar> notesNumChar;
     QHash<QChar, QChar> notesExtraChar;
     QPointer<BlockTool> blockTool;
           ProtocolDemo *pd;

         Ui::MainWindow *ui;
};
#endif // MAINWINDOW_H
